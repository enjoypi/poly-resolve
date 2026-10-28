import { $ } from "bun";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";

type Value = string | { level: string; priority?: number };
type Table = Record<string, Value>;

const manifest = "Cargo.toml";
const minimum = { rust: 100, clippy: 500 };

function rows(help: string, title: string): string[] {
  const lines = help.split("\n");
  const start = lines.findIndex((line) => line.trim() === title);
  if (start < 0 || !lines[start + 3]?.trim().startsWith("----")) throw new Error(`段落格式变化：${title}`);
  const names: string[] = [];
  for (const line of lines.slice(start + 4)) {
    if (!/^\s+\S/.test(line)) break;
    names.push(line.trim().split(/\s+/)[0].replace(/^clippy::/, "").replaceAll("-", "_"));
  }
  return names;
}

async function rejected(cmd: string[], prefix: string, names: string[]): Promise<Set<string>> {
  const flags = names.flatMap((name) => ["-W", `${prefix}${name}`]);
  const dir = await mkdtemp(`${tmpdir()}/lints-`);
  try {
    const out = await $`${cmd} --crate-type lib --crate-name probe --emit=metadata --out-dir ${dir} - ${flags} < ${Buffer.from("")}`
      .nothrow()
      .quiet();
    if (out.exitCode !== 0) throw new Error(out.stderr.toString());
    return new Set(Array.from(out.stderr.toString().matchAll(/lint:? `(?:clippy::)?(\w+)`/g), (m) => m[1]));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

const rank = (v: Value) => (typeof v === "object" ? 0 : v === "deny" ? 2 : 1);
const priority = (v: Value) => (typeof v === "object" ? (v.priority ?? 0) : 0);
const format = (v: Value) =>
  typeof v === "string"
    ? JSON.stringify(v)
    : `{ ${Object.entries(v).map(([k, x]) => `${k} = ${JSON.stringify(x)}`).join(", ")} }`;

function render(table: Table, lints: string[], groups: string[]): string {
  const keep = new Set([...lints, ...groups]);
  const merged: Table = {};
  for (const [name, value] of Object.entries(table)) {
    if (keep.has(name)) merged[name] = value;
    else console.warn(`移除失效 lint：${name}`);
  }
  for (const name of lints) merged[name] ??= "deny";
  const entries = Object.entries(merged).sort(
    ([a, x], [b, y]) => rank(x) - rank(y) || priority(x) - priority(y) || (a < b ? -1 : 1),
  );
  return entries
    .map(([name, v], i) => (i > 0 && rank(entries[i - 1][1]) !== rank(v) ? "\n" : "") + `${name} = ${format(v)}`)
    .join("\n");
}

function replace(text: string, header: string, body: string): string {
  const re = new RegExp(`(^\\[${header.replaceAll(".", "\\.")}\\]\\n)[^[]*`, "m");
  if (!re.test(text)) throw new Error(`缺少 [${header}]`);
  return text.replace(re, (_, head) => `${head}${body}\n\n`);
}

const [nightly, stableHelp] = await Promise.all([
  $`clippy-driver rustc -W help`.text(),
  $`rustc +stable -W help`.text(),
]);
const stable = new Set(rows(stableHelp, "Lint checks provided by rustc:"));
const rustAll = rows(nightly, "Lint checks provided by rustc:").filter((n) => stable.has(n));
const clippyAll = rows(nightly, "Lint checks loaded by this crate:");
const [rustNightlyBad, rustStableBad, clippyBad] = await Promise.all([
  rejected(["rustc"], "", rustAll),
  rejected(["rustc", "+stable"], "", rustAll),
  rejected(["clippy-driver", "rustc"], "clippy::", clippyAll),
]);
const rustBad = new Set([...rustNightlyBad, ...rustStableBad]);
const rust = rustAll.filter((n) => !rustBad.has(n));
const clippy = clippyAll.filter((n) => !clippyBad.has(n));
if (rust.length < minimum.rust || clippy.length < minimum.clippy) throw new Error("lint 数量异常");

const text = await Bun.file(manifest).text();
const lints = Bun.TOML.parse(text).workspace.lints as { rust: Table; clippy: Table };
const rustGroups = rows(nightly, "Lint groups provided by rustc:");
const clippyGroups = [...rows(nightly, "Lint groups loaded by this crate:"), "all"];
let next = replace(text, "workspace.lints.rust", render(lints.rust, rust, rustGroups));
next = replace(next, "workspace.lints.clippy", render(lints.clippy, clippy, clippyGroups));
next = `${next.trimEnd()}\n`;
if (process.argv.includes("--check")) {
  if (next !== text) {
    console.error("Cargo.toml lint 与当前 nightly 不同步，先跑 bun scripts/lints.ts");
    process.exit(1);
  }
} else {
  await Bun.write(manifest, next);
}
console.log(`rust ${rust.length} clippy ${clippy.length}`);

import { $ } from "bun";

const env = { ...process.env, CARGO_TERM_COLOR: process.stdout.isTTY ? "always" : "never" };
const steps = [
  { name: "lints", summary: true, run: $`bun scripts/lints.ts --check` },
  { name: "forbid", summary: false, run: $`bun scripts/forbid.ts` },
  { name: "fmt", summary: false, run: $`cargo fmt --all --check` },
  { name: "cov", summary: true, run: $`bun scripts/cov.ts` },
  { name: "web", summary: false, run: $`bun install --frozen-lockfile && bun x tsc --noEmit && bun test`.cwd("web") },
].map(({ name, summary, run }) => ({ name, summary, run: run.env(env).nothrow().quiet() }));
const noise = /process didn't exit successfully: `clippy-driver/;
const clean = (text: string) =>
  text
    .split("\n")
    .filter((line) => !noise.test(line))
    .join("\n");
let code = 0;
const results = await Promise.all(steps.map(({ run }) => run));
for (const [index, { exitCode, stdout, stderr }] of results.entries()) {
  const { name, summary } = steps[index];
  if (exitCode === 0) {
    const last = summary ? stdout.toString().trimEnd().split("\n").pop() : "";
    console.log(last ? `ok ${name}：${last}` : `ok ${name}`);
  } else {
    console.error(`fail ${name}`);
    process.stdout.write(clean(stdout.toString()));
    process.stderr.write(clean(stderr.toString()));
  }
  code ||= exitCode;
}
process.exit(code);

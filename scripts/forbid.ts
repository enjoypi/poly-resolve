import { $ } from "bun";

const targets = /\.rs$|(^|\/)Cargo\.toml$|^rust-toolchain\.toml$|^\.cargo\/config\.toml$|^scripts\/(?!forbid\.ts$)[^/]+\.ts$/;
const banned = [
  /#!?\[[^\]]*\bfeature\s*\(/,
  /coverage\(off\)/,
  /coverage_nightly/,
  /cargo-features/,
  /^\s*\[\s*unstable\s*[\].]/,
  /(^|[\s"'`=])-Z(?=[a-z"'\s]|$)/,
];
const read = (path: string) => Bun.file(path).text().catch((e) => (e.code === "ENOENT" ? "" : Promise.reject(e)));
const listed = await $`git ls-files -z --cached --others --exclude-standard`.text();
const paths = listed.split("\0").filter((f) => targets.test(f));
if (!paths.length) throw new Error("forbid 扫描清单为空");
const texts = await Promise.all(paths.map(read));
const hits = paths.flatMap((path, k) =>
  texts[k].split("\n").flatMap((line, i) => {
    const rule = banned.find((re) => re.test(line));
    return rule ? [`${path}:${i + 1} 含禁用项 ${rule}`] : [];
  }),
);
if (hits.length) {
  console.error(hits.join("\n"));
  process.exit(1);
}

import { $ } from "bun";
import { test } from "./llvm-cov.ts";

const out = "target/llvm-cov.json";
await test(["--json", "--output-path", out, "--no-clean"], false);
const { files, totals } = (await Bun.file(out).json()).data[0];
const gap = (kind: string) => totals[kind].count === 0 || totals[kind].covered !== totals[kind].count;
console.log(`branches ${totals.branches.covered}/${totals.branches.count} regions ${totals.regions.covered}/${totals.regions.count}`);
if (gap("branches")) {
  console.error("branches 覆盖率未达 100%，先补齐 branch，再看 region");
  for (const { branches, filename } of files) {
    for (const [line, column, , , hit, miss] of branches) {
      if (!hit || !miss) console.error(`${filename}:${line}:${column} 真 ${hit} 假 ${miss}`);
    }
  }
  process.exit(1);
}
if (gap("regions")) {
  console.error("regions 覆盖率未达 100%");
  await $`cargo llvm-cov report --branch --show-missing-lines 1>&2`.nothrow();
  console.error("怀疑旧产物混入时先跑 cargo llvm-cov clean");
  process.exit(1);
}

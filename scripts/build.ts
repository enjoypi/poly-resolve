import { bin, test } from "./llvm-cov.ts";

await test(["--no-report", "--", "--list"], true);
console.log(bin);

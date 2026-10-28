import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../src/style.css", import.meta.url), "utf8");

function variables(block: string): Record<string, string> {
  return Object.fromEntries(
    [...block.matchAll(/(--fs-\d):\s*([^;]+);/g)].map((match) => [match[1] ?? "", match[2] ?? ""]),
  );
}

test("字号只有 4 个变量，桌面正文 14px，手机另取一组", () => {
  const roots = css.match(/:root\s*\{[^}]*\}/g) ?? [];
  expect(roots).toHaveLength(2);
  expect(variables(roots[0] ?? "")).toEqual({
    "--fs-1": "12px",
    "--fs-2": "14px",
    "--fs-3": "16px",
    "--fs-4": "20px",
  });
  expect(Object.keys(variables(roots[1] ?? ""))).toEqual(["--fs-1", "--fs-2", "--fs-3", "--fs-4"]);
});

test("所有字号声明都取自变量，不用 font 简写", () => {
  const sizes = [...css.matchAll(/font-size:\s*([^;]+);/g)].map((match) => match[1]);
  expect(sizes.length).toBeGreaterThan(10);
  for (const size of sizes) expect(size).toMatch(/^var\(--fs-[1-4]\)$/);
  expect(css).not.toMatch(/(^|[\s;{])font:/);
});

test("样式不引用外部资源", () => {
  expect(css).not.toMatch(/@import|url\(|https?:/);
});

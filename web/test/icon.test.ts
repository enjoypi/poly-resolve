import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";

const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const link = /<link rel="icon" type="image\/svg\+xml" href="\.\/([^"]+)"/.exec(html);

test("index.html 以 svg 图标引用已有文件", () => {
  expect(link?.[1]).toBe("src/favicon.svg");
  expect(existsSync(new URL(`../${link?.[1]}`, import.meta.url))).toBe(true);
});

test("图标是自包含的 svg，带标题且不含脚本与外部引用", () => {
  const svg = readFileSync(new URL("../src/favicon.svg", import.meta.url), "utf8");
  expect(svg).toMatch(/^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg" viewBox="0 0 64 64"/);
  expect(svg).toContain("<title>Poly Resolve</title>");
  expect(svg).not.toMatch(/<script|<text|href=|url\(|@import/);
});

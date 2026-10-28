import { expect, test } from "bun:test";
import { Glob } from "bun";
import { generate } from "../gen";

test("api.gen.ts 与 api/openapi.json 同步", async () => {
  const committed = await Bun.file("src/api.gen.ts").text();
  expect(committed).toBe(await generate());
});

test("生成物不含注释", async () => {
  expect(await generate()).not.toContain("/*");
});

test("src 内不手写网络调用", async () => {
  const files = [...new Glob("src/*.ts").scanSync()].filter((f) => f !== "src/api.gen.ts");
  expect(files.length).toBeGreaterThan(0);
  for (const file of files) {
    expect(await Bun.file(file).text()).not.toMatch(/\bfetch\(|XMLHttpRequest|axios/);
  }
});

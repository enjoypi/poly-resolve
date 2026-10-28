import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import { badge } from "../src/badge";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

test("同一会话 id 永远同一图案，不同 id 图案不同", () => {
  const first = badge("cidLb8fXaJ+Lsiv/GGXNi/bog==");
  const again = badge("cidLb8fXaJ+Lsiv/GGXNi/bog==");
  const other = badge("cidOther");
  expect(first.style.backgroundImage).toBe(again.style.backgroundImage);
  expect(first.style.backgroundImage).not.toBe(other.style.backgroundImage);
  expect(first).not.toBe(again);
});

test("徽记是带完整 id 说明的图片角色，图案左右对称", () => {
  const element = badge("cid-1");
  expect(element.getAttribute("role")).toBe("img");
  expect(element.getAttribute("aria-label")).toBe("cid-1");
  const image = decodeURIComponent(element.style.backgroundImage);
  const cells = [...image.matchAll(/M(\d) (\d)h1v1h-1z/g)].map((match) => `${match[1]},${match[2]}`);
  for (const cell of cells) {
    const [column = "", row = ""] = cell.split(",");
    expect(cells).toContain(`${4 - Number(column)},${row}`);
  }
  expect(cells.length).toBeGreaterThan(0);
});

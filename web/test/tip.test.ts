import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import { attachTip } from "../src/tip";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const DELAY = 5;

function setup(nodes: Node[] | null = [document.createTextNode("完整内容")]) {
  const scroller = document.createElement("div");
  const cell = document.createElement("td");
  const tip = document.createElement("div");
  tip.hidden = true;
  scroller.append(cell);
  attachTip(scroller, tip, () => nodes ?? undefined, DELAY);
  return { cell, scroller, tip };
}

function clip(cell: HTMLElement) {
  Object.defineProperty(cell, "scrollWidth", { configurable: true, value: 100 });
}

function over(cell: HTMLElement, pointerType = "mouse") {
  cell.dispatchEvent(new PointerEvent("pointerover", { bubbles: true, pointerType }));
}

test("鼠标悬停在被截断的单元格上，延迟后显示完整内容并定位", async () => {
  const { cell, tip } = setup();
  clip(cell);
  over(cell);
  expect(tip.hidden).toBe(true);
  await Bun.sleep(DELAY * 6);
  expect(tip.hidden).toBe(false);
  expect(tip.textContent).toBe("完整内容");
  expect(tip.style.left).toMatch(/px$/);
  expect(tip.style.top).toMatch(/px$/);
});

test("触屏、未截断、无内容时不显示；标记为总是显示的单元格不看截断", async () => {
  const touch = setup();
  clip(touch.cell);
  over(touch.cell, "touch");
  await Bun.sleep(DELAY * 6);
  expect(touch.tip.hidden).toBe(true);
  const fits = setup();
  over(fits.cell);
  await Bun.sleep(DELAY * 6);
  expect(fits.tip.hidden).toBe(true);
  const empty = setup(null);
  clip(empty.cell);
  over(empty.cell);
  await Bun.sleep(DELAY * 6);
  expect(empty.tip.hidden).toBe(true);
  const always = setup();
  always.cell.dataset.always = "";
  over(always.cell);
  await Bun.sleep(DELAY * 6);
  expect(always.tip.hidden).toBe(false);
});

test("指针移开、按下、滚动、按键都会收起，且取消尚未显示的提示", async () => {
  for (const name of ["pointerout", "pointerdown", "scroll", "keydown"]) {
    const { cell, scroller, tip } = setup();
    clip(cell);
    over(cell);
    await Bun.sleep(DELAY * 6);
    expect(tip.hidden).toBe(false);
    scroller.dispatchEvent(new Event(name));
    expect(tip.hidden).toBe(true);
  }
  const { cell, scroller, tip } = setup();
  clip(cell);
  over(cell);
  scroller.dispatchEvent(new Event("pointerout"));
  await Bun.sleep(DELAY * 6);
  expect(tip.hidden).toBe(true);
  scroller.dispatchEvent(new PointerEvent("pointerover", { bubbles: true, pointerType: "mouse" }));
  await Bun.sleep(DELAY * 6);
  expect(tip.hidden).toBe(true);
});

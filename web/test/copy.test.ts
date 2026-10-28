import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import { copyButton } from "../src/copy";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const DELAY = 10;

async function until(check: () => boolean) {
  for (let attempt = 0; attempt < 400; attempt++) {
    if (check()) return;
    await Bun.sleep(5);
  }
  throw new Error("timeout");
}

function clipboard(value: unknown) {
  Object.defineProperty(navigator, "clipboard", { configurable: true, value });
}

test("点击写入完整文本并短暂显示已复制，随后恢复", async () => {
  const written: string[] = [];
  clipboard({
    writeText: async (text: string) => {
      written.push(text);
    },
  });
  const button = copyButton("# 标题\n正文", DELAY);
  expect(button.textContent).toBe("复制");
  button.click();
  await until(() => button.textContent === "已复制");
  expect(written).toEqual(["# 标题\n正文"]);
  await until(() => button.textContent === "复制");
});

test("写入被拒绝或没有剪贴板接口时显示复制失败，随后恢复", async () => {
  clipboard({
    writeText: async () => {
      throw new Error("denied");
    },
  });
  const denied = copyButton("x", DELAY);
  denied.click();
  await until(() => denied.textContent === "复制失败");
  await until(() => denied.textContent === "复制");
  clipboard(undefined);
  const missing = copyButton("x", DELAY);
  missing.click();
  await until(() => missing.textContent === "复制失败");
});

test("点击不向上冒泡", () => {
  clipboard({ writeText: async () => undefined });
  const holder = document.createElement("div");
  const button = copyButton("x", DELAY);
  let reached = false;
  holder.onclick = () => {
    reached = true;
  };
  holder.append(button);
  button.click();
  expect(reached).toBe(false);
});

import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import { anchor, target } from "../src/links";
import { inline } from "../src/markdown";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

function render(text: string) {
  const root = document.createElement("div");
  root.replaceChildren(...inline(text));
  return root;
}

test("Markdown 链接渲染成只显示标题的可点链接，地址里的 token 不出现在显示文字与提示里", () => {
  const root = render("前文 [屏蔽报警](https://example.com/a/b?token=SECRET&x=1) 后文");
  const link = root.querySelector("a");
  expect(link?.textContent).toBe("屏蔽报警");
  expect(link?.getAttribute("href")).toBe("https://example.com/a/b?token=SECRET&x=1");
  expect(link?.getAttribute("target")).toBe("_blank");
  expect(link?.getAttribute("rel")).toBe("noopener noreferrer");
  expect(link?.getAttribute("title")).toBe("example.com/a/b");
  expect(root.textContent).toBe("前文 屏蔽报警 后文");
  expect(root.textContent).not.toContain("SECRET");
});

test("多个链接与前后文本按原顺序保留", () => {
  const root = render("a [一](http://x.test/1) b [二](https://y.test/2)\nc");
  expect([...root.querySelectorAll("a")].map((link) => link.textContent)).toEqual(["一", "二"]);
  expect(root.textContent).toBe("a 一 b 二\nc");
});

test("标题为空时显示主机名", () => {
  const root = render("[](https://example.com/path)");
  expect(root.querySelector("a")?.textContent).toBe("example.com");
});

test("非 http、https 协议与格式不全的写法保持纯文本", () => {
  for (const text of [
    "[点我](javascript:alert(1))",
    "[点我](ftp://example.com/file)",
    "[点我](data:text/html;base64,AAAA)",
    "[点我] (https://example.com)",
    "[点我](https://)",
  ]) {
    const root = render(text);
    expect(root.querySelector("a")).toBeNull();
    expect(root.textContent).toBe(text);
  }
});

test("标题里的 HTML 只当文本，不生成元素", () => {
  const root = render('[<img src=x onerror="alert(1)">](https://example.com/)');
  expect(root.querySelector("img")).toBeNull();
  expect(root.querySelector("a")?.textContent).toBe('<img src=x onerror="alert(1)">');
});

test("没有链接时原文整体保留，空文本不生成节点", () => {
  expect(render("普通文本\n第二行").textContent).toBe("普通文本\n第二行");
  expect(inline("")).toEqual([]);
});

test("anchor 对无法解析的地址退回纯文本，target 返回 undefined", () => {
  expect(target("not a url")).toBeUndefined();
  expect(target("https://e.com/x")?.host).toBe("e.com");
  expect(anchor("标题", "not a url").textContent).toBe("[标题](not a url)");
});

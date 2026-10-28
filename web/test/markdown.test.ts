import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import { MD_LIMIT, PLAIN_LIMIT } from "../src/config";
import { blocks, inline, plain } from "../src/markdown";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

function html(nodes: Node[]): string {
  const holder = document.createElement("div");
  holder.append(...nodes);
  return holder.innerHTML;
}

test("plain 去掉标记并压成一行，链接只留标题，空标题用主机名", () => {
  const text = "## 标题\n> 引用 **粗** `码`\n[屏蔽报警](https://e.com/a?token=SECRET) 结束\n[](https://e.com/x)";
  expect(plain(text)).toBe("标题 引用 粗 码 屏蔽报警 结束 e.com");
  expect(plain("```\ncode\n```\n正文")).toBe("code 正文");
  expect(plain("")).toBe("");
});

test("plain 只取开头一段，避免处理超长正文", () => {
  expect(plain("a".repeat(PLAIN_LIMIT * 3))).toHaveLength(PLAIN_LIMIT);
});

test("blocks 生成标题、段落、引用、代码块，段内保留换行", () => {
  const text = "# 告警\n第一行\n第二行\n\n> 引用甲\n> 引用乙\n```\n<b>raw</b>\n```\n结尾";
  const holder = document.createElement("div");
  holder.append(...blocks(text));
  expect(holder.innerHTML).toBe(
    '<p class="heading"><strong>告警</strong></p><p>第一行\n第二行</p><blockquote>引用甲\n引用乙</blockquote><pre><code>&lt;b&gt;raw&lt;/b&gt;</code></pre><p>结尾</p>',
  );
});

test("未闭合的代码块照常输出，空文本不生成块", () => {
  expect(blocks("```\nabc").map((node) => node.outerHTML)).toEqual(["<pre><code>abc</code></pre>"]);
  expect(blocks("")).toEqual([]);
  expect(blocks("\n\n")).toEqual([]);
});

test("行内：粗体、代码、链接，粗体里可含链接，链接标题里的标记被压平", () => {
  expect(html(inline("a **b** `c` d"))).toBe("a <strong>b</strong> <code>c</code> d");
  const bold = html(inline("**[点我](https://e.com/p?token=S)**"));
  expect(bold).toBe(
    '<strong><a href="https://e.com/p?token=S" target="_blank" rel="noopener noreferrer" title="e.com/p">点我</a></strong>',
  );
  expect(html(inline("[**重点**](https://e.com/)")).includes(">重点</a>")).toBe(true);
});

test("不安全的写法保持纯文本：HTML、javascript、data、图片只当链接显示", () => {
  const hostile = `<img src=x onerror="alert(1)"><script>alert(2)</script>`;
  const holder = document.createElement("div");
  holder.append(...blocks(`${hostile}\n[x](javascript:alert(1)) [y](data:text/html,a)`));
  expect(holder.querySelector("img")).toBeNull();
  expect(holder.querySelector("script")).toBeNull();
  expect(holder.querySelector("a")).toBeNull();
  expect(holder.textContent).toContain(hostile);
  expect(html(inline("![图](https://e.com/i.png)"))).toContain("<a ");
  expect(holder.querySelector("[src]")).toBeNull();
});

test("超过上限的正文不做格式化", () => {
  const long = `**粗**${"x".repeat(MD_LIMIT)}`;
  const out = blocks(long);
  expect(out).toHaveLength(1);
  expect(out[0]?.querySelector("strong")).toBeNull();
  expect(out[0]?.textContent).toBe(long);
});

test("病态输入在限定时间内完成", () => {
  const cases = ["[".repeat(MD_LIMIT - 1), "[a](".repeat(5000), "**".repeat(5000), "`".repeat(MD_LIMIT - 1), `[${"a".repeat(MD_LIMIT - 10)}`];
  for (const text of cases) {
    const started = performance.now();
    blocks(text);
    plain(text);
    expect(performance.now() - started).toBeLessThan(500);
  }
});

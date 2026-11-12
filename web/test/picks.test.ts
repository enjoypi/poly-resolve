import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import { button, column, erase, item, mount, page, summary, type, until } from "./rig";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

test("点击行或按 Enter 打开详情，正文渲染 Markdown，关闭后焦点回到该行", async () => {
  const text = "# 告警\n详情 [屏蔽报警](https://example.com/a?token=SECRET)";
  const { root } = await mount([page([item("a", { text, zzz: 7 })])]);
  document.body.append(root);
  const row = root.querySelector<HTMLElement>("tbody tr");
  const dialog = root.querySelector("dialog");
  row?.click();
  expect(dialog?.open).toBe(true);
  expect(dialog?.querySelector(".sender")?.textContent).toBe("alice");
  expect(dialog?.querySelector(".when")?.textContent).toBe("2026-10-28 09:20:09 UTC+8");
  expect(dialog?.querySelector(".heading")?.textContent).toBe("告警");
  const link = dialog?.querySelector(".body a");
  expect(link?.getAttribute("href")).toBe("https://example.com/a?token=SECRET");
  expect(link?.getAttribute("title")).toBe("example.com/a");
  const names = [...(dialog?.querySelectorAll("dt") ?? [])].map((entry) => entry.textContent);
  expect(names).toContain("zzz");
  expect(names).toContain("key");
  dialog?.querySelector<HTMLElement>(".close")?.click();
  expect(dialog?.open).toBe(false);
  expect(document.activeElement).toBe(row);
  row?.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "Enter" }));
  expect(dialog?.open).toBe(true);
  dialog?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  expect(dialog?.open).toBe(false);
  root.remove();
});

test("正文为空时详情显示占位，缺少发送人与时间字段也能打开", async () => {
  const bare = { key: "x", text: "" };
  const { root } = await mount([page([bare])]);
  root.querySelector<HTMLElement>("tbody tr")?.click();
  const dialog = root.querySelector("dialog");
  expect(dialog?.open).toBe(true);
  expect(dialog?.querySelector(".body")?.textContent).toBe("[非文本消息]");
  expect(dialog?.querySelector(".when")?.textContent).toBe("");
});

function sender(root: HTMLElement, index: number) {
  const found = root.querySelectorAll<HTMLElement>('tbody td[data-col="sender_name"]')[index];
  if (!found) throw new Error(`no sender ${index}`);
  return found;
}

function filter(root: HTMLElement, key: string) {
  return root.querySelector<HTMLInputElement>(`tr.filters th[data-col="${key}"] input`)?.value;
}

test("点击发送人只筛选发送人，精确匹配，不开详情，再点移除，点正文仍开详情", async () => {
  const { root } = await mount([
    page([
      item("a", { conversation_id: "c1", sender_name: "李明" }),
      item("b", { conversation_id: "c1", sender_name: "李明华" }),
      item("c", { conversation_id: "c2", sender_name: "李明" }),
      item("d", { conversation_id: "c1", sender_name: "李明" }),
    ]),
  ]);
  document.body.append(root);
  const dialog = root.querySelector("dialog");
  expect(sender(root, 0).classList.contains("pick")).toBe(true);
  sender(root, 0).click();
  expect(column(root, "text")).toEqual(["text-a", "text-c", "text-d"]);
  expect(filter(root, "sender_name")).toBe('"李明"');
  expect(filter(root, "conversation_id")).toBe("");
  expect(dialog?.open).toBe(false);
  expect(summary(root)).toBe("显示 3／已加载 4 条。");
  root.querySelector<HTMLElement>('tbody td[data-col="text"]')?.click();
  expect(dialog?.open).toBe(true);
  dialog?.close();
  sender(root, 0).click();
  expect(filter(root, "sender_name")).toBe("");
  expect(column(root, "text")).toEqual(["text-a", "text-b", "text-c", "text-d"]);
  expect(erase(root, "sender_name")?.hidden).toBe(true);
});

test("点击会话只筛选该会话，点徽记与点单元格一样，不开详情，再点移除", async () => {
  const { root } = await mount([
    page([
      item("a", { conversation_id: "c1", sender_name: "alice" }),
      item("b", { conversation_id: "c2", sender_name: "alice" }),
      item("c", { conversation_id: "c1", sender_name: "bob" }),
    ]),
  ]);
  document.body.append(root);
  const dialog = root.querySelector("dialog");
  const cell = root.querySelector<HTMLElement>('tbody td[data-col="conversation_id"]');
  expect(cell?.classList.contains("pick")).toBe(true);
  cell?.querySelector<HTMLElement>(".badge")?.click();
  expect(column(root, "text")).toEqual(["text-a", "text-c"]);
  expect(filter(root, "conversation_id")).toBe('"c1"');
  expect(filter(root, "sender_name")).toBe("");
  expect(dialog?.open).toBe(false);
  sender(root, 1).click();
  expect(column(root, "text")).toEqual(["text-c"]);
  expect(filter(root, "conversation_id")).toBe('"c1"');
  expect(filter(root, "sender_name")).toBe('"bob"');
  root.querySelector<HTMLElement>('tbody td[data-col="conversation_id"]')?.click();
  expect(filter(root, "conversation_id")).toBe("");
  expect(filter(root, "sender_name")).toBe('"bob"');
  expect(column(root, "text")).toEqual(["text-c"]);
});

test("正文单元格有复制按钮，复制完整原文，不开详情；空正文没有按钮", async () => {
  const written: string[] = [];
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: {
      writeText: async (text: string) => {
        written.push(text);
      },
    },
  });
  const long = `# 标题\n${"字".repeat(700)}`;
  const { root } = await mount([page([item("a", { text: long }), item("b", { text: "" })])]);
  document.body.append(root);
  const dialog = root.querySelector("dialog");
  const buttons = root.querySelectorAll<HTMLButtonElement>('tbody td[data-col="text"] .copy');
  expect(buttons).toHaveLength(1);
  buttons[0]?.click();
  await until(() => written.length === 1);
  expect(written).toEqual([long]);
  expect(dialog?.open).toBe(false);
  buttons[0]?.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "Enter" }));
  expect(dialog?.open).toBe(false);
  expect(column(root, "text")[0]?.startsWith("标题")).toBe(true);
});

test("每个标题上有清除筛选按钮，该列有筛选才显示，只清该列", async () => {
  const { root } = await mount([
    page([
      item("a", { conversation_id: "c1", sender_name: "alice" }),
      item("b", { conversation_id: "c2", sender_name: "alice" }),
      item("c", { conversation_id: "c1", sender_name: "bob" }),
    ]),
  ]);
  const titles = root.querySelectorAll('tr.titles th[data-col]');
  expect(titles.length).toBeGreaterThan(0);
  for (const title of Array.from(titles)) expect(title.querySelector<HTMLElement>(".clear")?.hidden).toBe(true);
  expect(root.querySelector(".bar .clear")).toBeNull();
  expect(root.querySelector(".status .clear")).toBeNull();
  type(root, 'tr.filters th[data-col="text"] input', "text-");
  type(root, ".search", "alice");
  sender(root, 0).click();
  root.querySelector<HTMLElement>('tbody td[data-col="conversation_id"]')?.click();
  expect(erase(root, "sender_name")?.hidden).toBe(false);
  expect(erase(root, "conversation_id")?.hidden).toBe(false);
  expect(erase(root, "text")?.hidden).toBe(false);
  expect(erase(root, "sender_name")?.getAttribute("aria-label")).toBe("清除筛选 发送人");
  expect(column(root, "text")).toEqual(["text-a"]);
  erase(root, "conversation_id")?.click();
  expect(erase(root, "conversation_id")?.hidden).toBe(true);
  expect(filter(root, "conversation_id")).toBe("");
  expect(filter(root, "sender_name")).toBe('"alice"');
  expect(column(root, "text")).toEqual(["text-a", "text-b"]);
  type(root, 'tr.filters th[data-col="text"] input', "");
  expect(erase(root, "text")?.hidden).toBe(true);
  erase(root, "sender_name")?.click();
  expect(column(root, "text")).toEqual(["text-a", "text-b"]);
  expect(root.querySelector<HTMLInputElement>(".search")?.value).toBe("alice");
});

test("点击只设被点列的筛选，保留其他列的筛选与搜索", async () => {
  const { root } = await mount([
    page([
      item("a", { conversation_id: "c1", sender_name: "alice" }),
      item("b", { conversation_id: "c1", sender_name: "bob" }),
      item("c", { conversation_id: "c2", sender_name: "alice" }),
    ]),
  ]);
  type(root, 'tr.filters th[data-col="text"] input', "text-");
  type(root, ".search", "alice");
  expect(column(root, "text")).toEqual(["text-a", "text-c"]);
  sender(root, 1).click();
  expect(column(root, "text")).toEqual(["text-a", "text-c"]);
  expect(filter(root, "sender_name")).toBe('"alice"');
  expect(filter(root, "conversation_id")).toBe("");
  expect(filter(root, "text")).toBe("text-");
  expect(root.querySelector<HTMLInputElement>(".search")?.value).toBe("alice");
});

test("手机上的筛选按钮展开与收起表头面板", async () => {
  const { root } = await mount([page([item("a")])]);
  const toggle = button(root, "筛选");
  const app = root.querySelector(".app");
  toggle.click();
  expect(app?.classList.contains("panel-open")).toBe(true);
  expect(toggle.getAttribute("aria-expanded")).toBe("true");
  toggle.click();
  expect(app?.classList.contains("panel-open")).toBe(false);
});

test("列菜单按钮展开与 Esc 收起", async () => {
  const { root } = await mount([page([item("a")])]);
  const toggle = button(root, "列");
  const menu = root.querySelector<HTMLElement>(".menu");
  expect(menu?.hidden).toBe(true);
  toggle.click();
  expect(menu?.hidden).toBe(false);
  menu?.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "Escape" }));
  expect(menu?.hidden).toBe(true);
  menu?.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "a" }));
  expect(menu?.hidden).toBe(true);
});

test("点击来源与账号按显示值精确筛选，再点移除", async () => {
  const { root } = await mount([
    page([
      item("a", { account: "hf", source: "imap" }),
      item("b", { account: "default", source: "dingtalk" }),
      item("c", { account: "t4f", source: "imap" }),
    ]),
  ]);
  document.body.append(root);
  const cell = (key: string, index: number) =>
    root.querySelectorAll<HTMLElement>(`tbody td[data-col="${key}"]`)[index];
  expect(cell("source", 0)?.textContent).toBe("邮箱");
  expect(cell("source", 0)?.classList.contains("pick")).toBe(true);
  cell("source", 0)?.click();
  expect(filter(root, "source")).toBe('"邮箱"');
  expect(column(root, "text")).toEqual(["text-a", "text-c"]);
  cell("account", 1)?.click();
  expect(filter(root, "account")).toBe('"t4f"');
  expect(column(root, "text")).toEqual(["text-c"]);
  cell("source", 0)?.click();
  expect(filter(root, "source")).toBe("");
  expect(column(root, "text")).toEqual(["text-c"]);
});

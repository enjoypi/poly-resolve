import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import spec from "../../api/openapi.json";
import { connect } from "../src/client";
import { renderMessages } from "../src/messages";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const path = Object.keys(spec.paths).find((candidate) => candidate.endsWith("/list")) ?? "";
const NOON = Date.UTC(2026, 9, 28, 1, 20, 9);

type Reply = { body: unknown; status?: number };

function item(key: string, patch: Record<string, unknown> = {}) {
  return {
    account: "acct",
    conversation_id: "c1",
    key,
    occurred_at_ms: NOON,
    received_at_ms: NOON,
    sender_id: "u1",
    sender_name: "alice",
    source: "dingtalk",
    text: `text-${key}`,
    ...patch,
  };
}

function stub(replies: Reply[]) {
  const calls: { path: string; body: Record<string, unknown> }[] = [];
  const fetch = async (request: Request) => {
    calls.push({ path: new URL(request.url).pathname, body: await request.json() });
    const reply = replies[Math.min(calls.length, replies.length) - 1];
    return Response.json(reply?.body, { status: reply?.status ?? 200 });
  };
  return { calls, client: connect("http://localhost", fetch) };
}

async function until(check: () => boolean) {
  for (let attempt = 0; attempt < 400; attempt++) {
    if (check()) return;
    await Bun.sleep(5);
  }
  throw new Error("timeout");
}

function button(root: HTMLElement, label: string) {
  const found = [...root.querySelectorAll("button")].find((entry) => entry.textContent === label);
  if (!found) throw new Error(`no button ${label}`);
  return found;
}

function column(root: HTMLElement, key: string) {
  return [...root.querySelectorAll(`tbody td[data-col="${key}"]`)].map(
    (entry) => (entry.querySelector(".text") ?? entry).textContent,
  );
}

function type(root: HTMLElement, selector: string, value: string, event = "input") {
  const input = root.querySelector<HTMLInputElement>(selector);
  if (!input) throw new Error(`no input ${selector}`);
  input.value = value;
  input.dispatchEvent(new Event(event, { bubbles: true }));
}

async function mount(replies: Reply[]) {
  const { calls, client } = stub(replies);
  const root = document.createElement("div");
  await renderMessages(client, root, "Asia/Shanghai");
  return { calls, client, root };
}

function page(items: unknown[], cursor: string | null = null): Reply {
  return { body: { items, next_cursor: cursor } };
}

function summary(root: HTMLElement) {
  return root.querySelector(".summary")?.textContent;
}

async function settled(root: HTMLElement, calls: unknown[], count: number) {
  await until(() => calls.length === count && !button(root, "刷新").disabled);
}

test("请求路径取自 spec 并带页宽，表格按返回顺序显示默认列", async () => {
  const { calls, root } = await mount([page([item("a"), item("b")])]);
  expect(calls).toEqual([{ path, body: { limit: 200 } }]);
  expect(column(root, "text")).toEqual(["text-a", "text-b"]);
  expect(column(root, "occurred_at_ms")[0]).toBe("2026-10-28 09:20:09");
  expect(column(root, "sender_name")).toEqual(["alice", "alice"]);
  expect(root.querySelector('thead th[data-col="occurred_at_ms"] .sort')?.textContent).toBe("时间 UTC+8");
  expect(root.querySelector('tbody td[data-col="conversation_id"] .badge')).not.toBeNull();
  expect(root.querySelector('tbody td[data-col="key"]')).toBeNull();
  expect(summary(root)).toBe("显示 2／已加载 2 条。");
  expect(root.querySelector<HTMLElement>(".more")?.hidden).toBe(true);
});

test("加载更多带上游标并追加，重复的行只留一条，取完后隐藏按钮", async () => {
  const { calls, root } = await mount([
    page([item("a")], "10:k"),
    page([item("a"), item("b")]),
  ]);
  expect(summary(root)).toContain("还有更早的消息未加载，筛选与排序只含已加载的行。");
  button(root, "加载更多").click();
  await settled(root, calls, 2);
  expect(calls.at(1)?.body).toEqual({ limit: 200, cursor: "10:k" });
  expect(column(root, "text")).toEqual(["text-a", "text-b"]);
  expect(root.querySelector<HTMLElement>(".more")?.hidden).toBe(true);
});

test("刷新重取第一页并替换列表，保留排序与筛选", async () => {
  const { calls, root } = await mount([
    page([item("a", { text: "b" }), item("b", { text: "a" })]),
    page([item("c", { text: "z" }), item("d", { text: "y" })]),
  ]);
  type(root, 'tr.filters th[data-col="text"] input', "y");
  root.querySelector<HTMLElement>('tr.titles th[data-col="text"] button')?.click();
  button(root, "刷新").click();
  await settled(root, calls, 2);
  expect(calls.at(1)?.body).toEqual({ limit: 200 });
  expect(column(root, "text")).toEqual(["y"]);
});

test("出错时只显示错误 code 并保留已有列表", async () => {
  const { calls, root } = await mount([
    page([item("a")]),
    { body: { code: "storage" }, status: 503 },
  ]);
  button(root, "刷新").click();
  await settled(root, calls, 2);
  expect(root.querySelector(".error")?.textContent).toBe("storage");
  expect(column(root, "text")).toEqual(["text-a"]);
});

test("网络失败时显示 unreachable，没有数据时给出空状态", async () => {
  const root = document.createElement("div");
  const client = connect("http://localhost", async () => {
    throw new Error("down");
  });
  await renderMessages(client, root);
  expect(root.querySelector(".error")?.textContent).toBe("unreachable");
  expect(summary(root)).toBe("没有消息。");
  expect(root.querySelector<HTMLElement>(".more")?.hidden).toBe(true);
});

test("发送人为空时显示 sender_id，正文为空时显示占位", async () => {
  const { root } = await mount([
    page([item("a", { sender_name: "", sender_id: "u9", text: "" })]),
  ]);
  expect(column(root, "sender_name")).toEqual(["u9"]);
  expect(column(root, "text")).toEqual(["[非文本消息]"]);
});

test("正文按文本写入，不解析 HTML，单元格里没有链接元素", async () => {
  const hostile = `<img src=x onerror="alert(1)"><script>alert(2)</script>`;
  const text = `${hostile} [屏蔽报警](https://example.com/a?token=SECRET)`;
  const { root } = await mount([page([item("a", { text })])]);
  expect(column(root, "text")).toEqual([`${hostile} 屏蔽报警`]);
  expect(root.querySelector("img")).toBeNull();
  expect(root.querySelector("script")).toBeNull();
  expect(root.querySelector("tbody a")).toBeNull();
});

test("点击表头依次升序、降序、恢复默认，时间列默认标为降序", async () => {
  const { root } = await mount([
    page([item("a", { text: "b" }), item("b", { text: "a" }), item("c", { text: "c" })]),
  ]);
  const sort = (key: string) =>
    root.querySelector<HTMLElement>(`tr.titles th[data-col="${key}"] button`)?.click();
  const mark = (key: string) =>
    root.querySelector(`tr.titles th[data-col="${key}"]`)?.getAttribute("aria-sort");
  expect(mark("occurred_at_ms")).toBe("descending");
  expect(mark("text")).toBe("none");
  sort("text");
  expect(column(root, "text")).toEqual(["a", "b", "c"]);
  expect(mark("text")).toBe("ascending");
  expect(mark("occurred_at_ms")).toBe("none");
  sort("text");
  expect(column(root, "text")).toEqual(["c", "b", "a"]);
  expect(mark("text")).toBe("descending");
  sort("text");
  expect(column(root, "text")).toEqual(["b", "a", "c"]);
  expect(mark("occurred_at_ms")).toBe("descending");
});

test("列筛选与全局搜索叠加，状态栏显示命中数，清空两者恢复全部", async () => {
  const { root } = await mount([
    page([
      item("a", { text: "Disk full", sender_name: "alice" }),
      item("b", { text: "Disk ok", sender_name: "bob" }),
      item("c", { text: "cpu high", sender_name: "bob" }),
    ]),
  ]);
  type(root, 'tr.filters th[data-col="text"] input', "disk");
  expect(column(root, "text")).toEqual(["Disk full", "Disk ok"]);
  expect(summary(root)).toBe("显示 2／已加载 3 条。");
  type(root, ".search", "bob");
  expect(column(root, "text")).toEqual(["Disk ok"]);
  erase(root, "text")?.click();
  expect(column(root, "text")).toEqual(["Disk ok", "cpu high"]);
  type(root, ".search", "");
  expect(column(root, "text")).toEqual(["Disk full", "Disk ok", "cpu high"]);
  expect(root.querySelector<HTMLInputElement>(".search")?.value).toBe("");
  expect(root.querySelector<HTMLInputElement>('tr.filters th[data-col="text"] input')?.value).toBe("");
});

test("新增字段 zzz 前端零改动即出现列、可排序、可筛选", async () => {
  const { root } = await mount([
    page([item("a", { zzz: 10 }), item("b", { zzz: 9 }), item("c", { zzz: 100 })]),
  ]);
  expect(root.querySelector('tr.titles th[data-col="zzz"] .sort')?.textContent).toBe("zzz");
  root.querySelector<HTMLElement>('tr.titles th[data-col="zzz"] button')?.click();
  expect(column(root, "zzz")).toEqual(["9", "10", "100"]);
  type(root, 'tr.filters th[data-col="zzz"] input', ">9");
  expect(column(root, "zzz")).toEqual(["10", "100"]);
});

test("列菜单可显示默认隐藏的列", async () => {
  const { root } = await mount([page([item("a")])]);
  const box = [...root.querySelectorAll<HTMLInputElement>(".menu input")].find(
    (entry) => entry.parentElement?.textContent === "key",
  );
  expect(box?.checked).toBe(false);
  if (box) {
    box.checked = true;
    box.dispatchEvent(new Event("change", { bubbles: true }));
  }
  expect(column(root, "key")).toEqual(["a"]);
  expect(root.querySelector('tr.titles th[data-col="key"]')).not.toBeNull();
});

test("时间范围按预设与自定义起止作为请求参数，起止无效时不请求", async () => {
  const { calls, root } = await mount([page([item("a")]), page([]), page([]), page([])]);
  const before = Date.now();
  button(root, "1 小时").click();
  await settled(root, calls, 2);
  const from = calls.at(1)?.body.from_ms;
  expect(typeof from).toBe("number");
  expect(Number(from)).toBeGreaterThanOrEqual(before - 3_600_000);
  expect(calls.at(1)?.body.to_ms).toBeUndefined();
  expect(button(root, "1 小时").getAttribute("aria-pressed")).toBe("true");
  expect(summary(root)).toContain("时间范围：");
  type(root, "input.since", "2026-10-28T09:00", "change");
  type(root, "input.until", "2026-10-28T10:00", "change");
  await settled(root, calls, 4);
  expect(calls.at(3)?.body).toEqual({
    from_ms: Date.UTC(2026, 9, 28, 1, 0, 0),
    limit: 200,
    to_ms: Date.UTC(2026, 9, 28, 2, 0, 0),
  });
  expect(button(root, "全部").getAttribute("aria-pressed")).toBe("false");
  type(root, "input.until", "2026-10-28T08:00", "change");
  expect(root.querySelector(".note")?.textContent).toBe("起止时间无效");
  expect(calls).toHaveLength(4);
  button(root, "全部").click();
  await settled(root, calls, 5);
  expect(calls.at(4)?.body).toEqual({ limit: 200 });
  expect(root.querySelector(".note")?.textContent).toBe("");
});

test("加载全部翻到取完为止", async () => {
  const { calls, root } = await mount([
    page([item("a")], "1:a"),
    page([item("b")], "2:b"),
    page([item("c")]),
  ]);
  button(root, "加载全部").click();
  await settled(root, calls, 3);
  expect(column(root, "text")).toEqual(["text-a", "text-b", "text-c"]);
  expect(root.querySelector<HTMLElement>(".all")?.hidden).toBe(true);
});

test("加载全部到上限停止并提示缩小时间范围，出错即停", async () => {
  const pages = Array.from({ length: 12 }, (_, index) =>
    page(Array.from({ length: 200 }, (_, row) => item(`k${index}-${row}`)), `${index}:k`),
  );
  const { calls, root } = await mount(pages);
  button(root, "加载全部").click();
  await settled(root, calls, 10);
  expect(root.querySelectorAll("tbody tr")).toHaveLength(2000);
  expect(summary(root)).toContain("已达 2000 条上限，请缩小时间范围。");
  expect(root.querySelector<HTMLElement>(".more")?.hidden).toBe(true);
  const failing = await mount([page([item("a")], "1:a"), { body: { code: "storage" }, status: 503 }]);
  button(failing.root, "加载全部").click();
  await settled(failing.root, failing.calls, 2);
  expect(failing.root.querySelector(".error")?.textContent).toBe("storage");
  expect(failing.calls).toHaveLength(2);
});

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

function erase(root: HTMLElement, key: string) {
  return root.querySelector<HTMLButtonElement>(`tr.titles th[data-col="${key}"] .clear`);
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

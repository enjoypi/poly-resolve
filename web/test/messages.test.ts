import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import spec from "../../api/openapi.json";
import { connect } from "../src/client";
import { renderMessages } from "../src/messages";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const path = Object.keys(spec.paths).find((candidate) => candidate.endsWith("/list")) ?? "";
const NOON = Date.UTC(2026, 9, 28, 1, 20, 9);

type Page = { items: unknown[]; next_cursor: string | null };

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

function stub(replies: { body: unknown; status?: number }[]) {
  const calls: { path: string; body: unknown }[] = [];
  const fetch = async (request: Request) => {
    calls.push({ path: new URL(request.url).pathname, body: await request.json() });
    const reply = replies[Math.min(calls.length, replies.length) - 1];
    return Response.json(reply?.body, { status: reply?.status ?? 200 });
  };
  return { calls, client: connect("http://localhost", fetch) };
}

async function until(check: () => boolean) {
  for (let attempt = 0; attempt < 200; attempt++) {
    if (check()) return;
    await Bun.sleep(5);
  }
  throw new Error("timeout");
}

function buttons(root: HTMLElement) {
  const all = [...root.querySelectorAll("button")];
  const find = (label: string) => all.find((entry) => entry.textContent === label);
  return { more: find("加载更多"), refresh: find("刷新") };
}

function texts(root: HTMLElement, kind: string) {
  return [...root.querySelectorAll(`.${kind}`)].map((entry) => entry.textContent);
}

async function mount(replies: { body: unknown; status?: number }[]) {
  const { calls, client } = stub(replies);
  const root = document.createElement("div");
  await renderMessages(client, root, "Asia/Shanghai");
  return { calls, client, root };
}

test("请求路径取自 spec，首次不带游标，列表按返回顺序显示", async () => {
  const page: Page = { items: [item("a"), item("b")], next_cursor: null };
  const { calls, root } = await mount([{ body: page }]);
  expect(calls).toEqual([{ path, body: {} }]);
  expect(texts(root, "text")).toEqual(["text-a", "text-b"]);
  expect(texts(root, "time")[0]).toBe("2026-10-28 09:20:09 UTC+8");
  expect(texts(root, "sender")).toEqual(["alice", "alice"]);
  expect(texts(root, "conversation")).toEqual(["c1", "c1"]);
  expect(buttons(root).more?.hidden).toBe(true);
});

test("加载更多带上游标并追加，取完后隐藏按钮", async () => {
  const { calls, root } = await mount([
    { body: { items: [item("a")], next_cursor: "10:k" } },
    { body: { items: [item("b")], next_cursor: null } },
  ]);
  const { more } = buttons(root);
  expect(more?.hidden).toBe(false);
  more?.click();
  await until(() => calls.length === 2 && !more?.disabled);
  expect(calls.at(1)?.body).toEqual({ cursor: "10:k" });
  expect(texts(root, "text")).toEqual(["text-a", "text-b"]);
  expect(more?.hidden).toBe(true);
});

test("刷新重取第一页并替换列表", async () => {
  const { calls, root } = await mount([
    { body: { items: [item("a")], next_cursor: "10:k" } },
    { body: { items: [item("c")], next_cursor: null } },
  ]);
  const { refresh } = buttons(root);
  refresh?.click();
  await until(() => calls.length === 2 && !refresh?.disabled);
  expect(calls.at(1)?.body).toEqual({});
  expect(texts(root, "text")).toEqual(["text-c"]);
});

test("出错时只显示错误 code 并保留已有列表", async () => {
  const { calls, root } = await mount([
    { body: { items: [item("a")], next_cursor: null } },
    { body: { code: "storage" }, status: 503 },
  ]);
  const { refresh } = buttons(root);
  refresh?.click();
  await until(() => calls.length === 2 && !refresh?.disabled);
  expect(root.querySelector("p")?.textContent).toBe("storage");
  expect(texts(root, "text")).toEqual(["text-a"]);
});

test("发送人为空时显示 sender_id，正文为空时显示占位", async () => {
  const page: Page = {
    items: [item("a", { sender_name: "", sender_id: "u9", text: "" })],
    next_cursor: null,
  };
  const { root } = await mount([{ body: page }]);
  expect(texts(root, "sender")).toEqual(["u9"]);
  expect(texts(root, "text")).toEqual(["[非文本消息]"]);
});

test("正文按文本写入，不解析 HTML", async () => {
  const hostile = `<img src=x onerror="alert(1)"><script>alert(2)</script>`;
  const page: Page = { items: [item("a", { text: hostile })], next_cursor: null };
  const { root } = await mount([{ body: page }]);
  expect(texts(root, "text")).toEqual([hostile]);
  expect(root.querySelector("img")).toBeNull();
  expect(root.querySelector("script")).toBeNull();
});

test("网络失败时显示 unreachable", async () => {
  const root = document.createElement("div");
  const client = connect("http://localhost", async () => {
    throw new Error("down");
  });
  await renderMessages(client, root);
  expect(root.querySelector("p")?.textContent).toBe("unreachable");
  expect(buttons(root).more?.hidden).toBe(true);
});

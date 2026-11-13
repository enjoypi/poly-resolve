import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import spec from "../../api/openapi.json";
import { connect } from "../src/client";
import { TEXT } from "../src/config";
import { renderTopic } from "../src/topic";
import { renderTopics } from "../src/topics";
import { item as message, mount, page } from "./rig";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const NOW = Date.UTC(2026, 9, 28, 1, 20, 9);
const paths = Object.keys(spec.paths);
const route = (suffix: string) => paths.find((candidate) => candidate.endsWith(suffix)) ?? "";

type Call = { path: string; query: Record<string, string> };
type Reply = { body: unknown; status?: number };

function topic(id: string, patch: Record<string, unknown> = {}) {
  return {
    conversations: 2,
    first_at_ms: NOW - 3_600_000,
    label: `label-${id}`,
    last_at_ms: NOW,
    members: 4,
    people: 3,
    previous: 0.5,
    score: 0.75,
    sources: ["imap", "slack"],
    state: "open",
    topic_id: id,
    urgency: 0.5,
    verdict: `verdict-${id} <b>x</b>`,
    version: "abc123",
    ...patch,
  };
}

function entry(key: string, patch: Record<string, unknown> = {}) {
  return {
    conversation_id: "c1",
    key,
    occurred_at_ms: NOW,
    purged_at_ms: 0,
    reason: "",
    role: "",
    sender: "alice",
    source: "imap",
    state: "approved",
    text: `text-${key}`,
    via: "clue",
    ...patch,
  };
}

function overview() {
  return {
    items: [topic("t/1"), topic("t2", { label: "", previous: null, score: null, state: "odd", urgency: null, verdict: "" }), topic("t3", { previous: 0.9 }), topic("t4", { previous: 0.75 })],
    lines: [
      { count: 5, reason: "", state: "approved" },
      { count: 2, reason: "sealed", state: "withheld" },
      { count: 1, reason: "why", state: "weird" },
    ],
    topics: [
      { count: 1, reason: "", state: "open" },
      { count: 1, reason: "", state: "odd" },
    ],
  };
}

function detail() {
  return {
    changes: [
      { at_ms: NOW, from_topic: "t9", keys: 2, kind: "move", reason: "mail", serial: 1, state: "applied", to_topic: "t/1" },
      { at_ms: NOW, from_topic: "t8", keys: 0, kind: "fuse", reason: "", serial: 2, state: "applied", to_topic: "t/1" },
    ],
    timeline: [
      entry("a", { role: "own" }),
      entry("b", { reason: "sealed", source: "dingtalk", state: "withheld", text: "", via: "new" }),
      entry("c", { reason: "", role: "bot", source: "slack", state: "queued", via: "odd" }),
      entry("d", { purged_at_ms: NOW, text: "" }),
    ],
    topic: topic("t/1"),
  };
}

function stub(replies: Reply[]) {
  const calls: Call[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    calls.push({ path: url.pathname, query: Object.fromEntries(url.searchParams) });
    const reply = replies[Math.min(calls.length, replies.length) - 1];
    return Response.json(reply?.body, { status: reply?.status ?? 200 });
  };
  return { calls, client: connect("http://localhost", fetch) };
}

async function until(check: () => boolean) {
  for (let attempt = 0; attempt < 400; attempt++) {
    if (check()) return;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error("condition not met");
}

const cards = (root: HTMLElement) => root.querySelectorAll<HTMLElement>(".review");
const metas = (root: HTMLElement) => [...root.querySelectorAll(".meta")].map((node) => node.textContent ?? "");

test("话题列表：标题链到详情，显示状态、优先级构成、来源、最近进展、最新分析、计数与盲区", async () => {
  const { calls, client } = stub([{ body: overview() }]);
  const root = document.createElement("div");
  await renderTopics(client, root, "UTC");
  expect(calls).toEqual([{ path: route("/topics"), query: { limit: "200", state: "" } }]);
  expect(cards(root)).toHaveLength(4);
  const titles = [...root.querySelectorAll<HTMLAnchorElement>(".title")];
  expect(titles.map((node) => node.textContent)).toEqual(["label-t/1", "t2", "label-t3", "label-t4"]);
  expect(titles[0]?.getAttribute("href")).toBe("#topic/t%2F1");
  const [first = "", second = "", third = "", fourth = ""] = metas(root);
  expect(third).toContain("优先级 0.75↓（紧急 0.50）");
  expect(fourth).toContain("优先级 0.75（紧急 0.50）");
  for (const part of ["进行中", "优先级 0.75↑（紧急 0.50）", "3 人 2 个会话", "邮箱、slack", "4 条消息", "始于 2026-10-28 00:20:09", "最近 2026-10-28 01:20:09"]) {
    expect(first).toContain(part);
  }
  expect(second).toContain("odd");
  expect(second).toContain(`优先级 ${TEXT.none}（紧急 ${TEXT.none}）`);
  expect([...root.querySelectorAll(".payload")].map((node) => node.textContent)).toEqual(["verdict-t/1 <b>x</b>", "verdict-t3 <b>x</b>", "verdict-t4 <b>x</b>"]);
  expect(root.querySelector("b")).toBeNull();
  expect(root.querySelector(".count")?.textContent).toBe("话题：进行中 1 · odd 1");
  expect(root.querySelector(".lines")?.textContent).toBe("消息：已外发 5 · 未外发（仍是密文） 2 · weird（why） 1");
  expect(root.querySelector(".blind")?.textContent).toBe(TEXT.blind);
});

test("话题列表：按状态筛选与刷新重新请求，空、出错与网络失败", async () => {
  const { calls, client } = stub([
    { body: overview() },
    { body: { items: [], lines: [], topics: [] } },
    { body: { code: "unavailable" }, status: 503 },
  ]);
  const root = document.createElement("div");
  await renderTopics(client, root, "UTC");
  const pick = root.querySelector(".state") as HTMLSelectElement;
  expect(Array.from(pick.options, (node) => node.value)).toEqual(["", "open", "resolved", "cancelled", "archived"]);
  pick.value = "resolved";
  pick.dispatchEvent(new Event("change"));
  await until(() => root.querySelector(".count")?.textContent === TEXT.noTopics);
  expect(calls[1]?.query).toEqual({ limit: "200", state: "resolved" });
  expect(root.querySelector(".lines")?.textContent).toBe("");
  (root.querySelector(".refresh") as HTMLElement).click();
  await until(() => root.querySelector(".error")?.textContent === "unavailable");
  expect((root.querySelector(".back") as HTMLAnchorElement).getAttribute("href")).toBe("#");
  const down = connect("http://localhost", () => Promise.reject(new Error("down")));
  const other = document.createElement("div");
  await renderTopics(down, other);
  expect(other.querySelector(".error")?.textContent).toBe(TEXT.unreachable);
});

test("话题详情：跨来源时间线标出未外发成员及原因，列出自动调整记录", async () => {
  const { calls, client } = stub([{ body: detail() }]);
  const root = document.createElement("div");
  await renderTopic(client, root, "t/1", "UTC");
  expect(calls).toEqual([{ path: route("/topics/{topic_id}").replace("{topic_id}", "t%2F1"), query: {} }]);
  expect(root.querySelector(".title")?.textContent).toBe("label-t/1");
  expect([...root.querySelectorAll(".section")].map((node) => node.textContent)).toEqual([
    TEXT.timeline(4),
    TEXT.changes(2),
  ]);
  expect([...cards(root)].map((node) => node.className)).toEqual([
    "review topic",
    "review",
    "review flagged",
    "review flagged",
    "review",
    "review change",

    "review change",
  ]);
  const [head = "", own = "", sealed = "", queued = "", , moved = "", fused = ""] = metas(root);
  expect(head).toContain(TEXT.version("abc123"));
  for (const part of ["2026-10-28 01:20:09", "邮箱", "c1", "alice", "本人", "强线索"]) expect(own).toContain(part);
  for (const part of ["钉钉", "新话题", "未外发：仍是密文"]) expect(sealed).toContain(part);
  expect(queued).toContain("slack");
  expect(queued).toContain("bot");
  expect(queued).toContain("odd");
  expect(queued.endsWith("排队中")).toBe(true);
  for (const part of ["移动", "t9 → t/1（2 条）", "邮件回复链", "applied"]) expect(moved).toContain(part);
  expect(fused).toContain("fuse");
  expect(fused).not.toContain(" ·  · ");
  expect([...root.querySelectorAll(".payload")].map((node) => node.textContent)).toEqual([
    "verdict-t/1 <b>x</b>",
    "text-a",
    "text-c",
    TEXT.purged,
  ]);
  expect((root.querySelector(".back") as HTMLAnchorElement).getAttribute("href")).toBe("#topics");
});

test("话题详情：没有分析时不放正文框，刷新重取，出错与网络失败", async () => {
  const bare = { changes: [], timeline: [], topic: topic("t2", { verdict: "", version: "" }) };
  const { calls, client } = stub([{ body: bare }, { body: { code: "not_found" }, status: 404 }]);
  const root = document.createElement("div");
  await renderTopic(client, root, "t2", "UTC");
  expect(root.querySelector(".payload")).toBeNull();
  (root.querySelector(".refresh") as HTMLElement).click();
  await until(() => root.querySelector(".error")?.textContent === "not_found");
  expect(calls).toHaveLength(2);
  const down = connect("http://localhost", () => Promise.reject(new Error("down")));
  const other = document.createElement("div");
  await renderTopic(down, other, "t2");
  expect(other.querySelector(".error")?.textContent).toBe(TEXT.unreachable);
});

test("消息详情里的话题链到话题详情，没有话题时不放链接", async () => {
  const { root } = await mount([page([message("a", { topic: "t/1" }), message("b", { topic: "" })])]);
  document.body.append(root);
  const rows = root.querySelectorAll<HTMLElement>("tbody tr");
  rows[0]?.click();
  const link = root.querySelector<HTMLAnchorElement>("dialog dd a");
  expect(link?.textContent).toBe("t/1");
  expect(link?.getAttribute("href")).toBe("#topic/t%2F1");
  root.querySelector("dialog")?.close();
  rows[1]?.click();
  expect(root.querySelector("dialog dd a")).toBeNull();
  root.remove();
});

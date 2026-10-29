import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import spec from "../../api/openapi.json";
import { connect } from "../src/client";
import { REVIEWER_KEY, TEXT } from "../src/config";
import { renderReviews } from "../src/reviews";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const NOW = Date.UTC(2026, 9, 28, 1, 20, 9);
const paths = Object.keys(spec.paths);
const route = (suffix: string) => paths.find((candidate) => candidate.endsWith(suffix)) ?? "";
const reasons = [
  { code: "leak", label: "漏脱敏", placeholder: "[x]" },
  { code: "noise", label: "排除", placeholder: null },
];

type Call = { body: Record<string, unknown>; path: string; user: string | null };
type Reply = { body: unknown; status?: number };

function item(key: string, patch: Record<string, unknown> = {}) {
  return {
    batch_id: 1,
    conversation_id: "c1",
    flag: "",
    key,
    occurred_at_ms: NOW,
    reason: "",
    reviewer: "",
    sender: "alice",
    state: "pending",
    text: `text-${key}`,
    ...patch,
  };
}

function listing() {
  return {
    items: [item("a"), item("b", { flag: "unmasked", text: "<script>alert(1)</script>" })],
    reasons,
  };
}

function stub(replies: Reply[]) {
  const calls: Call[] = [];
  const fetch = async (request: Request) => {
    calls.push({
      body: await request.json(),
      path: new URL(request.url).pathname,
      user: request.headers.get("x-user"),
    });
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

async function open(replies: Reply[]) {
  localStorage.clear();
  const { calls, client } = stub(replies);
  const root = document.createElement("div");
  await renderReviews(client, root, "UTC");
  return { calls, root };
}

function cards(root: HTMLElement) {
  return root.querySelectorAll<HTMLElement>(".review");
}

function click(root: HTMLElement, selector: string, index = 0) {
  (root.querySelectorAll(selector)[index] as HTMLElement).click();
}

function pickReason(root: HTMLElement, value: string, index = 0) {
  const pick = root.querySelectorAll(".reason")[index] as HTMLSelectElement;
  pick.value = value;
  pick.dispatchEvent(new Event("change"));
}

test("渲染待审行：正文只作文本，批、会话、发送人、时间与原因下拉齐全", async () => {
  const { calls, root } = await open([{ body: listing() }]);
  expect(calls.map((call) => call.path)).toEqual([route("review/list")]);
  expect(cards(root)).toHaveLength(2);
  expect(root.querySelector(".count")?.textContent).toBe(TEXT.pending(2));
  const meta = root.querySelector(".meta")?.textContent ?? "";
  for (const part of [TEXT.batch(1), "c1", "alice", "2026-10-28 01:20:09"]) expect(meta).toContain(part);
  expect(root.querySelectorAll(".payload")[1]?.textContent).toBe("<script>alert(1)</script>");
  expect(cards(root)[0]?.className).toBe("review");
  expect(cards(root)[1]?.className).toBe("review flagged");
  expect(root.querySelectorAll(".meta")[1]?.textContent).toContain(TEXT.flagged("unmasked"));
  expect(root.querySelector("script")).toBeNull();
  const options = [...(cards(root)[0]?.querySelectorAll("option") ?? [])].map((node) => node.textContent);
  expect(options).toEqual([TEXT.reasonPick, "漏脱敏", "排除"]);
});

test("通过：带审核人请求头，成功后卡片移除", async () => {
  const { calls, root } = await open([{ body: listing() }, { body: { states: ["approved"] } }]);
  const reviewer = root.querySelector(".reviewer") as HTMLInputElement;
  reviewer.value = " alice ";
  reviewer.dispatchEvent(new Event("change"));
  expect(localStorage.getItem(REVIEWER_KEY)).toBe("alice");
  click(root, ".approve");
  await until(() => cards(root).length === 1);
  expect(calls[1]).toEqual({ body: { keys: ["a"] }, path: route("review/approve"), user: "alice" });
  expect(root.querySelector(".count")?.textContent).toBe(TEXT.pending(1));
});

test("通过：未填审核人时不带请求头，失败时保留卡片并显示错误码", async () => {
  const { calls, root } = await open([
    { body: listing() },
    { body: { code: "bad_request" }, status: 400 },
  ]);
  click(root, ".approve");
  await until(() => root.querySelector(".error")?.textContent === "bad_request");
  expect(cards(root)).toHaveLength(2);
  expect(calls[1]?.user).toBeNull();
});

test("整批通过：一次提交全部键，只移除已通过的卡片", async () => {
  const { calls, root } = await open([{ body: listing() }, { body: { states: ["approved", "rejected"] } }]);
  click(root, ".approve-all");
  await until(() => cards(root).length === 1);
  expect(calls[1]?.body).toEqual({ keys: ["a", "b"] });
  expect(cards(root)[0]?.querySelector(".payload")?.textContent).toContain("script");
});

test("整批通过：失败显示错误码，网络失败显示 unreachable", async () => {
  const failed = await open([{ body: listing() }, { body: { code: "unavailable" }, status: 503 }]);
  click(failed.root, ".approve-all");
  await until(() => failed.root.querySelector(".error")?.textContent === "unavailable");
  expect(cards(failed.root)).toHaveLength(2);
  const down = connect("http://localhost", async (request: Request) =>
    request.url.endsWith("/list") ? Response.json(listing()) : Promise.reject(new Error("down")),
  );
  const mixed = document.createElement("div");
  await renderReviews(down, mixed);
  click(mixed, ".approve-all");
  await until(() => mixed.querySelector(".error")?.textContent === TEXT.unreachable);
});

test("拒绝：选了不带占位符的原因即可点，请求不带原文", async () => {
  const { calls, root } = await open([{ body: listing() }, { body: { state: "rejected" } }]);
  const reject = root.querySelectorAll(".reject")[0] as HTMLButtonElement;
  const leak = root.querySelectorAll(".leak")[0] as HTMLTextAreaElement;
  const pick = root.querySelectorAll(".reason")[0] as HTMLSelectElement;
  expect(reject.disabled).toBe(true);
  expect(leak.hidden).toBe(true);
  pickReason(root, "noise");
  expect(reject.disabled).toBe(false);
  expect(leak.hidden).toBe(true);
  reject.click();
  await until(() => cards(root).length === 1);
  expect(calls[1]).toEqual({
    body: { key: "a", reason: "noise" },
    path: route("review/reject"),
    user: null,
  });
  pick.value = "";
  pick.dispatchEvent(new Event("change"));
  expect(reject.disabled).toBe(true);
});

test("拒绝：漏脱敏原因须粘贴原文，提交后清空输入框", async () => {
  const { calls, root } = await open([{ body: listing() }, { body: { code: "bad_request" }, status: 400 }]);
  const reject = root.querySelectorAll(".reject")[0] as HTMLButtonElement;
  const leak = root.querySelectorAll(".leak")[0] as HTMLTextAreaElement;
  pickReason(root, "leak");
  expect(leak.hidden).toBe(false);
  expect(leak.getAttribute("autocomplete")).toBe("off");
  expect(reject.disabled).toBe(true);
  leak.value = "  secret-abc ";
  leak.dispatchEvent(new Event("input"));
  expect(reject.disabled).toBe(false);
  reject.click();
  await until(() => root.querySelector(".error")?.textContent === "bad_request");
  expect(calls[1]?.body).toEqual({ key: "a", leak: "  secret-abc ", reason: "leak" });
  expect(leak.value).toBe("");
  expect(cards(root)).toHaveLength(2);
  expect(reject.disabled).toBe(false);
});

test("拒绝：网络失败显示 unreachable", async () => {
  const down = connect("http://localhost", async (request: Request) =>
    request.url.endsWith("/list") ? Response.json(listing()) : Promise.reject(new Error("down")),
  );
  const root = document.createElement("div");
  await renderReviews(down, root);
  pickReason(root, "noise");
  click(root, ".reject");
  await until(() => root.querySelector(".error")?.textContent === TEXT.unreachable);
});

test("空列表显示提示并隐藏整批按钮，刷新重新加载", async () => {
  const { calls, root } = await open([{ body: { items: [], reasons } }, { body: listing() }]);
  expect(root.querySelector(".count")?.textContent).toBe(TEXT.noReviews);
  expect((root.querySelector(".approve-all") as HTMLButtonElement).hidden).toBe(true);
  click(root, ".refresh");
  await until(() => cards(root).length === 2);
  expect((root.querySelector(".approve-all") as HTMLButtonElement).hidden).toBe(false);
  expect(calls).toHaveLength(2);
});

test("接口出错显示错误码，网络失败显示 unreachable", async () => {
  const failed = await open([{ body: { code: "unavailable" }, status: 503 }]);
  expect(failed.root.querySelector(".error")?.textContent).toBe("unavailable");
  const broken = document.createElement("div");
  const client = connect("http://localhost", async () => {
    throw new Error("down");
  });
  await renderReviews(client, broken);
  expect(broken.querySelector(".error")?.textContent).toBe(TEXT.unreachable);
  const down = connect("http://localhost", async (request: Request) =>
    request.url.endsWith("/list") ? Response.json(listing()) : Promise.reject(new Error("down")),
  );
  const mixed = document.createElement("div");
  await renderReviews(down, mixed);
  click(mixed, ".approve");
  await until(() => mixed.querySelector(".error")?.textContent === TEXT.unreachable);
});

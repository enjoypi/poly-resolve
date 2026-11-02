import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import spec from "../../api/openapi.json";
import { connect } from "../src/client";
import { PROMPTS, TEXT } from "../src/config";
import { renderPrompts } from "../src/prompts";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const paths = Object.keys(spec.paths);
const route = (suffix: string) => paths.find((candidate) => candidate.endsWith(suffix)) ?? "";

type Call = { body: unknown; method: string; path: string };
type Reply = { body: unknown; status?: number };

function listing() {
  return {
    items: [
      { name: "session", required: ["{messages}"], text: "ctx:{messages}" },
      { name: "custom", required: [], text: "<b>x</b>" },
    ],
  };
}

function stub(replies: Reply[]) {
  const calls: Call[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    calls.push({
      body: request.method === "GET" ? null : await request.json(),
      method: request.method,
      path: decodeURIComponent(url.pathname),
    });
    const reply = replies[calls.length - 1];
    if (reply === undefined) throw new Error("offline");
    return Response.json(reply.body, { status: reply.status ?? 200 });
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
  const { calls, client } = stub(replies);
  const root = document.createElement("div");
  await renderPrompts(client, root);
  return { calls, root };
}

function editor(root: HTMLElement, index = 0) {
  const card = root.querySelectorAll<HTMLElement>(".review")[index];
  if (!card) throw new Error("no card");
  return {
    card,
    revert: card.querySelector<HTMLButtonElement>(".revert"),
    save: card.querySelector<HTMLButtonElement>(".save"),
    state: card.querySelector(".state"),
    text: card.querySelector<HTMLTextAreaElement>("textarea"),
  };
}

function type(area: HTMLTextAreaElement | null, value: string) {
  if (!area) throw new Error("no textarea");
  area.value = value;
  area.dispatchEvent(new Event("input"));
}

test("每个提示词一张卡片：标签、说明、必含占位符与正文只作文本", async () => {
  const { calls, root } = await open([{ body: listing() }]);
  expect(calls.map((call) => call.path)).toEqual([route("/prompts")]);
  const first = editor(root);
  expect(first.card.querySelector("h2")?.textContent).toBe(PROMPTS.session?.label);
  expect(first.card.querySelector(".hint")?.textContent).toBe(PROMPTS.session?.hint);
  expect(first.card.querySelector(".mark")?.textContent).toBe("{messages}");
  expect(first.text?.value).toBe("ctx:{messages}");
  expect(first.save?.disabled).toBe(true);
  expect(first.revert?.hidden).toBe(true);
  const other = editor(root, 1);
  expect(other.card.querySelector("h2")?.textContent).toBe("custom");
  expect(other.text?.value).toBe("<b>x</b>");
  expect(root.querySelector("b")).toBeNull();
  expect(root.querySelector(".note")?.textContent).toBe(TEXT.promptNote);
});

test("缺占位符标红并禁用保存，撤销恢复加载时的正文", async () => {
  const { root } = await open([{ body: listing() }]);
  const first = editor(root);
  type(first.text, "lost");
  expect(first.card.querySelector(".mark")?.classList.contains("missing")).toBe(true);
  expect(first.save?.disabled).toBe(true);
  expect(first.state?.textContent).toBe(TEXT.dirty);
  type(first.text, "{messages}{messages}");
  expect(first.save?.disabled).toBe(true);
  type(first.text, "new {messages}");
  expect(first.card.querySelector(".mark")?.classList.contains("missing")).toBe(false);
  expect(first.save?.disabled).toBe(false);
  expect(first.revert?.hidden).toBe(false);
  first.revert?.click();
  expect(first.text?.value).toBe("ctx:{messages}");
  expect(first.save?.disabled).toBe(true);
  expect(first.state?.textContent).toBe("");
});

test("保存发 PATCH，成功后成为新的基准；失败显示错误码", async () => {
  const { calls, root } = await open([
    { body: listing() },
    { body: { name: "session" } },
    { body: { code: "refused" }, status: 422 },
  ]);
  const first = editor(root);
  type(first.text, "new {messages}");
  first.save?.click();
  await until(() => first.state?.textContent === TEXT.saved);
  expect(calls[1]).toEqual({ body: { text: "new {messages}" }, method: "PATCH", path: "/api/prompts/session" });
  expect(first.save?.disabled).toBe(true);
  expect(first.revert?.hidden).toBe(true);
  type(first.text, "again {messages}");
  first.save?.click();
  await until(() => first.state?.textContent === "refused");
  expect(first.save?.disabled).toBe(false);
});

test("有未保存修改时离开页面需确认", async () => {
  const { root } = await open([{ body: listing() }]);
  const leave = () => {
    const event = new Event("beforeunload", { cancelable: true });
    dispatchEvent(event);
    return event.defaultPrevented;
  };
  expect(leave()).toBe(false);
  type(editor(root).text, "new {messages}");
  expect(leave()).toBe(true);
  editor(root).revert?.click();
  expect(leave()).toBe(false);
});

test("读取失败与网络失败只显示错误码", async () => {
  const failed = await open([{ body: { code: "unavailable" }, status: 503 }]);
  expect(failed.root.querySelector(".error")?.textContent).toBe("unavailable");
  const offline = await open([]);
  expect(offline.root.querySelector(".error")?.textContent).toBe(TEXT.unreachable);
  const { root } = await open([{ body: listing() }]);
  const first = editor(root);
  type(first.text, "new {messages}");
  first.save?.click();
  await until(() => first.state?.textContent === TEXT.unreachable);
});

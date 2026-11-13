import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import spec from "../../api/openapi.json";
import { connect } from "../src/client";
import { TEXT } from "../src/config";
import { renderOutbound } from "../src/outbound";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const NOW = Date.UTC(2026, 9, 28, 1, 20, 9);
const paths = Object.keys(spec.paths);
const route = (suffix: string) => paths.find((candidate) => candidate.endsWith(suffix)) ?? "";

type Call = { method: string; path: string; query: Record<string, string> };
type Reply = { body: unknown; status?: number };

function item(key: string, patch: Record<string, unknown> = {}) {
  return {
    account: "hf",
    batch_id: 1,
    conversation_id: "c1",
    flag: "",
    key,
    occurred_at_ms: NOW,
    reason: "",
    role: "",
    sender: "alice",
    source: "imap",
    state: "approved",
    text: `text-${key}`,
    version: "abc123",
    ...patch,
  };
}

function listing() {
  return {
    items: [
      item("a", { role: "own", text: "pin [x] [n#a1b2] <script>alert(1)</script>" }),
      item("b", { reason: "sealed", sender: "", state: "withheld", text: "", version: "" }),
      item("c", { reason: "odd", role: "bot", source: "slack", state: "withheld" }),
    ],
    placeholders: ["[x]", "[n#{hash}]"],
  };
}

function stub(replies: Reply[]) {
  const calls: Call[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    calls.push({
      method: request.method,
      path: url.pathname,
      query: Object.fromEntries(url.searchParams),
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
  const { calls, client } = stub(replies);
  const root = document.createElement("div");
  await renderOutbound(client, root, "UTC");
  return { calls, root };
}

const cards = (root: HTMLElement) => root.querySelectorAll<HTMLElement>(".review");

test("只读列出外发记录：来源、会话、发送人、时间、角色与占位符标红", async () => {
  const { calls, root } = await open([{ body: listing() }]);
  expect(calls).toEqual([
    { method: "GET", path: route("/outbound"), query: { limit: "200", state: "approved" } },
  ]);
  expect(cards(root)).toHaveLength(3);
  expect(root.querySelector(".count")?.textContent).toBe(TEXT.outboundCount(3));
  const meta = root.querySelector(".meta")?.textContent ?? "";
  for (const part of [TEXT.batch(1), TEXT.version("abc123"), "邮箱 hf", "c1", "alice", "2026-10-28 01:20:09", "本人"]) {
    expect(meta).toContain(part);
  }
  expect(root.querySelector(".payload")?.textContent).toBe("pin [x] [n#a1b2] <script>alert(1)</script>");
  expect([...root.querySelectorAll(".masked")].map((mark) => mark.textContent)).toEqual(["[x]", "[n#a1b2]"]);
  expect(root.querySelector("script")).toBeNull();
  expect(root.querySelectorAll("button")).toHaveLength(1);
  expect(root.querySelector("textarea")).toBeNull();
});

test("未外发的记录高亮并写明原因，未知来源、角色与原因原样显示", async () => {
  const { root } = await open([{ body: listing() }]);
  expect(cards(root)[0]?.className).toBe("review");
  expect(cards(root)[1]?.className).toBe("review flagged");
  const sealed = cards(root)[1]?.querySelector(".meta")?.textContent ?? "";
  expect(sealed).toContain(TEXT.held("仍是密文"));
  expect(sealed).not.toContain(" ·  · ");
  const odd = cards(root)[2]?.querySelector(".meta")?.textContent ?? "";
  for (const part of ["slack hf", "bot", TEXT.held("odd")]) expect(odd).toContain(part);
});

test("切换状态与刷新都重新请求，空列表显示提示且不标红", async () => {
  const { calls, root } = await open([
    { body: listing() },
    { body: { items: [], placeholders: [] } },
    { body: { items: [item("z")], placeholders: [] } },
  ]);
  const pick = root.querySelector(".state") as HTMLSelectElement;
  expect(Array.from(pick.options, (node) => node.value)).toEqual(["approved", "withheld"]);
  pick.value = "withheld";
  pick.dispatchEvent(new Event("change"));
  await until(() => root.querySelector(".count")?.textContent === TEXT.noOutbound);
  expect(calls[1]?.query).toEqual({ limit: "200", state: "withheld" });
  (root.querySelector(".refresh") as HTMLElement).click();
  await until(() => cards(root).length === 1);
  expect(root.querySelector(".masked")).toBeNull();
  expect((root.querySelector(".back") as HTMLAnchorElement).getAttribute("href")).toBe("#");
});

test("失败显示错误码，网络失败显示 unreachable", async () => {
  const failed = await open([{ body: { code: "unavailable" }, status: 503 }]);
  expect(failed.root.querySelector(".error")?.textContent).toBe("unavailable");
  expect(cards(failed.root)).toHaveLength(0);
  const down = connect("http://localhost", () => Promise.reject(new Error("down")));
  const root = document.createElement("div");
  await renderOutbound(down, root);
  expect(root.querySelector(".error")?.textContent).toBe(TEXT.unreachable);
});

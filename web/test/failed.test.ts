import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import { connect } from "../src/client";
import { TEXT } from "../src/config";
import { renderFailed } from "../src/failed";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const NOW = Date.UTC(2026, 9, 28, 1, 20, 9);

type Call = { body: unknown; method: string; path: string };
type Reply = { body: unknown; status?: number };

function batch(id: number) {
  return { attempts: 3, batch_id: id, lines: 2, reason: "http:500", state: "failed", updated_at_ms: NOW };
}

function stub(replies: Reply[]) {
  const calls: Call[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    calls.push({
      body: request.method === "GET" ? Object.fromEntries(url.searchParams) : await request.json(),
      method: request.method,
      path: url.pathname,
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
  await renderFailed(client, root, "UTC");
  return { calls, root };
}

const cards = (root: HTMLElement) => root.querySelectorAll(".review");

test("列出失败批：批号、时间、尝试次数、条数与原因", async () => {
  const { calls, root } = await open([{ body: { items: [batch(1), batch(2)] } }]);
  expect(calls[0]).toEqual({ body: { limit: "200" }, method: "GET", path: "/api/batches" });
  expect(cards(root)).toHaveLength(2);
  expect(root.querySelector(".count")?.textContent).toBe(TEXT.failedCount(2));
  expect(root.querySelector(".meta")?.textContent).toBe(`${TEXT.batch(1)} · 2026-10-28 01:20:09 UTC+0`);
  expect(root.querySelector(".payload")?.textContent).toBe(TEXT.failedMeta(3, 2, "http:500"));
});

test("重发：成功移除卡片，失败显示错误码", async () => {
  const { calls, root } = await open([
    { body: { items: [batch(1), batch(2)] } },
    { body: { state: "ready" } },
    { body: { code: "clash" }, status: 409 },
  ]);
  (root.querySelectorAll(".resend")[0] as HTMLElement).click();
  await until(() => cards(root).length === 1);
  expect(calls[1]).toEqual({ body: { state: "ready" }, method: "PATCH", path: "/api/batches/1" });
  (root.querySelector(".resend") as HTMLElement).click();
  await until(() => root.querySelector(".error")?.textContent === "clash");
  expect(cards(root)).toHaveLength(1);
});

test("空列表、读取失败与网络失败", async () => {
  const empty = await open([{ body: { items: [] } }]);
  expect(empty.root.querySelector(".count")?.textContent).toBe(TEXT.noFailed);
  const broken = await open([{ body: { code: "unavailable" }, status: 503 }]);
  expect(broken.root.querySelector(".error")?.textContent).toBe("unavailable");
  const down = connect("http://localhost", async (request: Request) =>
    request.method === "GET" ? Response.json({ items: [batch(1)] }) : Promise.reject(new Error("down")),
  );
  const root = document.createElement("div");
  await renderFailed(down, root);
  (root.querySelector(".resend") as HTMLElement).click();
  await until(() => root.querySelector(".error")?.textContent === TEXT.unreachable);
  const offline = document.createElement("div");
  await renderFailed(connect("http://localhost", () => Promise.reject(new Error("down"))), offline);
  expect(offline.querySelector(".error")?.textContent).toBe(TEXT.unreachable);
  (root.querySelector(".refresh") as HTMLElement).click();
});

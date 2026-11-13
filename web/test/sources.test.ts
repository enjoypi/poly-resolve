import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import { connect } from "../src/client";
import { TEXT } from "../src/config";
import { renderSources } from "../src/sources";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const NOW = Date.UTC(2026, 9, 28, 1, 20, 9);

type Reply = { body: unknown; status?: number };

function health() {
  return {
    sources: [
      { account: "box", down_at_ms: NOW, kind: "imap", last_at_ms: null, ok_at_ms: 0, state: "retrying", streak: 3 },
      { account: "me", down_at_ms: 0, kind: "dingtalk", last_at_ms: NOW, ok_at_ms: NOW, state: "ok", streak: 0 },
      { account: "x", down_at_ms: NOW, kind: "odd", last_at_ms: null, ok_at_ms: NOW, state: "weird", streak: 1 },
    ],
    stages: [
      { kind: "analysis", streak: 2 },
      { kind: "notify", streak: 0 },
      { kind: "other", streak: 0 },
    ],
  };
}

function days() {
  return {
    days: [
      { calls: 4, date: "2026-10-28", failures: 1, input_tokens: 90, kind: "decision", output_tokens: 10 },
      { calls: 2, date: "2026-10-28", failures: 0, input_tokens: 0, kind: "other", output_tokens: 0 },
    ],
  };
}

function stub(routes: Record<string, Reply[]>) {
  const calls: string[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    calls.push(`${url.pathname}${url.search}`);
    const queue = routes[url.pathname] ?? [];
    const reply = queue.length > 1 ? queue.shift() : queue[0];
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

const metas = (root: HTMLElement) => [...root.querySelectorAll(".meta")].map((node) => node.textContent ?? "");

test("来源页：数据源状态、各环节连续失败与按日调用量", async () => {
  const { calls, client } = stub({ "/api/sources": [{ body: health() }], "/api/usage": [{ body: days() }] });
  const root = document.createElement("div");
  await renderSources(client, root, "UTC");
  expect(calls.sort()).toEqual(["/api/sources", "/api/usage?limit=14"]);
  expect([...root.querySelectorAll(".section")].map((node) => node.textContent)).toEqual([
    TEXT.sourcesTitle,
    TEXT.stagesTitle,
    TEXT.usageTitle,
  ]);
  const [down = "", up = "", odd = "", analysis = "", notify = "", other = "", spent = "", idle = ""] = metas(root);
  for (const part of ["邮箱 box", "重连中", TEXT.okAt(TEXT.none), TEXT.lastAt(TEXT.none), TEXT.downSince("2026-10-28 01:20:09 UTC+0", 3)]) {
    expect(down).toContain(part);
  }
  for (const part of ["钉钉 me", "正常", TEXT.lastAt("2026-10-28 01:20:09 UTC+0")]) expect(up).toContain(part);
  expect(up).not.toContain("起异常");
  for (const part of ["odd x", "weird"]) expect(odd).toContain(part);
  expect(analysis).toBe(TEXT.stage("分析", 2));
  expect(notify).toBe(TEXT.stage("通知", 0));
  expect(other).toBe(TEXT.stage("other", 0));
  for (const part of ["2026-10-28", "决策", TEXT.usage(4, 1, 90, 10)]) expect(spent).toContain(part);
  expect(idle).toContain("other");
  expect([...root.querySelectorAll(".review")].map((node) => node.className)).toEqual([
    "review flagged",
    "review",
    "review flagged",
    "review flagged",
    "review",
    "review",
    "review flagged",
    "review",
  ]);
});

test("来源页：没有记录、刷新、出错与网络失败", async () => {
  const empty = { sources: [], stages: [] };
  const { calls, client } = stub({
    "/api/sources": [{ body: empty }, { body: { code: "unavailable" }, status: 503 }, { body: empty }],
    "/api/usage": [{ body: { days: [] } }, { body: { days: [] } }, { body: { code: "invalid" }, status: 400 }],
  });
  const root = document.createElement("div");
  await renderSources(client, root, "UTC");
  expect([...root.querySelectorAll(".count")].map((node) => node.textContent)).toEqual([TEXT.noSources, TEXT.noUsage]);
  (root.querySelector(".refresh") as HTMLElement).click();
  await until(() => root.querySelector(".error")?.textContent === "unavailable");
  (root.querySelector(".refresh") as HTMLElement).click();
  await until(() => root.querySelector(".error")?.textContent === "invalid");
  expect(calls).toHaveLength(6);
  expect((root.querySelector(".back") as HTMLAnchorElement).getAttribute("href")).toBe("#");
  const down = connect("http://localhost", () => Promise.reject(new Error("down")));
  const other = document.createElement("div");
  await renderSources(down, other);
  expect(other.querySelector(".error")?.textContent).toBe(TEXT.unreachable);
});

import spec from "../../api/openapi.json";
import { connect } from "../src/client";
import { renderMessages } from "../src/messages";

export const path = Object.keys(spec.paths).find((candidate) => candidate.endsWith("/messages")) ?? "";
export const NOON = Date.UTC(2026, 9, 28, 1, 20, 9);

export type Reply = { body: unknown; status?: number };

export function item(key: string, patch: Record<string, unknown> = {}) {
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

export function stub(replies: Reply[]) {
  const calls: { path: string; body: Record<string, unknown> }[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    const query = [...url.searchParams].map(([name, value]) => [name, /^\d+$/.test(value) ? Number(value) : value]);
    calls.push({ path: url.pathname, body: Object.fromEntries(query) });
    const reply = replies[Math.min(calls.length, replies.length) - 1];
    return Response.json(reply?.body, { status: reply?.status ?? 200 });
  };
  return { calls, client: connect("http://localhost", fetch) };
}

export async function until(check: () => boolean) {
  for (let attempt = 0; attempt < 400; attempt++) {
    if (check()) return;
    await Bun.sleep(5);
  }
  throw new Error("timeout");
}

export function button(root: HTMLElement, label: string) {
  const found = [...root.querySelectorAll("button")].find((entry) => entry.textContent === label);
  if (!found) throw new Error(`no button ${label}`);
  return found;
}

export function column(root: HTMLElement, key: string) {
  return [...root.querySelectorAll(`tbody td[data-col="${key}"]`)].map(
    (entry) => (entry.querySelector(".text") ?? entry).textContent,
  );
}

export function type(root: HTMLElement, selector: string, value: string, event = "input") {
  const input = root.querySelector<HTMLInputElement>(selector);
  if (!input) throw new Error(`no input ${selector}`);
  input.value = value;
  input.dispatchEvent(new Event(event, { bubbles: true }));
}

export async function mount(replies: Reply[]) {
  const { calls, client } = stub(replies);
  const root = document.createElement("div");
  await renderMessages(client, root, "Asia/Shanghai");
  return { calls, client, root };
}

export function page(items: unknown[], cursor: string | null = null): Reply {
  return { body: { items, next_cursor: cursor } };
}

export function summary(root: HTMLElement) {
  return root.querySelector(".summary")?.textContent;
}

export async function settled(root: HTMLElement, calls: unknown[], count: number) {
  await until(() => calls.length === count && !button(root, "刷新").disabled);
}

export function erase(root: HTMLElement, key: string) {
  return root.querySelector<HTMLButtonElement>(`tr.titles th[data-col="${key}"] .clear`);
}

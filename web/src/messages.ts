import type { components } from "./api.gen";
import type { connect } from "./client";
import { stamp } from "./stamp";

type Item = components["schemas"]["MessagesListItem"];

const MORE = "加载更多";
const PLACEHOLDER = "[非文本消息]";
const REFRESH = "刷新";
const UNREACHABLE = "unreachable";

function button(label: string): HTMLButtonElement {
  const element = document.createElement("button");
  element.textContent = label;
  return element;
}

function cell(kind: string, text: string): HTMLElement {
  const element = document.createElement("span");
  element.className = kind;
  element.textContent = text;
  return element;
}

function row(item: Item, timeZone?: string): HTMLElement {
  const element = document.createElement("li");
  element.append(
    cell("time", stamp(item.occurred_at_ms, timeZone)),
    cell("sender", item.sender_name || item.sender_id),
    cell("conversation", item.conversation_id),
    cell("text", item.text || PLACEHOLDER),
  );
  return element;
}

export function renderMessages(
  client: ReturnType<typeof connect>,
  root: HTMLElement,
  timeZone?: string,
): Promise<void> {
  const refresh = button(REFRESH);
  const more = button(MORE);
  const list = document.createElement("ol");
  const status = document.createElement("p");
  let cursor: string | undefined;
  root.replaceChildren(refresh, list, more, status);

  async function load(append: boolean) {
    refresh.disabled = more.disabled = true;
    try {
      const { data, error } = await client.POST("/messages/list", {
        body: append && cursor ? { cursor } : {},
      });
      status.textContent = data ? "" : (error?.code ?? "");
      if (data) {
        if (!append) list.replaceChildren();
        list.append(...data.items.map((item) => row(item, timeZone)));
        cursor = data.next_cursor ?? undefined;
      }
    } catch {
      status.textContent = UNREACHABLE;
    }
    refresh.disabled = more.disabled = false;
    more.hidden = cursor === undefined;
  }

  refresh.onclick = () => load(false);
  more.onclick = () => load(true);
  more.hidden = true;
  return load(false);
}

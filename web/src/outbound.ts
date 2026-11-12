import type { components } from "./api.gen";
import type { connect } from "./client";
import { CARD_LIMIT, HELD, OUTBOUND_STATES, ROLES, SOURCES, TEXT } from "./config";
import { area, button } from "./dom";
import { stamp } from "./stamp";

type Client = ReturnType<typeof connect>;
type Item = components["schemas"]["OutboundItem"];

function escape(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function paint(text: string, marks: string[]): Node[] {
  if (marks.length === 0) return [document.createTextNode(text)];
  const pattern = new RegExp(`(${marks.map(escape).join("|")})`);
  return text
    .split(pattern)
    .filter((part) => part !== "")
    .map((part) => (marks.includes(part) ? area("span", "masked", part) : document.createTextNode(part)));
}

function option(label: string, value: string): HTMLOptionElement {
  const element = area("option", "", label) as HTMLOptionElement;
  element.value = value;
  return element;
}

class Outbox {
  private readonly count = area("p", "count");
  private readonly error = area("p", "error");
  private readonly items = area("section", "items");
  private readonly pick = area("select", "state") as HTMLSelectElement;

  constructor(
    private readonly client: Client,
    private readonly root: HTMLElement,
    private readonly timeZone?: string,
  ) {}

  private card(item: Item, marks: string[]): HTMLElement {
    const held = item.state === "withheld";
    const notes = [
      TEXT.batch(item.batch_id),
      `${SOURCES[item.source] ?? item.source} ${item.account}`,
      item.conversation_id,
      item.sender,
      stamp(item.occurred_at_ms, this.timeZone),
      ROLES[item.role] ?? item.role,
      held ? TEXT.held(HELD[item.reason] ?? item.reason) : "",
    ].filter((part) => part !== "");
    return area(
      "article",
      held ? "review flagged" : "review",
      area("p", "meta", notes.join(" · ")),
      area("pre", "payload", ...paint(item.text, marks)),
    );
  }

  async load(): Promise<void> {
    this.error.textContent = "";
    try {
      const { data, error } = await this.client.GET("/api/outbound", {
        params: { query: { limit: CARD_LIMIT, state: this.pick.value } },
      });
      if (data) this.show(data.items, data.placeholders);
      else this.error.textContent = error?.code ?? "";
    } catch {
      this.error.textContent = TEXT.unreachable;
    }
  }

  private show(items: Item[], marks: string[]): void {
    this.items.replaceChildren(...items.map((item) => this.card(item, marks)));
    this.count.textContent = items.length === 0 ? TEXT.noOutbound : TEXT.outboundCount(items.length);
  }

  start(): Promise<void> {
    const back = area("a", "back", TEXT.back) as HTMLAnchorElement;
    back.href = "#";
    this.pick.setAttribute("aria-label", TEXT.outboundState);
    this.pick.append(...Object.entries(OUTBOUND_STATES).map(([value, label]) => option(label, value)));
    this.pick.onchange = () => void this.load();
    const refresh = button(TEXT.refresh, "refresh", () => void this.load());
    const bar = area("header", "bar", area("h1", "brand", TEXT.brand), back, this.pick, refresh);
    this.root.replaceChildren(area("div", "reviews", bar, this.count, this.error, this.items));
    return this.load();
  }
}

export function renderOutbound(client: Client, root: HTMLElement, timeZone?: string): Promise<void> {
  return new Outbox(client, root, timeZone).start();
}

import type { components } from "./api.gen";
import type { connect } from "./client";
import { REVIEW_LIMIT, TEXT } from "./config";
import { stamp } from "./stamp";

type Client = ReturnType<typeof connect>;
type Item = components["schemas"]["ReviewItem"];
type Reason = components["schemas"]["ReviewReason"];

function area(tag: string, className: string, ...children: (Node | string)[]): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  element.append(...children);
  return element;
}

function button(label: string, className: string, click: () => void): HTMLButtonElement {
  const element = area("button", className, label) as HTMLButtonElement;
  element.type = "button";
  element.onclick = click;
  return element;
}

function option(label: string, value: string): HTMLOptionElement {
  const element = area("option", "", label) as HTMLOptionElement;
  element.value = value;
  return element;
}

class Queue {
  private readonly all = button(TEXT.approveAll, "approve-all", () => void this.approveAll());
  private readonly cards = new Map<string, HTMLElement>();
  private readonly count = area("p", "count");
  private readonly error = area("p", "error");
  private readonly items = area("section", "items");

  constructor(
    private readonly client: Client,
    private readonly root: HTMLElement,
    private readonly timeZone?: string,
  ) {}

  private async approveAll(): Promise<void> {
    for (const key of [...this.cards.keys()]) {
      if (!(await this.approve(key))) return;
    }
  }

  private card(item: Item, reasons: Reason[]): HTMLElement {
    const text = area("pre", "payload");
    text.textContent = item.text;
    const pick = area("select", "reason") as HTMLSelectElement;
    pick.setAttribute("aria-label", TEXT.reasonPick);
    pick.append(option(TEXT.reasonPick, ""), ...reasons.map((reason) => option(reason.label, reason.code)));
    const leak = area("textarea", "leak") as HTMLTextAreaElement;
    leak.setAttribute("autocomplete", "off");
    leak.placeholder = TEXT.leakPaste;
    leak.hidden = true;
    const reject = button(TEXT.reject, "reject", () => void this.reject(item.key, pick.value, leak));
    reject.disabled = true;
    const sync = () => {
      const learns = reasons.some((reason) => reason.code === pick.value && reason.placeholder);
      leak.hidden = !learns;
      reject.disabled = pick.value === "" || (learns && leak.value.trim() === "");
    };
    pick.onchange = sync;
    leak.oninput = sync;
    const flag = item.flag === "" ? "" : ` · ${TEXT.flagged(item.flag)}`;
    return area(
      "article",
      item.flag === "" ? "review" : "review flagged",
      area(
        "p",
        "meta",
        `${TEXT.batch(item.batch_id)} · ${item.conversation_id} · ${item.sender} · ${stamp(item.occurred_at_ms, this.timeZone)}${flag}`,
      ),
      text,
      area(
        "div",
        "verdict",
        button(TEXT.approve, "approve", () => void this.approve(item.key)),
        pick,
        leak,
        reject,
      ),
    );
  }

  private async approve(key: string): Promise<boolean> {
    this.error.textContent = "";
    try {
      const { error } = await this.client.PATCH("/api/reviews/{key}", {
        body: { state: "approved" },
        params: { path: { key } },
      });
      if (error) this.error.textContent = error.code;
      else this.drop(key);
      return !error;
    } catch {
      this.error.textContent = TEXT.unreachable;
      return false;
    }
  }

  private drop(key: string): void {
    this.cards.get(key)?.remove();
    this.cards.delete(key);
    this.tally();
  }

  async load(): Promise<void> {
    this.error.textContent = "";
    try {
      const { data, error } = await this.client.GET("/api/reviews", {
        params: { query: { limit: REVIEW_LIMIT } },
      });
      if (data) this.show(data.items, data.reasons);
      else this.error.textContent = error?.code ?? "";
    } catch {
      this.error.textContent = TEXT.unreachable;
    }
  }

  private async reject(key: string, reason: string, leak: HTMLTextAreaElement): Promise<void> {
    this.error.textContent = "";
    const pasted = leak.hidden ? undefined : leak.value;
    try {
      const { error } = await this.client.PATCH("/api/reviews/{key}", {
        body: { leak: pasted, reason, state: "rejected" },
        params: { path: { key } },
      });
      leak.value = "";
      if (error) this.error.textContent = error.code;
      else this.drop(key);
    } catch {
      this.error.textContent = TEXT.unreachable;
    }
  }

  private show(items: Item[], reasons: Reason[]): void {
    this.cards.clear();
    for (const item of items) this.cards.set(item.key, this.card(item, reasons));
    this.items.replaceChildren(...this.cards.values());
    this.tally();
  }

  start(): Promise<void> {
    const back = area("a", "back", TEXT.back) as HTMLAnchorElement;
    back.href = "#";
    const refresh = button(TEXT.refresh, "refresh", () => void this.load());
    const bar = area("header", "bar", area("h1", "brand", TEXT.brand), back, this.all, refresh);
    this.root.replaceChildren(area("div", "reviews", bar, this.count, this.error, this.items));
    return this.load();
  }

  private tally(): void {
    const total = this.cards.size;
    this.all.hidden = total === 0;
    this.count.textContent = total === 0 ? TEXT.noReviews : TEXT.pending(total);
  }
}

export function renderReviews(
  client: Client,
  root: HTMLElement,
  timeZone?: string,
): Promise<void> {
  return new Queue(client, root, timeZone).start();
}

import type { components } from "./api.gen";
import type { connect } from "./client";
import { REVIEW_HASH, REVIEW_LIMIT, TEXT } from "./config";
import { area, button } from "./dom";
import { stamp } from "./stamp";

type Client = ReturnType<typeof connect>;
type Batch = components["schemas"]["BatchItem"];

class Failures {
  private readonly cards = new Map<number, HTMLElement>();
  private readonly count = area("p", "count");
  private readonly error = area("p", "error");
  private readonly items = area("section", "items");

  constructor(
    private readonly client: Client,
    private readonly root: HTMLElement,
    private readonly timeZone?: string,
  ) {}

  private card(batch: Batch): HTMLElement {
    return area(
      "article",
      "review",
      area("p", "meta", `${TEXT.batch(batch.batch_id)} · ${stamp(batch.updated_at_ms, this.timeZone)}`),
      area("pre", "payload", TEXT.failedMeta(batch.attempts, batch.lines, batch.reason)),
      area("div", "verdict", button(TEXT.resend, "resend", () => void this.resend(batch.batch_id))),
    );
  }

  async load(): Promise<void> {
    this.error.textContent = "";
    try {
      const { data, error } = await this.client.GET("/api/batches", {
        params: { query: { limit: REVIEW_LIMIT } },
      });
      if (data) this.show(data.items);
      else this.error.textContent = error?.code ?? "";
    } catch {
      this.error.textContent = TEXT.unreachable;
    }
  }

  private async resend(id: number): Promise<void> {
    this.error.textContent = "";
    try {
      const { error } = await this.client.PATCH("/api/batches/{batch_id}", {
        body: { state: "ready" },
        params: { path: { batch_id: id } },
      });
      if (error) {
        this.error.textContent = error.code;
        return;
      }
      this.cards.get(id)?.remove();
      this.cards.delete(id);
      this.tally();
    } catch {
      this.error.textContent = TEXT.unreachable;
    }
  }

  private show(items: Batch[]): void {
    this.cards.clear();
    for (const batch of items) this.cards.set(batch.batch_id, this.card(batch));
    this.items.replaceChildren(...this.cards.values());
    this.tally();
  }

  start(): Promise<void> {
    const back = area("a", "back", TEXT.reviews) as HTMLAnchorElement;
    back.href = REVIEW_HASH;
    const refresh = button(TEXT.refresh, "refresh", () => void this.load());
    const bar = area("header", "bar", area("h1", "brand", TEXT.brand), back, refresh);
    this.root.replaceChildren(area("div", "reviews", bar, this.count, this.error, this.items));
    return this.load();
  }

  private tally(): void {
    const total = this.cards.size;
    this.count.textContent = total === 0 ? TEXT.noFailed : TEXT.failedCount(total);
  }
}

export function renderFailed(client: Client, root: HTMLElement, timeZone?: string): Promise<void> {
  return new Failures(client, root, timeZone).start();
}

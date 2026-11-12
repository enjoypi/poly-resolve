import type { components } from "./api.gen";
import type { connect } from "./client";
import { CARD_LIMIT, HELD, STATES, TEXT, TOPIC_STATES } from "./config";
import { area, button } from "./dom";
import { brief } from "./topic";

type Client = ReturnType<typeof connect>;
type Overview = components["schemas"]["TopicOverview"];
type Tally = components["schemas"]["TopicTally"];

function option(label: string, value: string): HTMLOptionElement {
  const element = area("option", "", label) as HTMLOptionElement;
  element.value = value;
  return element;
}

function tally(entries: Tally[], names: Record<string, string>): string {
  return entries
    .map((entry) => {
      const name = names[entry.state] ?? entry.state;
      const reason = entry.reason === "" ? "" : `（${HELD[entry.reason] ?? entry.reason}）`;
      return `${name}${reason} ${entry.count}`;
    })
    .join(" · ");
}

class Topics {
  private readonly count = area("p", "count");
  private readonly error = area("p", "error");
  private readonly items = area("section", "items");
  private readonly lines = area("p", "count lines");
  private readonly pick = area("select", "state") as HTMLSelectElement;

  constructor(
    private readonly client: Client,
    private readonly root: HTMLElement,
    private readonly timeZone?: string,
  ) {}

  async load(): Promise<void> {
    this.error.textContent = "";
    try {
      const { data, error } = await this.client.GET("/api/topics", {
        params: { query: { limit: CARD_LIMIT, state: this.pick.value } },
      });
      if (data) this.show(data);
      else this.error.textContent = error?.code ?? "";
    } catch {
      this.error.textContent = TEXT.unreachable;
    }
  }

  private show(overview: Overview): void {
    const { items, lines, topics } = overview;
    this.items.replaceChildren(
      ...items.map((item) =>
        area(
          "article",
          "review topic",
          ...brief(item, this.timeZone),
          ...(item.verdict === "" ? [] : [area("pre", "payload", item.verdict)]),
        ),
      ),
    );
    this.count.textContent = topics.length === 0 ? TEXT.noTopics : TEXT.topicTally(tally(topics, TOPIC_STATES));
    this.lines.textContent = lines.length === 0 ? "" : TEXT.lineTally(tally(lines, STATES));
  }

  start(): Promise<void> {
    const back = area("a", "back", TEXT.back) as HTMLAnchorElement;
    back.href = "#";
    this.pick.setAttribute("aria-label", TEXT.outboundState);
    this.pick.append(
      option(TEXT.every, ""),
      ...Object.entries(TOPIC_STATES).map(([value, label]) => option(label, value)),
    );
    this.pick.onchange = () => void this.load();
    const refresh = button(TEXT.refresh, "refresh", () => void this.load());
    const bar = area("header", "bar", area("h1", "brand", TEXT.brand), back, this.pick, refresh);
    this.root.replaceChildren(
      area("div", "reviews", bar, this.count, this.lines, area("p", "count blind", TEXT.blind), this.error, this.items),
    );
    return this.load();
  }
}

export function renderTopics(client: Client, root: HTMLElement, timeZone?: string): Promise<void> {
  return new Topics(client, root, timeZone).start();
}

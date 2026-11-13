import type { components } from "./api.gen";
import type { connect } from "./client";
import { PAGE_DAYS, SOURCE_STATES, SOURCES, STAGES, TEXT } from "./config";
import { area, button } from "./dom";
import { stamp } from "./stamp";

type Client = ReturnType<typeof connect>;
type Day = components["schemas"]["UsageDay"];
type Health = components["schemas"]["SourcesHealth"];
type Source = components["schemas"]["SourcesItem"];

class Sources {
  private readonly error = area("p", "error");
  private readonly items = area("section", "items");

  constructor(
    private readonly client: Client,
    private readonly root: HTMLElement,
    private readonly timeZone?: string,
  ) {}

  private day(entry: Day): HTMLElement {
    const notes = [
      entry.date,
      STAGES[entry.kind] ?? entry.kind,
      TEXT.usage(entry.calls, entry.failures, entry.input_tokens, entry.output_tokens),
    ];
    return area("article", entry.failures > 0 ? "review flagged" : "review", area("p", "meta", notes.join(" · ")));
  }

  async load(): Promise<void> {
    this.error.textContent = "";
    try {
      const [health, usage] = await Promise.all([
        this.client.GET("/api/sources"),
        this.client.GET("/api/usage", { params: { query: { limit: PAGE_DAYS } } }),
      ]);
      if (health.data && usage.data) this.show(health.data, usage.data.days);
      else this.error.textContent = health.error?.code ?? usage.error?.code ?? "";
    } catch {
      this.error.textContent = TEXT.unreachable;
    }
  }

  private show(health: Health, days: Day[]): void {
    const stages = health.stages.map((stage) =>
      area(
        "article",
        stage.streak > 0 ? "review flagged" : "review",
        area("p", "meta", TEXT.stage(STAGES[stage.kind] ?? stage.kind, stage.streak)),
      ),
    );
    this.items.replaceChildren(
      area("h2", "section", TEXT.sourcesTitle),
      ...(health.sources.length === 0 ? [area("p", "count", TEXT.noSources)] : health.sources.map((item) => this.source(item))),
      area("h2", "section", TEXT.stagesTitle),
      ...stages,
      area("h2", "section", TEXT.usageTitle),
      ...(days.length === 0 ? [area("p", "count", TEXT.noUsage)] : days.map((entry) => this.day(entry))),
    );
  }

  private source(item: Source): HTMLElement {
    const healthy = item.state === "ok";
    const notes = [
      `${SOURCES[item.kind] ?? item.kind} ${item.account}`,
      SOURCE_STATES[item.state] ?? item.state,
      TEXT.okAt(item.ok_at_ms === 0 ? TEXT.none : stamp(item.ok_at_ms, this.timeZone)),
      TEXT.lastAt(typeof item.last_at_ms === "number" ? stamp(item.last_at_ms, this.timeZone) : TEXT.none),
      healthy ? "" : TEXT.downSince(stamp(item.down_at_ms, this.timeZone), item.streak),
    ].filter((part) => part !== "");
    return area("article", healthy ? "review" : "review flagged", area("p", "meta", notes.join(" · ")));
  }

  start(): Promise<void> {
    const back = area("a", "back", TEXT.back) as HTMLAnchorElement;
    back.href = "#";
    const refresh = button(TEXT.refresh, "refresh", () => void this.load());
    const bar = area("header", "bar", area("h1", "brand", TEXT.brand), back, refresh);
    this.root.replaceChildren(area("div", "reviews", bar, this.error, this.items));
    return this.load();
  }
}

export function renderSources(client: Client, root: HTMLElement, timeZone?: string): Promise<void> {
  return new Sources(client, root, timeZone).start();
}

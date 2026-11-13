import type { components } from "./api.gen";
import type { connect } from "./client";
import { CHANGE_KINDS, CHANGE_REASONS, HELD, ROLES, SOURCES, STATES, TEXT, TOPIC_HASH, TOPIC_STATES, TOPICS_HASH, VIAS } from "./config";
import { area, button } from "./dom";
import { stamp } from "./stamp";

type Change = components["schemas"]["TopicChange"];
type Client = ReturnType<typeof connect>;
type Entry = components["schemas"]["TopicEntry"];
type Item = components["schemas"]["TopicItem"];

const ratio = (value?: number | null) => (typeof value === "number" ? value.toFixed(2) : TEXT.none);

function trend(score?: number | null, previous?: number | null): string {
  if (typeof score !== "number" || typeof previous !== "number" || score === previous) return "";
  return score > previous ? TEXT.rise : TEXT.fall;
}

export function brief(item: Item, timeZone?: string): HTMLElement[] {
  const sources = item.sources.map((source) => SOURCES[source] ?? source).join("、");
  const notes = [
    TOPIC_STATES[item.state] ?? item.state,
    TEXT.score(`${ratio(item.score)}${trend(item.score, item.previous)}`, ratio(item.urgency)),
    TEXT.reach(item.people, item.conversations),
    sources,
    TEXT.members(item.members),
    TEXT.first(stamp(item.first_at_ms, timeZone)),
    TEXT.latest(stamp(item.last_at_ms, timeZone)),
    item.version === "" ? "" : TEXT.version(item.version),
  ].filter((part) => part !== "");
  const title = area("a", "title", item.label === "" ? item.topic_id : item.label) as HTMLAnchorElement;
  title.href = `${TOPIC_HASH}${encodeURIComponent(item.topic_id)}`;
  return [title, area("p", "meta", notes.join(" · "))];
}

class Topic {
  private readonly error = area("p", "error");
  private readonly items = area("section", "items");

  constructor(
    private readonly client: Client,
    private readonly root: HTMLElement,
    private readonly timeZone?: string,
  ) {}

  private change(entry: Change): HTMLElement {
    const notes = [
      stamp(entry.at_ms, this.timeZone),
      CHANGE_KINDS[entry.kind] ?? entry.kind,
      TEXT.moved(entry.from_topic, entry.to_topic, entry.keys),
      CHANGE_REASONS[entry.reason] ?? entry.reason,
      entry.state,
    ].filter((part) => part !== "");
    return area("article", "review change", area("p", "meta", notes.join(" · ")));
  }

  private entry(line: Entry): HTMLElement {
    const held = line.state !== "approved";
    const state = STATES[line.state] ?? line.state;
    const notes = [
      stamp(line.occurred_at_ms, this.timeZone),
      SOURCES[line.source] ?? line.source,
      line.conversation_id,
      line.sender,
      ROLES[line.role] ?? line.role,
      VIAS[line.via] ?? line.via,
      held ? TEXT.status(state, HELD[line.reason] ?? line.reason) : "",
    ].filter((part) => part !== "");
    return area(
      "article",
      held ? "review flagged" : "review",
      area("p", "meta", notes.join(" · ")),
      ...(line.purged_at_ms > 0 ? [area("pre", "payload", TEXT.purged)] : line.text === "" ? [] : [area("pre", "payload", line.text)]),
    );
  }

  async load(topic: string): Promise<void> {
    this.error.textContent = "";
    try {
      const { data, error } = await this.client.GET("/api/topics/{topic_id}", {
        params: { path: { topic_id: topic } },
      });
      if (data) this.show(data);
      else this.error.textContent = error?.code ?? "";
    } catch {
      this.error.textContent = TEXT.unreachable;
    }
  }

  private show(detail: components["schemas"]["TopicDetail"]): void {
    const { changes, timeline, topic } = detail;
    this.items.replaceChildren(
      area(
        "article",
        "review topic",
        ...brief(topic, this.timeZone),
        ...(topic.verdict === "" ? [] : [area("pre", "payload", topic.verdict)]),
      ),
      area("h2", "section", TEXT.timeline(timeline.length)),
      ...timeline.map((line) => this.entry(line)),
      area("h2", "section", TEXT.changes(changes.length)),
      ...changes.map((entry) => this.change(entry)),
    );
  }

  start(topic: string): Promise<void> {
    const back = area("a", "back", TEXT.topics) as HTMLAnchorElement;
    back.href = TOPICS_HASH;
    const refresh = button(TEXT.refresh, "refresh", () => void this.load(topic));
    const bar = area("header", "bar", area("h1", "brand", TEXT.brand), back, refresh);
    this.root.replaceChildren(area("div", "reviews", bar, this.error, this.items));
    return this.load(topic);
  }
}

export function renderTopic(
  client: Client,
  root: HTMLElement,
  topic: string,
  timeZone?: string,
): Promise<void> {
  return new Topic(client, root, timeZone).start(topic);
}

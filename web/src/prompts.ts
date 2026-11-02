import type { components } from "./api.gen";
import type { connect } from "./client";
import { PROMPTS, TEXT } from "./config";
import { area, button } from "./dom";

type Client = ReturnType<typeof connect>;
type Item = components["schemas"]["PromptsListItem"];

class Editor {
  private base: string;
  private readonly marks: HTMLElement[];
  private readonly revert = button(TEXT.revert, "revert", () => this.undo());
  private readonly save = button(TEXT.save, "save", () => void this.store());
  private readonly state = area("span", "state");
  private readonly text = area("textarea", "text") as HTMLTextAreaElement;

  constructor(
    private readonly client: Client,
    private readonly item: Item,
  ) {
    this.base = item.text;
    this.marks = item.required.map((mark) => area("span", "mark", mark));
    this.text.value = item.text;
    this.text.spellcheck = false;
    this.text.setAttribute("autocomplete", "off");
    this.text.oninput = () => {
      this.state.textContent = this.dirty() ? TEXT.dirty : "";
      this.sync();
    };
    this.sync();
  }

  card(): HTMLElement {
    const shown = PROMPTS[this.item.name];
    return area(
      "article",
      "review prompt",
      area("h2", "title", shown?.label ?? this.item.name),
      area("p", "hint", shown?.hint ?? ""),
      area("p", "meta", ...this.marks),
      this.text,
      area("div", "verdict", this.save, this.revert, this.state),
    );
  }

  dirty(): boolean {
    return this.text.value !== this.base;
  }

  private async store(): Promise<void> {
    const text = this.text.value;
    try {
      const { error } = await this.client.PATCH("/api/prompts/{name}", {
        body: { text },
        params: { path: { name: this.item.name } },
      });
      if (error) {
        this.state.textContent = error.code;
        return;
      }
      this.base = text;
      this.state.textContent = TEXT.saved;
      this.sync();
    } catch {
      this.state.textContent = TEXT.unreachable;
    }
  }

  private sync(): void {
    const value = this.text.value;
    let complete = value.trim() !== "";
    for (const mark of this.marks) {
      const once = value.split(mark.textContent ?? "").length === 2;
      mark.classList.toggle("missing", !once);
      complete &&= once;
    }
    this.save.disabled = !this.dirty() || !complete;
    this.revert.hidden = !this.dirty();
  }

  private undo(): void {
    this.text.value = this.base;
    this.state.textContent = "";
    this.sync();
  }
}

export async function renderPrompts(client: Client, root: HTMLElement): Promise<void> {
  const error = area("p", "error");
  const items = area("section", "items");
  const back = area("a", "back", TEXT.back) as HTMLAnchorElement;
  back.href = "#";
  const bar = area("header", "bar", area("h1", "brand", TEXT.brand), back);
  root.replaceChildren(area("div", "reviews", bar, area("p", "count note", TEXT.promptNote), error, items));
  const editors: Editor[] = [];
  onbeforeunload = (event) => {
    if (editors.some((editor) => editor.dirty())) event.preventDefault();
  };
  try {
    const { data, error: failure } = await client.GET("/api/prompts");
    if (!data) {
      error.textContent = failure?.code ?? "";
      return;
    }
    for (const item of data.items) editors.push(new Editor(client, item));
    items.replaceChildren(...editors.map((editor) => editor.card()));
  } catch {
    error.textContent = TEXT.unreachable;
  }
}

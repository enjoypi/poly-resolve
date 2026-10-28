import type { Column } from "./columns";
import { PRESETS, TEXT } from "./config";
import { custom, preset, type Span } from "./range";

export type Handlers = {
  onColumn: (key: string, shown: boolean) => void;
  onPanel: () => void;
  onRange: (span: Span) => void;
  onRefresh: () => void;
  onSearch: (text: string) => void;
};

export type Toolbar = {
  fill: (columns: Column[], shown: (column: Column) => boolean) => void;
  panel: HTMLButtonElement;
  refresh: HTMLButtonElement;
  root: HTMLElement;
  search: HTMLInputElement;
};

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

function stamped(label: string, className: string): HTMLInputElement {
  const element = area("input", className) as HTMLInputElement;
  element.type = "datetime-local";
  element.setAttribute("aria-label", label);
  return element;
}

function ranges(handlers: Handlers, timeZone?: string): HTMLElement {
  const since = stamped(TEXT.since, "since");
  const until = stamped(TEXT.until, "until");
  const note = area("span", "note");
  const buttons = PRESETS.map((item) =>
    button(item.label, "preset", () => {
      since.value = "";
      until.value = "";
      note.textContent = "";
      press(buttons, item.label);
      handlers.onRange(preset(item, Date.now(), timeZone));
    }),
  );
  const edit = () => {
    const span = custom(since.value, until.value, timeZone);
    note.textContent = span ? "" : TEXT.rangeInvalid;
    if (!span) return;
    press(buttons, Object.keys(span).length === 0 ? PRESETS[0]?.label : undefined);
    handlers.onRange(span);
  };
  since.onchange = edit;
  until.onchange = edit;
  press(buttons, PRESETS[0]?.label);
  const group = area("div", "range", ...buttons, since, until, note);
  group.setAttribute("role", "group");
  group.setAttribute("aria-label", TEXT.range);
  return group;
}

function press(buttons: HTMLButtonElement[], label?: string): void {
  for (const item of buttons) item.setAttribute("aria-pressed", String(item.textContent === label));
}

export function buildToolbar(handlers: Handlers, timeZone?: string): Toolbar {
  const search = area("input", "search") as HTMLInputElement;
  const menu = area("div", "menu");
  const panel = button(TEXT.panel, "panel-toggle", () => handlers.onPanel());
  const refresh = button(TEXT.refresh, "refresh", () => handlers.onRefresh());
  const toggle = button(TEXT.columns, "columns-toggle", () => {
    menu.hidden = !menu.hidden;
    toggle.setAttribute("aria-expanded", String(!menu.hidden));
  });
  search.type = "search";
  search.placeholder = TEXT.search;
  search.setAttribute("aria-label", TEXT.search);
  search.oninput = () => handlers.onSearch(search.value);
  menu.hidden = true;
  menu.onkeydown = (event) => {
    if (event.key === "Escape") toggle.click();
  };
  panel.setAttribute("aria-expanded", "false");
  toggle.setAttribute("aria-expanded", "false");
  const root = area(
    "header",
    "bar",
    area("h1", "brand", TEXT.brand),
    ranges(handlers, timeZone),
    search,
    area("div", "columns", toggle, menu),
    panel,
    refresh,
  );
  const fill = (columns: Column[], shown: (column: Column) => boolean) => {
    menu.replaceChildren(
      ...columns.map((column) => {
        const box = area("input", "") as HTMLInputElement;
        box.type = "checkbox";
        box.checked = shown(column);
        box.onchange = () => handlers.onColumn(column.key, box.checked);
        return area("label", "choice", box, column.label);
      }),
    );
  };
  return { fill, panel, refresh, root, search };
}

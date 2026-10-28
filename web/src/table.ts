import { badge } from "./badge";
import { type Column, display, type Row } from "./columns";
import { DEFAULT_SORT, TEXT } from "./config";
import { blocks, plain } from "./markdown";
import { stamp, zone } from "./stamp";
import type { State } from "./view";

const owners = new WeakMap<Element, Row>();

export type Handlers = {
  onFilter: (key: string, text: string) => void;
  onSort: (key: string) => void;
};

function area(tag: string, className: string): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  return element;
}

function cell(item: Row, column: Column, timeZone?: string): HTMLElement {
  const element = document.createElement("td");
  const value = display(item, column, timeZone);
  element.dataset.col = column.key;
  element.dataset.label = column.label;
  element.className = [
    column.look ? `look-${column.look}` : "",
    column.kind === "text" ? "" : column.kind,
    column.primary ? "primary" : "",
  ]
    .filter((name) => name !== "")
    .join(" ");
  if (column.look === "badge" || column.kind === "time") element.dataset.always = "";
  if (column.look === "badge") {
    if (value !== "") element.append(badge(value));
  } else {
    element.textContent = column.look === "markdown" ? plain(value) : value;
  }
  return element;
}

function filterBox(column: Column, state: State, handlers: Handlers): HTMLElement {
  const slot = document.createElement("th");
  const input = document.createElement("input");
  slot.dataset.col = column.key;
  slot.className = column.kind === "number" ? "number" : "";
  input.type = "text";
  input.placeholder = column.label;
  input.value = state.filters[column.key] ?? "";
  input.setAttribute("aria-label", `${TEXT.panel} ${column.label}`);
  input.oninput = () => handlers.onFilter(column.key, input.value);
  slot.append(input);
  return slot;
}

function line(item: Row, columns: Column[], timeZone?: string): HTMLElement {
  const element = document.createElement("tr");
  element.tabIndex = 0;
  element.dataset.key = String(item.key ?? "");
  owners.set(element, item);
  element.append(...columns.map((column) => cell(item, column, timeZone)));
  return element;
}

function sortBox(column: Column, handlers: Handlers, timeZone?: string): HTMLElement {
  const title = document.createElement("th");
  const button = document.createElement("button");
  title.dataset.col = column.key;
  title.className = column.kind === "number" ? "number" : "";
  title.scope = "col";
  button.type = "button";
  button.className = "sort";
  button.textContent =
    column.kind === "time" ? `${column.label} ${zone(Date.now(), timeZone)}` : column.label;
  button.onclick = () => handlers.onSort(column.key);
  title.append(button);
  return title;
}

export function buildBody(
  body: HTMLTableSectionElement,
  columns: Column[],
  rows: Row[],
  timeZone?: string,
): void {
  body.replaceChildren(...rows.map((item) => line(item, columns, timeZone)));
}

export function buildHead(
  head: HTMLTableSectionElement,
  columns: Column[],
  state: State,
  handlers: Handlers,
  timeZone?: string,
): void {
  const titles = area("tr", "titles");
  const filters = area("tr", "filters");
  titles.append(...columns.map((column) => sortBox(column, handlers, timeZone)));
  filters.append(...columns.map((column) => filterBox(column, state, handlers)));
  head.replaceChildren(titles, filters);
  markSort(head, state);
}

export function markSort(head: HTMLTableSectionElement, state: State): void {
  for (const title of head.querySelectorAll<HTMLElement>("tr.titles th")) {
    let value = "none";
    if (state.sort) {
      if (state.sort.key === title.dataset.col) {
        value = state.sort.dir === "asc" ? "ascending" : "descending";
      }
    } else if (title.dataset.col === DEFAULT_SORT) {
      value = "descending";
    }
    title.setAttribute("aria-sort", value);
  }
}

export function rowOf(element: Element | null): Row | undefined {
  const holder = element?.closest("tr");
  return holder ? owners.get(holder) : undefined;
}

export function tipOf(
  target: HTMLElement,
  columns: Column[],
  timeZone?: string,
): Node[] | undefined {
  const item = rowOf(target);
  const column = columns.find((entry) => entry.key === target.dataset.col);
  if (!item || !column) return undefined;
  const value = display(item, column, timeZone);
  if (value === "") return undefined;
  const raw = item[column.key];
  if (column.kind === "time" && typeof raw === "number") {
    return [document.createTextNode(stamp(raw, timeZone))];
  }
  if (column.look !== "markdown") return [document.createTextNode(value)];
  const wrapper = area("div", "md");
  wrapper.append(...blocks(value));
  return [wrapper];
}

import { MAX_ROWS, TEXT } from "./config";
import { describe, type Span } from "./range";

export type Handlers = {
  onAll: () => void;
  onMore: () => void;
};

export type Report = {
  capped: boolean;
  loaded: number;
  more: boolean;
  shown: number;
  span: Span;
};

export type Status = {
  error: HTMLElement;
  lock: (locked: boolean) => void;
  root: HTMLElement;
  update: (report: Report, timeZone?: string) => void;
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

export function buildStatus(handlers: Handlers): Status {
  const summary = area("p", "summary");
  const error = area("p", "error");
  const more = button(TEXT.more, "more", () => handlers.onMore());
  const all = button(TEXT.all, "all", () => handlers.onAll());
  summary.setAttribute("role", "status");
  const root = area("footer", "status", summary, error, more, all);
  const lock = (locked: boolean) => {
    more.disabled = locked;
    all.disabled = locked;
  };
  const update = (report: Report, timeZone?: string) => {
    const counts = report.loaded === 0 ? TEXT.empty : TEXT.counts(report.shown, report.loaded);
    let notice = "";
    if (report.capped) notice = TEXT.capped(MAX_ROWS);
    else if (report.more) notice = TEXT.partial;
    const span = Object.keys(report.span).length === 0 ? "" : TEXT.span(describe(report.span, timeZone));
    summary.textContent = `${counts}${notice}${span}`;
    more.hidden = !report.more || report.capped;
    all.hidden = more.hidden;
  };
  more.hidden = true;
  all.hidden = true;
  return { error, lock, root, update };
}

import { badge } from "./badge";
import { type Column, content, display, type Row } from "./columns";
import { DETAIL, TEXT } from "./config";
import { blocks } from "./markdown";
import { stamp } from "./stamp";

function area(tag: string, className: string, ...children: (Node | string)[]): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  element.append(...children);
  return element;
}

function fields(item: Row, columns: Column[], timeZone?: string): HTMLElement {
  const list = document.createElement("dl");
  for (const column of columns) {
    const raw = item[column.key];
    const shown =
      column.kind === "time" && typeof raw === "number"
        ? stamp(raw, timeZone)
        : display(item, column, timeZone);
    list.append(area("dt", "name", column.label), area("dd", "value", shown));
  }
  const holder = document.createElement("details");
  holder.className = "fields";
  holder.append(area("summary", "", TEXT.fields), list);
  return holder;
}

function head(item: Row, columns: Column[], close: () => void, timeZone?: string): HTMLElement {
  const find = (key: string) => columns.find((column) => column.key === key);
  const conversation = find(DETAIL.conversation);
  const sender = find(DETAIL.sender);
  const clock = item[DETAIL.time];
  const button = area("button", "close", TEXT.close) as HTMLButtonElement;
  const mark = conversation ? display(item, conversation, timeZone) : "";
  button.type = "button";
  button.onclick = close;
  return area(
    "header",
    "head",
    ...(mark === "" ? [] : [badge(mark)]),
    area("strong", "sender", sender ? display(item, sender, timeZone) : ""),
    area("span", "when", typeof clock === "number" ? stamp(clock, timeZone) : ""),
    button,
  );
}

function body(item: Row, columns: Column[], timeZone?: string): HTMLElement {
  const column = columns.find((entry) => entry.key === DETAIL.body);
  const text = column ? content(item, column, timeZone) : "";
  const holder = area("div", "body md");
  if (text === "") holder.append(area("p", "", column ? display(item, column, timeZone) : ""));
  else holder.append(...blocks(text));
  return holder;
}

export function openDetail(
  dialog: HTMLDialogElement,
  item: Row,
  columns: Column[],
  opener: HTMLElement,
  timeZone?: string,
): void {
  dialog.replaceChildren(
    head(item, columns, () => dialog.close(), timeZone),
    body(item, columns, timeZone),
    fields(item, columns, timeZone),
  );
  dialog.onclose = () => opener.focus();
  dialog.onclick = (event) => {
    if (event.target === dialog) dialog.close();
  };
  dialog.showModal();
}

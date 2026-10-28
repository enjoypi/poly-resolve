import { MD_LIMIT, PLAIN_LIMIT } from "./config";
import { anchor, target } from "./links";

const FENCE = "```";
const HEADING = /^\s{0,3}#{1,6}\s+(.*)$/;
const INLINE =
  /`([^`\n]{1,500})`|\*\*([^*\n]{1,500})\*\*|!?\[([^\]\n]{0,200})\]\((https?:\/\/[^)\s]{1,4000})\)/g;
const QUOTE = /^\s{0,3}>\s?(.*)$/;

type Pending = { kind: "p" | "quote"; lines: string[] };

function element(tag: string, ...children: (Node | string)[]): HTMLElement {
  const created = document.createElement(tag);
  created.append(...children);
  return created;
}

function flatten(text: string): string {
  return text.replace(
    INLINE,
    (_whole: string, code?: string, bold?: string, label?: string, address?: string) => {
      if (code !== undefined) return code;
      if (bold !== undefined) return flatten(bold);
      return label || target(address ?? "")?.host || "";
    },
  );
}

function strip(line: string): string {
  const heading = HEADING.exec(line) ?? QUOTE.exec(line);
  return heading ? (heading[1] ?? "") : line;
}

function token(code?: string, bold?: string, label = "", address = ""): Node {
  if (code !== undefined) return element("code", code);
  if (bold !== undefined) return element("strong", ...inline(bold));
  return anchor(flatten(label), address);
}

export function blocks(text: string): HTMLElement[] {
  if (text.length > MD_LIMIT) return [element("p", text)];
  const out: HTMLElement[] = [];
  let pending: Pending | undefined;
  let fence: string[] | undefined;
  const close = () => {
    if (pending) {
      const tag = pending.kind === "quote" ? "blockquote" : "p";
      out.push(element(tag, ...inline(pending.lines.join("\n"))));
    }
    pending = undefined;
  };
  const open = (kind: Pending["kind"]) => {
    if (pending?.kind !== kind) {
      close();
      pending = { kind, lines: [] };
    }
  };
  for (const line of text.split("\n")) {
    const heading = HEADING.exec(line);
    const quote = QUOTE.exec(line);
    if (fence) {
      if (line.trimStart().startsWith(FENCE)) {
        out.push(element("pre", element("code", fence.join("\n"))));
        fence = undefined;
      } else fence.push(line);
    } else if (line.trimStart().startsWith(FENCE)) {
      close();
      fence = [];
    } else if (line.trim() === "") close();
    else if (heading) {
      close();
      const title = element("p", element("strong", ...inline(heading[1] ?? "")));
      title.className = "heading";
      out.push(title);
    } else if (quote) {
      open("quote");
      pending?.lines.push(quote[1] ?? "");
    } else {
      open("p");
      pending?.lines.push(line);
    }
  }
  if (fence) out.push(element("pre", element("code", fence.join("\n"))));
  close();
  return out;
}

export function inline(text: string): Node[] {
  const nodes: Node[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const [whole = "", code, bold, label, address] = match;
    const start = match.index ?? 0;
    if (start > last) nodes.push(document.createTextNode(text.slice(last, start)));
    nodes.push(token(code, bold, label, address));
    last = start + whole.length;
  }
  if (last < text.length) nodes.push(document.createTextNode(text.slice(last)));
  return nodes;
}

export function plain(text: string): string {
  return text
    .slice(0, PLAIN_LIMIT)
    .split("\n")
    .filter((line) => !line.trimStart().startsWith(FENCE))
    .map((line) => flatten(strip(line)))
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
}

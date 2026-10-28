import { type Column, content, display, type Row } from "./columns";

const COLLATOR = new Intl.Collator("zh-CN", { numeric: true });
const COMPARE = /^(>=|<=|>|<|=)?\s*(-?\d+(?:\.\d+)?)$/;
const RANGE = /^(-?\d+(?:\.\d+)?)\s*\.\.\s*(-?\d+(?:\.\d+)?)$/;

export type Dir = "asc" | "desc";

export type Sort = { dir: Dir; key: string };

export type State = { filters: Record<string, string>; search: string; sort?: Sort };

type Test = (value: number) => boolean;

type Value = number | string | undefined;

function compare(left: Value, right: Value, dir: Dir): number {
  if (left === undefined || right === undefined) {
    if (left === right) return 0;
    return left === undefined ? 1 : -1;
  }
  const order =
    typeof left === "number" && typeof right === "number"
      ? left - right
      : COLLATOR.compare(String(left), String(right));
  return dir === "asc" ? order : -order;
}

function contains(haystack: string, needle: string): boolean {
  const lower = haystack.toLowerCase();
  return needle
    .toLowerCase()
    .split(/\s+/)
    .filter((term) => term !== "")
    .every((term) => lower.includes(term));
}

function matches(row: Row, column: Column, text: string, timeZone?: string): boolean {
  const test = column.kind === "number" ? predicate(text.trim()) : undefined;
  const raw = row[column.key];
  if (test) return typeof raw === "number" && test(raw);
  return contains(display(row, column, timeZone), text);
}

function predicate(expression: string): Test | undefined {
  const range = RANGE.exec(expression);
  if (range) {
    const [, low = "", high = ""] = range;
    return (value) => value >= Number(low) && value <= Number(high);
  }
  const single = COMPARE.exec(expression);
  if (!single) return undefined;
  const [, operator = "=", digits = ""] = single;
  const bound = Number(digits);
  const tests: Record<string, Test> = {
    "<": (value) => value < bound,
    "<=": (value) => value <= bound,
    "=": (value) => value === bound,
    ">": (value) => value > bound,
    ">=": (value) => value >= bound,
  };
  return tests[operator];
}

function sortValue(row: Row, column: Column, timeZone?: string): Value {
  if (column.kind !== "text") {
    const raw = row[column.key];
    return typeof raw === "number" ? raw : undefined;
  }
  const shown = content(row, column, timeZone);
  return shown === "" ? undefined : shown;
}

export function applyView(rows: Row[], columns: Column[], state: State, timeZone?: string): Row[] {
  const active = columns.filter((column) => (state.filters[column.key] ?? "").trim() !== "");
  const needle = state.search.trim();
  const kept = rows.filter(
    (row) =>
      active.every((column) => matches(row, column, state.filters[column.key] ?? "", timeZone)) &&
      (needle === "" ||
        contains(columns.map((column) => display(row, column, timeZone)).join("\n"), needle)),
  );
  const { sort } = state;
  const target = columns.find((column) => column.key === sort?.key);
  if (!sort || !target) return kept;
  return kept
    .map((row) => ({ row, value: sortValue(row, target, timeZone) }))
    .sort((left, right) => compare(left.value, right.value, sort.dir))
    .map((entry) => entry.row);
}

export function nextSort(current: Sort | undefined, key: string): Sort | undefined {
  if (current?.key !== key) return { dir: "asc", key };
  return current.dir === "asc" ? { dir: "desc", key } : undefined;
}

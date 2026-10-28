import { type Field, FIELDS, type Kind } from "./config";
import { clock } from "./stamp";

export type Column = Field & { key: string; kind: Kind };

export type Row = Record<string, unknown>;

function guess(key: string, rows: Row[]): Kind {
  const values = rows
    .map((row) => row[key])
    .filter((value) => value !== undefined && value !== null);
  if (values.length > 0 && values.every((value) => Number.isInteger(value)) && key.endsWith("_ms")) {
    return "time";
  }
  return values.length > 0 && values.every((value) => typeof value === "number") ? "number" : "text";
}

function render(value: unknown, kind: Kind, timeZone?: string): string {
  if (value === undefined || value === null) return "";
  if (kind === "time" && typeof value === "number") return clock(value, timeZone);
  if (typeof value === "string") return value;
  return typeof value === "object" ? JSON.stringify(value) : String(value);
}

export function content(row: Row, column: Column, timeZone?: string): string {
  const own = row[column.key];
  const empty = own === undefined || own === null || own === "";
  return render(empty && column.fallback ? row[column.fallback] : own, column.kind, timeZone);
}

export function display(row: Row, column: Column, timeZone?: string): string {
  return content(row, column, timeZone) || (column.placeholder ?? "");
}

export function inferColumns(rows: Row[]): Column[] {
  const extra = [...new Set(rows.flatMap((row) => Object.keys(row)))]
    .filter((key) => !(key in FIELDS))
    .sort();
  return [...Object.keys(FIELDS), ...extra].map((key) => {
    const field = FIELDS[key];
    return { ...field, key, kind: field?.kind ?? guess(key, rows), label: field?.label ?? key };
  });
}

import type { Preset } from "./config";
import { clock, parse, stamp } from "./stamp";

const DATE_LENGTH = 10;

export type Span = { from_ms?: number; to_ms?: number };

export function custom(since: string, until: string, timeZone?: string): Span | undefined {
  const from_ms = since === "" ? undefined : parse(since, timeZone);
  const to_ms = until === "" ? undefined : parse(until, timeZone);
  const broken = (since !== "" && from_ms === undefined) || (until !== "" && to_ms === undefined);
  if (broken || (from_ms !== undefined && to_ms !== undefined && from_ms >= to_ms)) {
    return undefined;
  }
  return { ...(from_ms === undefined ? {} : { from_ms }), ...(to_ms === undefined ? {} : { to_ms }) };
}

export function describe(span: Span, timeZone?: string): string {
  const from = span.from_ms === undefined ? "最早" : stamp(span.from_ms, timeZone);
  const to = span.to_ms === undefined ? "现在" : stamp(span.to_ms, timeZone);
  return `${from} 至 ${to}`;
}

export function preset(item: Preset, now: number, timeZone?: string): Span {
  if (item.today) {
    const midnight = parse(`${clock(now, timeZone).slice(0, DATE_LENGTH)}T00:00`, timeZone);
    return midnight === undefined ? {} : { from_ms: midnight };
  }
  return item.back_ms === undefined ? {} : { from_ms: now - item.back_ms };
}

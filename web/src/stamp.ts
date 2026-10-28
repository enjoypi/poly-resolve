const LOCAL = /^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2}))?$/;

type Parts = Record<"day" | "hour" | "minute" | "month" | "second" | "year" | "zone", string>;

function offset(millis: number, timeZone?: string): number {
  const wall = parts(millis, timeZone);
  const shown = Date.UTC(
    Number(wall.year),
    Number(wall.month) - 1,
    Number(wall.day),
    Number(wall.hour),
    Number(wall.minute),
    Number(wall.second),
  );
  return shown - (millis - (millis % 1000));
}

function parts(millis: number, timeZone?: string): Parts {
  const found = new Intl.DateTimeFormat("en-US", {
    day: "2-digit",
    hour: "2-digit",
    hourCycle: "h23",
    minute: "2-digit",
    month: "2-digit",
    second: "2-digit",
    timeZone,
    timeZoneName: "shortOffset",
    year: "numeric",
  }).formatToParts(millis);
  const part = (type: string) => found.find((entry) => entry.type === type)?.value ?? "";
  const zone = part("timeZoneName").replace("GMT", "UTC");
  return {
    day: part("day"),
    hour: part("hour"),
    minute: part("minute"),
    month: part("month"),
    second: part("second"),
    year: part("year"),
    zone: zone === "UTC" ? "UTC+0" : zone,
  };
}

export function clock(millis: number, timeZone?: string): string {
  const wall = parts(millis, timeZone);
  return `${wall.year}-${wall.month}-${wall.day} ${wall.hour}:${wall.minute}:${wall.second}`;
}

export function parse(text: string, timeZone?: string): number | undefined {
  const match = LOCAL.exec(text);
  if (!match) return undefined;
  const [, year = "", month = "", day = "", hour = "", minute = "", second = "0"] = match;
  const wall = Date.UTC(
    Number(year),
    Number(month) - 1,
    Number(day),
    Number(hour),
    Number(minute),
    Number(second),
  );
  const guess = wall - offset(wall, timeZone);
  return wall - offset(guess, timeZone);
}

export function stamp(millis: number, timeZone?: string): string {
  return `${clock(millis, timeZone)} ${zone(millis, timeZone)}`;
}

export function zone(millis: number, timeZone?: string): string {
  return parts(millis, timeZone).zone;
}

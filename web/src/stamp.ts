export function stamp(millis: number, timeZone?: string): string {
  const parts = new Intl.DateTimeFormat("en-US", {
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
  const part = (type: string) => parts.find((entry) => entry.type === type)?.value ?? "";
  const offset = part("timeZoneName").replace("GMT", "UTC");
  const zone = offset === "UTC" ? "UTC+0" : offset;
  return `${part("year")}-${part("month")}-${part("day")} ${part("hour")}:${part("minute")}:${part("second")} ${zone}`;
}

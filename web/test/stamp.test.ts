import { expect, test } from "bun:test";
import { clock, parse, stamp, zone } from "../src/stamp";

const NOON = Date.UTC(2026, 9, 28, 1, 20, 9);

test("按指定时区格式化并标注偏移", () => {
  expect(stamp(NOON, "Asia/Shanghai")).toBe("2026-10-28 09:20:09 UTC+8");
  expect(stamp(NOON, "America/New_York")).toBe("2026-10-27 21:20:09 UTC-4");
  expect(stamp(NOON, "Asia/Kolkata")).toBe("2026-10-28 06:50:09 UTC+5:30");
});

test("零偏移写作 UTC+0", () => {
  expect(stamp(NOON, "UTC")).toBe("2026-10-28 01:20:09 UTC+0");
});

test("午夜按 00 时显示", () => {
  expect(stamp(Date.UTC(2026, 0, 1, 0, 0, 0), "UTC")).toBe("2026-01-01 00:00:00 UTC+0");
});

test("缺省时区取运行环境", () => {
  expect(stamp(NOON)).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} UTC(?:[+-]\d{1,2}(?::\d{2})?)?$/);
});

test("clock 不带时区，zone 单独给出偏移", () => {
  expect(clock(NOON, "Asia/Shanghai")).toBe("2026-10-28 09:20:09");
  expect(zone(NOON, "Asia/Shanghai")).toBe("UTC+8");
  expect(zone(NOON, "UTC")).toBe("UTC+0");
});

test("parse 是 clock 的逆运算，秒可省略", () => {
  for (const zoneName of ["Asia/Shanghai", "America/New_York", "Asia/Kolkata", "UTC"]) {
    expect(parse(clock(NOON, zoneName).replace(" ", "T"), zoneName)).toBe(NOON);
  }
  expect(parse("2026-10-28T09:20", "Asia/Shanghai")).toBe(NOON - 9000);
  expect(parse("2026-10-28 09:20:09", "Asia/Shanghai")).toBe(NOON);
});

test("parse 在夏令时前后取正确偏移", () => {
  expect(parse("2026-07-01T00:00", "America/New_York")).toBe(Date.UTC(2026, 6, 1, 4, 0, 0));
  expect(parse("2026-12-01T00:00", "America/New_York")).toBe(Date.UTC(2026, 11, 1, 5, 0, 0));
});

test("parse 对不合格的文本返回 undefined，缺省时区取运行环境", () => {
  expect(parse("not a time", "UTC")).toBeUndefined();
  expect(parse("2026-10-28", "UTC")).toBeUndefined();
  expect(parse(clock(NOON).replace(" ", "T"))).toBe(NOON);
});

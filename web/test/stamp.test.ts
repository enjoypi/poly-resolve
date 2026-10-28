import { expect, test } from "bun:test";
import { stamp } from "../src/stamp";

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

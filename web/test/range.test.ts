import { expect, test } from "bun:test";
import { custom, describe, preset } from "../src/range";
import { PRESETS } from "../src/config";

const NOON = Date.UTC(2026, 9, 28, 1, 20, 9);
const ZONE = "Asia/Shanghai";

function find(label: string) {
  const found = PRESETS.find((item) => item.label === label);
  if (!found) throw new Error(label);
  return found;
}

test("全部不限制时间", () => {
  expect(preset(find("全部"), NOON, ZONE)).toEqual({});
});

test("1 小时与 7 天按当前时刻回推", () => {
  expect(preset(find("1 小时"), NOON, ZONE)).toEqual({ from_ms: NOON - 3_600_000 });
  expect(preset(find("7 天"), NOON, ZONE)).toEqual({ from_ms: NOON - 604_800_000 });
});

test("今天从所选时区的零点算起", () => {
  expect(preset(find("今天"), NOON, ZONE)).toEqual({ from_ms: Date.UTC(2026, 9, 27, 16, 0, 0) });
  expect(preset(find("今天"), NOON, "UTC")).toEqual({ from_ms: Date.UTC(2026, 9, 28, 0, 0, 0) });
});

test("自定义起止：两端、单端、都空", () => {
  expect(custom("2026-10-28T09:00", "2026-10-28T10:00", ZONE)).toEqual({
    from_ms: Date.UTC(2026, 9, 28, 1, 0, 0),
    to_ms: Date.UTC(2026, 9, 28, 2, 0, 0),
  });
  expect(custom("2026-10-28T09:00", "", ZONE)).toEqual({ from_ms: Date.UTC(2026, 9, 28, 1, 0, 0) });
  expect(custom("", "2026-10-28T10:00", ZONE)).toEqual({ to_ms: Date.UTC(2026, 9, 28, 2, 0, 0) });
  expect(custom("", "", ZONE)).toEqual({});
});

test("自定义起止无效时返回 undefined", () => {
  expect(custom("2026-10-28T10:00", "2026-10-28T10:00", ZONE)).toBeUndefined();
  expect(custom("2026-10-28T11:00", "2026-10-28T10:00", ZONE)).toBeUndefined();
  expect(custom("bad", "", ZONE)).toBeUndefined();
  expect(custom("", "bad", ZONE)).toBeUndefined();
});

test("describe 写出带时区的区间，缺端用最早与现在", () => {
  expect(describe({ from_ms: NOON }, ZONE)).toBe("2026-10-28 09:20:09 UTC+8 至 现在");
  expect(describe({ to_ms: NOON }, ZONE)).toBe("最早 至 2026-10-28 09:20:09 UTC+8");
});

import { expect, test } from "bun:test";
import { inferColumns, type Row } from "../src/columns";
import { applyView, nextSort, type State } from "../src/view";

const NOON = Date.UTC(2026, 9, 28, 1, 20, 9);

function rows(): Row[] {
  return [
    { key: "k1", occurred_at_ms: NOON + 3000, sender_name: "张三", text: "Disk full", zzz: 10 },
    { key: "k2", occurred_at_ms: NOON + 2000, sender_name: "", sender_id: "u2", text: "cpu high", zzz: 9 },
    { key: "k3", occurred_at_ms: NOON + 1000, sender_name: "李四", text: "", zzz: 100 },
    { key: "k4", occurred_at_ms: NOON, sender_name: "王五", text: "Disk ok" },
  ];
}

function keys(state: Partial<State>, data: Row[] = rows()): string[] {
  const full: State = { filters: {}, search: "", ...state };
  return applyView(data, inferColumns(data), full, "Asia/Shanghai").map((row) => String(row.key));
}

test("没有排序与筛选时保持原顺序", () => {
  expect(keys({})).toEqual(["k1", "k2", "k3", "k4"]);
});

test("文本排序按中文与自然序，空值无论升降都在最后", () => {
  expect(keys({ sort: { dir: "asc", key: "text" } })).toEqual(["k2", "k1", "k4", "k3"]);
  expect(keys({ sort: { dir: "desc", key: "text" } })).toEqual(["k4", "k1", "k2", "k3"]);
});

test("发送人排序用回退后的显示值", () => {
  expect(keys({ sort: { dir: "asc", key: "sender_name" } })).toEqual(["k3", "k4", "k1", "k2"]);
});

test("数字与时间按数值排序，缺失值在后，同值保持原顺序", () => {
  expect(keys({ sort: { dir: "asc", key: "zzz" } })).toEqual(["k2", "k1", "k3", "k4"]);
  expect(keys({ sort: { dir: "desc", key: "zzz" } })).toEqual(["k3", "k1", "k2", "k4"]);
  expect(keys({ sort: { dir: "asc", key: "occurred_at_ms" } })).toEqual(["k4", "k3", "k2", "k1"]);
  expect(keys({ sort: { dir: "asc", key: "source" } })).toEqual(["k1", "k2", "k3", "k4"]);
});

test("排序列不存在时忽略排序", () => {
  expect(keys({ sort: { dir: "asc", key: "nope" } })).toEqual(["k1", "k2", "k3", "k4"]);
});

test("文本筛选分词、全部命中、不区分大小写，列之间为且", () => {
  expect(keys({ filters: { text: "disk" } })).toEqual(["k1", "k4"]);
  expect(keys({ filters: { text: "DISK  ok" } })).toEqual(["k4"]);
  expect(keys({ filters: { text: "disk", sender_name: "张" } })).toEqual(["k1"]);
  expect(keys({ filters: { text: "   " } })).toEqual(["k1", "k2", "k3", "k4"]);
});

test("时间列按显示值包含匹配", () => {
  expect(keys({ filters: { occurred_at_ms: "09:20:09" } })).toEqual(["k4"]);
  expect(keys({ filters: { occurred_at_ms: "2026-10-28 09:20" } })).toEqual(["k1", "k2", "k3", "k4"]);
});

test("新增字段 zzz 零改动即可排序、筛选、被全局搜索命中", () => {
  expect(keys({ filters: { zzz: ">9" } })).toEqual(["k1", "k3"]);
  expect(keys({ filters: { zzz: ">=10" } })).toEqual(["k1", "k3"]);
  expect(keys({ filters: { zzz: "<10" } })).toEqual(["k2"]);
  expect(keys({ filters: { zzz: "<=10" } })).toEqual(["k1", "k2"]);
  expect(keys({ filters: { zzz: "=100" } })).toEqual(["k3"]);
  expect(keys({ filters: { zzz: "100" } })).toEqual(["k3"]);
  expect(keys({ filters: { zzz: "9..10" } })).toEqual(["k1", "k2"]);
  expect(keys({ filters: { zzz: "10" } })).toEqual(["k1"]);
  expect(keys({ search: "100" })).toEqual(["k3"]);
});

test("数字列的非数字表达式按包含匹配，缺失值不命中数字比较", () => {
  expect(keys({ filters: { zzz: "x" } })).toEqual([]);
  expect(keys({ filters: { zzz: ">0" } })).toEqual(["k1", "k2", "k3"]);
});

test("全局搜索跨所有字段含隐藏列与回退值", () => {
  expect(keys({ search: "u2" })).toEqual(["k2"]);
  expect(keys({ search: "k3" })).toEqual(["k3"]);
  expect(keys({ search: "cpu high" })).toEqual(["k2"]);
  expect(keys({ search: "" })).toEqual(["k1", "k2", "k3", "k4"]);
  expect(keys({ search: "disk", filters: { sender_name: "王" } })).toEqual(["k4"]);
});

test("nextSort 依次为升序、降序、取消，换列重新从升序开始", () => {
  const asc = nextSort(undefined, "text");
  expect(asc).toEqual({ dir: "asc", key: "text" });
  const desc = nextSort(asc, "text");
  expect(desc).toEqual({ dir: "desc", key: "text" });
  expect(nextSort(desc, "text")).toBeUndefined();
  expect(nextSort(desc, "key")).toEqual({ dir: "asc", key: "key" });
});

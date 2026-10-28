import { expect, test } from "bun:test";
import { display, inferColumns, type Column } from "../src/columns";

const NOON = Date.UTC(2026, 9, 28, 1, 20, 9);
const KNOWN = [
  "occurred_at_ms",
  "conversation_id",
  "sender_name",
  "text",
  "key",
  "received_at_ms",
  "sender_id",
  "source",
  "account",
  "priority",
  "analysis",
];

function find(columns: Column[], key: string): Column {
  const column = columns.find((entry) => entry.key === key);
  if (!column) throw new Error(`no column ${key}`);
  return column;
}

test("没有数据时仍给出全部已知列，默认隐藏的列带标记", () => {
  const columns = inferColumns([]);
  expect(columns.map((column) => column.key)).toEqual(KNOWN);
  expect(columns.filter((column) => column.hidden).map((column) => column.key)).toEqual([
    "key",
    "received_at_ms",
    "sender_id",
    "source",
    "account",
    "analysis",
  ]);
});

test("已知列按配置顺序，未知字段随后按字母序且默认显示", () => {
  const columns = inferColumns([{ zzz: 5, text: "a", aaa: "x" }, { zzz: 6, extra: true }]);
  expect(columns.map((column) => column.key)).toEqual([...KNOWN, "aaa", "extra", "zzz"]);
  expect(find(columns, "zzz").hidden).toBeUndefined();
  expect(find(columns, "zzz").label).toBe("zzz");
  expect(find(columns, "text").label).toBe("正文");
});

test("未知字段按值推断类型：_ms 整数为时间，数值为数字，其余为文本", () => {
  const columns = inferColumns([
    { seen_ms: 1000, ratio_ms: 1.5, score: 3, mixed: 1, name: "a", empty: null },
    { seen_ms: 2000, ratio_ms: 2, score: 4, mixed: "x", name: "b", empty: null },
  ]);
  const kinds = Object.fromEntries(columns.map((column) => [column.key, column.kind]));
  expect(kinds).toMatchObject({
    empty: "text",
    mixed: "text",
    name: "text",
    ratio_ms: "number",
    score: "number",
    seen_ms: "time",
  });
  expect(kinds.occurred_at_ms).toBe("time");
});

test("显示值：时间带格式、发送人回退、占位、对象转 JSON、空值为空串", () => {
  const columns = inferColumns([]);
  const show = (row: Record<string, unknown>, key: string) =>
    display(row, find(columns, key), "Asia/Shanghai");
  expect(show({ occurred_at_ms: NOON }, "occurred_at_ms")).toBe("2026-10-28 09:20:09");
  expect(show({ sender_name: "", sender_id: "u9" }, "sender_name")).toBe("u9");
  expect(show({ sender_name: "alice", sender_id: "u9" }, "sender_name")).toBe("alice");
  expect(show({ text: "" }, "text")).toBe("[非文本消息]");
  expect(show({}, "text")).toBe("[非文本消息]");
  expect(show({ key: null }, "key")).toBe("");
  expect(show({ key: { a: 1 } }, "key")).toBe('{"a":1}');
  expect(show({ key: 7 }, "key")).toBe("7");
  expect(show({ key: true }, "key")).toBe("true");
  expect(show({ priority: 0.95 }, "priority")).toBe("0.95");
  expect(show({ priority: null }, "priority")).toBe("");
  expect(show({ analysis: "建议回复" }, "analysis")).toBe("建议回复");
});

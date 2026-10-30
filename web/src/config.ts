export type Field = {
  fallback?: string;
  hidden?: boolean;
  kind?: Kind;
  label: string;
  look?: Look;
  placeholder?: string;
  primary?: boolean;
};

export type Kind = "number" | "text" | "time";

export type Look = "badge" | "markdown";

export type Preset = { back_ms?: number; label: string; today?: boolean };

export const COPY_FEEDBACK_MS = 1500;

export const DEFAULT_SORT = "occurred_at_ms";

export const DETAIL = {
  body: "text",
  conversation: "conversation_id",
  sender: "sender_name",
  time: "occurred_at_ms",
};

export const FIELDS: Record<string, Field> = {
  occurred_at_ms: { kind: "time", label: "时间", primary: true },
  conversation_id: { label: "会话", look: "badge", primary: true },
  sender_name: { fallback: "sender_id", label: "发送人", primary: true },
  text: { label: "正文", look: "markdown", placeholder: "[非文本消息]", primary: true },
  key: { hidden: true, label: "key" },
  received_at_ms: { hidden: true, kind: "time", label: "接收时间" },
  sender_id: { hidden: true, label: "发送人 ID" },
  source: { hidden: true, label: "来源" },
  account: { hidden: true, label: "账号" },
  priority: { kind: "number", label: "优先级" },
  analysis: { hidden: true, label: "分析" },
  topic: { hidden: true, label: "话题" },
};

export const MAX_ROWS = 2000;

export const MD_LIMIT = 20000;

export const PAGE_LIMIT = 200;

export const PIN = [DETAIL.conversation, DETAIL.sender];

export const PLAIN_LIMIT = 600;

export const PRESETS: Preset[] = [
  { label: "全部" },
  { back_ms: 3_600_000, label: "1 小时" },
  { label: "今天", today: true },
  { back_ms: 604_800_000, label: "7 天" },
];

export const REVIEW_HASH = "#reviews";

export const REVIEW_LIMIT = 200;


export const TEXT = {
  all: "加载全部",
  approve: "通过",
  approveAll: "整批通过",
  back: "消息",
  batch: (id: number) => `批 ${id}`,
  brand: "Poly Resolve",
  capped: (max: number) => `已达 ${max} 条上限，请缩小时间范围。`,
  clear: "清除筛选",
  clearMark: "×",
  close: "关闭",
  columns: "列",
  copied: "已复制",
  copy: "复制",
  copyFailed: "复制失败",
  counts: (shown: number, loaded: number) => `显示 ${shown}／已加载 ${loaded} 条。`,
  empty: "没有消息。",
  fields: "全部字段",
  flagged: (flag: string) => `本地模型提示：${flag}`,
  leakPaste: "粘贴漏掉的原文",
  more: "加载更多",
  noReviews: "没有待审核项。",
  panel: "筛选",
  partial: "还有更早的消息未加载，筛选与排序只含已加载的行。",
  pending: (count: number) => `待审 ${count} 条`,
  range: "时间范围",
  rangeInvalid: "起止时间无效",
  reasonPick: "拒绝原因",
  refresh: "刷新",
  reject: "拒绝",
  reviews: "审核",
  search: "搜索全部字段",
  since: "起",
  span: (text: string) => `时间范围：${text}。`,
  unreachable: "unreachable",
  until: "止",
};

export const TIP_DELAY_MS = 400;

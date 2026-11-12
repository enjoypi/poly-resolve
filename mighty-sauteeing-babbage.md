# 实现 docs/requirements.md：话题跨来源全自动归并

## Context

`docs/requirements.md` 已于 e2c8f43 改成目标需求，代码仍是旧行为，主要差距如下：
- 本人消息、过期消息直接跳过，`line.rs:70` 判定 own/expired。
- 自动邮件和列表邮件不入库，`mail.rs:185` 直接丢弃。
- 保留人工审核与限额。
- 话题按会话切段，归入后不再合并，按空闲窗口隐式关闭。
- codex 续写时只补发增量。
- 占位符为固定字符串。
- 没有话题视图。

用户已定：
- 分 4 期实现，每期都能单独上线，各一个 commit。
- 线上旧数据迁移后保留。
- hash 用最快的 blake3 keyed。

## 通用

- 测试先行；每期跑 `bun scripts/check.ts`，改了路由先跑 `bun scripts/gen.ts`；同期同步 README、对应 CLAUDE.md、docs。
- 迁移只用三种方式：
  - 新表写进 `adapters/src/store.rs:41` 的 `SCHEMA`，用 `IF NOT EXISTS`；
  - 补列用 `ensure`；
  - 数据迁移追加到 `adapters/src/ledger.rs` 的 `MIGRATE`，必须幂等。
- 每期都写迁移测试：先用旧 SQL 建库，再 `Ledger::open` 两次，结果须一致。
- 旧表 `placements`、`learned_rules` 不删，只是不再写入。

## 第 ① 期：已完成（3a9bcb0）

实现与原计划的出入，后续各期以此为准：
- `role` 放在 `transcript::Row`（pack 的行与 brief 的行共用），布局类型在 `pack/layout.rs`；sort 的 `Placement.topic` 可空，空即无关。
- `learned_rules` 仍参与脱敏（只读），`rerender`、`rules_rev` 保留；第 ③ 期改为按原文重新脱敏时一并处理。
- `PLACE_WITHHELD` 不限本批，靠索引 `lines_by_conversation`；第 ② 期改 `members` 表时保留这个索引与写法。
- 没有问题的批不外发，直接按空应答落结果。
- 邮箱重拉用 `backfills` 表（版本 1）；以后再需要重拉就加版本号。
- 文件拆分只做了 `pack.rs`，其余三个删掉审核代码后未超限；第 ② 期动 `ledger.rs`、`config.rs` 前先看行数。
- “排队的转交新的先出”的用例已删，第 ② 期改按话题优先级排队时补新用例。
- 线上库演练办法见 adapters/CLAUDE.md“测试”。

## 第 ② 期：已完成

实现与原计划的出入，后续各期以此为准：
- 优先级在 `apply` 事务里用 SQL 重算（`REACH`、`RESCORE`），没有 domain 的 `priority.rs`；紧急度取话题里最新一条有概率的成员，不由 `sort` 给出，`sort` 不再收阈值。
- 强线索经 `linked` 传给 `pack`，同会话相邻（`attach`）保留；同一批里互不相识的两段即使线索相同也各起话题，留给第 ④ 期按共同线索合并。
- 线索不在 intake 写，由独立的 `index` 步骤补（含旧行），以 `clues(kind = 'seen')` 标记已建；钉钉转发没有可用的原消息标识，转发关系留给第 ③ 期的正文摘要。
- `relations` 只在“选了已有话题但置信度不足”时累加，同一对话题不会自然累加到多次；第 ④ 期要另出是非题来累加。话题改名不改 `relations`，读时经 `topic_aliases` 解析。
- `brief` 不再按总长截断（`fit` 已删），`context_max_chars` 只限单行。
- 通知占位为 `{state}{score}{urgency}{people}{conversations}{sources}{changes}{late}`；`triage.notice` 改成结构（`template`、`late`、`late_after_ms`、`states`），第 ④ 期的汇总模板加在这里。
- 回复首行 JSON 不合规按分析失败重试（沿用 `max_attempts`）；重放已存分析时一律通知。
- 归档只看 `last_at_ms`：强线索重开的归档话题若最后一条消息仍超龄，下一轮又归档。
- 旧话题的 `reach` 为 `{}`，要到下次有新成员才重算；第 ④ 期的话题视图须容忍空值。

## 第 ③ 期：keyed hash 占位、人员对应、转发去重

- 依赖：`blake3 = { version = "*", default-features = false, features = ["std"] }`。若出现 linker-info 或 `multiple_crate_versions`，改加 `pure` feature。
- `redact.rs`：
  - `Rule` 加 `kind` 和 `group`。有 kind 的规则把匹配值规范化，算 `keyed_hash`，占位写成 `<kind>#<前 8 位十六进制>`。单号不配规则。
  - `marks` 改为输出占位前缀的正则，供页面标红。
- 密钥：环境变量 `TRIAGE_HASH_KEY`（32 字节），由 cli 读入 `Context`。不实现 Debug，不进请求体；缺失时开关开启即启动失败。
- 重新脱敏以 `messages.text` 原文为源，叠加 `lines.spans`（本地模型给出的遮盖片段），满足“每次外发前按当时规则”；删掉 STALE 和 RESTATE。
- 新表 `identities(source, sender_id, person_id, name, email)`。`features/triage/people.rs` 判定同一人：邮箱相同即确定，规范化姓名全库唯一也算确定，有冲突时视为不同的人。reach 的人数按 `person_id` 计。
- 转发去重：`lines.digest = blake3::hash(规范化正文)`。同一批内只外发一份，并注明出现在 N 处；reach 也按 digest 去重。
- 钉钉资料邮箱：先确认 dws 有查询资料的命令；没有就退回到只按姓名和签名判定，并写进盲区。
- 配置：`redact.patterns[].kind/group`、`redact.hash_chars`；同步改 `cli/config.yaml` 和 `domain/tests/it/redact.rs` 的样例。
- 风险写进 README：换密钥后历史占位对不上。

## 第 ④ 期：自动合并、拆分、移动，通知合并，话题视图

- 新表 `changes(id, kind, from_topic, to_topic, keys, reason, state, at_ms)`。`alerts` 加 `merged_into` 列和 `merged` 状态。
- `features/triage/regroup.rs` 判定三种变更：
  - 合并：两个话题出现共同强线索，或 `relations.count` 达到 `merge_after`。
  - 移动：强线索指向另一个话题。
  - 拆分：对近期变动过的话题加一道是非题“这段仍属话题 X 吗”，多次判否即拆。每批题数有上限。
- 话题有分析在途时，变更先记为 pending，等本轮结束再应用。本轮结论转 quiet，再开新一轮分析。
- 合并：保留较早的话题，较晚话题的 id 写进 alias，全量续写较早的会话。拆分：拆出的部分新建话题；原话题 FORGET_THREAD，再新建会话。
- `deliver`：`notice_cooldown_ms` 内同一话题只发最新一条，其余标为 merged；启动时若有多个话题待发，汇总成一条。
- 新 feature `topics`，`Trace::Redacted`：
  - `GET /api/topics`：话题列表、各状态计数、盲区；
  - `GET /api/topics/{id}`：时间线（含未外发项及原因、归入方式）和变更记录。
- 前端：新增 `topics.ts`、`topic.ts`，hash 路由 `#topics`、`#topic/<id>`；`detail.ts` 与话题列可跳转。
- 配置：`topics.merge_after`、`limits.notice_cooldown_ms`、`notice.summary`。

## 风险

- 第 ② 期本人消息也触发重新分析，codex 调用量上升；按 score 排队可以缓解。
- 第 ② 期按通用主题（如“回复”“通知”）容易误归并，主题线索要设最小长度和黑名单。
- 第 ④ 期合并、拆分与会话改名交织在一起，最容易出现竞态。测试重点：分析在途时的变更延后；alias 只跳一跳。

## 验证

- 每期 `bun scripts/check.ts` 全绿，覆盖率要求 branches 和 regions 满额。
- 用线上库副本只读演练迁移：迁移前后 `lines` 各状态的计数要能对上；锚点之后没有不带 line 的消息；每个 approved 或 withheld 的行都至少有一条 `members`。
- 每期按 `/serve` 起服，`curl /api/health` 验证可用；第 ① 期查 `/api/outbound`，第 ④ 期查 `/api/topics`。

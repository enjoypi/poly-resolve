# 清空 TODO：收紧线索归并、上线 ①–④ 期、人员按邮箱对应、上线后观察

## Context

`TODO.md` 剩 12 项，分四类：一个代码缺口（强线索无范围上限）、①–④ 期的上线动作、上线后的观察项、人员按邮箱对应。线上（本机，pm3 管理）仍是话题第 ① 期之前的版本。

三个角度的推敲（实现、发布、批判）得出的关键事实：

- 一条共同的链接、文件名或主题就会自动合并两个话题或移动消息，出错后页面不能纠正；`merge_after` 只管复核投票，管不到线索合并。线上库副本演练的 267 次自动调整全部由线索引起。
- 新批号只取 `lines` 的最大批号，迁移清零后可能与 `batches` 已有批号相撞，之后新行不再外发。
- 首次启动全量补建线索期间计数偏低，范围上限尚未生效就会发生合并。
- 重处理的约 1500 条历史消息都早于 1 小时，不触发分析与通知；库里没有积压的提醒。
- `serve` 在开库前有两个退出点（缺变量退出 2、冒烟失败退出 8），可用来无副作用地校验新配置。
- 库迁移在出现 `listening` 时已完成且不可逆；`backup-20261112` 里的二进制是旧版，不能用于回退。
- 线上各命令的 PATH 写在 `env.PATH`，必须保留，不迁到 `vars`。
- 线上密钥实际在 `~/.config/pm3/poly-resolve.enc.yaml`；仓库里没有 `secrets/prod.enc.yaml`。

用户已定：四类都做；弱线索经复核确认再合并；授权与管理员同意已取得（含查询钉钉资料里的邮箱）；除密钥外线上操作由 AI 执行，`TRIAGE_HASH_KEY` 由 AI 生成并用 `sops set` 直接写入（不显示、不落明文）；①–④ 期一次上线，直接放开并设刹车线。

## 第 A 期：收紧线索归并（代码，一个 commit）——已完成

演练（线上库副本、本地桩）：线索直接引起的调整 28 次，全部来自 `mail`、`digest`；弱线索记疑 522 对，待复核；批号检查通过；一致性核对全为 0。会话数最多的话题（15 个会话）全是旧版遗留的归属。

线索分两档：`mail`（回复链）与 `digest`（同一内容转发）为强线索，行为不变；配置正则的线索与 `subject` 为弱线索。

1. **范围上限**：新配置 `triage.topics.clues.max_conversations`（`NonZeroU32`，默认 5）。弱线索值出现的不同会话数超过它即作废。`adapters/src/ledger/sql.rs` 加宏 `strong!(别名)` 生成判定片段，`LINKED` 与 `DRIFT` 共用。
2. **弱线索不直接合并**：`DRIFT` 的 `clue` 分支只取强线索。新增一条语句把“成员有共同弱线索（未超上限）的两个话题”写入 `relations(kind = 'maybe')`（已有该对则不重复加），由现有 `REVIEWS` 出复核题、`VOTE` 计票、达 `merge_after` 才合并。弱线索不再触发移动。
3. **封批归入防扇出**：`LINKED` 里弱线索只在该行恰好指向一个话题时生效，指向多个话题时不直接归入，改由决策接口选择。
4. **补建线索期间不调整**：`regroup` 在 `Ledger::unindexed` 仍有积压时跳过（`adapters/src/triage/regroup.rs`）。
5. **批号修复**：`FORM` 的新批号取 `lines.batch_id` 与 `batches.id` 两者最大值加 1。
6. **留痕**：`changes.reason` 记触发来源（`mail`、`digest`、`vote`、`apart`），页面原样显示。

测试先行，放 `adapters/tests/it/triage/`：
- `clues.rs`：超上限的线索不再归入、已归入的不回滚；同一会话内重复不计；回复链不受上限约束；弱线索指向两个话题时走选择题。
- `regroup.rs`：共同弱线索只记 `maybe` 不合并；复核判同达 `merge_after` 次后合并；强线索仍直接合并；线索未建完时不调整。
- `batch.rs`：批号不与已有批相撞。
- `seal.rs`：`digest` 不受上限约束。`policy.rs`：`max_conversations: 0` 被拒。

同步：`cli/config.yaml`、两处测试配置、README 的 `triage.topics` 行、`adapters/CLAUDE.md`、`docs/requirements.md` 第 86 行（弱线索须多次判为同一件事才合并）、`TODO.md` 删对应项。

验证：`bun scripts/check.ts` 全绿；线上库只读备份到 `.debug/`，用临时测试加本地桩重演练（沿用上次做法，用线上线索规则），验收：线索直接引起的变动只来自 `mail`、`digest`；最大话题的会话数不异常；批号检查通过；一致性核对全为 0。演练产物随后删除。

## 第 B 期：上线 ①–④ 期与第 A 期

### 准备（不停机）

1. `bun scripts/check.ts`，再 `cargo +stable test --release --locked && cargo +stable build --release --locked`。
2. 密钥：`openssl rand -hex 32 | jq -R . | sops set --value-stdin <文件> '["TRIAGE_HASH_KEY"]'`，分别写入 `~/.config/pm3/poly-resolve.enc.yaml` 与 `secrets/dev.enc.yaml`（各用独立密钥，写入后不再更换）。`SCREEN_URL` 同法写入 pm3 那份，值取线上配置里现有的回环地址。
3. 基线快照（只读计数）：`alerts`、`lines`、`batches`、`topics` 各状态数量，近 1 小时消息数，日志里 WARN、ERROR 的种类与条数。
4. 生成 `~/.config/poly-resolve/config.next.yaml`：复制线上配置后逐项文本修改，保留锚点、本机路径、`env.PATH`、`questions.threshold: 0.85`、`screen` 段。
   - 删：`triage.review`、`limits` 的四个旧键、`sizes.session_max_ms`、`sizes.topics_max_open`。
   - 加或换（照 `cli/config.yaml`）：`limits.notice_cooldown_ms`、`notice` 整段、`questions.review`、`questions.topic.none_label`、`redact` 整段、`screen.min_chars`、`session.omitted`、`topics` 整段。
   - `screen.http.url` 改回 `${SCREEN_URL}`。
   - 发件文件夹数据源：需要用户给出两个邮箱发件文件夹的准确名字；拿不到就本次不加，留在 TODO。
5. 提示词放 `prompts.next/`：`topic`、`urgent`、`screen` 用线上版，`session`、`followup`、`review` 用仓库版。前端构建到全新目录 `web.next/`。二进制放 `~/.local/bin/poly-resolve.next`。
6. 两级校验，都不开库：
   - A：空环境跑新二进制加新配置，期望退出 2 并列出缺失变量名；与 `pm3 show` 里的变量名（只取名字与长度，不看值）比对，差集须为空。
   - B：去掉邮箱数据源、端口与库指向临时路径、决策地址指向拒绝连接的回环口，用假值跑，期望 `triage.decide: unreachable` 退出 8 且临时库未生成。

### 切换（停机约 5 分钟，选安静时段）

1. `pm3 stop poly-resolve`，确认无残留子进程。
2. 备份到 `backup-<日期>/`：库用 `sqlite3 -readonly … ".backup"` 并跑 `integrity_check` 与计数核对；另备份二进制、配置、提示词、web、`codex-home`；记下切换时刻。备份权限 600。
3. 用 `mv` 换入配置、提示词、web、二进制（不原地覆盖已签名的二进制）。
4. `pm3 start poly-resolve`，日志出现 `listening`，重启次数为 0。

回退：未出现 `listening` 时库未动，把四组 `mv` 反过来即可；出现之后放弃则整体回退——停服、挪走迁移后的库三件套、放回备份库与旧二进制、配置、提示词、web、`codex-home`，再启动。

### 验证与刹车

- 立即：`/api/health`、`/api/topics`、`/api/prompts`（含 `review`）、页面 `#topics`。
- 每 10 分钟（只读计数）：待处理行数单调下降；`batches` 无 `failed`；线索已建完；旧式邮件会话标识为 0；`changes` 各类数量；切换后新发通知数；最大话题的会话数。日志只允许基线里已有的 WARN。
- 刹车线：30 分钟内通知超过 6 条或 2 小时超过 15 条，先把 `questions.threshold` 设为 1.01 并重启，不够再 `triage.mode: disabled`，最后才整体回退。任何时候不清表。
- 漏通知：30 分钟后待处理仍超过 500 条，临时把 `limits.lookback_ms` 调到 2 小时并重启，清空后改回；清空后查“分数达阈值却没有提醒”的话题，交用户过目。

完成后：`TODO.md` 删上线相关各项；`backup-<日期>` 观察期结束后删除。

## 第 C 期：人员按邮箱对应（代码，上线稳定后做，一个 commit）——验证后止步

验证结论：消息里的发送人标识是 openDingTalkId，经 `contact user get-by-open-dingtalk-id` 只得 `user_id`；`contact user get` 与 `contact user search` 都不返回邮箱；`get-self` 有 `orgEmail` 但只限本人；花名册 `profile` 报“操作人无花名册管理权限”。拿不到同事的邮箱，按计划止步，结论已写入需求文档的盲区。

先验证：在沙箱策略下真实调用一次 `dws contact user get-by-dingtalk-id`（查本人），确认参数形式、返回里邮箱字段名、失败时的退出码与输出。字段不含邮箱则本期止步，把结论记进需求文档的盲区。

实现：
- 钉钉采集任务里查询资料（`adapters/src/dingtalk/` 新文件 `contacts.rs`，`dws.rs` 加 `contact`）：每个发送人只查一次，结果存新表 `contacts(source, sender_id, email, attempts, retry_at_ms, updated_at_ms)`；失败按退避加抖动重试，本轮遇到失败即停；日志不带 id 与邮箱；只留邮箱，其余资料字段不落库。
- 新配置 `ingest.sources[].dingtalk.dws.contact`（命令）与 `contacts{batch, retry_initial_ms, retry_max_ms}`。
- domain `triage/people.rs`：请求带 `email`；有效地址（资料邮箱，否则发送人标识本身是邮箱）得 `email:<小写地址>`；同名规则保留，同名者地址互相矛盾时不合并。
- `identities` 加 `email` 列并清空重算；`STRANGERS` 带出资料邮箱，资料晚到时触发重算。
- 邮件签名解析不做（引用与转发里的签名易把两个人并成一个），需求文档继续列为盲区。

测试：domain `people.rs` 各规则；`fake-dws.sh` 加 `contact` 分支；`adapters/tests/it/ingest/contacts.rs`（只查一次、失败退避、无邮箱不再查、停机打断）；`seal.rs` 影响面只计一次与资料晚到重算；`migrate.rs` 补列。

同步 README、`adapters/CLAUDE.md`、`domain/CLAUDE.md`、`docs/requirements.md` 第 88 行、配置样例、`TODO.md`。`bun scripts/check.ts` 全绿后按第 B 期的切换步骤再上线一次（线上配置加两个新键）。

## 第 D 期：上线后观察（到期执行，结论写回配置或提示词）

查询只做计数与分类；需要看原文的由用户本人看。

- 上线次日，`misses` 表：按 `kind` 与日期计数。`rules` 里同一片段出现 3 次以上，请用户看原文后写成脱敏规则；`unscreened` 超过同期筛查行数 5% 调大 `screen.max_tokens`；`unmasked` 超过 2% 改 `screen.md`。
- 上线次日，`changes` 表：按类型、来源、日期计数，随机抽 10 条在话题详情页核对。误合并 2 条以上把 `merge_after` 调到 3 或改 `review.md`；误拆 2 条以上把 `split_after` 调到 3。
- 上线次日，`subject` 线索：跨 3 个以上会话而互不相关的主题加进 `topics.clues.subject.ignore`。
- 一周后：话题分数分布、每日通知数、决策接口非 200 的次数。据此复核 `questions.threshold`、`min_confidence`、`max_conversations`，限速则调小 `limits.batch`。

每完成一项从 `TODO.md` 删一项，清空后删除文件。

## 关键文件

- `adapters/src/ledger/sql.rs`、`adapters/src/triage/regroup.rs`、`adapters/src/triage/config/topics.rs`、`adapters/src/triage/batch.rs`
- `adapters/src/dingtalk/dws.rs`、`adapters/src/store.rs`、`domain/src/features/triage/people.rs`
- `cli/config.yaml`、`~/.config/poly-resolve/config.yaml`、`~/.config/pm3/poly-resolve.enc.yaml`
- 复用：`sql.rs` 的 `REVIEWS`、`VOTE`、`RELATE`；`store.rs` 的 `Column` 迁移机制；`adapters/src/jitter.rs`；测试的 `rig.rs` 与 `fake-dws.sh`

## 约束

- 每期测试先行，`bun scripts/check.ts` 全绿（branches、regions 满额），文件不超过 512 行。
- 提交一期一个，单行不多于 64 字符；推送等用户发话。
- 线上查询不输出消息正文；不解密、不显示任何密钥。

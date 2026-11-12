# adapters

## 职责

- I/O：dws 子进程、SQLite 存储、收集与分级编排；可用 tokio；依赖 domain，被 cli 依赖
- 不写业务规则：解析、去重键、时间换算、脱敏都在 domain，经 `dispatch` 调用；业务字符串与数值取自配置

## 模块

- `config.rs`：`Settings`（cli 配置的 `ingest` 段，`deny_unknown_fields`）：共享项与 `sources`；`verify`（非空、无无类型项、标签不重复，cli 加载时调用）、`check`（各源自检并发）、`missing`、`profile(标签)`、`prepare`（建库建表开 WAL，cli 起任务前同步调一次，免得多连接同时首次切 WAL 互锁成 `database is locked`）
- `ingest.rs`：`Source`（每个可选字段一种类型：`dingtalk`、`imap`；不用枚举，因引用上 match 枚举撞 `pattern_type_mismatch`，取值一律 `Option` 组合子）、`Site`（`Settings` 与 `&Context` 打包）、`Profile`（本人地址列表、通知会话、密文长度，均可空，供 triage 按消息来源判定）、`label(来源, 账号)`、`run(site, source, stop)` 按类型分派
- `store.rs`：
  - `connect(path, busy_ms)`：`busy_timeout`、`journal_mode = WAL`、`synchronous = NORMAL` 与 `SCHEMA`，`Archive::open`、`Ledger::open`、`Settings::prepare` 共用；旧库缺列由 `ensure(conn, Column)` 补（`lines.attempts`；`alerts.score`；`topics.until_key`；`batches.revivals`，补列时把已有 `failed` 批的 `revivals` 置 -1，表示不自动重来、只等人工重发）
  - `Shelf`：`inbox::Reader`、`review::Docket` 与 `batches::Roster` 的 SQLite 实现，`Settings::shelf()` 构造；每次调用才打开（删库造 503 的用例依赖它，不改成长连接）并设 `busy_timeout`，`page`、`list`、`source` 只读（`SQLITE_OPEN_READ_ONLY`），`settle`、`revive`、`draw` 读写，都不建库不建表；`draw` 在 `IMMEDIATE` 事务里把至多 `limit` 条最新的 `queued` 行编入新批（批号＝最大批号 + 1），未封批里还有 `screening/pending/leaked` 行时不动并返回 `None`，没有 `queued` 行返回批号与条数均为 0；`revive` 只把 `failed` 且当前没有 `ready` 批的批置回 `ready`，`revivals` 取 `MAX(revivals, 0) + 1`，返回该批当前状态
  - `page`：单条 SQL 用 `json_group_array(json_object(…))` 取回再反序列化成 `Row`，不按来源过滤，行值比较 `(occurred_at_ms, key) < (游标)` 翻页，叠加 `Query` 的时间范围 `[since, until)`（走 `messages_by_when` 索引）；失败记 warn 并返回 `AppError::Storage`
  - `settle`：`Transaction::new_unchecked(Immediate)` 一个事务里先写漏判样本 `misses`（kind `review`，文字取重新脱敏前的 `lines.text`，片段为学到的字面量规则）与 `learned_rules`（`INSERT OR IGNORE`），两者都仅当该行仍是 `pending`，再 `UPDATE … WHERE state = 'pending'`，最后读回当前状态
  - `Archive`：`INSERT OR IGNORE … json_each` 直接吃 domain 返回的 JSON（`save_event` 包成数组，`save_page` 取 `$.envelopes`）；水位取 `MAX(occurred_at_ms)`，不另建表；错误装箱
- `ingest/cycle.rs`：各数据源共用的 `Halt`（`Busy`、`Exit`）、`Retry`（`Failed`、`Progress`、`Stalled` 取上限）、`fault`（SQLite 忙为 `Busy`，其余为 `failure_exit_code`）与 `Backoff::new(初始, 上限)`（翻倍到上限，实际等待经 `jitter::spread` 随机取 ±1/4，下一轮仍按未抖动的值翻倍；采集重连与 triage 的 lane 退避共用）
- `jitter.rs`：`spread(wait)` 在 `[wait - wait/4, wait + wait/4]` 内随机取值（`RandomState` 哈希，无额外依赖），供 `Backoff`；triage 的决策、筛查、分析、通知与失败批自动重来在 SQL 里判到期：`ledger.rs` 的宏 `wait!(次数)` 为 `MIN(retry_after_ms << MIN(MAX(次数 - 1, 0), 20), retry_max_ms)`，`due!(时间列, 次数)` 为 `时间 + wait + 时间 % (2 * (wait / 4) + 1) - wait / 4 <= now_ms`，四条语句共用（`BATCH_NEXT`、`NEXT_ALERT`、`SCREENING` 用各自的 `attempts`，`REVIVE` 用 `revivals + 1`）；参数经 `triage::pace` 放在 `$.pace`；每条记录的抖动由其失败时间推出、固定不变，改小间隔即时生效；日志里的 `retry_in_ms` 由 `triage::retry_in(次数)` 给出未抖动的等待
- `dingtalk.rs`：钉钉采集 `Run`，`run(site, config, stop)` 返回退出码；`dingtalk/config.rs` 为每源配置（含 `Dws`、`Process`、本人与通知会话的 `Var`）
- `dingtalk/bridge.rs`：持有 `&Context`，构造请求调 domain 的 `ingest` feature；`Page` 是 `parse_page` 结果的可改写副本（`envelopes_mut`、`render`），保存的是改写后的页面
- `dingtalk/dws.rs`：`Dws`（`listen` 起监听子进程，stdin、stdout 管道，stderr 继承；`pull` 一次性执行并收 stdout；`decrypt` 一次性执行，追加 `--text=<密文>`；`lookup` 一次性执行，追加 `--msg-ids=<ID,…>`；三个一次性命令共用 `output`，超过 `dws.timeout_ms` 返回 `TimedOut` 并经 `kill_on_drop` 杀掉子进程，调用方按失败处理）、`Listener`（`next_line`、`stop`）；`plaintext(&Output)` 从解密命令的 JSON 取 `plaintext`，缺失或非 JSON 为 `None`
- 自身不 spawn：每个数据源与 triage 的两条循环由 cli 各起一个任务，对外的 future 须 `Send`；任务内的并发只用 `futures_util`（`buffered`、`buffer_unordered`、`join_all`），不起任务；流的元素与闭包捕获须是拥有的值或同一批长寿引用（如 `Arc`），对借用的元素写 `map(异步闭包)` 会让 cli 的 `Box::pin` 报 `lifetime bound not satisfied`（rust#100013）
- `race.rs`：`race(stop, work)` 用 `poll_fn` 先 poll 停机信号再 poll 工作，代替被 clippy 禁的 `select!`；返回 `None` 即被停机打断
- `seatbelt.rs`：`Sandbox`（`bin`、`canary`、`paths`、`probe`、`profile`、`vars`，`deny_unknown_fields`）
  - `command(program)` 拼 `bin -f <真实路径 profile> -D 名=值 … -- program`；`paths` 与 `profile` 用 `canonicalize(..).unwrap_or_default()`，路径不存在即得空值，`sandbox-exec` 以 65 拒绝，不新增分支；`vars` 取环境变量，缺失为空
  - `check(probe, name)` 同时跑 `canary`（须非 0）与 `probe`（须 0），错误是字符串；`Dws::check`、triage 的 `check` 用 `join_all` 并发，报错按声明顺序取第一个
  - 钉钉源 `dws` 的 4 个 `Process`、triage 的 `codex` 与 `notify` 各带一份 `sandbox`，命令一律经 `command` 起，不得绕过；`Dws::check`、`triage::config::Settings::check` 供 cli 启动自检。决策与本地模型使用进程内 HTTP，不经 Seatbelt
- `triage.rs`：`Setup`、`Lane`、`run`、`validate`
- `triage/config.rs`：`Settings`、`Exec`、`Var`；`triage/endpoint.rs`：HTTP 地址、可选密钥与代理、超时与本机／外部地址校验；`Location`（字面量或 `${NAME}`）也供 IMAP；代理只配给 HTTPS 决策地址，本地模型配代理即拒绝
- `imap.rs`：邮箱采集。`Run` 用 `race` 包住整轮 `Worker::session`，被打断即退出码 0；`Worker` 持 `Mutex<Archive>`（异步方法收 `&self` 时 future 须 `Send`）；`login`：`open` → 问候须 `* OK` → `CAPABILITY` →（有 `ID` 能力）`ID` → `LOGIN`；`follow`：`SELECT` 取 `UIDVALIDITY`、`UIDNEXT - 1` 为已见 → 按水位经 domain `ingest.since` 得日期 `UID SEARCH SINCE` → 只取最新 `search_max` 个、每批 `fetch_batch` 个 `UID FETCH (UID INTERNALDATE BODY.PEEK[]<0.message_bytes>)` → 循环：有 `IDLE` 能力则 `IDLE` 等 `EXISTS` 或 `idle_ms` 到期后 `DONE`，否则睡 `poll_ms`，再取 `<已见+1>:*` 并滤掉不大于已见的；每封交 domain `ingest.parse_mail`，`skip` 非空不入库，`save_event` 入库；失败原因 `imap:auth`（带 `[AUTHENTICATIONFAILED]` 等码的 NO，记 ERROR、`Retry::Stalled`）、`imap:<状态>`、`imap:bye`、`imap:eof`、`imap:greeting`、`imap:idle`、`imap:line`、`imap:literal`、`imap:credential`、`timeout:<阶段>`；写失败不立即报错，由随后的读取暴露；`check` 供启动自检（登录后 `LOGOUT`）
- `imap/wire.rs`：`Conn`（递增标签 `A<n>`、缓冲、读行上限 `line_bytes`、`{n}` 字面量上限 `message_bytes`、遇 `* BYE` 即失败）、`Reply::checked`、`quoted`（拒非 ASCII 与控制字符，转义 `\` 与 `"`）；`imap/config.rs`：`url` 为带 `${NAME}` 的 `imaps://` 或本机明文 `imap://127.0.0.1|[::1]`（明文不得配代理），字段不超过 16 个
- `net.rs`：`Dial`（主机、端口、`Layer::Plain|Tls`、代理、测试用根证书）→ `open` 得 `Link`（`read`、`write_all`）；`Wire` 包 tokio `TcpStream`，同步 `Read`/`Write` 内部用 `try_read`/`try_write` 并记下所需就绪方向，`WouldBlock` 时等 `ready(want)`，空闲不轮询；`SslStream<Wire>` 与 `Wire` 经 `Stream` trait 取回 `Wire`；`net/tunnel.rs`：HTTP `CONNECT`（逐字节读响应头到空行，上限 8 KiB，只认 2xx）与 SOCKS5（无认证、域名由代理解析，方法被拒 `proxy:method`、REP 非 0 `proxy:<码>`、主机超 255 字节 `proxy:host`）；`loopback`、`proxied`（`http://` 或 `socks5://` 加回环主机与端口原样）供各配置校验
- `http.rs`：异步 HTTP/1.1 JSON POST，连接经 `net::open`；证书按目标主机校验；不读环境代理变量、不跟随重定向、代理失败不回退直连；超时按所处阶段报 `timeout:connect|proxy|tls|send|read`；响应头无法解析时立即结束读取；支持 Content-Length、chunked 与连接关闭响应；错误不携带 URL、密钥或正文
- `triage/desk.rs`：调 domain `triage` feature；`Desk::rules` 为库里学到的规则在前、配置的在后；`Desk::prompt(name)` 每次读文件并经 domain `prompts.check` 校验，读不到记 warn `prompt unreadable`、不合规记 warn `prompt rejected`（只带名字与长度），都返回 `None`：`brief` 为 `None` 时该话题本轮不 claim、留在 `pending`；`pack` 为 `None` 时不封批、已封批的重打包失败；筛查为 `None` 时本轮取到的行全部 `deferred`
- `triage/prompts.rs`：`Folder`（`Settings::prompts()` 构造，目录为 `triage.prompts_dir`，名字为 `followup`、`session`、`topic`、`urgent`，配了 `screen` 再加 `screen`，文件名 `<名字>.md`）实现 domain `prompts::Library`；路径只由白名单名字拼出，不取自请求；`save` 先写同目录 `<名字>.md.tmp` 再 `rename`，rename 失败删掉临时文件；读写失败记 warn `prompt storage failed` 并返回 `Storage`
- `triage/intake.rs`：入库、成批、重新脱敏；`triage/screen.rs`：本地模型筛查；`triage/batch.rs`：封批、发送、落结果；`triage/alert.rs`：分析、通知；三者是以 `&Shift` 为参数的自由函数（`multiple_inherent_impl` 禁止给 `Shift` 再写 impl）
- 外发的话题选项：`OPEN_TOPICS` 带出话题最早一条已通过行的原文 `first` 与 `created_at_ms`，domain 按当前规则先脱敏再截断生成说明，存库的 label 只供页面；hints 收窄时保留 `created_at_ms` 晚于该段最早一行入库时间的话题，hints 全不匹配开放话题时不收窄
- `ledger.rs`：`lines`、`batches`、`answers`、`topics`、`placements`、`alerts`、`learned_rules`、`misses` 的读写；参数打成一个 JSON 对象经 `json_extract(?1, …)` 取值（绕开参数个数限制），读返回 `json_group_array`，失败只记 warn、计入 `Ledger::faults()` 并返回空或 0；多语句事务经 `committed`（`BEGIN IMMEDIATE` … `COMMIT`，任一步失败则 `ROLLBACK`）；事务返回的行数只累计带参数的步骤：`BEGIN`、`COMMIT` 的 `execute` 返回上一条写语句的行数，计入会让影响 0 行的认领被当成成功
- `pipe.rs`：`feed(Job)` 起子进程（args 里的 `${名}` 由 `Exec::args` 换成同名 vars 的值，stderr 置 null，stdin 写入后关闭，超时 `TimedOut`，`kill_on_drop`）；`probe(exec)` 供 `Settings::check`

## 采集流程

- 每轮 `session`：先起监听子进程（事件暂存在管道里），再补拉，再逐行入库；一轮结束后按退避等待再开下一轮
- 补拉：`window` 取区间（库内水位减重叠，无记录则回溯 `initial_backfill_ms`）→ `pull` → `parse_page` → `save_page`；窗口被拒、拉取失败、页面无法解析只记 warn，不阻断监听
- 密文：实时事件解析后，`Bridge::cipher` 带 `cipher_min_length` 调 domain `ingest.cipher` 判定；是密文则经 `Dws::decrypt` 解密，再用 domain `ingest.reveal` 换成明文入库；解密失败（含命令不存在、输出非 JSON、无 `plaintext`）按密文入库并记 warn；`INSERT OR IGNORE` 使补拉的明文不覆盖已入库的密文
- 补拉页的每条 envelope 在 `absorb` 里经 `unseal` 走同一套判定与解密
- 启动自愈：`drive` 在 `reconcile` 之后调 `heal`；`Archive::sealed` 用 `GLOB '*||[0-9]*||[0-9]*||[0-9]*'` 预筛库里最新的至多 `heal_limit` 条（是否密文仍由 domain 判定），逐条交给 `unsealed`，文本变了才经 `Archive::replace`（`WHERE text = 旧文本`，防并发覆盖）更新；解密失败保持原样；`SCHEMA` 的触发器 `messages_unsealed` 在 `messages.text` 更新后删掉同键 `skipped/sealed` 的 `lines` 行，让 intake 在 `lookback_ms` 内重新分级
- 转发消息：envelope 带 `lookup` 时，实时事件在 `unsealed` 后经 `assembled`、补拉在 `absorb` 里经 `assemble` 统一处理：收集 ID，按 `lookup_batch` 分批调 `Dws::lookup`，`dws::children` 取 `messages[].forwarded[]` 的 `{sender, text}`，再经 domain `ingest.assemble` 重组；查询失败、无子消息按原样入库
- 解密或查询期间收到停机信号：该事件或整页不入库，补拉会补回
- 退出：停机信号 → 0；`dws` 退出码落在 `manual_exit_codes` → `manual_exit_code`；存储错误 → `failure_exit_code`
- 退避：从 `backoff_initial_ms` 起逐次翻倍到 `backoff_max_ms`；钉钉曾收到消息的一轮结束后复位；IMAP 会话以 `Tally{received,rounds}` 计数，收到过邮件或完成过一轮等待（IDLE 正常结束或一次 `poll_ms` 睡眠）即复位，登录或 SELECT 后立刻断开仍递增
- IMAP 加密连接（security-framework）把对端重置当作正常关闭，读到 `Ok(0)`，故生产日志只有 `imap:eof`，区分不出重置与关闭；不为此改 `net.rs`
- 写库遇 `DatabaseBusy`、`DatabaseLocked`（`Halt::Busy`）记 warn `store busy`，本轮按失败退避，补拉补回；其余存储错误致命
- 解密并发：补拉页的 `unseal` 与 `heal` 经 `decrypted` 用 `buffered(dws.parallel)` 解密，保持顺序，整组外包一次 `race`，被打断整页或整组不入库；实时事件仍逐条（水位与补拉重叠的前提）；钉钉工具并发解密已用真实密文实测（1 至 16 路、跨令牌刷新点均无报错，锁文件不被改写；8 路起总耗时近线性增长），配置取 4

## 分级与转交

- `triage::run(setup, lane, stop)` 返回 `()`，永不使 serve 退出；每条 lane 各开一个账本连接，打开失败记 error 后返回。`mode: disabled` 只记录开关状态；首次启用把 `now - lookback_ms` 固化到 `triage_state`，重启与停用后再启用都不前移；`intake` 的实际下界取锚点与 `now - backlog_max_ms` 的较大者，被截掉时记一条 info `backlog older than backlog_max_ms left out`
- `Lane::Batches` 每轮：`intake` → `rerender` → `form` → `screen` → `seal` → `send` → 睡 `poll_ms`；`Lane::Alerts` 每轮：激活等待项 → 并行分析与通知 → 睡 `poll_ms`；本轮有故障（账本读写失败、`line unavailable`、`batch repack failed`，经 `Ledger::faults()` 与 `Shift::fault()` 计数）时 `Shift::pause` 改按 `Backoff`（从 `poll_ms` 翻倍到 `retry_max_ms`）等待并记 WARN `lane backing off`，一轮无故障即复位；两条 lane 并行，独立话题的外部调用用 `join_all` 并行，SQLite 读写与 claim 在调用前后串行；每个状态只有一个 lane 消费，不需认领中间态；`exec` 与 HTTP 经 `race` 打断，被打断返回 `None` 逐层 `?` 退出
- 先记账再调用：外部调用前 `claim_*` 把状态尝试与 `calls` 流水写在同一事务（`attempts + 1`、`updated_at_ms` 置当前，`WHERE state = 起点 AND attempts < max_attempts`），影响 0 行不调用；被停机打断则 `release_*` 删除流水并退回尝试。失败请求保留流水，`requests_since` 按 `calls` 计实际请求；失败只写 `reason` 并在 `attempts ≥ max_attempts` 时转 `failed`；`next_*` 跳过已用尽次数的
- `intake`：`Ledger::unlined` 取上述下界起还没有 `lines` 行的消息，从新到旧，每轮至多 `intake` 条；逐条调 domain `triage.line`（规则无效则记 warn、该条留待下一轮），写 `lines`（`INSERT OR IGNORE`，`queued` 或带原因 `skipped`，`rules_rev` 记当时最大的规则 id）
- `rerender`：把 `rules_rev` 落后的 `queued/pending/leaked/screening` 行经 domain `redact` 重新脱敏（先在审的，再从新到旧的 `queued`；已裁决的行不改写），`leaked` 行回到 `pending`
- `Settings::draw()` 给 domain `review::Draw`：开关关闭或 `review.mode: auto` 为 `off`，配了 `screen` 为 `screened(batch)`，否则 `reviewed(batch)`；`Settings::marks()` 收集脱敏规则、拒绝原因与筛查的占位符交 domain `redact::marks`
- `form`：只在 `review.mode: auto` 时执行，人工模式由 `PATCH /api/reviews` 经 `Shelf::draw` 取批；先经 `EXPIRE` 把发生时间早于 `now - lookback_ms` 的 `queued` 行置为 `skipped`（`reason = 'expired'`），有则记 info `lines expired`；没有未封批（有行 `batch_id > 0` 但 `batches` 里没有该批）时，把至多 `batch` 条 `queued` 行编入新批（批号＝最大批号 + 1）：从旧到新，同一时间按 `key` 升序；未配 `triage.screen` 时状态为 `approved`（审核人 `auto`），配了则为 `screening`
- `screen`（可选）：取至多 `batch` 条到期的 `screening` 行（`reason = 'deferred'` 的按 `lines.attempts` 退避），提示词与开放话题每轮取一次，每行各调一次 domain `triage.screen` 生成单行请求，经 `buffer_unordered(screen.parallel)` 并发发给本机模型；每完成一条外包一次 `race`，成功即交 domain `triage.screened` 并在一个事务里先写 `misses`（有采用的片段为 `rules`，`flag` 为 `unmasked` 即 `unmasked`，取遮盖前文字）再写回 `lines.text/flag/hints`：`manual` 转 `pending`，`auto` 时 `flag` 非空转 `rejected`（原因 `screen`）、其余转 `approved`；无法解析或结果为 `missing`（`bad_answer`，含输出被 `screen.max_tokens` 截断）的行立即由 `abandon` 以 `flag = unscreened` 落库（处置同 `unmasked`，样本 kind 为 `unscreened`），记 WARN `line screening abandoned`，不重试；其余失败（连接、状态、超时）的行本轮结束后一并 `DEFER`（`attempts + 1`），逐条记 WARN `line screening failed`，不放行，不设次数上限；被停机打断时已完成的行已落库，在途的各记一条 `stopped`，状态不改
- `seal`（`Ledger::sealable`）：批内无 `screening/pending/leaked` 行，且没有其他批处于 `ready`（或本批没有已通过行）时，取已通过的行，加上开放话题（`last_at_ms` 在 `session_max_ms` 内，至多 `topics_max_open` 个）和各会话仍开放的话题（`attach`），调 domain `triage.pack` 得冻结的请求体与 `layout`，写 `batches`（`ready`）；没有已通过的行则直接写 `sent`、`payload` 为空关批
- `send`：先自动重来至多一个 `failed` 批（`REVIVE`：失败超过 `retry_after_ms`、`0 ≤ revivals < max_revivals`、当前没有 `ready` 批），再按 `calls` 中实际决策请求数（`max_decisions_per_hour`）取最早的 `ready` 批；限额只阻止新请求，已存应答仍可重放；尝试上限为 `max_attempts × (revivals + 1)`，`attempts` 不归零，故 `calls` 主键 `(kind, subject, attempt)` 不冲突；`BATCH_SEAL` 写 `ready` 前同样要求没有 `ready` 批，防人工重发与封批并发
  - 没有已存应答时，每次尝试前用 `packed` 按当前规则、开放话题与 `attach` 重新打包；新 `payload`、`layout` 随 `BATCH_CLAIM` 在同一语句写回，该语句要求不存在 id 大于打包所用 rev 的 `learned_rules`，否则不 claim、下轮重打包；外发与判答都用新打包结果；“一次只外发一批”由 `SEALABLE` 保证，`BATCH_NEXT` 的退避条件会跳过等待中的批
  - URL 与 key 经 HTTP 配置从环境变量读取，请求体走 JSON POST，argv 不含密钥；响应交 domain `triage.sort`
  - 失败（连接、HTTP 状态、超时、响应无法解析）`reason` 为 `connect`、`proxy_connect`、`proxy:<码>`、`http:<码>`、`timeout:<阶段>`、`io`、`bad_answer` 或 URL 校验错误，达 `max_attempts` 转 `failed`
  - 成功先把应答写进 `answers`，写不进就不调用 `apply`；再经 `Ledger::apply` 一个事务写结果；`next_batch` 带出已存的应答，有则直接重放、不再外发，故 `apply` 失败不会重复调用决策接口
  - `apply` 事务：写 `placements`；写 `topics`（`ON CONFLICT` 取概率最大值与最近时间）；对 `urgent` 且含新行（决策时 `now - occurred_at_ms ≤ lookback_ms`）的话题开 `alerts`（键 `<话题>@<批号>`，初始 `waiting`，`score` 记本批触发分）；同话题已有未认领的项（`waiting` 或 `pending`、无分析、`attempts = 0`）时不新开，由 `MERGE_ALERT` 把它的 `created_at_ms` 刷新为当前、`score` 取较大值；只含超龄行的话题不转交；批置 `sent` 且 `attempts` 归零；崩溃发生在存应答前则整批重发
- `analyze`：取当前可转交的 `pending` 项（按 `calls` 检查小时额度与同话题冷却，`created_at_ms` 新的先出；未分析且话题 `last_at_ms` 早于 `now - session_max_ms` 的不出队、留在表里，`HELD` 也不计），认领时核对 `created_at_ms` 未变（构造提示词后被合并刷新的项下轮重来），话题的已通过行（`Ledger::topic_lines`）交 domain `triage.brief`
  - 话题 ID 与 Codex 会话 ID 相同。首次返回 `thread_id` 时，事务内迁移临时话题 ID 并级联 `placements/alerts/calls`，并在 `topic_aliases` 记下旧 ID（先把指向旧 ID 的别名改指新 ID，保持一跳）；`MIGRATE` 同样写别名；`PLACE`、`TOPICS`、`OPEN_ALERT` 与 `BATCH_LINES` 的 hints 经别名解析，旧 layout 与旧提示写回时落到新 ID；`thread_at_ms` 在 `session_max_ms` 内则续写：`followup.md` 模板、只含 `(until_ms, until_key)` 之后的行、命令追加 `session.resume`；否则新建：`session.md` 模板、追加 `session.fresh`
  - codex 的 stdin 传提示词，stdout 是 `--json` 事件流，经 domain `triage.reply` 取会话 ID 与最终回复；退出非 0、输出无法解析、回复为空都算失败，空输出的 `reason` 为 `exit:0`
  - 成功先把分析、会话 ID 与游标写库，再推进 `analyzed`；推进失败下轮重放已存分析，不重跑 Codex。失败 `attempts + 1` 并把 `thread_at_ms` 归零，让下次新建会话
- `deliver`：用 `triage.notice` 渲染（`{label}` 取 `topics.label`，`{probability}` 取 `alerts.score`，旧行无 `score` 时回落话题概率），dws 发送（参数末尾追加 `notify_target` 与 `--idempotency-key=<话题>@<批号>`）→ `sent`；失败达 `max_attempts` 转 `failed`；阶段推进（`ADVANCE_ALERT … WHERE state = from`）把 `attempts` 与 `updated_at_ms` 归零，故发送失败只重试发送
- 审核：`lines.state` 即审核状态，裁决由 cli 的 `PATCH /api/reviews/{key}` 经 domain `review.update` 调 `Shelf`；`rejected` 是终态，`leaked` 经 `rerender` 回到 `pending`；审核没有期限
- `validate(setup)`：先检查 `batch ≤ 200`、`topics_max_open + batch ≤ 254`（决策接口 `questions ≤ 400`、`choices ≤ 255`），再校验决策与本地模型 URL，再用空文本调 domain `redact`，规则无效即 `Err`，最后逐个 `Desk::prompt`，任一为 `None` 报 `triage.prompts_dir: <名字>: unusable`；cli 启动自检用它
- 外发内容（决策请求体、codex 提示词）只能来自 domain 渲染后的结果，adapters 不做文本替换
- 防自循环：domain 的 `line` 跳过本人发出的与通知会话的消息；本人与通知会话按消息的 `source:account` 取对应数据源的 `Profile`，取不到（无此概念的源）则不判定；通知以本人身份发出，补拉可能再收到，靠这两条切断
- 停机：在途子进程被打断并 kill，状态不改，下次启动续做（至少一次，发送靠幂等键去重）
- 排查（只读：`sqlite3 -readonly <store_path>`，WAL 下库所在目录须可写以建 `-shm`）：
  - `lines` 按 `state, reason` 计数，本人、通知会话、空正文、密文均为 `skipped`，自动模式下过期的为 `skipped/expired`
  - `batches` 由 `ready` 变 `sent`；`failed` 的批按 `max_revivals` 自动重来，`revivals = -1` 的旧批只能人工重发；`answers` 存决策应答
  - 已通过的行都有 `placements`
  - `alerts` 由 `waiting` → `pending` → `analyzed` → `sent`；冷却或限额时停在 `pending`，话题关闭后未分析的 `pending` 不再出队
  - 日志关键词：`lines expired`、`batch formed`、`batch sealed`、`batch decided`、`alert analyzed`、`alert sent`、`failed batch revived`，WARN `decision failed`（带 `batch_id`、`attempt`、`revivals`、`exhausted`、`retry_in_ms`）、`alert analysis failed`、`alert step failed`（带 `key`、`attempt`、`exhausted`、`retry_in_ms`）、`line screening failed`（带 `key`、`attempt`、`reason`、`retry_in_ms`）、`lane backing off`（带 `lane`、`attempt`、`faults`、`retry_in_ms`）；采集重连 `dingtalk session ended`、`imap session ended`（带 `account`、`attempt`、`retry_in_ms`，`attempt` 为 `Backoff::streak()`，收到过消息的一轮后回到 1）
  - 等待原因只在变化时记一条 info（`Shift::noted` 按主题记上次原因）：`decision quota reached`、`handoff held`（`reason` 为 `quota` 或 `cooldown`）、`batch waiting`（`state` 为 `review@<批号>` 或 `sending@<批号>`）、`backlog … left out`；`batch claim skipped` 表示打包后又学到规则、下轮重打包

## 新增数据源

- `ingest::Source` 加一个可选字段与 `checks`、`label`、`missing`、`profile`、`run` 各一处取值；`<kind>.rs` 与 `<kind>/` 放采集与配置；domain 新增该格式的解析 feature，产出与钉钉相同的 envelope JSON，`Archive` 不改
- 会话 ID 须自成命名空间，免得与其他源撞成同一段；没有本人、通知会话、密文概念的源 `Profile` 留空
- 补测试与假程序，同步 README 配置表与 docs/requirements.md

## 约定

- 账本与采集共库，多任务跨线程并发写：开 WAL，读写互不阻塞、写者之间串行；除 `apply` 与 `settle` 的 `BEGIN IMMEDIATE` 短事务外每次写单条语句，不跨 await 持事务；多语句写不得用 DEFERRED（WAL 下读升写直接 `BUSY_SNAPSHOT`，不等 `busy_timeout`）；`Archive::open` 与 `Ledger::open` 都经 `connect` 执行 `SCHEMA`（`IF NOT EXISTS`），`Shelf` 不建表
- 单写者（一个线程独占写连接、各任务经 channel 提交）暂不做；日志频繁出现 `store busy`，或某源要求 `synchronous = FULL` 且写入达每秒数百条时再评估
- 需确认语义的数据源（消息队列 ack、邮件已读标记）只在 `save_*` 返回 Ok 后确认，`Halt::Busy` 不确认；写库与确认之间不可被停机打断
- 不清账本表：清 `lines` 会重新外发；清 `alerts` 会让幂等键 `<话题>@<批号>` 复用，通知被 dws 当作重复吞掉；`learned_rules` 始终保留；`misses`（漏判样本，含已过规则脱敏的消息文字）不经接口与日志返回，保留期待定
- 日志只记行数、数量、标识、状态、次数、长度、退出码，不含消息正文、提示词、分析、响应、规则、地址、账号与密码。所有外部交互经 `trace.rs` 的 `external(action, request, &Outcome)` 记一条 debug：字段 `transport/session_id/user/feature/action/params/request/response/ok/elapsed_us/attempt/code/status`，request、response 只记 `redacted:<字节数>`，`ok` 即 `status == "success"`，status 只取 `success` 或固定原因串（不放服务器原文），字段值在宏外算好；来源经 `Caller::new(transport, feature, user).session(id)`：
  - triage：`external`/`triage`，action 为 `decide`、`screen`、`analyze`、`notify`
  - IMAP：`imap`/`imap`/account，`session_id` 为 `<account>#<进程内连接序号>`；action 为 `connect`（params `tls|plain+direct|http|socks5|socks5h`）、命令动词（`CAPABILITY`、`ID`、`LOGIN`、`SELECT`、`UID SEARCH`、`UID FETCH`、`LOGOUT`，`UID` 类 params 为参数，request、response 为本条命令的读写字节）、`IDLE`（status `exists` 为服务器推送、`refresh` 为 `idle_ms` 到期，`elapsed_us` 为整轮时长）、`mail`（params `<UIDVALIDITY>:<UID>`，status `success`/`skip:<原因>`/`rejected`）；`parse_mail` 的 dispatch 带同一 session_id；不记 Message-ID
  - dws：`dws`/`dingtalk`/account，action 为 `pull`（params 起止时间）、`decrypt`、`lookup`（`code` 为退出码，status 经 `finished`：`success`、`exit:<码>`、`io:<种类>`）、`listen`（params `start`、`stop`）；listen 的 stderr 改为管道，`next_line` 每次先取完已到的错误行，各记一条 info `dws stderr`（`account`、`line`），写出时由 cli 统一脱敏；被停机打断的调用不记
- HTTP 错误不输出 URL 或原始错误文本
- 子进程环境一律 `env_clear` 后只放配置的 env 与 vars；命令用 `PATH` 找程序时 `vars` 须含 `PATH`；dws 的 env 应含 `DO_NOT_TRACK=1` 关闭遥测
- `triage::config::Settings` 的字段不得超过 16 个：`derive(Debug)` 会生成 `&[&dyn Debug; N]`，N 为 17 时超过 256 字节触发 `large_stack_arrays`；新增配置并入 `session`、`review`、`redact` 等子结构
- 停机先丢子进程 stdin（dws 视 EOF 为优雅退出并取消订阅），超过 `child_exit_timeout_ms` 才 kill；kill -9 会泄漏服务端订阅（再启报已存在、重复投递）
- `Option` 的管道句柄用 `map`、`map_or`、`take` 处理，不写取不到的分支（覆盖率会缺 region）
- `race<T>` 是泛型，每个实例（读行、补拉、睡眠）都要有“被停机打断”与“正常完成”用例

## 沙箱策略

- 通用：
  - 一个进程只能有一层 Seatbelt，外层内再 `sandbox-exec` 更严格的策略或 `codex sandbox` 都退出 71；codex 的 `-s read-only` 现保留作纵深防御，若启动即 71 改 `-s danger-full-access`
  - `process-exec` 对符号链接无效，策略放行真实路径所在目录（如 `/opt/homebrew/Cellar/curl`、`/opt/homebrew/Caskroom/codex`）
  - 网络规则的主机只能是 `*` 或 `localhost`，只能限端口
  - 代理：`codex.sb`、`dws.sb` 见到参数 `PROXY` 时取其最后一个 `:` 后的端口，只放行 `localhost:<端口>`、不放行 `*:443`，未传则照旧直连 443；`PROXY` 为空或带结尾 `/` 时策略报 invalid port 退出 65
  - `-D` 值在 `ps` 可见，不传 secret
  - `sandbox-exec` 用 execvp，PID 与管道语义不变；失败码：策略或参数无效 65，被包装程序不存在 71
  - 规则不全的后果是命令失败，故每条新规则都要能在 `log stream` 里对上拒绝记录；拒绝记录里的进程名可能是 `sh`、`security` 而不是 `dws`
  - 策略不写注释
- `dws.sb`：
  - 读写 `~/.dws`、`~/Library/Application Support/dws-cli`（令牌）、`~/Library/Caches/dws`、`~/Library/Keychains`、`~/.local/bin/.dws-runtime.lock` 与 `.dws-runtime-manifest.json`
  - 监听时 dws 经 `/bin/sh`、`/bin/bash` 起事件总线子进程，在 `/tmp/dws-event-<uid>/` 建 0700 目录与 unix socket，须由 dws 自己建，预建 0755 的目录会令其报权限错
  - 读 Keychain 时反复起 `/usr/bin/security`（继承沙箱，需读自身与 `/usr/bin`、`se_SecurityMessages`、共享内存 `com.apple.AppleDatabaseChanged`）
  - 其余 mach 服务、CFPreferences、`NSIRD` 临时目录的拒绝记录不致命，不放行
- `codex.sb`：
  - TLS 用系统证书库校验，须放行 mach `com.apple.SecurityServer`；缺它表现为不停 `Reconnecting… error sending request` 而无网络拒绝记录
  - 启动时 canonicalize 相对的 `CODEX_HOME` 需读 `PROJ` 目录本身（`literal`，不放行其下文件）
  - 其余拒绝记录（偏好、FSEvents、process-fork、`/opt/homebrew/bin`）不致命，不放行
  - `-s`、`-C` 写在 `resume` 前对续写同样生效

## 测试

- 每个测试用独立的 `Sandbox` 目录（`CARGO_TARGET_TMPDIR`）；同一测试二进制不可并行跑多份（沙箱目录同名）；以 `sleep` 当停机信号，时长取大值避免慢机误判
- 含外部命令的用例（解密、查询）停机等待 `SHORT` 取 4 秒：负载高时 0.7、2.5 秒都不够
- 监听输入每行须以换行结尾（`feed`），否则 `next_line` 要到 EOF 才返回最后一行
- tracing 宏的字段表达式只在有 subscriber 时求值：经 `run_logged` 装 subscriber（`tests/it/capture.rs` 的 `Capture::dispatch`，先确保常驻 `ANCHOR`，原因见 domain/CLAUDE.md），否则字段里的表达式覆盖不到
- 假 dws `tests/it/fixtures/fake-dws.sh`，行为由钉钉源配置的 `dws.env` 控制：
  - listen 先往 stderr 打一行 `source state: connected`；`FAKE_LISTEN`（exit、hang，默认等 stdin 结束）、`FAKE_CODE`，输入取 `FAKE_DIR` 下的 `listen.txt`
  - `FAKE_PULL`（fail、slow，默认输出 `pull.json`），参数写入 `pull-args.txt`
  - `FAKE_LOOKUP`（fail、junk、slow，默认输出 `lookup.json`），参数逐行追加到 `lookup-args.txt`
  - `FAKE_DECRYPT`（fail、junk、slow，默认输出 `{"ok":true,"plaintext":"CLEARTEXT"}`），参数写入 `decrypt-args.txt` 并逐行追加到 `decrypt-calls.txt`（并发解密用例按行数断言）
  - `slow` 睡 30 秒：support 的 `dws.timeout_ms` 取 60000，让“停机打断”用例仍由停机信号结束；超时用例在 `tests/it/ingest/timeout.rs` 把 `timeout_ms` 改成 200
- 假 triage `tests/it/fixtures/fake-triage.sh` 只跑 codex 与 send，第一个参数为角色，行为由 `env` 控制；参数、stdin、调用次数写入 `<角色>-args.txt`、`-stdin.txt`、`-calls.txt`：
  - codex：`FAKE_CODEX`（fail、empty、junk、resume_fail、escape、slow）；默认输出 `thread.started`（会话 ID 取 `FAKE_THREAD`，默认 T1）与含 `ANALYSIS` 的 `agent_message` 两行 JSONL
  - send：`FAKE_SEND`（fail、slow）
- 决策与本地模型用 `tests/it/triage/stub.rs` 的本机动态端口 HTTP stub；每个 `Sandbox` 独立端口。请求写入 `<角色>-calls.txt`（JSONL：方法、路径、头）与 `<角色>-body.txt`；连接并发处理（`for_each_concurrent`）；行为由目录内 `decide-stub.json`、`screen-stub.json` 的 `mode`（fail、junk、slow）与概率、话题、置信度字段控制，`screen.json` 可整包覆盖响应；`only` 为子串，只有请求体含它的请求才套用 `mode` 与 `screen.json`（缺省空串即全部），用来让同一批里只有某一行失败或变慢；筛查每个请求只有一行，夹具里的别名一律 `L1`。补丁顶层 `stub.decide`、`stub.screen` 由 `rig::behave` 写成这些文件，不进生产配置
- 假 IMAP 在 `tests/it/imap/fake.rs`：std 线程按会话脚本 `[(命令前缀, 回包)]` 应答，回包里 `<tag>` 换成本条命令的标签、`\u{0}` 处暂停 150 ms；`DONE` 沿用 IDLE 的标签；命令不匹配或脚本用完即断开，结尾加永不匹配的步骤可让服务器一直挂着；收到的命令经 channel 供断言；凭据取 `.cargo/config.toml` 的 `POLY_RESOLVE_TEST_IMAP_*`；SOCKS5 用例在 `tests/it/http/socks.rs`
- 代理用例在 `tests/it/http/proxy.rs`：线程 stub 读到空行后按设定回包或经 `pipe` 双向转发；可用代理与格式不合规的代理分别取 `.cargo/config.toml` 的 `POLY_RESOLVE_TEST_PROXY`、`POLY_RESOLVE_TEST_BAD_PROXY`（测试不能 `set_var`）
- HTTPS 用例由 `openssl s_server -quiet` 从 stdin 回写固定响应（`-www` 不回应 POST）；`fixtures/tls` 为 P-256 自签叶证书，须 `CA:FALSE`、`extendedKeyUsage=serverAuth`、SAN `DNS:localhost`、有效期不超过 825 天，否则 Security Framework 拒绝：`openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -days 825 -nodes -subj /CN=localhost -addext subjectAltName=DNS:localhost -addext extendedKeyUsage=serverAuth -addext basicConstraints=critical,CA:FALSE -keyout key.pem -outform DER -out cert.der`
- 存储故障注入：
  - 预建缺列的表（`BAD_SCHEMA`，在 `heal` 的查询处失败）
  - 对已建表加触发器 `REFUSE_INSERT`、`REFUSE_UPDATE`（写失败而查询正常）
  - 水位列为 TEXT（`TEXT_STAMP`）
  - 列齐全但 `occurred_at_ms` 为 TEXT（`BAD_STAMP`，读取行解码失败；须同时建 `placements`、`alerts`，`Shelf` 连表）
  - 账本写失败靠对 `lines`、`alerts` 加 `RAISE(ABORT)` 触发器，读失败靠把 `batches` 重建成缺列表；认领失败靠 `BEFORE UPDATE OF attempts` 触发器
  - 忙锁：另一连接持 `BEGIN IMMEDIATE`，`busy_timeout_ms` 取 50（`tests/it/ingest/busy.rs`）
- 沙箱测试：
  - `fixtures/fake.sb` 只放行 `/bin`、`/usr/bin`、fixtures 与 `DIR`，显式禁 `/usr/bin/true` 作金丝雀，无网络，只读 `/private/etc/ssl` 让 curl 能走到连接；`open.sb`（`allow default`）作对照
  - 假脚本 `escape` 读写 `$FAKE_OUTSIDE`、`net`（dws pull）用 `/usr/bin/curl` 连 `$FAKE_PORT`，用例分别断言被拒与对照放行
  - `sandbox.rs` 覆盖 `check` 各分支，探测参数 `noop` 让假脚本退出 0
  - 想让单个命令起不来：改它自己的 `sandbox.bin`（io 错误）或 `bin`（71）
- 用例与 helper 分布：
  - `tests/it/support.rs`：`Sandbox`（`fence` 给每个命令配 sandbox，`outside()` 是沙箱目录外含 `secret.txt` 的目录；`config(patch)` 把 `SHARED` 里的键放共享项、其余放唯一的钉钉源，`shared(sources)`、`source(patch)` 供自拼数据源列表；`triage(patch)` 生成 triage 配置，默认 `review.mode: auto`）、`run_logged`（跑第一个数据源）、`triage_logged`（同一 future 里 `join` 两条 lane）、`triage_validate`
  - `tests/it/feeds.rs`：dws 监听行与补拉页的构造（`event`、`page`、`sealed_page` 等）
  - `tests/it/sources.rs`：数据源列表的校验、缺失变量、无类型项
  - `tests/it/triage.rs` 及 `tests/it/triage/` 下：`batch.rs`（成批、掩码、逐条优先级、回溯）、`review.rs`（manual 全流程、漏脱敏学规则后重审、拒绝关批）、`topic.rs`（并入已有话题、同会话续接、长停顿分段、低置信度、合并本批的段）、`thread.rs`（会话绑定、续写、超龄、续写失败、输出无法解析）、`screen.rs`（本地模型筛查）、`claim.rs`（先记账再调用、两条 lane 互不阻塞）；`rig.rs` 放共用的播种（`seed` 等）与读取（`calls`、`decided`、`logged` 等）
  - `tests/it/review.rs` 测 `Shelf` 的 `Docket`，取批在 `review/draw.rs`；`triage/policy.rs` 测 `draw()` 与 `marks()`；人工模式的 triage 用例先 `start` 入队，再用 rig 的 `draw` 取批
  - `tests/it/imap.rs` 及 `tests/it/imap/`：`settings(sandbox, url, patch)` 生成只有一个邮箱源的配置，`whole` 另可改共享项；`failure.rs`（协议与存储失败、停机）、`target.rs`（启动自检与地址校验）
  - 话题 ID 在测试里是 `t<批号>-<段序号>`；同一库跨多次 `start` 运行时用 `seed_from`、`seed_stamped` 给后来的消息更晚的时间

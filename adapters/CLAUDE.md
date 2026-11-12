# adapters

## 职责

- I/O：dws 子进程、SQLite 存储、收集与分级编排；可用 tokio；依赖 domain，被 cli 依赖
- 不写业务规则：解析、去重键、时间换算、脱敏都在 domain，经 `dispatch` 调用；业务字符串与数值取自配置

## 模块

- `config.rs`：`Settings`（cli 配置的 `ingest` 段，`deny_unknown_fields`）：共享项与 `sources`；`verify`（非空、无无类型项、标签不重复，cli 加载时调用）、`check`（各源自检并发）、`missing`、`profile(标签)`、`prepare`（建库建表开 WAL，cli 起任务前同步调一次，免得多连接同时首次切 WAL 互锁成 `database is locked`）
- `ingest.rs`：`Source`（每个可选字段一种类型：`dingtalk`、`imap`；不用枚举，因引用上 match 枚举撞 `pattern_type_mismatch`，取值一律 `Option` 组合子）、`Site`（`Settings` 与 `&Context` 打包）、`Profile`（本人地址列表、通知会话、密文长度，均可空，供 triage 按消息来源判定）、`label(来源, 账号)`、`run(site, source, stop)` 按类型分派
- `store.rs`：
  - `connect(path, busy_ms)`：`busy_timeout`、`journal_mode = WAL`、`synchronous = NORMAL` 与 `SCHEMA`，`Archive::open`、`Ledger::open`、`Settings::prepare` 共用；`SCHEMA` 每次连接都执行：建表建索引（`lines_by_conversation` 供未外发成员找相邻行），先删旧触发器 `messages_unsealed`、`messages_revealed` 再建 `messages_decrypted`；旧库缺列由 `ensure(conn, Column)` 按静态表 `COLUMNS` 补（放栈上的数组超 256 字节撞 `large_stack_arrays`）：`lines.attempts`；`lines.role`；`lines.digest`；`alerts.score`；`alerts.changes`；`topics.until_key`；`topics.state` 一项连带补 `urgency`、`reach`、`score`、`verdict`、`closed_at_ms`；`batches.revivals`，补列时把已有 `failed` 批的 `revivals` 置 -1，表示不自动重来、只等人工重发）
  - `Shelf`：`inbox::Reader`、`outbound::Outbox` 与 `batches::Roster` 的 SQLite 实现，`Settings::shelf()` 构造；每次调用才打开（删库造 503 的用例依赖它，不改成长连接）并设 `busy_timeout`，`page`、两个 `list` 只读（`SQLITE_OPEN_READ_ONLY`），`revive` 读写，都不建库不建表；`Outbox::list` 按状态取 `lines`（连 `messages` 取来源与账号，没有对应消息为空串），发生时间从新到旧；`revive` 只把 `failed` 且当前没有 `ready` 批的批置回 `ready`，`revivals` 取 `MAX(revivals, 0) + 1`，返回该批当前状态；`Outbox` 与 `Roster` 的 `list` 同名同参，调用处只导入其一
  - `page`：单条 SQL 用 `json_group_array(json_object(…))` 取回再反序列化成 `Row`（连 `members`、`topics`、`lines` 填优先级、话题、处理状态与原因；一条消息属于多个话题时取话题 ID 最小的一个，优先级取话题 `score`，没有则取该成员的概率），不按来源过滤，行值比较 `(occurred_at_ms, key) < (游标)` 翻页，叠加 `Query` 的时间范围 `[since, until)`（走 `messages_by_when` 索引）；失败记 warn 并返回 `AppError::Storage`
  - `Archive::backfilled(scope, version)`、`mark_backfilled(scope, version)`：`backfills` 表按来源、账号、版本记一次性重拉是否做完（`INSERT OR IGNORE`），IMAP 用它补回旧版本不收的自动邮件
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
- `triage/config.rs`：`Settings`、`Exec`、`Var`；`triage/config/topics.rs`：`Topics`、`Clues`、`Reach`、`Notice` 等话题与通知的子结构（单文件不超过 512 行）；`triage/endpoint.rs`：HTTP 地址、可选密钥与代理、超时与本机／外部地址校验；`Location`（字面量或 `${NAME}`）也供 IMAP；代理只配给 HTTPS 决策地址，本地模型配代理即拒绝
- `imap.rs`：邮箱采集。`Run` 用 `race` 包住整轮 `Worker::session`，被打断即退出码 0；`Worker` 持 `Mutex<Archive>`（异步方法收 `&self` 时 future 须 `Send`）；`login`：`open` → 问候须 `* OK` → `CAPABILITY` →（有 `ID` 能力）`ID` → `LOGIN`；`follow`：`SELECT`（文件夹名经 `utf7::encode` 转 modified UTF-7 再 `escaped`）取 `UIDVALIDITY`、`UIDNEXT - 1` 为已见 → 按水位（`backfills` 里没有本账号 `BACKFILL` 版本的记录时不看水位、按 `initial_backfill_ms` 重拉，首轮补拉完成后 `caught_up` 记下）经 domain `ingest.since` 得日期 `UID SEARCH SINCE` → 只取最新 `search_max` 个、每批 `fetch_batch` 个 `UID FETCH (UID INTERNALDATE BODY.PEEK[]<0.message_bytes>)` → 循环：有 `IDLE` 能力则 `IDLE` 等 `EXISTS` 或 `idle_ms` 到期后 `DONE`，否则睡 `poll_ms`，再取 `<已见+1>:*` 并滤掉不大于已见的；每封交 domain `ingest.parse_mail`，`skip` 非空（仅 DMARC 未通过）不入库，`save_event` 入库；失败原因 `imap:auth`（带 `[AUTHENTICATIONFAILED]` 等码的 NO，记 ERROR、`Retry::Stalled`）、`imap:<状态>`、`imap:bye`、`imap:eof`、`imap:greeting`、`imap:idle`、`imap:line`、`imap:literal`、`imap:credential`、`timeout:<阶段>`；写失败不立即报错，由随后的读取暴露；`check` 供启动自检（登录后 `LOGOUT`）
- `imap/wire.rs`：`Conn`（递增标签 `A<n>`、缓冲、读行上限 `line_bytes`、`{n}` 字面量上限 `message_bytes`、遇 `* BYE` 即失败）、`Reply::checked`、`quoted`（拒非 ASCII 与控制字符，供用户名与密码）与 `escaped`（只转义 `\` 与 `"`，供已编码的文件夹名）；`imap/utf7.rs`：`encode` 把可打印 ASCII 以外的字符按 UTF-16BE 的 modified base64 包进 `&…-`，`&` 写作 `&-`；`imap/config.rs`：`url` 为带 `${NAME}` 的 `imaps://` 或本机明文 `imap://127.0.0.1|[::1]`（明文不得配代理），字段不超过 16 个
- `net.rs`：`Dial`（主机、端口、`Layer::Plain|Tls`、代理、测试用根证书）→ `open` 得 `Link`（`read`、`write_all`）；`Wire` 包 tokio `TcpStream`，同步 `Read`/`Write` 内部用 `try_read`/`try_write` 并记下所需就绪方向，`WouldBlock` 时等 `ready(want)`，空闲不轮询；`SslStream<Wire>` 与 `Wire` 经 `Stream` trait 取回 `Wire`；`net/tunnel.rs`：HTTP `CONNECT`（逐字节读响应头到空行，上限 8 KiB，只认 2xx）与 SOCKS5（无认证、域名由代理解析，方法被拒 `proxy:method`、REP 非 0 `proxy:<码>`、主机超 255 字节 `proxy:host`）；`loopback`、`proxied`（`http://` 或 `socks5://` 加回环主机与端口原样）供各配置校验
- `http.rs`：异步 HTTP/1.1 JSON POST，连接经 `net::open`；证书按目标主机校验；不读环境代理变量、不跟随重定向、代理失败不回退直连；超时按所处阶段报 `timeout:connect|proxy|tls|send|read`；响应头无法解析时立即结束读取；支持 Content-Length、chunked 与连接关闭响应；错误不携带 URL、密钥或正文
- `triage/desk.rs`：调 domain `triage` feature；`Desk::new` 读 `redact.hash_key` 的环境变量造 `Seal`（读不到为 `None`），每次 `ask` 用 `Context::with_seal` 附上；`Desk::rules` 为库里已有的 `learned_rules`（旧版本人工审核学到的，不再新增）在前、配置的在后；`Desk::prompt(name)` 每次读文件并经 domain `prompts.check` 校验，读不到记 warn `prompt unreadable`、不合规记 warn `prompt rejected`（只带名字与长度），都返回 `None`：`brief` 为 `None` 时该话题本轮不 claim、留在 `pending`；`pack` 为 `None` 时不封批、已封批的重打包失败；筛查为 `None` 时本轮取到的行全部 `deferred`
- `triage/prompts.rs`：`Folder`（`Settings::prompts()` 构造，目录为 `triage.prompts_dir`，名字为 `followup`、`session`、`topic`、`urgent`，配了 `screen` 再加 `screen`，文件名 `<名字>.md`）实现 domain `prompts::Library`；路径只由白名单名字拼出，不取自请求；`save` 先写同目录 `<名字>.md.tmp` 再 `rename`，rename 失败删掉临时文件；读写失败记 warn `prompt storage failed` 并返回 `Storage`
- `triage/intake.rs`：入库、成批、重新脱敏；`triage/screen.rs`：本地模型筛查；`triage/batch.rs`：封批、发送、落结果；`triage/alert.rs`：分析、通知；三者是以 `&Shift` 为参数的自由函数（`multiple_inherent_impl` 禁止给 `Shift` 再写 impl）
- 外发的话题选项：`OPEN_TOPICS` 带出话题最早一条已通过行的原文 `first` 与 `created_at_ms`，domain 按当前规则先脱敏再截断生成说明，存库的 label 只供页面；hints 收窄时保留 `created_at_ms` 晚于该段最早一行入库时间的话题，hints 全不匹配开放话题时不收窄
- `ledger/sql.rs`：账本的全部 SQL 常量，`ledger.rs` 只放方法并以 `sql::NAME` 引用（两文件各不超过 512 行）
- `ledger.rs`：`lines`、`batches`、`answers`、`topics`、`members`、`clues`、`relations`、`identities`、`alerts`、`learned_rules`（只读）、`misses` 的读写（旧表 `placements` 只在迁移时读）；参数打成一个 JSON 对象经 `json_extract(?1, …)` 取值（绕开参数个数限制），读返回 `json_group_array`，失败只记 warn、计入 `Ledger::faults()` 并返回空或 0；多语句事务经 `committed`（`BEGIN IMMEDIATE` … `COMMIT`，任一步失败则 `ROLLBACK`）；事务返回的行数只累计带参数的步骤：`BEGIN`、`COMMIT` 的 `execute` 返回上一条写语句的行数，计入会让影响 0 行的认领被当成成功
- `pipe.rs`：`feed(Job)` 起子进程（args 里的 `${名}` 由 `Exec::args` 换成同名 vars 的值，stderr 置 null，stdin 写入后关闭，超时 `TimedOut`，`kill_on_drop`）；`probe(exec)` 供 `Settings::check`

## 采集流程

- 每轮 `session`：先起监听子进程（事件暂存在管道里），再补拉，再逐行入库；一轮结束后按退避等待再开下一轮
- 补拉：`window` 取区间（库内水位减重叠，无记录则回溯 `initial_backfill_ms`）→ `pull` → `parse_page` → `save_page`；窗口被拒、拉取失败、页面无法解析只记 warn，不阻断监听
- 密文：实时事件解析后，`Bridge::cipher` 带 `cipher_min_length` 调 domain `ingest.cipher` 判定；是密文则经 `Dws::decrypt` 解密，再用 domain `ingest.reveal` 换成明文入库；解密失败（含命令不存在、输出非 JSON、无 `plaintext`）按密文入库并记 warn；`INSERT OR IGNORE` 使补拉的明文不覆盖已入库的密文
- 补拉页的每条 envelope 在 `absorb` 里经 `unseal` 走同一套判定与解密
- 启动自愈：`drive` 在 `reconcile` 之后调 `heal`；`Archive::sealed` 用 `GLOB '*||[0-9]*||[0-9]*||[0-9]*'` 预筛库里最新的至多 `heal_limit` 条（是否密文仍由 domain 判定），逐条交给 `unsealed`，文本变了才经 `Archive::replace`（`WHERE text = 旧文本`，防并发覆盖）更新；解密失败保持原样；`SCHEMA` 的触发器 `messages_decrypted` 在 `messages.text` 更新后删掉同键 `withheld/sealed` 的 `lines` 行及其 `members`、`clues`、旧 `placements`，让 intake 重新分级、重建线索
- 转发消息：envelope 带 `lookup` 时，实时事件在 `unsealed` 后经 `assembled`、补拉在 `absorb` 里经 `assemble` 统一处理：收集 ID，按 `lookup_batch` 分批调 `Dws::lookup`，`dws::children` 取 `messages[].forwarded[]` 的 `{sender, text}`，再经 domain `ingest.assemble` 重组；查询失败、无子消息按原样入库
- 解密或查询期间收到停机信号：该事件或整页不入库，补拉会补回
- 退出：停机信号 → 0；`dws` 退出码落在 `manual_exit_codes` → `manual_exit_code`；存储错误 → `failure_exit_code`
- 退避：从 `backoff_initial_ms` 起逐次翻倍到 `backoff_max_ms`；钉钉曾收到消息的一轮结束后复位；IMAP 会话以 `Tally{received,rounds}` 计数，收到过邮件或完成过一轮等待（IDLE 正常结束或一次 `poll_ms` 睡眠）即复位，登录或 SELECT 后立刻断开仍递增
- IMAP 加密连接（security-framework）把对端重置当作正常关闭，读到 `Ok(0)`，故生产日志只有 `imap:eof`，区分不出重置与关闭；不为此改 `net.rs`
- 写库遇 `DatabaseBusy`、`DatabaseLocked`（`Halt::Busy`）记 warn `store busy`，本轮按失败退避，补拉补回；其余存储错误致命
- 解密并发：补拉页的 `unseal` 与 `heal` 经 `decrypted` 用 `buffered(dws.parallel)` 解密，保持顺序，整组外包一次 `race`，被打断整页或整组不入库；实时事件仍逐条（水位与补拉重叠的前提）；钉钉工具并发解密已用真实密文实测（1 至 16 路、跨令牌刷新点均无报错，锁文件不被改写；8 路起总耗时近线性增长），配置取 4

## 分级与转交

- `triage::run(setup, lane, stop)` 返回 `()`，永不使 serve 退出；每条 lane 各开一个账本连接，打开失败记 error 后返回。`mode: disabled` 只记录开关状态；首次启用把 `now - lookback_ms` 固化到 `triage_state`，重启与停用后再启用都不前移；`intake` 的下界就是这个锚点，不设积压上限；`Ledger::open` 每次执行幂等的 `MIGRATE`：话题改名之外，删掉旧版本 `skipped/own`、`skipped/expired` 的行让 intake 重收，`pending`、`leaked` 回到 `queued`，`skipped/sealed|empty` 与 `rejected/screen` 改成 `withheld`；旧 `placements` 里还没有任何成员记录的键抄进 `members`（`via = legacy`），没有 `score` 的话题以 `probability` 充当 `urgency` 与 `score`；旧邮件的 `messages.conversation_id` 按 `raw` 的回复关系改成 `mail:<账号>:thread:<根 id>`（`raw` 不是 JSON 的不动），`lines` 跟着改
- `Lane::Batches` 每轮：`intake` → `index` → `rerender` → `form` → `screen` → `seal` → 归档 → `send` → `regroup` → 睡 `poll_ms`；`Lane::Alerts` 每轮：激活等待项 → `recap` → 并行分析与通知 → 睡 `poll_ms`；本轮有故障（账本读写失败、`line unavailable`、`batch repack failed`，经 `Ledger::faults()` 与 `Shift::fault()` 计数）时 `Shift::pause` 改按 `Backoff`（从 `poll_ms` 翻倍到 `retry_max_ms`）等待并记 WARN `lane backing off`，一轮无故障即复位；两条 lane 并行，独立话题的外部调用用 `join_all` 并行，SQLite 读写与 claim 在调用前后串行；每个状态只有一个 lane 消费，不需认领中间态；`exec` 与 HTTP 经 `race` 打断，被打断返回 `None` 逐层 `?` 退出
- 先记账再调用：外部调用前 `claim_*` 把状态尝试与 `calls` 流水写在同一事务（`attempts + 1`、`updated_at_ms` 置当前，`WHERE state = 起点 AND attempts < max_attempts`），影响 0 行不调用；被停机打断则 `release_*` 删除流水并退回尝试。失败请求保留流水，`requests_since` 按 `calls` 计实际请求；失败只写 `reason` 并在 `attempts ≥ max_attempts` 时转 `failed`；`next_*` 跳过已用尽次数的
- `intake`：`Ledger::unlined` 取锚点起还没有 `lines` 行的消息（带出 `raw` 里的 `bulk`，`raw` 不是 JSON 时为空），从新到旧，每轮至多 `intake` 条；逐条调 domain `triage.line`（规则无效则记 warn、该条留待下一轮），写 `lines`（`INSERT OR IGNORE`，状态取 domain 给的 `queued`、`withheld`（原因 `empty`、`sealed`）或 `skipped`（原因 `notice`），`role` 为 `own`、`auto` 或空，`digest` 为 domain 给的正文摘要，`rules_rev` 记当时最大的规则 id）
- `index`：`Ledger::unindexed` 取还没有 `clues(kind = 'seen')` 标记的非 `skipped` 行（含升级前的旧行），每轮至多 `intake` 条，原文与 `raw`（非 JSON 为 null）交 domain `triage.clues`，线索、`seen` 标记与该行的 `digest`（非空时为 `digest` 线索，转发到别处的同一内容由此归入同一话题）一条语句写进 `clues`；正则无效记 warn `clues unavailable` 并计故障；线索取自 `messages` 原文，只留本机
- `identify`：`Ledger::strangers` 在有消息的发送人不在 `identities` 时返回全部发送人（每人取最新一条消息的名字），交 domain `triage.people` 后整表 `INSERT OR REPLACE`（记 info `senders identified`）；新发送人与已有同名对撞名时，原先的对应会被拆开
- 归档：每轮把 `last_at_ms` 早于 `now - topics.archive_after_ms` 的话题置 `archived`（记 info `idle topics archived`）；归档话题不进 `OPEN_TOPICS`，只会经强线索（`linked`）得到新成员并在 `apply` 里重开；重开后若最后一条消息仍超龄，下一轮又被归档
- `rerender`：把 `rules_rev` 落后的 `queued`、`screening` 行经 domain `redact` 重新脱敏（只在库里有比行更新的 `learned_rules` 时发生）
- `Settings::marks()` 收集脱敏规则与筛查的占位符交 domain `redact::marks`
- `form`：没有未封批（有行 `batch_id > 0` 但 `batches` 里没有该批）时，把至多 `batch` 条 `queued` 行编入新批（批号＝最大批号 + 1）：从旧到新，同一时间按 `key` 升序；未配 `triage.screen` 时状态为 `approved`（`reviewer` 记 `auto`），配了则为 `screening`
- `screen`（可选）：取至多 `batch` 条到期的 `screening` 行（`reason = 'deferred'` 的按 `lines.attempts` 退避），提示词与开放话题每轮取一次，每行各调一次 domain `triage.screen` 生成单行请求，经 `buffer_unordered(screen.parallel)` 并发发给本机模型；每完成一条外包一次 `race`，成功即交 domain `triage.screened` 并在一个事务里先写 `misses`（有采用的片段为 `rules`，`flag` 为 `unmasked` 即 `unmasked`，取遮盖前文字）再写回 `lines.text/flag/hints`：`flag` 非空转 `withheld`（`unmasked` 的原因 `risky`，`unscreened` 的原因 `unscreened`）、其余转 `approved`；片段最短长度取 `screen.min_chars`；无法解析或结果为 `missing`（`bad_answer`，含输出被 `screen.max_tokens` 截断）的行立即由 `abandon` 以 `flag = unscreened` 落库（处置同 `unmasked`，样本 kind 为 `unscreened`），记 WARN `line screening abandoned`，不重试；其余失败（连接、状态、超时）的行本轮结束后一并 `DEFER`（`attempts + 1`），逐条记 WARN `line screening failed`，不放行，不设次数上限；被停机打断时已完成的行已落库，在途的各记一条 `stopped`，状态不改
- `seal`（`Ledger::sealable`）：批内无 `screening` 行，且没有其他批处于 `ready`（或本批没有已通过行）时，取已通过的行，加上未归档的话题（`OPEN_TOPICS`，最近有消息的在前，至多 `topics.max_open` 个）、各会话最近一条成员所在的话题（`attach`，domain 再按 `session_gap_ms` 判是否相邻）和强线索命中的话题（`Ledger::linked`：本批的行与别的成员有相同 `kind, value` 线索），调 domain `triage.pack` 得冻结的请求体与 `layout`，写 `batches`（`ready`）；没有已通过的行则直接写 `sent`、`payload` 为空关批
- `send`：先自动重来至多一个 `failed` 批（`REVIVE`：失败超过 `retry_after_ms`、`0 ≤ revivals < max_revivals`、当前没有 `ready` 批），再取最早的 `ready` 批，不设请求限额；已存应答直接重放；重新打包后没有任何问题的批（如只有本人消息的新会话）不外发，直接按空应答落结果；尝试上限为 `max_attempts × (revivals + 1)`，`attempts` 不归零，故 `calls` 主键 `(kind, subject, attempt)` 不冲突；`BATCH_SEAL` 写 `ready` 前同样要求没有 `ready` 批，防人工重发与封批并发
  - 没有已存应答时，每次尝试前用 `packed` 按当前规则、开放话题与 `attach` 重新打包；新 `payload`、`layout` 随 `BATCH_CLAIM` 在同一语句写回，该语句要求不存在 id 大于打包所用 rev 的 `learned_rules`，否则不 claim、下轮重打包；外发与判答都用新打包结果；“一次只外发一批”由 `SEALABLE` 保证，`BATCH_NEXT` 的退避条件会跳过等待中的批
  - URL 与 key 经 HTTP 配置从环境变量读取，请求体走 JSON POST，argv 不含密钥；响应交 domain `triage.sort`
  - 失败（连接、HTTP 状态、超时、响应无法解析）`reason` 为 `connect`、`proxy_connect`、`proxy:<码>`、`http:<码>`、`timeout:<阶段>`、`io`、`bad_answer` 或 URL 校验错误，达 `max_attempts` 转 `failed`
  - 成功先把应答写进 `answers`，写不进就不调用 `apply`；再经 `Ledger::apply` 一个事务写结果；`next_batch` 带出已存的应答，有则直接重放、不再外发，故 `apply` 失败不会重复调用决策接口
  - `apply` 事务：写 `members`（主键 `key, topic_id`，一行可属于多个话题；`via` 记归入方式，`created_at_ms` 记本次落结果的时间，后面几步靠它认出本次动过的话题；domain 给的话题为空的行不写，由 `UNRELATED` 置为 `unrelated/no_topic`）；`PLACE_WITHHELD` 把还没有归属的 `withheld` 行并入同会话、`session_gap_ms` 内时间最近的已归属行所在的全部话题（不限本批，故晚到的未外发行在下一次落结果时归入）；`RELATE` 累加 `relations`（`a` 新话题、`b` 可能相关的话题、`kind = maybe`）；写 `topics`（`ON CONFLICT` 取概率最大值与最近时间）；`REOPEN` 把本次得到成员的非 `open` 话题改回 `open`；`REACH` 重算紧急度（最新一条有概率的成员）与影响面（会话数、来源类型数、去掉本人、自动与未外发行后的人数；人按 `identities.person_id` 计，没有则按 `来源:发送人`；话题里 `digest` 相同的行只有最早一条计入会话数与人数），`RESCORE` 按 `topics.reach` 权重算 `score`；对含新行（决策时 `now - occurred_at_ms ≤ lookback_ms` 且不是自动邮件）的话题，`score` 不低于阈值或已有 codex 会话（`thread_id` 非空）才开 `alerts`（键 `<话题>@<批号>`，初始 `waiting`，`score` 记当时的话题分）；同话题已有未认领的项（`waiting` 或 `pending`、无分析、`attempts = 0`）时不新开，由 `MERGE_ALERT` 把它的 `created_at_ms` 刷新为当前、`score` 取较大值；只含超龄行的话题不转交；批置 `sent` 且 `attempts` 归零；崩溃发生在存应答前则整批重发
- `analyze`：取 `pending` 项（不设额度与冷却，话题 `score` 高的先出），认领时核对 `created_at_ms` 未变（构造提示词后被合并刷新的项下轮重来），话题的已通过行与未外发行（`Ledger::topic_lines`，未外发行只带原因 `omitted`、不带文字与发送人，各行带 `role` 与来源类型 `source`）交 domain `triage.brief`
  - 话题 ID 与 Codex 会话 ID 相同。首次返回 `thread_id` 时，事务内迁移临时话题 ID 并级联 `members/alerts/calls`（`relations` 不改，读时经别名解析），并在 `topic_aliases` 记下旧 ID（先把指向旧 ID 的别名改指新 ID，保持一跳）；`MIGRATE` 同样写别名；`PLACE`、`TOPICS`、`OPEN_ALERT` 与 `BATCH_LINES` 的 hints 经别名解析，旧 layout 与旧提示写回时落到新 ID；`thread_id` 非空且 `thread_at_ms > 0` 则续写：`followup.md` 模板、命令追加 `session.resume`；否则新建：`session.md` 模板、追加 `session.fresh`；两种都给话题的全部行
  - codex 的 stdin 传提示词，stdout 是 `--json` 事件流，经 domain `triage.reply`（带上话题当前 `state`）取会话 ID、状态、标题、变化要点与正文；退出非 0、输出无法解析、回复首行不是合规 JSON 都算失败，`reason` 为 `exit:0`
  - 成功先在一个事务里写分析与变化要点（`alerts.analysis/changes`）、会话 ID、话题的 `state`、`closed_at_ms`、标题（非空才改 `label`）与 `verdict`，再推进：结论没变且状态没变（domain 给 `quiet`）转 `quiet`、不通知，否则转 `analyzed`；推进失败下轮重放已存分析（一律转 `analyzed`），不重跑 Codex。失败 `attempts + 1` 并把 `thread_at_ms` 归零，让下次新建会话
- `deliver`：用 `triage.notice` 渲染（标题、状态、`score`、紧急度、影响面取话题当前值，旧话题无 `score` 时回落 `alerts.score` 与话题概率；话题最后一条消息早于 `alerts.created_at_ms` 超过 `notice.late_after_ms` 时带 `late_ms`），dws 发送（参数末尾追加 `notify_target` 与 `--idempotency-key=<话题>@<批号>`）→ `sent`；失败达 `max_attempts` 转 `failed`；阶段推进（`ADVANCE_ALERT … WHERE state = from`）把 `attempts` 与 `updated_at_ms` 归零，故发送失败只重试发送
- 通知合并：`gate` 带 `cooldown_ms`，同一话题上次 `sent` 距今不足 `limits.notice_cooldown_ms` 的 `analyzed` 项先不发；`Ledger::supersede_notices` 把同话题较旧的待发结论置 `merged`（`merged_into` 记话题），只发最新一条；分析落库时 `Ledger::superseded`（话题有 `pending` 的变更，或已有更新的待分析项）为真则本次结论转 `quiet`；lane 第一轮（`ticks == 0`）`recap`：待发结论多于一个话题时经 domain `triage.recap` 合成一条汇总发出，各项随之 `sent`
- 自动调整（`triage/regroup.rs`，Batches lane 在 `send` 之后）：`Ledger::drift` 取候选（`clue` 两个话题的成员有共同强线索、`vote` 复核判同达 `topics.merge_after` 次、`apart` 复核判不属于达 `topics.split_after` 次；涉及的话题已有 `pending` 变更的不取）→ domain `triage.regroup` 定为 merge、move、split → `Ledger::propose` 写 `changes(pending)` → 逐条 `Ledger::settle`：一个事务（`sql::SETTLE` 按序执行），话题有在途分析（`alerts.pending`）时 `CHANGE_CLAIM` 不认领、留到下轮；merge 保留较早的话题，较晚的写 `topic_aliases`（保持一跳）并删除，其未发的提醒置 `merged`；split 的新话题 ID 为 `s<变更号>`、不继承会话；受影响的话题重算 `reach`、`score` 并开一条 `origin` 为变更类型的提醒（键 `<话题>@c<变更号>`）
- 复核问题：封批时 `Ledger::reviews` 取至多 `topics.review_max` 道（先是 `relations.maybe` 未满 `merge_after` 的话题对，再是经 `choice` 归入、还没被判 `kept` 的会话首行与其话题），随 `pack` 外发；`apply` 里 `VOTE` 按 `questions.review.threshold` 记 `maybe`／`unlike` 或 `kept`／`apart` 计数，`UNLIKE` 把判不同达 `merge_after` 次的话题对删掉、不再复核
- 话题视图：`Shelf` 实现 domain `topics::Board`（`OVERVIEW`、`DETAIL` 两条 SQL 共用 `topic!()` 宏拼话题摘要）；详情先经 `topic_aliases` 解析旧 ID，`withheld` 行的 `text` 置空，变更记录至多 100 条
- 外发记录：cli 的 `GET /api/outbound` 经 domain `outbound.list` 调 `Shelf`，只读；没有人工审核，`rejected` 只存在于旧数据
- `validate(setup)`：先检查 `batch ≤ 200`、`topics.max_open + batch ≤ 254`（决策接口 `questions ≤ 400`、`choices ≤ 255`），再校验决策与本地模型 URL，再用空文本调 domain `redact` 与 `clues`，规则或线索正则无效即 `Err`，最后逐个 `Desk::prompt`，任一为 `None` 报 `triage.prompts_dir: <名字>: unusable`；cli 启动自检用它
- 外发内容（决策请求体、codex 提示词）只能来自 domain 渲染后的结果，adapters 不做文本替换
- 防自循环：domain 的 `line` 跳过通知会话的消息，本人发出的只标 `role = own`、不出紧急题；本人与通知会话按消息的 `source:account` 取对应数据源的 `Profile`，取不到（无此概念的源）则不判定；通知以本人身份发出，补拉可能再收到，靠通知会话这一条切断
- 停机：在途子进程被打断并 kill，状态不改，下次启动续做（至少一次，发送靠幂等键去重）
- 排查（只读：`sqlite3 -readonly <store_path>`，WAL 下库所在目录须可写以建 `-shm`）：
  - `lines` 按 `state, reason, role` 计数：通知会话为 `skipped/notice`，空正文、密文、筛查拦下的为 `withheld`，自动邮件没有话题可并的为 `unrelated/no_topic`；锚点之后的消息都应有 `lines` 行
  - `batches` 由 `ready` 变 `sent`；`failed` 的批按 `max_revivals` 自动重来，`revivals = -1` 的旧批只能人工重发；`answers` 存决策应答
  - 已通过的行都有 `members`；`withheld` 行在同会话相邻行归属后才有；`members.via` 为 `adjacent`、`clue`、`choice`、`new`、`neighbour`、`legacy`
  - `alerts` 由 `waiting` → `pending` → `analyzed` → `sent`，结论没变或被新进展顶替的停在 `quiet`，被更新结论或合并取代的为 `merged`；`changes` 由 `pending` → `applied` 或 `dropped`；`topics.state` 为 `open`、`resolved`、`cancelled`、`archived`
  - 日志关键词：`batch formed`、`batch sealed`、`batch decided`、`alert analyzed`、`alert sent`、`failed batch revived`，WARN `decision failed`（带 `batch_id`、`attempt`、`revivals`、`exhausted`、`retry_in_ms`）、`alert analysis failed`、`alert step failed`（带 `key`、`attempt`、`exhausted`、`retry_in_ms`）、`line screening failed`（带 `key`、`attempt`、`reason`、`retry_in_ms`）、`lane backing off`（带 `lane`、`attempt`、`faults`、`retry_in_ms`）；采集重连 `dingtalk session ended`、`imap session ended`（带 `account`、`attempt`、`retry_in_ms`，`attempt` 为 `Backoff::streak()`，收到过消息的一轮后回到 1）
  - 等待原因只在变化时记一条 info（`Shift::noted` 按主题记上次原因）：`batch waiting`（`state` 为 `screening@<批号>` 或 `sending@<批号>`）；`batch claim skipped` 表示打包后库里出现了更新的规则、下轮重打包

## 新增数据源

- `ingest::Source` 加一个可选字段与 `checks`、`label`、`missing`、`profile`、`run` 各一处取值；`<kind>.rs` 与 `<kind>/` 放采集与配置；domain 新增该格式的解析 feature，产出与钉钉相同的 envelope JSON，`Archive` 不改
- 会话 ID 须自成命名空间，免得与其他源撞成同一段；没有本人、通知会话、密文概念的源 `Profile` 留空
- 补测试与假程序，同步 README 配置表与 docs/requirements.md

## 约定

- 账本与采集共库，多任务跨线程并发写：开 WAL，读写互不阻塞、写者之间串行；除 `apply` 与 `settle` 的 `BEGIN IMMEDIATE` 短事务外每次写单条语句，不跨 await 持事务；多语句写不得用 DEFERRED（WAL 下读升写直接 `BUSY_SNAPSHOT`，不等 `busy_timeout`）；`Archive::open` 与 `Ledger::open` 都经 `connect` 执行 `SCHEMA`（`IF NOT EXISTS`），`Shelf` 不建表
- 单写者（一个线程独占写连接、各任务经 channel 提交）暂不做；日志频繁出现 `store busy`，或某源要求 `synchronous = FULL` 且写入达每秒数百条时再评估
- 需确认语义的数据源（消息队列 ack、邮件已读标记）只在 `save_*` 返回 Ok 后确认，`Halt::Busy` 不确认；写库与确认之间不可被停机打断
- 不清账本表：清 `lines` 会重新外发；清 `alerts` 会让幂等键 `<话题>@<批号>` 复用，通知被 dws 当作重复吞掉；`learned_rules` 始终保留（仍参与脱敏）；`misses`（漏判样本，含已过规则脱敏的消息文字）不经接口与日志返回，保留期待定
- 日志只记行数、数量、标识、状态、次数、长度、退出码，不含消息正文、提示词、分析、响应、规则、地址、账号与密码。所有外部交互经 `trace.rs` 的 `external(action, request, &Outcome)` 记一条 debug：字段 `transport/session_id/user/feature/action/params/request/response/ok/elapsed_us/attempt/code/status`，request、response 只记 `redacted:<字节数>`，`ok` 即 `status == "success"`，status 只取 `success` 或固定原因串（不放服务器原文），字段值在宏外算好；来源经 `Caller::new(transport, feature, user).session(id)`：
  - triage：`external`/`triage`，action 为 `decide`、`screen`、`analyze`、`notify`
  - IMAP：`imap`/`imap`/account，`session_id` 为 `<account>#<进程内连接序号>`；action 为 `connect`（params `tls|plain+direct|http|socks5|socks5h`）、命令动词（`CAPABILITY`、`ID`、`LOGIN`、`SELECT`、`UID SEARCH`、`UID FETCH`、`LOGOUT`，`UID` 类 params 为参数，request、response 为本条命令的读写字节）、`IDLE`（status `exists` 为服务器推送、`refresh` 为 `idle_ms` 到期，`elapsed_us` 为整轮时长）、`mail`（params `<UIDVALIDITY>:<UID>`，status `success`/`skip:<原因>`/`rejected`）；`parse_mail` 的 dispatch 带同一 session_id；不记 Message-ID
  - dws：`dws`/`dingtalk`/account，action 为 `pull`（params 起止时间）、`decrypt`、`lookup`（`code` 为退出码，status 经 `finished`：`success`、`exit:<码>`、`io:<种类>`）、`listen`（params `start`、`stop`）；listen 的 stderr 改为管道，`next_line` 每次先取完已到的错误行，各记一条 info `dws stderr`（`account`、`line`），写出时由 cli 统一脱敏；被停机打断的调用不记
- HTTP 错误不输出 URL 或原始错误文本
- 子进程环境一律 `env_clear` 后只放配置的 env 与 vars；命令用 `PATH` 找程序时 `vars` 须含 `PATH`；dws 的 env 应含 `DO_NOT_TRACK=1` 关闭遥测
- `triage::config::Settings` 的字段不得超过 16 个：`derive(Debug)` 会生成 `&[&dyn Debug; N]`，N 为 17 时超过 256 字节触发 `large_stack_arrays`；新增配置并入 `session`、`redact` 等子结构
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
  - codex：`FAKE_CODEX`（fail、empty、junk、resume_fail、escape、slow；`titled` 首行带标题 `TITLE` 与变化 `moved`，`same` 首行 `changed:false`，`resolved` 首行状态 `resolved`）；默认输出 `thread.started`（会话 ID 取 `FAKE_THREAD`，默认 T1）与 `agent_message`（首行 `{"changed":true,"status":"open"}`，第二行 `ANALYSIS`）两行 JSONL
  - send：`FAKE_SEND`（fail、slow）
- 决策与本地模型用 `tests/it/triage/stub.rs` 的本机动态端口 HTTP stub；每个 `Sandbox` 独立端口。请求写入 `<角色>-calls.txt`（JSONL：方法、路径、头）与 `<角色>-body.txt`；连接并发处理（`for_each_concurrent`）；行为由目录内 `decide-stub.json`、`screen-stub.json` 的 `mode`（fail、junk、slow）与概率、话题、置信度、复核概率（`review`）字段控制，`screen.json` 可整包覆盖响应；`only` 为子串，只有请求体含它的请求才套用 `mode` 与 `screen.json`（缺省空串即全部），用来让同一批里只有某一行失败或变慢；筛查每个请求只有一行，夹具里的别名一律 `L1`。补丁顶层 `stub.decide`、`stub.screen` 由 `rig::behave` 写成这些文件，不进生产配置
- 假 IMAP 在 `tests/it/imap/fake.rs`：std 线程按会话脚本 `[(命令前缀, 回包)]` 应答，回包里 `<tag>` 换成本条命令的标签、`\u{0}` 处暂停 150 ms；`DONE` 沿用 IDLE 的标签；命令不匹配或脚本用完即断开，结尾加永不匹配的步骤可让服务器一直挂着；收到的命令经 channel 供断言；凭据取 `.cargo/config.toml` 的 `POLY_RESOLVE_TEST_IMAP_*`；SOCKS5 用例在 `tests/it/http/socks.rs`
- 代理用例在 `tests/it/http/proxy.rs`：线程 stub 读到空行后按设定回包或经 `pipe` 双向转发；可用代理与格式不合规的代理分别取 `.cargo/config.toml` 的 `POLY_RESOLVE_TEST_PROXY`、`POLY_RESOLVE_TEST_BAD_PROXY`（测试不能 `set_var`）
- HTTPS 用例由 `openssl s_server -quiet` 从 stdin 回写固定响应（`-www` 不回应 POST）；`fixtures/tls` 为 P-256 自签叶证书，须 `CA:FALSE`、`extendedKeyUsage=serverAuth`、SAN `DNS:localhost`、有效期不超过 825 天，否则 Security Framework 拒绝：`openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -days 825 -nodes -subj /CN=localhost -addext subjectAltName=DNS:localhost -addext extendedKeyUsage=serverAuth -addext basicConstraints=critical,CA:FALSE -keyout key.pem -outform DER -out cert.der`
- 存储故障注入：
  - 预建缺列的表（`BAD_SCHEMA`，在 `heal` 的查询处失败）
  - 对已建表加触发器 `REFUSE_INSERT`、`REFUSE_UPDATE`（写失败而查询正常）
  - 水位列为 TEXT（`TEXT_STAMP`）
  - 列齐全但 `occurred_at_ms` 为 TEXT（`BAD_STAMP`，读取行解码失败；须同时建 `members`、`topics`、`alerts`，`Shelf` 连表）
  - 账本写失败靠对 `lines`、`alerts` 加 `RAISE(ABORT)` 触发器，读失败靠把 `batches` 重建成缺列表；认领失败靠 `BEFORE UPDATE OF attempts` 触发器
  - 忙锁：另一连接持 `BEGIN IMMEDIATE`，`busy_timeout_ms` 取 50（`tests/it/ingest/busy.rs`）
- 线上数据演练（改账本、迁移或流水线后做）：`sqlite3 <线上库> ".backup <副本>"` 取副本放 `.debug/`，写一个不入库的临时用例把副本拷进 `Sandbox`，先以 `mode: disabled` 跑一遍触发 `MIGRATE`，再开启跑到没有 `queued` 行；决策、codex、通知全用本机桩，不连钉钉与邮箱，不外发；核对：账本无故障日志、锚点后的消息都有 `lines` 行、`approved` 行都有 `members`、非 `skipped` 行都有 `seen` 线索标记、每批落结果耗时正常（缺索引会到秒级）；完事删掉副本、临时用例与沙箱目录
- 沙箱测试：
  - `fixtures/fake.sb` 只放行 `/bin`、`/usr/bin`、fixtures 与 `DIR`，显式禁 `/usr/bin/true` 作金丝雀，无网络，只读 `/private/etc/ssl` 让 curl 能走到连接；`open.sb`（`allow default`）作对照
  - 假脚本 `escape` 读写 `$FAKE_OUTSIDE`、`net`（dws pull）用 `/usr/bin/curl` 连 `$FAKE_PORT`，用例分别断言被拒与对照放行
  - `sandbox.rs` 覆盖 `check` 各分支，探测参数 `noop` 让假脚本退出 0
  - 想让单个命令起不来：改它自己的 `sandbox.bin`（io 错误）或 `bin`（71）
- 用例与 helper 分布：
  - `tests/it/support.rs`：`Sandbox`（`fence` 给每个命令配 sandbox，`outside()` 是沙箱目录外含 `secret.txt` 的目录；`config(patch)` 把 `SHARED` 里的键放共享项、其余放唯一的钉钉源，`shared(sources)`、`source(patch)` 供自拼数据源列表；`triage(patch)` 生成 triage 配置）、`run_logged`（跑第一个数据源）、`triage_logged`（同一 future 里 `join` 两条 lane）、`triage_validate`
  - `tests/it/feeds.rs`：dws 监听行与补拉页的构造（`event`、`page`、`sealed_page` 等）
  - `tests/it/sources.rs`：数据源列表的校验、缺失变量、无类型项
  - `tests/it/triage.rs` 及 `tests/it/triage/` 下：`batch.rs`（成批、掩码、逐条优先级、回溯）、`backlog.rs`（超过回溯时长的积压照常决策但不转交）、`withheld.rs`（未外发行归入话题、给 codex 的占位、解密后重新分级）、`migrate.rs`（旧状态迁移，跑两遍结果相同）、`repack.rs`（重放不重打包、库里规则对等待中的批与筛查中的行生效）、`mailbox.rs`（本人与自动邮件的角色）、`clues.rs`（线索入库、跨会话按线索归入、线索正则无效）、`lifecycle.rs`（按分排队、归档与重开、影响面抬分、结论不变不通知、标题与晚到说明）、`topic.rs`（并入已有话题、同会话续接、长停顿分段、低置信度、合并本批的段）、`thread.rs`（会话绑定、续写、会话作废后新建、续写失败、输出无法解析）、`seal.rs`（hash 占位跨行一致、同一内容只外发一份且影响面只计一次、跨来源同名的人只计一次）、`screen.rs`（本地模型筛查）、`regroup.rs`（按线索与复核票数合并、移动、拆分，分析在途时延后，别名保持一跳）、`notices.rs`（冷却期内只发最新一条、被顶替的结论不通知、启动汇总）、`claim.rs`（先记账再调用、两条 lane 互不阻塞）；`rig.rs` 放共用的播种（`seed` 等）与读取（`calls`、`decided`、`logged` 等）；`decide-stub.json` 跨多次 `start` 保留，后一次要恢复默认须显式给 `stub.decide: {}`；想让转交停在 `pending` 又不认领，删掉 `prompts/session.md`
  - `tests/it/outbound.rs` 测 `Shelf` 的 `Outbox`，`tests/it/topics.rs` 测 `Shelf` 的 `Board`（列表与计数、经别名取详情、未外发行不带正文、读失败）；`triage/policy.rs` 测配置校验与 `marks()`
  - `tests/it/imap.rs` 及 `tests/it/imap/`：`settings(sandbox, url, patch)` 生成只有一个邮箱源的配置，`whole` 另可改共享项；`failure.rs`（协议与存储失败、停机）、`target.rs`（启动自检与地址校验）
  - 话题 ID 在测试里是 `t<批号>-<段序号>`；同一库跨多次 `start` 运行时用 `seed_from`、`seed_stamped` 给后来的消息更晚的时间

# adapters

## 职责

- I/O：dws 子进程、SQLite 存储、收集编排；可用 tokio；依赖 domain，被 cli 依赖
- 不写业务规则：解析、去重键、时间换算都在 domain，经 `dispatch` 调用（`bridge.rs` 构造请求），业务字符串与数值取自配置

## 模块

- `config.rs`：`Settings`（cli 配置的 `ingest` 段，`deny_unknown_fields`）、`Dws`、`Process`；对外 `pub mod`，不写 `pub use`
- `store.rs`：`Shelf`（`inbox::Reader` 的 SQLite 实现）：每次 `page` 才只读打开（`SQLITE_OPEN_READ_ONLY`，不建表、不建文件），单条 SQL 用 `json_group_array(json_object(…))` 取回再反序列化成 `Row`，按配置的 source 与 account 过滤，行值比较 `(occurred_at_ms, key) < (游标)` 翻页，并叠加 `Query` 的时间范围 `[since, until)`（走 `messages_by_time` 索引）；任何失败记 warn（不含正文）并返回 `AppError::Storage`；不开 WAL，靠 rusqlite 默认 5 秒 busy_timeout 与带 `LIMIT` 的短读避免阻塞 `ingest`；`Settings::shelf()` 构造
- `store.rs`：`Archive`，SQLite；`INSERT OR IGNORE … json_each` 直接吃 domain 返回的 JSON（`save_event` 包成数组，`save_page` 取 `$.envelopes`）；水位取 `MAX(occurred_at_ms)`，不另建表；错误装箱
- `bridge.rs` 的 `Page` 是 `parse_page` 结果的可改写副本（`envelopes_mut`、`render`），保存的是改写后的页面
- `dws.rs`：`Dws`（`lookup` 一次性执行，ID 以 `--msg-ids=<ID,…>` 追加；`listen` 起监听子进程，stdin、stdout 管道，stderr 继承；`pull` 一次性执行并收 stdout；`decrypt` 一次性执行，密文以 `--text=<密文>` 追加）、`Listener`（`next_line`、`stop`）；`plaintext(&Output)` 从解密命令的 JSON 取 `plaintext`，缺失或非 JSON 为 `None`
- `triage.rs`（`Setup`、`run`、`validate`）与 `triage/config.rs`（`Settings`、`Exec`、`Var`）、`triage/desk.rs`（调 domain `triage` feature）、`triage/intake.rs`（入库、成批、重新脱敏）、`triage/batch.rs`（封批、发送、落结果）、`triage/alert.rs`（分析、通知）：消息分级与转交的 worker，见下“分级与转交”；`ledger.rs`：`lines`、`batches`、`topics`、`placements`、`alerts`、`learned_rules` 的读写，所有查询与写入的参数打成一个 JSON 对象经 `json_extract(?1, …)` 取值（绕开参数个数限制），读返回 `json_group_array`，失败只记 warn 并返回空或 0；`Ledger::apply` 是唯一的多语句事务（`BEGIN IMMEDIATE` … `COMMIT`，任一步失败则 `ROLLBACK`）；`pipe.rs`：`feed(Job)` 起子进程（`env_clear` 后只放配置的 env 与 vars，args 里的 `${名}` 由 `Exec::args` 换成同名 vars 的值，stderr 置 null，stdin 写入后关闭，超时 `TimedOut`，`kill_on_drop`）
- `seatbelt.rs`：`Sandbox`（`bin`、`canary`、`paths`、`probe`、`profile`、`vars`，`deny_unknown_fields`）：`command(program)` 拼 `bin -f <真实路径 profile> -D 名=值 … -- program`，`paths` 与 `profile` 用 `canonicalize(..).unwrap_or_default()`，路径不存在即得空值，`sandbox-exec` 以 65 拒绝，不新增分支；`vars` 取环境变量，缺失为空；`check(probe, name)` 先跑 `canary`（须非 0）再跑 `probe`（须 0），错误是字符串。`ingest.dws` 的 4 个 `Process` 与 triage 的 3 个 `Exec` 各带一份 `sandbox`，命令一律经 `command` 起，不得绕过；`Dws::check`、`triage::config::Settings::check` 供 cli 启动自检
- `race.rs`：`race(stop, work)` 用 `poll_fn` 先 poll 停机信号再 poll 工作，代替被 clippy 禁的 `select!`；返回 `None` 即被停机打断
- `ingest.rs`：`run(settings, context, stop)` 返回退出码，单任务，不 spawn；`context` 由 cli 构造，`ingest` 只用其调 domain 的 `ingest` feature
- `bridge.rs`：持有 `&Context`，调 domain 的 `ingest` feature

## 流程

- 每轮 `session`：先起监听子进程（事件暂存在管道里），再补拉，再逐行入库；一轮结束后按退避等待再开下一轮
- 补拉：`window` 取区间（库内水位减重叠，无记录则回溯 `initial_backfill_ms`）→ `pull` → `parse_page` → `save_page`；窗口被拒、拉取失败、页面无法解析只记 warn，不阻断监听
- 密文：`ingest` 解析实时事件后，`Bridge::cipher` 带配置的 `cipher_min_length` 调 domain 的 `ingest.cipher` 判定（返回密文或空）；是密文则经 `Dws::decrypt` 解密并用 domain 的 `ingest.reveal` 换成明文再入库；解密失败（含命令不存在、输出非 JSON、无 `plaintext`）按密文入库并记 warn（只记长度）；解密期间收到停机信号则该事件不入库（补拉会补回）；`INSERT OR IGNORE` 使补拉的明文不覆盖已入库的密文
- 补拉页的每条 envelope 在 `absorb` 里经 `unseal` 走同一套判定与解密；启动自愈：`drive` 在 `reconcile` 之后调 `heal`——`Archive::sealed` 用 `GLOB '*||[0-9]*||[0-9]*||[0-9]*'` 预筛库里最新的至多 `heal_limit` 条（只是预筛，是否密文仍由 domain 的 `cipher` 判定），逐条交给 `unsealed`，文本变了才经 `Archive::replace`（`WHERE text = 旧文本`，防并发覆盖）更新；解密失败保持原样，下次启动再试；事件处理中解密失败的消息要等下次启动自愈
- 转发消息：domain 的 envelope 带 `lookup`（正文里有整行 base64 且不短于 `cipher_min_length` 时为消息 ID，否则空）；实时事件在 `unsealed` 后经 `assembled`、补拉在 `absorb` 里经 `assemble` 统一处理：收集有 `lookup` 的 ID，按 `lookup_batch` 分批调 `Dws::lookup`（`--msg-ids=` 逗号分隔），`dws::children` 取 `messages[].forwarded[]` 的 `{sender, text}`，再经 domain 的 `ingest.assemble` 重组；查询失败、无子消息按原样入库；查询期间收到停机信号则该事件或整页不入库（补拉会补回）
- 退出：停机信号 → 0；`dws` 退出码落在 `manual_exit_codes` → `manual_exit_code`；存储错误 → `failure_exit_code`
- 退避：从 `backoff_initial_ms` 起逐次翻倍到 `backoff_max_ms`；曾收到消息的一轮结束后复位

## 分级与转交

- `triage::run(setup, stop)` 单任务不 spawn，返回 `()`，永不使 serve 退出；`mode: disabled` 立即返回；账本打开失败记 error 后返回；每轮 `tick`：`intake` → `rerender` → `form` → `seal` → `send`（异步，决策）→ 逐条 pending 转交 `analyze`（异步，codex）→ 逐条 analyzed 转交 `deliver`（异步，通知）→ 睡 `poll_ms`；`exec` 经 `race` 打断，被打断返回 `None` 逐层 `?` 退出；`intake.rs`、`batch.rs`、`alert.rs` 里是以 `&Shift` 为参数的自由函数（`multiple_inherent_impl` 禁止给 `Shift` 再写 impl）
- 入库：`Ledger::unlined` 取回溯内（`lookback_ms`）还没有 `lines` 行的消息，从新到旧，每轮至多 `intake` 条；逐条调 domain `triage.line`（规则取 `Desk::rules`：库里学到的在前、配置的在后；规则无效则 `line` 返回请求无效、记 warn、该条留待下一轮，不外发），写 `lines`（`INSERT OR IGNORE`，`queued` 或带原因 `skipped`，记 `rules_rev`＝当时最大的规则 id）；`rerender` 把 `rules_rev` 落后的 `queued/pending/leaked/screening` 行经 domain `redact` 重新脱敏，`leaked` 行回到 `pending`
- 本地模型（`triage/screen.rs`，可选）：`triage.screen` 缺省为 `None` 时整段跳过，`form` 编入新批的行状态是 `approved`／`pending`；配了则是 `screening`，`screen` 取至多 `batch` 条 `screening` 行（`reason = 'deferred'` 的要等过 `retry_after_ms`），连同开放话题调 domain `triage.screen` 得请求体（OpenAI 兼容的 chat 格式，只含别名 `L<n>`），经 `exec` 发给本机模型，响应交 domain `triage.screened`：片段必须是该行文字的子串且不短于 `learn_min_chars` 才被 `placeholder` 遮盖，话题候选须在开放话题内，写回 `lines.text/flag/hints`；`manual` 转 `pending`（`flag` 非空的审核页置顶），`auto` 时 `flag` 非空转 `rejected`（原因 `screen`）、其余转 `approved`；失败（非 0 退出、超时、无法解析）整批标 `deferred` 不放行；`pack` 的请求里 `hints` 非空的段只列这些话题；`Settings::check`、`missing()` 把 `screen.exec` 纳入，`Settings` 字段数已满 16 个
- 成批：`form` 在没有未结批（有行 `batch_id > 0` 且其批还不是 `sent/failed`）时，把最新的至多 `batch` 条 `queued` 行编入新批（从新到旧，批号＝最大批号 + 1），状态按 `review.mode` 为 `pending`（审核人空）或 `approved`（审核人 `auto`）；`seal`（`Ledger::sealable`）批内无 `screening/pending/leaked` 行时，取已通过的行，加上开放话题（`last_at_ms` 在 `session_max_ms` 内，至多 `topics_max_open` 个）和各会话仍开放的话题（`attach`），调 domain `triage.pack` 得到冻结的请求体与 `layout`，写 `batches`（`ready`）；没有已通过的行则直接写 `sent` 关批
- 决策：`send` 按每小时已发 `batches` 数（`max_decisions_per_hour`）取最早的 `ready` 批，curl（URL 与 key 经 `vars` 注入 `TRIAGE_URL`、`TRIAGE_KEY`，用 `--variable %NAME` 与 `--expand-url`、`--expand-header` 取用，请求体走 stdin，argv 不含密钥）发送冻结的 `payload`，响应交 domain `triage.sort`；非 0 退出、超时、启动失败、响应无法解析都 `attempts + 1`（`reason` 为 `exit:<码>`、`io:<种类>` 或 `bad_answer`，不含正文），达 `max_attempts` 转 `failed`；成功则 `Ledger::apply` 一个事务写 `placements`、`topics`（`ON CONFLICT` 取概率最大值与最近时间）、对 `urgent` 话题开 `alerts`（键 `<话题>@<批号>`，同话题 `handoff_cooldown_ms` 内已有、或一小时内已达 `max_handoffs_per_hour` 则忽略）、批置 `sent`；崩溃发生在事务前则整批重发
- 审核：`lines` 表的 `state` 即审核状态，审核裁决由 cli 经 domain 的 `review` 路由调 `Shelf` 的 `Docket` 实现（`store.rs`：每次调用才以读写（`settle`）或只读（`list`、`source`）打开，不建库不建表；`settle` 在一个事务里先写 `learned_rules`（`INSERT OR IGNORE`，仅当该行仍是 `pending`）再 `UPDATE … WHERE state = 'pending'`，最后读回当前状态）；`rejected` 是终态，`leaked` 经 `rerender` 回到 `pending`；审核没有期限，不过期
- 转交：`analyze` 取 `alerts` 里 `pending` 的一条，话题的已通过行（`Ledger::topic_lines`）交 domain `triage.brief`：话题有 `thread_id` 且 `thread_at_ms` 在 `session_max_ms` 内则 `resume`（`followup` 模板、只含 `until_ms` 之后的行、命令追加 `session.resume`，`{thread}` 换成会话 ID），否则 `fresh`（`prompt` 模板、追加 `session.fresh`）；codex（stdin 传提示词，stdout 是 `--json` 事件流，经 `triage.reply` 取会话 ID 与最终回复，退出非 0、输出无法解析、回复为空都算失败，空输出的 `reason` 为 `exit:0`）→ `analyzed`，并把 `topics` 的 `thread_id`、`until_ms` 写回；失败时 `attempts + 1` 并清空该话题的 `thread_id`（续写失败后下一次分析新建会话）；`deliver` 用 `triage.notice` → dws 发送（参数末尾追加 `notify_target` 与 `--idempotency-key=<话题>@<批号>`）→ `sent`；失败 `attempts + 1` 并更新 `updated_at_ms`，达到 `max_attempts` 转 `failed`；阶段推进把 `attempts` 与 `updated_at_ms` 归零，故发送失败只重试发送
- 脱敏：`Desk` 把规则随请求交给 domain，adapters 不做任何文本替换；`triage::validate(setup)` 先检查 `batch ≤ 200`、`topics_max_open + batch ≤ 254`（决策接口 `questions ≤ 400`、`choices ≤ 255`），再用空文本调 domain `redact`，规则无效即 `Err`，cli 启动自检用它（退出码取 `sandbox_exit_code`）；外发内容（决策请求体、codex 提示词）只能来自 domain 渲染后的结果
- 防自循环：domain 的 `line` 跳过本人发出的与通知会话的消息；通知经 dws 以本人身份发出，补拉可能再收到，靠这两条切断
- 停机：在途子进程被打断并 kill，状态不改，下次启动续做（至少一次，发送靠幂等键去重）
- 账本与采集共库，除 `apply` 与 `settle` 的短事务外每次写单条语句、不跨 await 持事务，避免撞 5 秒 busy_timeout 让采集误判存储失败；`Archive::open` 与 `Ledger::open` 都执行 `SCHEMA`（`IF NOT EXISTS`），`Shelf` 不建表；旧版的 `decisions`、`handoffs`、`reviews`、`threads` 表不再使用，也不迁移
- 日志：`rows`、`opened`、`formed`、状态、次数、长度，不含正文、提示词、分析与响应；规则不入日志

## 约定

- 沙箱事实：一个进程只能有一层 Seatbelt（外层内再 `sandbox-exec` 更严格的策略或 `codex sandbox` 都 71）；`process-exec` 对符号链接无效，策略放行真实路径所在目录（`/opt/homebrew/Cellar/curl`、`/opt/homebrew/Caskroom/codex`）；网络规则的主机只能是 `*` 或 `localhost`，只能限端口；`-D` 值在 `ps` 可见，不传 secret；`sandbox-exec` 用 execvp，PID 与管道语义不变；失败码：策略或参数无效 65，被包装程序不存在 71；规则不全的后果是命令失败，故每条新规则都要能在 `log stream` 里对上拒绝记录；策略不写注释
- `codex.sb` 实机收敛结果：codex 用系统证书库校验 TLS，须放行 mach `com.apple.SecurityServer`，缺它表现为不停 `Reconnecting… error sending request` 而无网络拒绝记录；启动时 canonicalize 相对的 `CODEX_HOME` 需读 `PROJ` 目录本身（`literal`，不放行其下文件）；其余拒绝记录（偏好、FSEvents、process-fork、`/opt/homebrew/bin`）不致命，不放行；`-s`、`-C` 写在 `resume` 前对续写同样生效
- `dws.sb` 实机收敛结果（`dws event consume` 能订阅并连上）：读写 `~/.dws`、`~/Library/Application Support/dws-cli`（令牌）、`~/Library/Caches/dws`、`~/Library/Keychains`、`~/.local/bin/.dws-runtime.lock` 与 `.dws-runtime-manifest.json`；监听时 dws 经 `/bin/sh`、`/bin/bash` 起事件总线子进程，在 `/tmp/dws-event-<uid>/` 建 0700 目录与 unix socket（须由 dws 自己建，预建 0755 的目录会令其报权限错），读 Keychain 时反复起 `/usr/bin/security`（继承沙箱，需读自身与 `/usr/bin`、`se_SecurityMessages`、共享内存 `com.apple.AppleDatabaseChanged`）；其余 mach 服务、CFPreferences、`NSIRD` 临时目录的拒绝记录不致命，不放行；排错看 `log show --predicate 'sender == "Sandbox"'`，拒绝记录里的进程名可能是 `sh`、`security` 而不是 `dws`
- `triage::config::Settings` 的字段不得超过 16 个：`derive(Debug)` 会生成 `&[&dyn Debug; N]`，N 为 17 时超过 256 字节触发 `large_stack_arrays`；新增配置并入 `session`、`review`、`redact` 等子结构
- 子进程环境一律 `env_clear` 后只放配置的 env 与 vars（含采集的 dws）；命令用 `PATH` 找程序时 `vars` 须含 `PATH`
- 停机先丢子进程 stdin（dws 视 EOF 为优雅退出并取消订阅），超过 `child_exit_timeout_ms` 才 kill；kill -9 会泄漏服务端订阅（再启报已存在、重复投递）
- 子进程环境经 `dws.env` 配置，应含 `DO_NOT_TRACK=1` 关闭 dws 遥测
- `Option` 的管道句柄用 `map`、`map_or`、`take` 处理，不写取不到的分支（覆盖率会缺 region）
- `race<T>` 是泛型，每个实例（读行、补拉、睡眠）都要有“被停机打断”与“正常完成”用例
- 日志不含消息正文，只记长度、数量、退出码

## 测试

- 假 dws：`tests/it/fixtures/fake-dws.sh`，行为由配置 `dws.env` 里的 `FAKE_LISTEN`（exit、hang，默认等 stdin 结束）、`FAKE_PULL`（fail、slow，默认输出 `pull.json`）、`FAKE_CODE` 与 `FAKE_DIR` 下的 `listen.txt`、`pull.json` 控制；补拉参数写入 `pull-args.txt`；`lookup` 行为由 `FAKE_LOOKUP`（fail、junk、slow，默认输出 `lookup.json`）控制，参数逐行追加到 `lookup-args.txt`；`decrypt` 行为由 `FAKE_DECRYPT`（fail、junk、slow，默认输出 `{"ok":true,"plaintext":"CLEARTEXT"}`）控制，参数写入 `decrypt-args.txt`
- 监听输入每行须以换行结尾（`feed`），否则 `next_line` 要到 EOF 才返回最后一行
- 触发存储错误：预建缺列的表（`BAD_SCHEMA`，在 `heal` 的查询处失败）、对已建表加触发器 `REFUSE_INSERT`、`REFUSE_UPDATE`（`INSERT`、`UPDATE` 失败而查询正常）、水位列为 TEXT（`TEXT_STAMP`）；读取行解码失败：列齐全但 `occurred_at_ms` 为 TEXT（`BAD_STAMP`）
- tracing 宏的字段表达式只在有 subscriber 时求值：经 `run_logged` 装 subscriber（含常驻 `ANCHOR`，原因见 domain/CLAUDE.md），否则 `Summary` 的访问方法覆盖不到
- 沙箱测试：`fixtures/fake.sb`（只放行 `/bin`、`/usr/bin`、fixtures 与 `DIR`，显式禁 `/usr/bin/true` 作金丝雀，无网络，只读 `/private/etc/ssl` 让 curl 能走到连接）、`open.sb`（`allow default`，作对照）；`Sandbox::fence` 给每个命令配 sandbox，`Sandbox::outside()` 是沙箱目录外含 `secret.txt` 的目录；假脚本 `escape` 读写 `$FAKE_OUTSIDE`、`net`（dws pull）用 `/usr/bin/curl` 连 `$FAKE_PORT`，用例分别断言被拒与对照放行；`sandbox.rs` 覆盖 `check` 各分支，探测参数 `noop` 让假脚本退出 0；想让单个命令起不来，改它自己的 `sandbox.bin`（io 错误）或 `bin`（71）
- 假 triage：`tests/it/fixtures/fake-triage.sh`，第一个参数为角色（decide、codex、send），行为由 `env` 里的 `FAKE_DECIDE`（fail、junk、slow；默认按 stdin 里的题名作答：`urgent_<n>` 取 `FAKE_PS`（空格分隔的第 n 项）或 `FAKE_P`（默认 0.95），`topic_<n>` 取 `FAKE_TOPICS` 第 n 项（默认 `new`）与 `FAKE_CONFIDENCE`（默认 0.9））、`FAKE_CODEX`（fail、empty、junk、resume_fail、slow；默认输出 `thread.started`（会话 ID 取 `FAKE_THREAD`，默认 T1）与含 `ANALYSIS` 的 `agent_message` 两行 JSONL）、`FAKE_SEND`（fail、slow）控制，参数、stdin、调用次数写入 `<角色>-args.txt`、`-stdin.txt`、`-calls.txt`，`decide-env.txt` 记录注入的 `TRIAGE_URL|TRIAGE_KEY`；用例放 `tests/it/triage.rs`，按主题拆在 `tests/it/triage/` 下的 `batch.rs`（成批、掩码、逐条优先级、回溯）、`review.rs`（manual 全流程、漏脱敏学规则后重审、拒绝关批）、`topic.rs`（并入已有话题、同会话续接、长停顿分段、低置信度、合并本批的段）、`thread.rs`（会话绑定、续写、超龄、续写失败、输出无法解析）；`Sandbox::triage` 默认 `review.mode: auto`；话题 ID 在测试里是 `t<批号>-<段序号>`；同一库跨多次 `start` 运行时用 `seed_from`、`seed_stamped` 给后来的消息更晚的时间；`tests/it/review.rs` 测 `Shelf` 的 `Docket`；helper 在 `tests/it/triage/rig.rs`，配置由 `Sandbox::triage` 生成，运行经 `triage_logged`，校验经 `triage_validate`；账本写失败靠对 `lines`、`alerts` 加 `RAISE(ABORT)` 触发器，读失败靠把 `batches` 重建成缺列表；`BAD_STAMP` 须同时建 `placements`、`alerts`（`Shelf` 连表）
- 含外部命令的用例（解密、查询）停机等待 `SHORT` 取 4 秒：负载高时 0.7、2.5 秒都不够，会偶发失败；同一测试二进制不可并行跑多份（沙箱目录同名）
- 每个测试用独立的 `Sandbox` 目录（`CARGO_TARGET_TMPDIR`）；以 `sleep` 当停机信号，时长取大值避免慢机误判

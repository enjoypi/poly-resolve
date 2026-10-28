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

## 约定

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
- 含外部命令的用例（解密、查询）停机等待 `SHORT` 取 4 秒：负载高时 0.7、2.5 秒都不够，会偶发失败；同一测试二进制不可并行跑多份（沙箱目录同名）
- 每个测试用独立的 `Sandbox` 目录（`CARGO_TARGET_TMPDIR`）；以 `sleep` 当停机信号，时长取大值避免慢机误判

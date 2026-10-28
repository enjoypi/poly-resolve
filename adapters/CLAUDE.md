# adapters

## 职责

- I/O：dws 子进程、SQLite 存储、收集编排；可用 tokio；依赖 domain，被 cli 依赖
- 不写业务规则：解析、去重键、时间换算都在 domain，经 `dispatch` 调用（`bridge.rs` 构造请求），业务字符串与数值取自配置

## 模块

- `config.rs`：`Settings`（cli 配置的 `ingest` 段，`deny_unknown_fields`）、`Dws`、`Process`；对外 `pub mod`，不写 `pub use`
- `store.rs`：`Archive`，SQLite；`INSERT OR IGNORE … json_each` 直接吃 domain 返回的 JSON（`save_event` 包成数组，`save_page` 取 `$.envelopes`）；水位取 `MAX(occurred_at_ms)`，不另建表；错误装箱
- `dws.rs`：`Dws`（`listen` 起监听子进程，stdin、stdout 管道，stderr 继承；`pull` 一次性执行并收 stdout）、`Listener`（`next_line`、`stop`）
- `race.rs`：`race(stop, work)` 用 `poll_fn` 先 poll 停机信号再 poll 工作，代替被 clippy 禁的 `select!`；返回 `None` 即被停机打断
- `ingest.rs`：`run(settings, errors, stop)` 返回退出码，单任务，不 spawn
- `bridge.rs`：调 domain 的 `ingest` feature

## 流程

- 每轮 `session`：先起监听子进程（事件暂存在管道里），再补拉，再逐行入库；一轮结束后按退避等待再开下一轮
- 补拉：`window` 取区间（库内水位减重叠，无记录则回溯 `initial_backfill_ms`）→ `pull` → `parse_page` → `save_page`；窗口被拒、拉取失败、页面无法解析只记 warn，不阻断监听
- 退出：停机信号 → 0；`dws` 退出码落在 `manual_exit_codes` → `manual_exit_code`；存储错误 → `failure_exit_code`
- 退避：从 `backoff_initial_ms` 起逐次翻倍到 `backoff_max_ms`；曾收到消息的一轮结束后复位

## 约定

- 停机先丢子进程 stdin（dws 视 EOF 为优雅退出并取消订阅），超过 `child_exit_timeout_ms` 才 kill；kill -9 会泄漏服务端订阅（再启报已存在、重复投递）
- 子进程环境经 `dws.env` 配置，应含 `DO_NOT_TRACK=1` 关闭 dws 遥测
- `Option` 的管道句柄用 `map`、`map_or`、`take` 处理，不写取不到的分支（覆盖率会缺 region）
- `race<T>` 是泛型，每个实例（读行、补拉、睡眠）都要有“被停机打断”与“正常完成”用例
- 日志不含消息正文，只记长度、数量、退出码

## 测试

- 假 dws：`tests/it/fixtures/fake-dws.sh`，行为由配置 `dws.env` 里的 `FAKE_LISTEN`（exit、hang，默认等 stdin 结束）、`FAKE_PULL`（fail、slow，默认输出 `pull.json`）、`FAKE_CODE` 与 `FAKE_DIR` 下的 `listen.txt`、`pull.json` 控制；补拉参数写入 `pull-args.txt`
- 监听输入每行须以换行结尾（`feed`），否则 `next_line` 要到 EOF 才返回最后一行
- 触发存储错误：预建缺列的表（`BAD_SCHEMA`）、水位列为 TEXT（`TEXT_STAMP`）
- tracing 宏的字段表达式只在有 subscriber 时求值：经 `run_logged` 装 subscriber（含常驻 `ANCHOR`，原因见 domain/CLAUDE.md），否则 `Summary` 的访问方法覆盖不到
- 每个测试用独立的 `Sandbox` 目录（`CARGO_TARGET_TMPDIR`）；以 `sleep` 当停机信号，时长取大值避免慢机误判

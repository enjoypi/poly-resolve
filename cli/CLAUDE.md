# cli

## 职责

- 参数解析、配置加载、日志初始化、HTTP 传输、接口说明生成；子命令只有 `serve`、`ingest`、`spec`；业务一律调 `domain::features::dispatch`，不写业务逻辑

## 配置

- `config.rs` 的 `File` 带 `#[serde(deny_unknown_fields)]`；新增字段（含沙箱字段）同时改 `File`、`config.yaml`、`tests/it/support.rs` 的 `YAML`（沙箱锚点 `&fence` 只写一处）与 `tests/it/config.rs` 用例
- 段落顺序：`sandbox_exit_code`、`messages`（`domain::inbox::Paging`）、`web_dir`、`triage`（`adapters::triage::config::Settings`）都写在 `ingest` 之前，因为 `missing_ingest_fails` 以 `ingest:` 截断 YAML；`ingest` 段是 `adapters::config::Settings`（共享项加 `sources` 列表），字段与校验在 adapters；`config::load` 末尾调 `ingest.verify()`，数据源列表为空、有无类型项或重复即退出 2
- `addr` 为 `SocketAddr`，`config::load` 拒绝非回环 IP（IPv4、IPv6 均支持）
- `Var`：`triage` 段里 `"${NAME}"` 写法的值，整串须为 `${NAME}`；加载时只校验写法、不读环境，用时才读（不存值，`Debug` 只显示名字）
- `missing()`：`serve` 在 `triage.mode: enabled` 且 `triage.missing()` 与 `ingest.missing()`（各数据源的本人、通知会话）合起来有变量读不到（含空值）时以退出码 2 退出并列出变量名；`ingest` 与未启用时不检查；测试不能 `set_var`（`unsafe_code` forbid），用 `${HOME}`、`${PATH}`、`${CARGO_PKG_NAME}` 表示已设，一个不存在的名字表示缺失
- `verify`：`serve` 与 `ingest` 在读完配置、检查完 `missing()` 后、绑定端口前调用：`ingest.check()`（各数据源并发）；`triage.mode` 为 `enabled` 时再 `triage.check()` 与 `adapters::triage::validate`；任一失败以 `sandbox_exit_code` 退出并把原因写 stderr（`fail`）
- `Settings::context(&reader)` 构造 `domain::features::Context`：`Rules` 取 `messages` 分页、`triage.review.reasons` 与 `triage.review.learn_min_chars`，`Ports` 把同一个 `adapters::store::Shelf` 既作 `Reader` 又作 `Docket`；HTTP 处理与 `ingest` 各用 `settings.ingest().shelf()`

## 退出码

- 来源：成功 0；启动自检失败取 `sandbox_exit_code`；监听失败取 `serve_exit_code`；收集存储失败、须人工处理取 `ingest.failure_exit_code`、`ingest.manual_exit_code`（由 `adapters::ingest::run` 返回）；停机排空超时取 `shutdown_timeout_exit_code`；参数或配置错误为 2，因发生在读配置前，故写死在代码（与 clap 一致）；写输出失败至少为 1
- 收尾用无分支写法 `emit(code, &io::Result<()>)`，即 `ExitCode::from(code.max(u8::from(written.is_err())))`
- 用户可见取值表在 README“返回码”，改动时同步

## 日志

- 日志写 stderr，`spec` 写 stdout；subscriber 不设全局，用 `dispatcher::with_default` 绑定 `Settings::log()`；跨 await 用 `with_subscriber(settings.log().clone())`（`with_default` 只管同步闭包）；任务只经 `Crew::launch` 起，`with_subscriber` 范围外不打日志

## HTTP

- 路由取自 `catalog::build()` 里有绑定的条目，`route(path, on(方法, 处理))`；方法字符串经 `Method::from_bytes`、`MethodFilter::try_from` 无分支映射；处理闭包把 `RawQuery` 与 `RawPathParams`（已解码，经 `serde_urlencoded::to_string` 重新编码，消息 ID 里的 `+`、`/` 才不出错）拼成 params，请求体作 request，经 `Call::with_params` 交 dispatch
- 状态码取 `AppError::status()`，handler 状态为 `Arc<Settings>`；响应头 `content-type: application/json`；axum 路由层 404、405 不经 dispatch、不打日志
- `guard` 中间件包住全部路由：Host 须等于 `listener.local_addr()` 或 `localhost:<端口>`，否则 403（防 DNS 重绑定）；GET 以外的方法须 `content-type: application/json`，否则 415
- 只用 GET 与 PATCH：跨站 PATCH 须预检，无 OPTIONS 路由即无法发起；不加 POST 路由、不加 CORS
- `x-user`、`x-session-id` 代码不校验，任何路由（含 `review`）都不要求（部署要求见 README“HTTP 调用”）

## 服务与采集

- runtime 为 `multi_thread`；`service.rs` 的 `Crew` 持 `JoinSet<u8>`、内部停机 `watch` 与系统信号 `shutdown()`；`Crew::launch` 是唯一起任务处（`JoinSet::spawn` 加 `with_subscriber`，clippy.toml 禁其余 spawn）
- 任务：每个数据源一个（`gather`，按下标 `skip(i).take(1)` 取源，不写取不到的分支），`serve` 另起 triage 两条 lane（`work`）；任务内由 `Arc<Settings>` 构造 Shelf、Context，停机 future 是 `watch` 置位
- `run(listener, settings)`：先 `prepare` 库，再生成全部任务（`Crew::new` 同步注册信号）→ `http::start`（同步打 `listening`，测试的 `start` 依赖它是首行）→ 逐个 `launch` → `supervise`；`collect(settings)` 供 `ingest` 子命令，同样监督，以立即完成的 future 代替 server
- 监督：收到信号即置 `watch`；任务结果 `unwrap_or(failure_exit_code)`（只有 panic 才是 `JoinError`），非 0 也置 `watch` 让其余任务正常停止，取各码最大值；任务全部结束后，有非 0 码即返回该码并丢弃 server，否则等 server 排空返回其结果；server 先完成也要等任务（让 dws 正常退出）；两种顺序与非 0 都要有用例
- triage 关闭时 `triage::run` 立即返回 0

## 页面

- `assets.rs`：`GET /` 取 `web_dir/index.html`，`GET /{file}` 取 `web_dir/<file>`，不经 dispatch、不打日志；`file` 须全由字母、数字、`.`、`_`、`-` 组成且不以 `.` 开头（axum 的 `Path` 已百分号解码，`%2F`、`..` 都在此被拒）；读失败一律 404 空响应体
- content-type 按扩展名查 `TYPES`（css、html、js、svg），其余 `application/octet-stream`；`bun build` 产物平铺在输出目录，单段路径够用，不支持子目录
- 已有路径用错方法 405（如 POST `/{file}`、POST `/api/reviews`）；无此路径 404

## 停机

- `signal.rs` 的 `shutdown()`（serve 与 ingest 共用）用 `signal(kind).ok()` 加 `poll_fn` 轮询，注册失败取 Pending；不 spawn
- `http::start` 在 `listening` 日志前同步调 `shutdown()` 两次，axum 优雅停机与 `deadline` 各一份，`Crew::new` 再调一次（tokio 广播同一信号），都在起任务前，避免注册前的信号按默认行为杀进程
- `poll_fn` 内先 poll `deadline`（信号后再等 `shutdown_timeout_ms`）再 poll 服务：服务先完成退出 0；仅 `deadline` 到期返回超时码并丢弃服务
- 返回 `ExitCode` 而非 `process::exit`，残留连接随 runtime 在 main 返回时取消
- 日志：`shutdown signal received`（info，`timeout_ms`）、`shutdown complete`（info）、`shutdown timeout`（error，`timeout_ms`、`exit_code`）
- 已收到部分请求字节（请求头或请求体不完整）的连接会拖住停机；空闲或未发数据的连接立即关闭

## spec

- 子命令不读配置，把 `domain::features::catalog::build` 拼成 OpenAPI 3.1 写 stdout；参数 schema 经 `$ref` 在 `components` 取出，属性名出现在路径模板 `{x}` 里的为 `in: path`，其余 `in: query`，`required` 取自 schema；有请求体才写 `requestBody` 与 415；每个操作都有 403；`x-session-id`、`x-user` 常量在 `http.rs`，handler 与 spec 共用；`ErrorBody.code` 取自配置故只声明 string；description 用英文；`tests/it/spec.rs` 比对 `api/openapi.json`

## 测试

- helper 放 `tests/it/support.rs`，临时文件写 `CARGO_TARGET_TMPDIR`
- 子进程测试用 `CARGO_BIN_EXE_cli`，须以 SIGINT 或 SIGTERM 让进程正常退出（被 kill 的进程不写 profraw）；serve 监听 `127.0.0.1:0`，从 listening 日志取地址
- 起 serve 或 ingest 须用 `support::live(dir)` 的配置：把假 dws（复用 `../adapters/tests/it/fixtures/fake-dws.sh`）、假 triage、`fake.sb`、沙箱目录、`FAKE_DIR` 与可写库换成 `dir` 下的路径；默认 YAML 的 `bin` 都不存在，直接起会在沙箱自检失败
- 故障用例：存储失败把库路径换成不存在的目录（`ingest.rs` 的 `unwritable`）；沙箱自检失败把 `fixtures/fake.sb` 换成不存在的文件（`absent`）；triage 的自检失败把 `decide` 的 `bin` 换成不存在的程序；503 在启动后删库文件制造
- 日志含采集行，断言用 `index_of` 按内容找，不数行；ingest 用例等到 `event stored` 日志再发 SIGTERM；排空用例覆盖任务先完成，`FAKE_LISTEN: hang` 用例覆盖 server 先完成，`FAKE_LISTEN: exit` 加 `FAKE_CODE: "2"` 用例覆盖数据源须人工处理时全体停止；开 triage 的用例等到 `event stored` 与 `line opened` 证明各任务日志都在
- `tests/it/serve/` 是 `serve.rs` 的子模块，以访问其私有的 `Server`、`start`、`get`、`send`、`raw`、`stop`（`send` 自动带 `Host: <addr>` 与 JSON content-type）：
  - `assets.rs`：临时 `web_dir`，含目录外的 `secret.txt` 验证不可越界
  - `messages.rs`：用 `adapters::store::Archive` 播种临时库（含两个来源），经 HTTP 翻页与按时间筛选
  - `reviews.rs`：用 dev-dependency `rusqlite` 往库里插 `lines` 行，键含 `+`、`/`，覆盖通过、重复通过、拒绝、409、422、404、漏脱敏拒绝（响应里不含粘贴的原文），以及 `Shelf` 的 `Docket` 的失败日志分支（通过、拒绝、`source` 各一），因其 `tracing` 参数只在有 subscriber 时求值
  - `host.rs`：Host 放行与 403、415、内部动作路径 404、POST 405
- `YAML` 的钉钉源嵌在 `ingest.sources` 列表里，`dws.env` 下的键缩进 12 格，按行替换时照此缩进
- 配置用例经 `support::load`（`ingest` 子命令）：配置错误退出 2，配置有效则在沙箱自检处以 `sandbox_exit_code` 退出
- 停机测试：
  - 在途连接：连接 A 发完整请求头与不足 `Content-Length` 的请求体并保持
  - 同步点：另开连接完成一次完整请求（A 的数据已先到），再发信号；A 为 PATCH，补足后的响应码不作断言
  - 超时用例：须持有 A 到进程退出（EOF 会令进程以 0 退出）
  - 排空用例：等到 `shutdown signal received` 日志后再补足请求体
  - 配置：`shutdown_timeout_ms` 取大值避免慢机误超时，超时用例改成 50

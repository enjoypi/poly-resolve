# cli

## 职责

- 参数解析、配置加载、日志初始化、CLI 与 HTTP 传输、接口说明生成；业务一律调 `domain::features::dispatch`，不写业务逻辑

## 配置

- `config.rs` 的 `File` 带 `#[serde(deny_unknown_fields)]`；新增字段同时改 `File`、`config.yaml`、`tests/it/support.rs` 的 `YAML` 与 `tests/it/config.rs` 用例
- 退出码 `sandbox_exit_code`（顶层，写在 `ingest` 之前）：`serve` 与 `ingest` 在读完配置、检查完 triage 变量后、绑定端口前调 `verify`——`ingest.check()`，`triage.mode` 为 `enabled` 时再 `triage.check()`；失败以该码退出并把原因写 stderr（`fail`）；`call` 不检查。新增沙箱字段时同步 `File`、`config.yaml`、`tests/it/support.rs` 的 `YAML`（锚点 `&fence` 只写一处）
- `messages`（`domain::inbox::Paging`）与 `web_dir` 写在 `ingest` 段之前（`missing_ingest_fails` 以 `ingest:` 截断 YAML）
- `Settings::context(&reader)` 构造 `domain::features::Context`：`Rules` 取 `messages` 分页、`triage.review.reasons` 与 `triage.review.learn_min_chars`，`Ports` 把同一个 `adapters::store::Shelf` 既作 `Reader` 又作 `Docket`；`call`、`handle`、`ingest` 各用 `settings.ingest().shelf()`（形参叫 `reader`，避免 `similar_names`）
- `triage` 启用时 `verify` 在 `triage.check()` 之后调 `adapters::triage::validate`：`triage.limits.batch` 超过 200、`triage.sizes.topics_max_open + batch` 超过 254、或 `triage.redact.patterns` 有无效正则，都以 `sandbox_exit_code` 退出；`serve` 的 HTTP 另带 `review` 路由（不需 `x-user`）
- `addr` 为 `SocketAddr`，`config::load` 拒绝非回环 IP（IPv4、IPv6 均支持），`call` 同样校验
- `triage` 段（`adapters::triage::config::Settings`，也写在 `ingest` 之前）里 `"${NAME}"` 写法的值是 `Var`：整串须为 `${NAME}`，加载时只校验写法、不读环境，用时才读（不存值，`Debug` 只显示名字）；`serve` 在 `triage.mode: enabled` 且有变量读不到（含空值）时以退出码 2 退出并列出变量名（`missing()`），`call`、`ingest` 与未启用时不检查；测试不能 `set_var`（`unsafe_code` forbid），用 `${HOME}`、`${PATH}`、`${CARGO_PKG_NAME}` 表示已设，一个不存在的名字表示缺失

## 退出码

- 来源：成功 0；启动自检失败取配置 `sandbox_exit_code`；监听失败取配置 `serve_exit_code`；收集存储失败、须人工处理取配置 `ingest.failure_exit_code`、`ingest.manual_exit_code`（由 `adapters::ingest::run` 返回）；停机排空超时取 `shutdown_timeout_exit_code`；业务错误取 `AppError::exit_code(errors)`；参数或配置错误为 2，读配置前发生故写在代码（与 clap 一致）；写输出失败至少为 1
- 收尾用无分支写法 `emit(code, &io::Result<()>)`，即 `ExitCode::from(code.max(u8::from(written.is_err())))`
- 用户可见取值表在 README“返回码”，改动时同步

## call

- 参数在 `args.rs` 的 `Invocation`（`Command::Call(Box<Invocation>)`，枚举变体大小受 clippy 限制）：字段按字母序，位置参数用 `#[arg(index = N)]` 固定为 FEATURE、ACTION；经 `Invocation::call()` 得 `domain::features::Call`

## 日志

- 日志写 stderr，CLI 响应写 stdout；subscriber 不设全局，用 `dispatcher::with_default` 绑定 `Settings::log()`

## HTTP

- `POST /{feature}/{action}`，请求体即 request；状态码取 `AppError::status()`，handler 状态为 `Arc<Settings>`；响应头 `content-type: application/json`；axum 路由层 404、405 不经 dispatch、不打日志
- `x-user`、`x-session-id` 代码不校验（部署要求见 README“HTTP 调用”）

## ingest

- `service.rs`：`collect(settings, stop)` 由 `ingest` 子命令与 `serve` 共用；`run(listener, settings)` 在同进程里跑 `http::run` 与 `collect`，不 spawn，用 `poll_fn` 轮询两者：先 serve 后 collect（`listening` 是首行日志，测试的 `start` 依赖它）；collect 以非 0 返回（须人工处理、存储失败）即返回该码并丢弃 server，为 0（停机信号）则等 server 排空后返回 server 的结果；server 先完成也要等 collect（让 dws 正常退出，直接丢弃会留下 dws 子进程与订阅）；两种完成顺序都要有用例；triage worker 是第三个 future（`work`，用另一份 `shutdown()`），collect、worker、server 都结束才返回，collect 非 0 即丢弃 worker（其子进程 `kill_on_drop`）；关闭时 `triage::run` 立即返回
- `ingest --config`：`ingest` 段是 `adapters::config::Settings`，字段与校验在 adapters，cli 只在 `File` 里引用；停机信号取 `signal.rs` 的 `shutdown()`，经 `pin!` 以 `&mut` 传入 `adapters::ingest::run`；日志用 `with_subscriber(settings.log().clone())` 绑定（跨 await 有效，`with_default` 只管同步闭包）

## 页面

- `assets.rs`：`GET /` 取 `web_dir/index.html`，`GET /{file}` 取 `web_dir/<file>`，不经 dispatch、不打日志；`file` 须全由字母、数字、`.`、`_`、`-` 组成且不以 `.` 开头（axum 的 `Path` 已百分号解码，`%2F`、`..` 都在此被拒）；读失败一律 404 空响应体
- content-type 按扩展名查 `TYPES`（css、html、js、svg），其余 `application/octet-stream`；`bun build` 产物平铺在输出目录，单段路径够用，不支持子目录
- `GET /a/b` 命中接口路径只收 POST 的路由，故 405；POST 到 `/{file}` 同样 405

## 停机

- `signal.rs` 的 `shutdown()`（serve 与 ingest 共用）用 `signal(kind).ok()` 加 `poll_fn` 轮询，注册失败取 Pending；不 spawn
- `run` 在 `listening` 日志前同步调 `shutdown()` 两次，axum 优雅停机与 `deadline` 各一份（tokio 广播同一信号），避免注册前的信号按默认行为杀进程
- `poll_fn` 内先 poll `deadline`（信号后再等 `shutdown_timeout_ms`）再 poll 服务：服务先完成退出 0；仅 `deadline` 到期返回超时码并丢弃服务
- 返回 `ExitCode` 而非 `process::exit`，残留连接随 runtime 在 main 返回时取消
- 日志：`shutdown signal received`（info，`timeout_ms`）、`shutdown complete`（info）、`shutdown timeout`（error，`timeout_ms`、`exit_code`）
- 已收到部分请求字节（请求头或请求体不完整）的连接会拖住停机；空闲或未发数据的连接立即关闭

## spec

- 子命令不读配置，把 `domain::features::catalog::build` 拼成 OpenAPI 3.1 写 stdout；`x-session-id`、`x-user` 常量在 `http.rs`，handler 与 spec 共用；`ErrorBody.code` 取自配置故只声明 string；description 用英文；`tests/it/spec.rs` 比对 `api/openapi.json`

## 测试

- helper 放 `tests/it/support.rs`，临时文件写 `CARGO_TARGET_TMPDIR`
- 子进程测试用 `CARGO_BIN_EXE_cli`，须以 SIGINT 或 SIGTERM 让进程正常退出（被 kill 的进程不写 profraw）；serve 监听 `127.0.0.1:0`，从 listening 日志取地址
- serve 随带采集：测试须用 `support::live(dir)` 的配置（假 dws、`dir` 下可写库），默认 YAML 的 `bin` 都不存在，起 serve 会先在沙箱自检失败；日志含采集行，断言用 `index_of` 按内容找，不数行；503 用例在启动后删库文件制造；排空用例覆盖 collect 先完成，`FAKE_LISTEN: hang` 用例覆盖 server 先完成
- serve 的页面、消息与审核测试放 `tests/it/serve/`（`assets.rs`、`messages.rs`、`reviews.rs`；后者用 dev-dependency `rusqlite` 往库里插 `lines` 行，覆盖逐条通过、拒绝、漏脱敏拒绝（响应里不含粘贴的原文）与 `Shelf` 的 `Docket` 的失败日志分支（`approve`、`reject`、`source` 各一），因其 `tracing` 参数只在有 subscriber 时求值），作为 `serve.rs` 的子模块以访问其私有的 `Server`、`start`、`get`、`post`、`stop`；页面用临时 `web_dir`（含目录外的 `secret.txt` 验证不可越界）；消息用 `adapters::store::Archive` 播种临时库，同一份配置分别走 `call` 子进程与 HTTP，断言响应一致
- ingest 测试：假 dws 夹具复用 `../adapters/tests/it/fixtures/fake-dws.sh`，配置里把 `bin`、`FAKE_DIR`、`store_path` 换成临时目录；等到 `event stored` 日志再发 SIGTERM
- `live(dir)` 把假 dws、假 triage、`fake.sb` 与沙箱目录换进 YAML；存储失败用例在其结果里把库路径换成不存在的目录（`ingest.rs` 的 `unwritable`）；沙箱自检失败用例把 `fixtures/fake.sb` 换成不存在的文件（`absent`），triage 的用例把 `decide` 的 `bin` 换成不存在的程序
- `tests/it/redact.rs` 读取真实的 `cli/config.yaml` 里 `triage.redact.patterns`，经 `cli call triage redact` 跑样例表：每条规则至少一例必须被替换、几例明文必须原样；规则是数据，改规则必须同步样例，源码里的中文用 `\u{…}` 转义（`non_ascii_literal`）
- 停机测试：
  - 在途连接：连接 A 发完整请求头与不足 `Content-Length` 的请求体并保持
  - 同步点：另开连接完成一次完整请求（A 的数据已先到），再发信号
  - 超时用例：须持有 A 到进程退出（EOF 会令进程以 0 退出）
  - 排空用例：等到 `shutdown signal received` 日志后再补足请求体
  - 配置：`shutdown_timeout_ms` 取大值避免慢机误超时，超时用例改成 50

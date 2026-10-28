# cli

## 职责

- 参数解析、配置加载、日志初始化、CLI 与 HTTP 传输、接口说明生成；业务一律调 `domain::features::dispatch`，不写业务逻辑

## 配置

- `config.rs` 的 `File` 带 `#[serde(deny_unknown_fields)]`；新增字段同时改 `File`、`config.yaml`、`tests/it/support.rs` 的 `YAML` 与 `tests/it/config.rs` 用例
- `addr` 为 `SocketAddr`，`config::load` 拒绝非回环 IP（IPv4、IPv6 均支持），`call` 同样校验
- `${...}` 环境变量展开尚未实现，引入首个 secret 时在 `config::load` 实现

## 退出码

- 来源：成功 0；监听失败取配置 `serve_exit_code`；停机排空超时取 `shutdown_timeout_exit_code`；业务错误取 `AppError::exit_code(errors)`；参数或配置错误为 2，读配置前发生故写在代码（与 clap 一致）；写输出失败至少为 1
- 收尾用无分支写法 `emit(code, &io::Result<()>)`，即 `ExitCode::from(code.max(u8::from(written.is_err())))`
- 用户可见取值表在 README“返回码”，改动时同步

## call

- 参数在 `args.rs` 的 `Invocation`（`Command::Call(Box<Invocation>)`，枚举变体大小受 clippy 限制）：字段按字母序，位置参数用 `#[arg(index = N)]` 固定为 FEATURE、ACTION；经 `Invocation::call()` 得 `domain::features::Call`

## 日志

- 日志写 stderr，CLI 响应写 stdout；subscriber 不设全局，用 `dispatcher::with_default` 绑定 `Settings::log()`

## HTTP

- `POST /{feature}/{action}`，请求体即 request；状态码取 `AppError::status()`，handler 状态为 `Arc<Settings>`；响应头 `content-type: application/json`；axum 路由层 404、405 不经 dispatch、不打日志
- `x-user`、`x-session-id` 代码不校验（部署要求见 README“HTTP 调用”）

## 停机

- `http.rs` 的 `shutdown()` 用 `signal(kind).ok()` 加 `poll_fn` 轮询，注册失败取 Pending；不 spawn
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
- 停机测试：
  - 在途连接：连接 A 发完整请求头与不足 `Content-Length` 的请求体并保持
  - 同步点：另开连接完成一次完整请求（A 的数据已先到），再发信号
  - 超时用例：须持有 A 到进程退出（EOF 会令进程以 0 退出）
  - 排空用例：等到 `shutdown signal received` 日志后再补足请求体
  - 配置：`shutdown_timeout_ms` 取大值避免慢机误超时，超时用例改成 50

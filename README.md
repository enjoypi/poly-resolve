# PolyResolve

通过命令行或 HTTP 调用业务功能。功能与规则见 [docs/requirements.md](docs/requirements.md)。

## 前置条件

- macOS 或 Linux
- rustup 与 stable 工具链：`rustup toolchain install stable`
- 仅查看消息页面需要 Bun：构建前端用 `bun run build`
- 仅收集钉钉消息（`ingest`）需要：钉钉工作台命令行 `dws`，安装后执行 `dws auth login` 登录；企业管理员须开通“CLI 访问管理”，补拉另需“消息搜索”权益

## 构建

```sh
cargo +stable build --release --locked
```

产物：`target/release/cli`。

## 配置

YAML 文件，各项均必填，不允许其他字段。示例见 `cli/config.yaml`。

| 项 | 说明 |
|---|---|
| addr | 服务监听地址，格式 `IP:端口`（IPv6 写作 `"[::1]:端口"`），不支持主机名；须为回环地址（如 127.0.0.1、::1）；端口 0 表示随机分配 |
| log_level | 日志级别：off、error、warn、info、debug、trace；日志为 JSON 行，写 stderr，debug 级记录每次调用 |
| serve_exit_code | 服务地址绑定失败时的退出码，1 至 255 |
| shutdown_timeout_exit_code | 停机等待超时被强制退出时的退出码，1 至 255 |
| shutdown_timeout_ms | 收到停机信号后等待在途请求完成的最长毫秒数，至少 1 |
| web_dir | 页面文件所在目录，即前端构建产物目录（如 `web/dist`）；目录或文件不存在时页面请求返回 404 |
| messages.default / max | 查看消息接口每页默认条数与单页上限，至少 1 |
| ingest.account | 账号标识，写入每条消息记录，区分不同登录员工 |
| ingest.source | 来源标识，写入每条消息记录，如 dingtalk |
| ingest.source_utc_offset_minutes | 钉钉返回的无时区时间所属时区相对 UTC 的分钟数，北京时间为 480 |
| ingest.store_path | SQLite 文件路径，文件不存在则创建，所在目录须已存在；内含消息正文，须限制目录权限 |
| ingest.initial_backfill_ms | 库内无记录时，首次补拉回溯的毫秒数 |
| ingest.reconcile_overlap_ms | 补拉从库内最新消息时间再往前多回溯的毫秒数 |
| ingest.backoff_initial_ms / backoff_max_ms | 钉钉工具中断后重启的等待毫秒数，逐次翻倍直到上限，至少 1 |
| ingest.child_exit_timeout_ms | 停机时等待钉钉工具退出的最长毫秒数，超时强杀，至少 1 |
| ingest.failure_exit_code | 存储失败时的退出码，1 至 255 |
| ingest.manual_exit_code | 钉钉工具提示登录失效或需授权、须人工处理时的退出码，1 至 255 |
| ingest.dws.env | 传给钉钉工具的环境变量，应含 DO_NOT_TRACK 以关闭其遥测 |
| ingest.dws.listen / pull | 实时监听与补拉所用命令：bin 为可执行文件，args 为参数；补拉自动追加 --start、--end |
| ingest.dws.manual_exit_codes | 钉钉工具退出码落在此列表时视为须人工处理 |
| errors.<错误>.code / exit_code | 该错误的响应体 code 与命令行退出码（1 至 255）；<错误> 取 bad_request、storage、unknown_action，见“返回码” |

## 命令行调用

```sh
cli call <FEATURE> <ACTION> --config <CONFIG> --json <JSON> [--session-id <SESSION_ID>] [--user <USER>]
```

| 参数 | 必填 | 说明 |
|---|---|---|
| FEATURE | 是 | 功能名，见“接口结构” |
| ACTION | 是 | 动作名，见“接口结构” |
| --config | 是 | 配置文件路径 |
| --json | 是 | 请求内容（JSON） |
| --session-id | 否 | 会话 ID，写入日志 |
| --user | 否 | 用户标识，写入日志 |

响应（含业务错误）以一行 JSON 写到 stdout：

```sh
./target/release/cli call health check --config cli/config.yaml --json '{}'
# {"status":"ok"}
```

帮助：`cli --help`、`cli call --help`、`cli serve --help`。

## 启动服务

```sh
./target/release/cli serve --config cli/config.yaml
```

启动后 stderr 输出 `listening` 日志，含实际地址（log_level 为 info 或更详细时可见）。同一进程还会挂起钉钉消息收集（等同 `ingest`，须先 `dws auth login`），消息写入 `ingest.store_path`，页面与接口读取同一份库；收集遇到须人工处理或存储失败时，服务以对应退出码退出。

Ctrl+C（SIGINT）或 SIGTERM 触发停机：不再接受新连接，在途请求完成、钉钉工具正常退出后以退出码 0 退出；超过 shutdown_timeout_ms 仍未完成则强制退出，退出码取 shutdown_timeout_exit_code，在途连接被重置。停机期间再次发信号无效。停机过程写 stderr 日志，超时记为 error 级。

## 收集钉钉消息

```sh
./target/release/cli ingest --config cli/config.yaml
```

只收集、不提供页面时用本命令（`serve` 已包含同样的收集）。以登录员工本人身份收集其可见的全部单聊与群聊消息，写入 `ingest.store_path`：启动时先按已有记录补拉，再实时监听；钉钉工具中断后自动重启并再次补拉；同一消息只记一条。

Ctrl+C（SIGINT）或 SIGTERM 触发停机：让钉钉工具正常退出后以退出码 0 结束，超过 child_exit_timeout_ms 则强杀。日志同样写 stderr，不含消息正文。

## 查看消息

```sh
cd web && bun install --frozen-lockfile && bun run build
./target/release/cli serve --config cli/config.yaml
```

浏览器打开服务地址（如 `http://127.0.0.1:15801/`），按时间从新到旧显示 `ingest.account` 的消息：“刷新”重取最新一页，“加载更多”取更早的消息；时间按浏览器所在时区显示并标注偏移（如 `2026-10-28 09:20:09 UTC+8`）。消息来自 `ingest.store_path`，由 `serve` 同进程收集写入；只读已有的库、不连钉钉时用 `cli call messages list`。库不存在或读取失败时返回 503 与 `storage`。

接口 `POST /messages/list`（命令行 `cli call messages list`）：

| 字段 | 说明 |
|---|---|
| cursor | 可选，上一页响应的 next_cursor；缺省取最新一页 |
| limit | 可选，每页条数，1 至 messages.max，缺省取 messages.default |

响应含 `items`（不含原始内容）与 `next_cursor`（没有更早的消息时为 null）。

## HTTP 调用

`POST /<feature>/<action>`，请求体为 JSON，响应头 `content-type: application/json`。

| 请求头 | 说明 |
|---|---|
| x-session-id | 会话 ID，可选，写入日志 |
| x-user | 用户标识，可选，写入日志 |

部署要求：两个请求头由前置网关鉴权后写入，网关须剥离客户端自带的同名头。页面与消息接口都会返回消息正文，须一并置于网关认证之后。

```sh
curl -d '{}' http://127.0.0.1:15801/health/check
# {"status":"ok"}

curl -H 'x-session-id: s1' -H 'x-user: alice' -d '{}' http://127.0.0.1:15801/health/check
# {"status":"ok"}
```

## 接口结构

请求与响应结构以生成的接口说明 `api/openapi.json`（OpenAPI 3.1）为准，也可用 `cli spec` 输出到 stdout。

## 返回码

业务错误响应体为 `{"code":"<code>"}`，code 与退出码取自配置，下表数值为 `cli/config.yaml` 的取值。

| 情形 | HTTP | 退出码 | 输出 |
|---|---|---|---|
| 成功 | 200 | 0 | JSON |
| 请求内容无效 | 400 | 3 | `{"code":"bad_request"}` |
| 消息库不存在或读取失败 | 503 | errors.storage.exit_code | `{"code":"storage"}`（下表数值取 `cli/config.yaml`：10） |
| 功能或动作不存在 | 404 | 4 | `{"code":"unknown_action"}` |
| `--help` | — | 0 | 帮助写 stdout |
| 命令行参数错误 | — | 2 | 错误写 stderr |
| 配置错误（文件不存在、YAML 无效、缺少或未知字段、取值无效） | — | 2 | stderr 输出 `路径: 原因` |
| 服务地址绑定失败 | — | serve_exit_code | 错误写 stderr |
| 停机等待超时 | — | shutdown_timeout_exit_code | 日志写 stderr |
| 收集（`ingest` 或 `serve`）时存储失败 | — | ingest.failure_exit_code | 错误写 stderr 日志 |
| 收集（`ingest` 或 `serve`）时钉钉工具须人工处理 | — | ingest.manual_exit_code | 错误写 stderr 日志 |
| 写 stdout 失败 | — | 至少 1 | — |
| HTTP 路径不是 `/`、`/<文件>`、`/<feature>/<action>` | 404 | — | 空响应体 |
| 页面文件不存在或文件名含字母、数字、`.`、`_`、`-` 以外的字符或以 `.` 开头 | 404 | — | 空响应体 |
| 页面路径用 GET 以外的方法，或接口路径用 POST 以外的方法 | 405 | — | 空响应体 |
| HTTP 请求体不是 UTF-8 | 400 | — | 纯文本 |

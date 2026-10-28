# harness-rs

通过命令行或 HTTP 调用业务功能。功能与规则见 [docs/requirements.md](docs/requirements.md)。

## 前置条件

- macOS 或 Linux
- rustup 与 stable 工具链：`rustup toolchain install stable`

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
| errors.<错误>.code / exit_code | 该错误的响应体 code 与命令行退出码（1 至 255）；<错误> 取 bad_request、unknown_action，见“返回码” |

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

启动后 stderr 输出 `listening` 日志，含实际地址（log_level 为 info 或更详细时可见）。

Ctrl+C（SIGINT）或 SIGTERM 触发停机：不再接受新连接，在途请求完成后以退出码 0 退出；超过 shutdown_timeout_ms 仍未完成则强制退出，退出码取 shutdown_timeout_exit_code，在途连接被重置。停机期间再次发信号无效。停机过程写 stderr 日志，超时记为 error 级。

## HTTP 调用

`POST /<feature>/<action>`，请求体为 JSON，响应头 `content-type: application/json`。

| 请求头 | 说明 |
|---|---|
| x-session-id | 会话 ID，可选，写入日志 |
| x-user | 用户标识，可选，写入日志 |

部署要求：两个请求头由前置网关鉴权后写入，网关须剥离客户端自带的同名头。

```sh
curl -d '{}' http://127.0.0.1:42436/health/check
# {"status":"ok"}

curl -H 'x-session-id: s1' -H 'x-user: alice' -d '{}' http://127.0.0.1:42436/health/check
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
| 功能或动作不存在 | 404 | 4 | `{"code":"unknown_action"}` |
| `--help` | — | 0 | 帮助写 stdout |
| 命令行参数错误 | — | 2 | 错误写 stderr |
| 配置错误（文件不存在、YAML 无效、缺少或未知字段、取值无效） | — | 2 | stderr 输出 `路径: 原因` |
| 服务地址绑定失败 | — | serve_exit_code | 错误写 stderr |
| 停机等待超时 | — | shutdown_timeout_exit_code | 日志写 stderr |
| 写 stdout 失败 | — | 至少 1 | — |
| HTTP 路径不是 `/<feature>/<action>` | 404 | — | 空响应体 |
| HTTP 方法不是 POST | 405 | — | 空响应体 |
| HTTP 请求体不是 UTF-8 | 400 | — | 纯文本 |

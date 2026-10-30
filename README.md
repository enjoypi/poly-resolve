# Poly Resolve

通过命令行或 HTTP 调用业务功能。功能与规则见 [docs/requirements.md](docs/requirements.md)。

## 前置条件

- macOS（外部命令用系统自带的 `/usr/bin/sandbox-exec` 隔离，见“子进程沙箱”）
- rustup 与 stable 工具链：`rustup toolchain install stable`
- 仅查看消息页面需要 Bun
- 仅收集钉钉消息需要：钉钉工作台命令行 `dws`，安装后执行 `dws auth login` 登录；企业管理员须开通“CLI 访问管理”，补拉另需“消息搜索”权益
- 仅消息分级与转交需要：sops、codex，见“消息分级与转交”

## 构建

```sh
cargo +stable build --release --locked
```

产物：`target/release/cli`。

## 配置

YAML 文件，各项均必填（注明可选的除外），不允许其他字段。示例见 `cli/config.yaml`。

| 项 | 说明 |
|---|---|
| addr | 服务监听地址，格式 `IP:端口`（IPv6 写作 `"[::1]:端口"`），不支持主机名；须为回环地址（如 127.0.0.1、::1）；端口 0 表示随机分配 |
| log_level | 日志级别：off、error、warn、info、debug、trace；日志为 JSON 行，写 stderr，debug 级记录每次调用 |
| sandbox_exit_code | 启动时沙箱自检或 triage 配置校验失败的退出码，1 至 255 |
| serve_exit_code | 服务地址绑定失败时的退出码，1 至 255 |
| shutdown_timeout_exit_code | 停机等待超时被强制退出时的退出码，1 至 255 |
| shutdown_timeout_ms | 收到停机信号后等待在途请求完成的最长毫秒数，至少 1 |
| web_dir | 前端构建产物目录（如 `web/dist`）；目录或文件不存在时页面请求返回 404 |
| messages.default / max | 查看消息接口每页默认条数与单页上限，至少 1 |
| errors.<错误>.code / exit_code | 该错误的响应体 code 与命令行退出码（1 至 255）；<错误> 取 bad_request、storage、unknown_action，见“返回码” |
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
| ingest.cipher_min_length | 判定密文的正文最短字符数，至少 1 |
| ingest.heal_limit | 每次启动时最多重新解密多少条库里的密文消息，至少 1 |
| ingest.lookup_batch | 一次向钉钉工具查询转发消息详情的消息数，1 至 50 |
| ingest.dws.env / vars | 传给钉钉工具的环境变量：env 为字面量，应含 `DO_NOT_TRACK` 以关闭其遥测；vars 为 `${NAME}` 写法，运行时读同名环境变量（HOME、PATH 等）；子进程环境先清空再只放这两类 |
| ingest.dws.listen / pull / decrypt / lookup | 实时监听、补拉、解密、查询转发详情所用命令：bin 为可执行文件，args 为参数，sandbox 见“子进程沙箱”；程序自动追加参数：pull 追加 `--start`、`--end`，decrypt 追加 `--text=<密文>`（标准输出须为含 `plaintext` 的 JSON），lookup 追加 `--msg-ids=<消息 ID,…>`（标准输出须为含 `messages[].forwarded[]` 的 JSON） |
| ingest.dws.manual_exit_codes | 钉钉工具退出码落在此列表时视为须人工处理 |
| triage.mode | disabled 或 enabled；enabled 时 `serve` 启动即检查所需环境变量，缺失则以退出码 2 退出并列出变量名 |
| triage.model | 决策接口的模型名 |
| triage.questions | urgent：每条消息一问的优先级问题（`{line}` 换成消息别名）；topic：每个新会话段一问的话题归属问题（`{segment}` 换成段别名；`min_confidence` 低于它按新话题；`new_label` 是“新话题”选项的说明）；threshold：概率不低于它即高优先级，大于 1 则只打分不转交 |
| triage.limits | batch 每批最多消息条数，至多 200；intake 每轮最多入库的新消息条数；poll_ms 轮询间隔；lookback_ms 只处理多久以内的消息；max_attempts / retry_after_ms 失败重试次数与间隔；max_decisions_per_hour 每小时决策请求数；max_handoffs_per_hour 每小时转交次数；handoff_cooldown_ms 同一话题两次转交的最短间隔 |
| triage.sizes | session_gap_ms 同一会话内相邻消息间隔超过它即另起一段；context_max_chars 单条消息与提示词转写的字数上限；label_chars 话题标签的字数；topics_max_open 一次最多列出多少个开放话题供归并，与 batch 之和至多 254；session_max_ms 话题空闲多久后关闭、codex 会话最长沿用多久；result_max_chars 分析结果字数上限 |
| triage.redact | patterns 是按序执行的脱敏规则（pattern 为正则，placeholder 为替换文本，可含 `${1}`）；unknown_sender 是没有发送人名时的写法；任一正则无效则 `serve` 以 sandbox_exit_code 退出 |
| triage.review | mode 为 manual（逐条确认后才外发）或 auto（自动通过）；learn_min_chars 粘贴的漏脱敏原文至少多长；reasons 拒绝原因（code 与 label；带 placeholder 的是漏脱敏类，拒绝时须粘贴漏掉的原文；不带的拒绝即排除） |
| triage.session | prompt 第一次发话题转写的提示词模板；followup 同一话题后续只发新增消息的模板（`{messages}` 处填入消息）；fresh 新会话追加在 codex 参数后的参数；resume 续写追加的参数（`{thread}` 换成会话 ID） |
| triage.notice | 发给本人的通知模板（`{probability}`、`{topic}`、`{analysis}`） |
| triage.decide / codex / notify / screen.exec | 外部命令，共同字段：bin、args、env（字面量环境变量）、vars（`${NAME}` 写法，运行时读同名环境变量传给子进程；args 里的 `${NAME}` 若在 vars 里声明则换成其值）、sandbox、timeout_ms；子进程环境先清空再只放 env 与 vars |
| triage.codex.args | 须含 `--json`，不含 `--ephemeral` 与结尾的 `-` |
| triage.notify.args | 须含 `--yes` 并以接收人参数名结尾；程序追加 `notify_target` 与 `--idempotency-key=<话题>@<批号>` |
| triage.self_sender_id / notify_conversation_id / notify_target | 本人的发送人 ID、通知所在会话 ID、通知接收人，均写成 `${NAME}`；前两者对应的消息不参与决策 |
| triage.screen | 可选，见“本地模型筛查”：exec 外部命令；instructions 系统提示词；model 模型名；placeholder 遮盖用的占位符 |

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

## HTTP 调用

`POST /<feature>/<action>`，请求体为 JSON，响应头 `content-type: application/json`。

| 请求头 | 说明 |
|---|---|
| x-session-id | 会话 ID，可选，写入日志 |
| x-user | 用户标识，可选，写入日志 |

部署要求：两个请求头由前置网关鉴权后写入，网关须剥离客户端自带的同名头。页面与全部接口都须置于网关认证之后。

```sh
curl -H 'x-session-id: s1' -H 'x-user: alice' -d '{}' http://127.0.0.1:15801/health/check
# {"status":"ok"}
```

## 接口结构

请求与响应结构以生成的接口说明 `api/openapi.json`（OpenAPI 3.1）为准，也可用 `cli spec` 输出到 stdout。

## 启动服务

```sh
./target/release/cli serve --config cli/config.yaml
```

启动后 stderr 输出 `listening` 日志，含实际地址（log_level 为 info 或更详细时可见）。同一进程还会收集钉钉消息（须先 `dws auth login`），写入 `ingest.store_path`，页面与接口读取同一份库；收集遇到须人工处理或存储失败时，服务以对应退出码退出。

停机：Ctrl+C（SIGINT）或 SIGTERM。不再接受新连接，在途请求完成、钉钉工具正常退出后以退出码 0 退出；超过 shutdown_timeout_ms 仍未完成则以 shutdown_timeout_exit_code 强制退出，在途连接被重置。停机期间再次发信号无效。

## 只收集消息

```sh
./target/release/cli ingest --config cli/config.yaml
```

只收集、不提供页面与接口时用本命令，收集行为与 `serve` 相同。停机时让钉钉工具正常退出后以退出码 0 结束，超过 child_exit_timeout_ms 则强杀。

## 查看消息

```sh
cd web && bun install --frozen-lockfile && bun run build && cd ..
./target/release/cli serve --config cli/config.yaml
```

浏览器打开服务地址（如 `http://127.0.0.1:15801/`）。

- 取数：工具栏的“全部／1 小时／今天／7 天”或自定义起止限定时间范围；“刷新”重取最新一页（保留筛选与排序），“加载更多”取更早的消息，“加载全部”一直取到取完或达到上限
- 排序：点表头依次升序、降序、恢复默认
- 筛选：每列有筛选框，多个词须全部命中；数字列另支持 `>5`、`<=3`、`1..10`；词两端加英文双引号（如 `"李明"`）则整格精确匹配；工具栏的搜索框跨所有字段；列标题上的“×”只清除该列的筛选
- 点选：点一行的会话或发送人，只看该会话或该发送人的消息，再点一次移除
- 列：“列”菜单可显示默认隐藏的列
- 正文：左侧“复制”按钮复制完整原文；点击一行（或键盘 Enter）打开详情；手机上表头收进“筛选”面板

只读已有的库、不连钉钉时用 `cli call messages list --config cli/config.yaml --json '{}'`。

## 消息分级与转交

由 `triage.mode` 开关。开启后 `serve` 同进程处理库里未处理的消息：规则脱敏 → 本地模型筛查（可选）→ 外发审核 → 决策接口 → codex 分析 → 钉钉通知本人。规则见 docs/requirements.md。关闭时无需 sops，照常 `serve`。

### 准备

1. 取得员工授权与企业管理员同意：脱敏后的消息内容会发往决策接口与 codex。
2. 用 sops 保存变量：在项目根执行 `sops secrets/dev.enc.yaml`（开发用）或 `sops secrets/prod.enc.yaml`（release 用），写入扁平 YAML。加密规则取自上级目录的 `.sops.yaml`，私钥在 `~/.config/sops/age/keys.txt`。

| 变量 | 含义 |
|---|---|
| DECISIONS_URL | 决策接口的完整地址 |
| OPENAI_API_KEY | 决策接口的 API key |
| CODEX_API_KEY | codex 专用的 API key |
| CODEX_BASE_URL | codex 模型接口的完整地址（如 `https://<主机>/v1`，Responses 协议，端口须为 443）；展开进 codex 参数，`ps` 里本机可见 |
| SELF_SENDER_ID | 本人的发送人 ID（消息里的 `sender_id`） |
| NOTIFY_CONVERSATION_ID | 通知所在会话的 ID（消息里的 `conversation_id`） |
| NOTIFY_TARGET | 通知接收人的 openDingTalkId（本人） |

`SELF_SENDER_ID`、`NOTIFY_CONVERSATION_ID` 填错会把本人消息外发，通知还会触发新一轮分级；首次开启后在审核页确认本人消息与通知没有出现。

3. 建 codex 专用目录：`mkdir -p .debug/codex-home .debug/codex-work`
4. 把 `cli/config.yaml` 的 `triage.mode` 改为 `enabled`

### 启动

```sh
sops exec-env secrets/prod.enc.yaml './target/release/cli serve --config cli/config.yaml'
```

变量只经环境变量传给子进程，不出现在命令行参数与日志里；缺任一变量，`serve` 以退出码 2 退出并列出缺的变量名。首次开启会把 `lookback_ms` 内的未处理消息全部送进审核。codex 的会话文件落在 `.debug/codex-home/sessions`，含脱敏后的对话，不自动清理。

### 外发审核

浏览器打开 `<服务地址>/#reviews`，逐条通过或拒绝，顶部“整批通过”一次通过当前列出的全部。拒绝须选原因；漏脱敏类原因还须粘贴漏掉的原文，系统据此学出规则并让该条重新脱敏后回到待审。一批不审完，不会外发，也不开下一批。`review.mode: auto` 时不经人工。

接口：`POST /review/list`、`/review/approve`、`/review/reject`，命令行同样可用 `cli call review …`。试脱敏规则：`cli call triage redact --config cli/config.yaml --json '{"text":"…","rules":[…]}'`。

### 本地模型筛查

可选，不配 `triage.screen` 即不启用。本机自行下载模型权重并启动 `llama-server`，`exec` 用 curl 调其 OpenAI 兼容接口：`exec.bin: curl`，`exec.args` 带 `-q -sS --fail-with-body --max-time 30 -H "Content-Type: application/json" --data-binary @- http://localhost:8080/v1/chat/completions`，沙箱的 `profile` 指向 `cli/sandbox/screen.sb`。模型不可达或返回无法解析时，该批一直重试、不外发。

### 关闭与停转交

- 关闭：`triage.mode` 改回 `disabled` 并重启，收集照常
- 只停转交、照常打分：`triage.questions.threshold` 设为大于 1 并重启
- 不要清库里的表：会让消息重新外发、通知被当成重复而吞掉、学到的规则丢失

## 子进程沙箱

钉钉工具 `dws`、决策用 `curl`、`codex`、本地模型筛查的 `curl` 都经 `/usr/bin/sandbox-exec` 启动，策略为 `deny default`，只放行各自必需的文件、网络端口与系统服务；没有关闭沙箱的配置。策略文件在 `cli/sandbox/`（`dws.sb`、`curl.sb`、`codex.sb`、`screen.sb`），由各命令的 `sandbox` 引用：

| 字段 | 说明 |
|---|---|
| bin | 沙箱程序，`/usr/bin/sandbox-exec` |
| profile | 策略文件，相对路径按启动目录 |
| paths | 路径参数（名到路径），先转成真实路径传给策略；路径不存在则沙箱拒绝启动 |
| vars | `${NAME}` 写法的环境变量作参数（如 `HOME`）；只放路径类的值，参数在 `ps` 里可见，不放 secret |
| probe | 自检时试跑的参数，如 `[--help]` |
| canary | 自检时策略必须禁止执行的程序，如 `/usr/bin/true` |

启动自检：`serve`、`ingest` 启动时，对每个会运行的命令先试跑 `canary`（须被拒）与 `bin` 加 `probe`（须退出 0），任一失败即以 `sandbox_exit_code` 退出，并在 stderr 写明哪条命令、什么原因。自检只证明能启动、禁令生效，不证明 Keychain、DNS、TLS 的放行规则齐全。

排错：

- 沙箱退出 65：路径无法解析（目录或策略文件不存在）；退出 71：命令不存在。都按命令失败处理，不会不加沙箱运行；升级 dws、codex、curl 后真实路径变化，自检同样会失败
- codex 启动即退出 71：把 codex 参数里的 `-s read-only` 改为 `-s danger-full-access`，由外层沙箱把关
- 规则不全时命令失败。另开终端看拒绝记录：`log stream --style compact --predicate 'sender == "Sandbox"'`，按 `deny(1) <操作> <对象>` 逐条补进对应 `.sb`，再复测：`/usr/bin/sandbox-exec -f cli/sandbox/dws.sb -D HOME=$HOME -- dws --help`
- `dws auth login` 在沙箱外执行
- 不要预建 `/tmp/dws-event-<uid>/`，由 dws 自己建，权限不对会令监听失败

## 返回码

业务错误响应体为 `{"code":"<code>"}`，code 与退出码取自配置，下表数值为 `cli/config.yaml` 的取值。

| 情形 | HTTP | 退出码 | 输出 |
|---|---|---|---|
| 成功 | 200 | 0 | JSON |
| 请求内容无效 | 400 | 3 | `{"code":"bad_request"}` |
| 功能或动作不存在 | 404 | 4 | `{"code":"unknown_action"}` |
| 消息库不存在或读取失败 | 503 | 10 | `{"code":"storage"}` |
| `--help` | — | 0 | 帮助写 stdout |
| 命令行参数错误 | — | 2 | 错误写 stderr |
| 配置错误（文件不存在、YAML 无效、缺少或未知字段、取值无效） | — | 2 | stderr 输出 `路径: 原因` |
| triage 开启而缺环境变量 | — | 2 | stderr 列出变量名 |
| 启动时沙箱自检或 triage 配置校验失败 | — | sandbox_exit_code | 错误写 stderr |
| 服务地址绑定失败 | — | serve_exit_code | 错误写 stderr |
| 停机等待超时 | — | shutdown_timeout_exit_code | 日志写 stderr |
| 收集时存储失败 | — | ingest.failure_exit_code | 错误写 stderr 日志 |
| 收集时钉钉工具须人工处理 | — | ingest.manual_exit_code | 错误写 stderr 日志 |
| 写 stdout 失败 | — | 至少 1 | — |
| HTTP 路径不是 `/`、`/<文件>`、`/<feature>/<action>` | 404 | — | 空响应体 |
| 页面文件不存在，或文件名含字母、数字、`.`、`_`、`-` 以外的字符，或以 `.` 开头 | 404 | — | 空响应体 |
| 页面路径用 GET 以外的方法，或接口路径用 POST 以外的方法 | 405 | — | 空响应体 |
| HTTP 请求体不是 UTF-8 | 400 | — | 纯文本 |

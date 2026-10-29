# Poly Resolve

通过命令行或 HTTP 调用业务功能。功能与规则见 [docs/requirements.md](docs/requirements.md)。

## 前置条件

- macOS（外部命令用系统自带的 `/usr/bin/sandbox-exec` 隔离，见“子进程沙箱”）
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
| sandbox_exit_code | `serve`、`ingest` 启动时沙箱自检失败的退出码，1 至 255 |
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
| ingest.dws.env / vars | 传给钉钉工具的环境变量：env 为字面量，应含 DO_NOT_TRACK 以关闭其遥测；vars 为 `${NAME}` 写法，运行时读同名环境变量（HOME、PATH 等）；子进程环境先清空再只放这两类 |
| ingest.dws.decrypt / listen / lookup / pull 与 triage.decide / codex / notify 的 sandbox | 子进程沙箱，必填，字段见“子进程沙箱” |
| ingest.dws.listen / pull | 实时监听与补拉所用命令：bin 为可执行文件，args 为参数；补拉自动追加 --start、--end |
| ingest.cipher_min_length | 判定三方密文用：去掉末尾 `||版本||类型||长度` 后的 base64 正文至少多少字符，至少 1 |
| ingest.dws.decrypt | 解密所用命令：bin 为可执行文件，args 为参数；自动追加 `--text=<密文>`，标准输出须为含 `plaintext` 的 JSON |
| ingest.heal_limit | 每次启动时最多重新解密多少条库里的密文消息，至少 1 |
| ingest.lookup_batch | 一次向钉钉工具查询转发消息详情的消息数，至少 1（钉钉工具上限 50） |
| ingest.dws.lookup | 查询消息详情所用命令：bin 为可执行文件，args 为参数；自动追加 `--msg-ids=<消息 ID,…>`，标准输出须为含 `messages[].forwarded[]` 的 JSON |
| ingest.dws.manual_exit_codes | 钉钉工具退出码落在此列表时视为须人工处理 |
| triage.mode | disabled 或 enabled；enabled 时 `serve` 启动即检查所需环境变量，缺失则以退出码 2 退出 |
| triage.model / questions | 决策接口的模型名；questions.urgent 是每条消息一问的优先级问题（`{line}` 换成消息别名），questions.topic 是每个新会话段一问的话题归属问题（`{segment}` 换成段别名，`min_confidence` 低于它按新话题，`new_label` 是“新话题”选项的说明），questions.threshold 是概率不低于它即高优先级 |
| triage.limits | batch 每批最多消息条数（≤ 200，决策请求的问题数不超过 400）；intake 每轮最多入库的新消息条数；poll_ms 轮询间隔；lookback_ms 只处理多久以内的消息；max_attempts / retry_after_ms 失败重试次数与间隔；max_decisions_per_hour 每小时决策请求数；max_handoffs_per_hour 每小时转交次数；handoff_cooldown_ms 同一话题两次转交的最短间隔 |
| triage.sizes | session_gap_ms 同一会话内相邻消息间隔超过它即另起一段（话题的延续也按此判断）；context_max_chars 单条消息与提示词转写的字数上限；label_chars 话题标签（取首条消息）的字数；topics_max_open 一次最多列出多少个开放话题供归并（≤ 200）；session_max_ms 话题空闲多久后关闭、codex 会话最长沿用多久；result_max_chars 分析结果字数上限 |
| triage.redact | 外发前的脱敏：patterns 是按序执行的规则（pattern 为正则，placeholder 为替换文本，可含 `${1}`），unknown_sender 是没有发送人名时的写法；任一正则无效则 `serve` 以 sandbox_exit_code 退出，决策与分析均不外发 |
| triage.review | 外发前的人工审核：mode 为 manual（逐条确认后才外发）或 auto（自动通过）；learn_min_chars 粘贴的漏脱敏原文至少多长；reasons 拒绝时可选的原因（code 与 label；带 placeholder 的是漏脱敏类，须粘贴漏掉的原文，系统据此学出字面量规则，占位符取该原因的 placeholder；不带的拒绝即排除，该条不外发） |
| triage.session | codex 会话：prompt 第一次发话题转写的提示词模板、followup 同一话题后续只发新增消息的模板（`{messages}` 处填入消息 JSON）；fresh 新会话追加在 codex 参数后的参数；resume 续写追加的参数（`{thread}` 换成会话 ID） |
| triage.screen | 可选，缺省不启用：规则脱敏之后、人工审核之前，调本机小模型再筛一遍。exec 是外部命令（用法同 decide，一般是 `curl` 调本机 `llama-server` 的 OpenAI 兼容接口，沙箱用 `cli/sandbox/screen.sb`，只放行本机端口），instructions 是系统提示词，model 是模型名，placeholder 是遮盖用的占位符。模型只能多遮、不能少遮，也不替代人工审核 |
| triage.notice | 发给本人的通知模板（`{probability}`、`{topic}`、`{analysis}`） |
| triage.decide / codex / notify | 三个外部命令（codex 的 args 须含 `--json`、不含 `--ephemeral` 与结尾的 `-`）：bin、args、env（字面量环境变量）、sandbox、vars（`${NAME}` 写法，运行时读同名环境变量传给子进程，不设默认值；args 里的 `${NAME}` 若在 vars 里声明，则换成其值，其余原样保留）、timeout_ms；子进程环境先清空再只放 env 与 vars；notify 的 args 须含 `--yes`（dws 发消息默认要交互确认）并以接收人参数名结尾，程序追加 `notify_target` 与 `--idempotency-key=<话题>@<批号>` |
| triage.self_sender_id / notify_conversation_id / notify_target | 本人的发送人 ID、通知所在会话 ID、通知接收人，均写成 `${NAME}`；前两者对应的消息不参与决策 |
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

实时事件的正文可能是三方密文（单聊、群聊都有），格式为多行 base64 加末尾 `||版本||类型||长度`，与钉钉工具自身的判定一致；判为密文的入库前自动调用钉钉工具解密，普通文字（含 `||` 也不算）不会触发；解密失败则按密文入库并记 warn（日志不含正文）。合并转发消息的正文里，被加密的子消息只留有截断的密文片段（整行都是 base64）；遇到这种消息，入库前向钉钉工具查询其详情，把正文重组为“标题行 + 每条子消息的 发送人:解密后内容”；查询失败则保持原样并记 debug。补拉得到的消息同样先判断、再解密；补拉得到的明文不会覆盖已入库的记录。每次启动（补拉之后）还会从库里挑出最新的至多 `ingest.heal_limit` 条密文消息，重新解密并原地更新，所以此前解密失败而按密文入库的消息，重启或断线重连后会自动恢复；仍失败的留待下次。库里已有的密文片段（转发消息）仍需删库后重新收集。

Ctrl+C（SIGINT）或 SIGTERM 触发停机：让钉钉工具正常退出后以退出码 0 结束，超过 child_exit_timeout_ms 则强杀。日志同样写 stderr，不含消息正文。

## 查看消息

```sh
cd web && bun install --frozen-lockfile && bun run build
./target/release/cli serve --config cli/config.yaml
```

浏览器打开服务地址（如 `http://127.0.0.1:15801/`），深色表格按时间从新到旧显示 `ingest.account` 的消息，桌面与手机都可用。

- 取数：工具栏的“全部／1 小时／今天／7 天”或自定义起止（按浏览器时区）限定时间范围，范围由服务端筛选；“刷新”重取最新一页（保留筛选与排序），“加载更多”取更早的消息，“加载全部”一直取到取完或 2000 条上限
- 排序与筛选：点表头依次升序、降序、恢复默认；每列有筛选框（多个词全部命中，数字列另支持 `>5`、`<=3`、`1..10`，词两端加英文双引号如 `"李明"` 则整格精确匹配），工具栏的搜索框跨所有字段；点击一行的会话或发送人，对应列的筛选框自动填入精确匹配词，只看该会话或该发送人的消息（两者可叠加，其余筛选与搜索保留），再点一次移除该筛选；某列有筛选时，其标题上出现“×”，点击只清除该列的筛选；它们只作用于已加载的行，状态栏会写明“显示 N／已加载 M 条”以及是否还有更早的消息未加载；消息接口新增字段时，页面自动多出对应的列，可排序可筛选，“列”菜单可显示默认隐藏的列（key、接收时间、发送人 ID、来源、账号）
- 查看全文：表格里每条正文压成单行，左侧有“复制”按钮，复制该条完整正文原文（不开详情，空正文没有该按钮）；鼠标悬停被截断的单元格可看完整内容，点击（或键盘 Enter）一行打开详情（点会话、发送人是筛选，不开详情），详情里渲染正文的 Markdown 并列出全部字段；手机上每行折成两行，表头收进“筛选”面板
- Markdown：渲染标题、引用、围栏代码块、粗体、行内代码与 `[标题](地址)` 链接（只认 http、https，新标签页打开，悬停提示只含主机与路径，不含地址里的查询参数），其余写法（列表、表格、图片、HTML）按原文显示
- 时间按浏览器所在时区显示并标注偏移（如 `2026-10-28 09:20:09 UTC+8`）

消息来自 `ingest.store_path`，由 `serve` 同进程收集写入；只读已有的库、不连钉钉时用 `cli call messages list`。库不存在或读取失败时返回 503 与 `storage`。

接口 `POST /messages/list`（命令行 `cli call messages list`）：

| 字段 | 说明 |
|---|---|
| cursor | 可选，上一页响应的 next_cursor；缺省取最新一页 |
| limit | 可选，每页条数，1 至 messages.max，缺省取 messages.default |

响应含 `items`（不含原始内容）与 `next_cursor`（没有更早的消息时为 null）。

## 消息分级与转交

默认关闭（`triage.mode: disabled`）。开启后 `serve` 同进程把库里全部未处理的消息逐条脱敏、经人工逐条审核，成批发给决策接口：每条消息得一个优先级，每个新会话段归入已有话题或另起话题，话题与 codex 会话一一对应；高优先级的话题交给 `codex exec` 只读分析，结果存库（页面“优先级”“话题”列，详情里的“分析”）并经钉钉发给本人。消息内容会发往决策接口与 codex，须先取得授权。关闭时无需 sops，照常 `serve`。

流水线：程序规则脱敏 → 本地小模型筛查（可选，`triage.screen`）→ 人工逐条审核（`review.mode: auto` 则跳过）→ 决策接口（优先级与话题）→ codex 分析 → 通知。

本地小模型：不配 `triage.screen` 就整段跳过；配了之后，每条入批的消息先交给它判断是否还有密钥并给出话题候选。它返回的片段必须是该条文字的子串且不短于 `learn_min_chars` 才会被遮盖；判为高风险却给不出可遮盖的片段，该条打上提示——`manual` 时在审核页置顶并标出，`auto` 时直接排除（原因 `screen`）；话题候选只用来缩小决策接口的选项范围。模型不可达、返回无法解析时按 `retry_after_ms` 重试，期间不放行任何一条。本机需自己下载模型权重并启动 `llama-server`，配置示例：`exec.bin: curl`、`exec.args` 里带 `-q -sS --fail-with-body --max-time 30 -H "Content-Type: application/json" --data-binary @- http://localhost:8080/v1/chat/completions`，沙箱的 `profile` 指向 `cli/sandbox/screen.sb`。

准备：

1. 用 sops 保存变量：在项目根执行 `sops secrets.enc.yaml`（文件名须以 `.enc.yaml` 结尾，加密规则取自上级目录的 `.sops.yaml`；已有私钥 `~/.config/sops/age/keys.txt`），写入扁平 YAML：

| 变量 | 含义 |
|---|---|
| DECISIONS_URL | 决策接口的完整地址 |
| OPENAI_API_KEY | 决策接口的 API key |
| CODEX_API_KEY | codex 专用的 API key |
| CODEX_BASE_URL | codex 模型接口的完整地址（如 `https://<主机>/v1`，Responses 协议，端口须为 443）；展开进 codex 参数，`ps` 里本机可见 |
| SELF_SENDER_ID | 本人的发送人 ID（消息里的 `sender_id`） |
| NOTIFY_CONVERSATION_ID | 通知所在会话的 ID（消息里的 `conversation_id`） |
| NOTIFY_TARGET | 通知接收人的 openDingTalkId（本人） |

2. 建 codex 专用目录：`mkdir -p .debug/codex-home .debug/codex-work`
3. 把 `cli/config.yaml` 的 `triage.mode` 改为 `enabled`

启动：

```sh
sops exec-env secrets.enc.yaml './target/release/cli serve --config cli/config.yaml'
```

变量只经环境变量传给子进程，不出现在命令行参数与日志里；开启后缺任一变量，`serve` 以退出码 2 退出并列出缺的变量名。

行为：

- 全部未处理的消息都会入库：本人发出的、发往通知会话的、正文为空的、仍是密文的只记为已跳过，其余进入审核；每批取最新的至多 `batch` 条（从新到旧），上一批没处理完不开下一批，新到的消息等下一批
- 脱敏：发送人只写名字（无名写 `unknown_sender`），不外发发送人 ID、会话 ID 与消息键（请求里只用 `C1`、`L3`、`S2` 这样的别名）；按 `triage.redact.patterns` 把密钥与凭据（私钥整块、各平台令牌、Bearer 与 Cookie、连接串、“密码”“口令”“验证码”等关键词后的值、长随机串）、所有链接与地址、文件路径与密钥文件名、手机号、身份证、银行卡、邮箱换成占位符；人名、公司名不处理；不处理全角变体、拆在两条消息里的密钥与被编码包裹的密钥，脱敏只降低暴露，不替代授权；`cli call triage redact` 可用来试规则，`cli/tests/it/redact.rs` 的样例表防回归
- 审核：每条脱敏后的消息先冻结入库，页面 `#reviews` 里逐条通过或拒绝（顶部“整批通过”一次通过当前列出的全部）；一批里没有待审的行才外发，只外发已通过的；拒绝必须选原因，漏脱敏类原因还须粘贴漏掉的原文，系统学出字面量规则（存入 `learned_rules`，不经接口与日志返回），该条用新规则重新脱敏后回到待审，之后每条消息外发前都再套一遍全部规则；其余原因即排除；审核没有期限，一批不审完就一直等，不丢消息；`mode: auto` 时不经人工、记审核人为 auto
- 决策：一个请求里每条消息一问优先级（概率不低于 `questions.threshold` 即高优先级）；每个新会话段再一问话题归属，选项是开放话题（取首条消息的脱敏摘录作说明）、本批更早的段、“新话题”；同一会话在 `session_gap_ms` 内已有开放话题的，直接并入，不提问；答案缺失、被拒答、置信度不足、选项无效都当新话题；话题取成员的最高概率
- 转交：高优先级话题按话题合并，`handoff_cooldown_ms` 内同一话题只转交一次；codex 只读（关闭其全部工具、专用 `CODEX_HOME` 与空工作目录，并在沙箱内运行），提示词只含已通过审核的文本；同一话题的后续分析经 `codex exec resume` 续写同一会话，只发新增消息，话题空闲超过 `session_max_ms` 则新建；续写失败即丢弃该话题的会话记录；会话文件落在 `.debug/codex-home/sessions`，不自动清理；通知发往 `notify_target`，带幂等标识，重试不会重复送达
- 失败按 `max_attempts` 与 `retry_after_ms` 重试，用尽后留为失败；决策或转交失败不影响收集，也不使服务退出
- 库里 `lines`（逐条的脱敏文本与审核状态）、`batches`（冻结的决策请求）、`placements`（消息到话题与概率）、`topics`（话题与 codex 会话）、`alerts`（转交）、`learned_rules`（学到的规则）记录全部状态；`lines.reason` 可按原因统计以优化规则；日志不含消息正文、提示词与分析结果；旧版的 `decisions`、`handoffs`、`reviews`、`threads` 表不再使用，可手动删除

审核接口 `POST /review/list`（`{limit?, state?}`，缺省列待审核）、`/review/approve`（`{keys}`）、`/review/reject`（`{key, reason, leak?}`）；已不是待审核的项再操作时原样返回当前状态；命令行同样可用 `cli call review …`。

试跑建议：先保持关闭，用 `sops exec-env secrets.enc.yaml 'curl …'` 对合成文本调一次决策接口（含一个 `choice` 问题），核对返回里 `answers[]` 的 `probability`、`choice`、`confidence`；再开启（`review.mode: manual`）并把 `questions.threshold` 临时设为大于 1，只打分不转交，在审核页逐条检查外发内容、对漏脱敏的选原因拒绝并粘贴原文，同时观察话题归并是否合理、调 `min_confidence`；然后用合成的提示注入消息验证 codex 不读文件、不外传，并先用 `dws … --dry-run` 确认通知能发到本人；首次开启前先手跑一次 `codex exec --json` 与 `codex exec resume <会话 ID>`，确认事件格式与 `-s`、`-C` 对续写同样生效。每小时限额起初设小。

## 子进程沙箱

采集用的 `dws`，分级用的 `curl`、`codex` 与发送通知的 `dws`，都经 macOS 自带的 `/usr/bin/sandbox-exec` 启动，策略为 `deny default`，只放行各自必需的文件、网络端口与系统服务；没有关闭沙箱的配置。策略文件在 `cli/sandbox/`（`dws.sb`、`curl.sb`、`codex.sb`），由各命令的 `sandbox` 引用：

| 字段 | 说明 |
|---|---|
| bin | 沙箱程序，`/usr/bin/sandbox-exec` |
| profile | 策略文件，相对路径按启动目录 |
| paths | 路径参数（名到路径），先转成真实路径传给策略；路径不存在则沙箱拒绝启动 |
| vars | `${NAME}` 写法的环境变量作参数（如 `HOME`）；只放路径类的值，参数在 `ps` 里可见，不放 secret |
| probe | 自检时试跑的参数，如 `[--help]` |
| canary | 自检时策略必须禁止执行的程序，如 `/usr/bin/true` |

- 启动自检：`serve`、`ingest` 启动时，对每个会运行的命令先试跑 `canary`（须被拒）与 `bin` 加 `probe`（须退出 0），任一失败即以 `sandbox_exit_code` 退出并在 stderr 写明哪条命令、什么原因；`triage.mode: enabled` 时同样检查三个 triage 命令。自检只证明能启动、禁令生效，不证明 Keychain、DNS、TLS 的放行规则齐全
- 路径无法解析（目录或策略文件不存在）时 `sandbox-exec` 退出 65，命令不存在时退出 71，都按命令失败处理，不会不加沙箱运行；codex 的目录须先建好：`mkdir -p .debug/codex-home .debug/codex-work`
- `dws auth login` 在沙箱外执行
- 只能有一层沙箱：codex 自带的 `-s read-only` 嵌套在外层沙箱内可能以 71 失败，现保留作纵深防御（codex 已关闭全部工具，一般不会触发）；若 codex 启动即 71，把 `-s` 改为 `danger-full-access`，由外层沙箱唯一把关
- 已在真实环境验证：采集用的 `dws.sb`（监听订阅、补拉、解密、转发查询）；`codex.sb`（新建、`resume` 续写、只读与 `-C` 生效；TLS 校验需 `com.apple.SecurityServer`，启动需读项目根目录本身）；`curl.sb` 能连上决策接口；发送通知的 `dws`（`--yes` 真发、同一幂等键重发被拒为 repeated、通知经补拉回库）
- 事件总线：`dws` 监听时自己在 `/tmp/dws-event-<uid>/` 建 0700 目录与 socket，不要预先建这个目录，权限不对会令监听失败
- 收敛策略：真实环境下规则可能不全（Keychain、DNS、TLS 用到的系统服务名）。另开终端看拒绝记录：`log stream --style compact --predicate 'sender == "Sandbox"'`，按 `deny(1) <操作> <对象>` 逐条补进对应 `.sb`，再用 `/usr/bin/sandbox-exec -f cli/sandbox/dws.sb -D HOME=$HOME -- dws --help` 复测
- 残余风险：网络只能按端口限制，443 可连任意主机；沙箱内的 `dws` 仍能读 Keychain 与 `~/.dws`；基础策略放行全局文件元数据读取；`sandbox-exec` 已被系统标为弃用；升级 dws、codex、curl 后若真实路径变化，自检会失败而非裸跑

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
| 启动时沙箱自检失败 | — | sandbox_exit_code | 错误写 stderr |
| 服务地址绑定失败 | — | serve_exit_code | 错误写 stderr |
| 停机等待超时 | — | shutdown_timeout_exit_code | 日志写 stderr |
| 收集（`ingest` 或 `serve`）时存储失败 | — | ingest.failure_exit_code | 错误写 stderr 日志 |
| 收集（`ingest` 或 `serve`）时钉钉工具须人工处理 | — | ingest.manual_exit_code | 错误写 stderr 日志 |
| 写 stdout 失败 | — | 至少 1 | — |
| HTTP 路径不是 `/`、`/<文件>`、`/<feature>/<action>` | 404 | — | 空响应体 |
| 页面文件不存在或文件名含字母、数字、`.`、`_`、`-` 以外的字符或以 `.` 开头 | 404 | — | 空响应体 |
| 页面路径用 GET 以外的方法，或接口路径用 POST 以外的方法 | 405 | — | 空响应体 |
| HTTP 请求体不是 UTF-8 | 400 | — | 纯文本 |

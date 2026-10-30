# domain

## 职责

- 业务逻辑；不依赖传输层（axum、clap、tokio 等），tracing-subscriber 只作 dev-dependency

## 调用

- 业务只经 `features::dispatch(&Context, &Call)` 调用；`Call` 由 `Origin`（transport、session_id、user）、`Target`（feature、action）与 request（JSON 请求体）经各自的 `new` 构造，HTTP 的路径与查询参数经 `with_params` 以 urlencoded 字符串传入
- `Context::new(errors, Rules, Ports)` 由调用方（cli、adapters）构造：`Rules::new(&Paging, &[Reason], learn_min_chars)`；`Ports::new(reader, docket)`，`reader` 是 `inbox::Reader`（存储读取），`docket` 是 `review::Docket`（审核台读写），均由 adapters 实现
- handler 包装：`typed`（`fn(Req) -> Resp`）、`fallible`（`fn(&Req) -> Result`）解 JSON 请求体，供内部动作；`listed`（`fn(&Params, &Context) -> Result`）只解 params；`keyed`（`fn(&Params, Req, &Context) -> Result`）解 params 与请求体；JSON 共用 `decode`，urlencoded 共用 `params`（`serde_urlencoded`）；health 不解码；`Route.call` 为 `fn(&Call, &Context)`，不需要上下文的路由闭包忽略第二个参数
- `route` 的 `&&` 两侧都要有用例：功能不存在、功能存在而动作不存在、两者都存在
- `catalog::build` 供 cli 拼路由与 OpenAPI：只列 `Http::Bound(方法, 路径模板, 描述)` 的路由，`Entry` 给出 `method`、`path`、`params`、`body`、`response`；描述函数 `bare`（无参数无请求体）、`asked`（只有 params）、`changed`（params 加请求体）；schema 引用路径为 `/components/schemas`，错误体 `ErrorBody` 取自 `Body`

## 新功能

- 新建 `src/features/<f>.rs`
- `features.rs` 加 `mod <f>;`，每个 action 在 `ROUTES` 加一项：`call` 按签名选包装，`http` 对外为 `Http::Bound("get"|"patch", "/api/<资源>", 描述)`、内部为 `Http::Hidden`；路由与接口说明共用此表；`features.rs` 已近 512 行，新增功能时把 `ROUTES` 拆到子模块
- `ROUTES` 项另有 `failures`：该路由可能返回的全部错误（含 `BadRequest`，如 `messages.list` 为 `[BadRequest, Storage]`），供接口说明声明状态码，须与实际一致
- 含消息内容的路由标 `Trace::Redacted`
- 自定义类型的 Req、Resp 派生 `JsonSchema`，并加 `#[schemars(rename = "<Feature><Action>Request")]`（Resp 同理）避免跨功能重名
- 对外整数可能超过 2^53 时不用 i64（生成的 TS 为 number）
- 新建 `tests/it/features/<f>.rs`，`tests/it/features.rs` 加 `mod <f>;`；`tests/it/catalog.rs` 补条目与 schema 断言
- 跑 `bun scripts/gen.ts` 更新 `api/openapi.json` 与 `web/src/api.gen.ts`
- 更新 docs/requirements.md 的业务规则
- 包装、`decode`、`params`、描述函数都是泛型（见根“测试与覆盖率”），故每个请求类型至少有一条格式错误的测试（参数与请求体分别测），绑定路由的描述函数由 `catalog::build` 走到；Hidden 路由不生成 schema

## 约定

- `alloc::collections` 优先于 `std`（`std_instead_of_alloc`），故 `lib.rs` 须 `extern crate alloc`
- `ingest::clock`、`ingest/cipher::sealed` 对 `features` 内可见，供 triage 复用

## ingest

- `ingest` feature 是钉钉格式；新数据源另建 feature，产出同样的 envelope
- `parse_event`（钉钉实时事件一行）、`parse_page`（补拉一页）→ `IngestEnvelope`，请求共用 `IngestParseRequest`：account、source、body、received_at_ms（调用方给，保持纯函数）、utc_offset_minutes、cipher_min_length；`window` 把毫秒区间转成补拉用的 `YYYY-MM-DD HH:MM:SS`（按偏移）
- 字段名按候选列表取首个有值的（`pick`），不用 serde alias：补拉消息可同时含 `messageId` 与 `openMessageId`，alias 会报重复字段
- 时间 `create_time` 或 `createTime`：数字当 UTC 毫秒，字符串先试 RFC3339，再试无时区的 `%Y-%m-%d %H:%M:%S`（按偏移换算）；早于 1970 取 0
- 补拉一页内无法解析的消息计入 `skipped`，不使整页失败；`next_cursor` 仅在 `hasMore` 为真时给出（服务端取完后仍会返回游标，不能只看游标），非空表示未取完
- `key` 为 `<source>:<account>:<message_id>`，实时与补拉共用，供存储去重
- `cipher`（`typed`）：`{envelope, min_length}` → `{cipher}`（是密文则为 `text`，否则 null）；规则同 dws 的 `IsEncrypted`（`ingest/cipher.rs`）：trim 后以 `||版本||类型||长度` 结尾（三段全是数字、非空），其余部分 trim 后不短于 `min_length` 且只含字母数字与 `+/=` 和空白
- `reveal`（`typed`）：`{envelope, plaintext}` → envelope，仅把 `text` 换成 `plaintext`；如何解密由 adapters 决定
- `fragments`（`ingest/cipher.rs`）：正文任一行（trim 后）整行只含字母数字与 `+/=` 且不短于 `cipher_min_length`，即视为留有密文片段；`parse_event`、`parse_page` 据此给 envelope 填 `lookup`（消息 ID，否则空）
- `assemble`（`typed`）：`{envelope, children: [{sender, text}]}`，正文重组为“原正文首行 + 每条子消息的 `sender:text`”，`lookup` 置空；子消息由 adapters 向钉钉工具查询
- `cipher`、`reveal`、`assemble` 标 `Trace::Redacted`

## messages

- `messages.list`（`listed`，`GET /api/messages`）：查询参数 `cursor?, limit?, from_ms?, to_ms?` → `{items, next_cursor}`；`items` 为 `inbox::Row`（不含 raw），标 `Trace::Redacted`
- `inbox::Row` 的 `priority`（该消息的决策概率）、`topic`（话题 ID）、`analysis`（话题最近一次转交的分析）可空，由 adapters 连表填入；其他 reader 留空
- `inbox::Reader` 是 domain 定义、adapters 实现的读取接口（`Debug + Sync`：`Context` 持有 `&dyn Reader` 跨 await 时须 Send）；契约：按 `occurred_at_ms` 降序、同值按 `key` 降序，返回严格小于游标的至多 `limit` 条；读失败返回 `AppError::Storage`
- 分页规则在 domain：`limit` 缺省取 `Paging.default`，为 0 或超过 `Paging.max` 拒绝；向 reader 多取 1 条判断是否有下一页，有才给 `next_cursor`
- 时间范围 `from_ms`、`to_ms`（UTC 毫秒）是半开区间 `[from_ms, to_ms)`，可只给一端；两端都给且 `from_ms >= to_ms` 按请求无效；经 `inbox::Window`（字段 `since`、`until`，`min_ident_chars` 不允许 `to`）放进 `Query`，由 reader 与游标条件叠加过滤
- 游标是不透明字符串 `<occurred_at_ms>:<key>`，按第一个冒号切分（key 自带冒号）；解析失败按请求无效处理

## triage

- 九个路由，请求都带消息内容，均标 `Trace::Redacted`；`notice`、`screen` 为 `typed`，其余为 `fallible`；`notice` 按值收请求以满足 `needless_pass_by_value`，用解构消耗
- `line`（`triage/line.rs`）：`{message{conversation_id,sender_id,sender_name,text}, rules, max_chars, unknown_sender, self_sender_id, notify_conversation_id, cipher_min_length}` → `{sender, text, skip}`；后三项可为 null（数据源无此概念），为 null 即不做该项判定；依次判本人发送（`own`）、通知会话（`notice`）、空正文（`empty`）、仍是密文（`sealed`）则只给 `skip`（此时不编译规则），否则发送人与正文过 `redact::Mask`，正文按 `max_chars` 截断
- `pack`（`triage/pack.rs`）：`{lines[{key,conversation_id,occurred_at_ms,sender,text,hints?}], attach[{conversation_id,last_ms,probability?,topic_id}], topics[{topic,label,probability?}], batch_id, gap_ms, label_chars, max_chars, model, new_label, questions{urgent,topic}, rules}` → `{body, layout}`
  - 行按会话、时间切段（相邻间隔 `<= gap_ms`），再按首行时间排序
  - 每行一个 `predicate`（`name` 为 `<urgent.name>_<行号>`）
  - `attach` 里同会话且间隔内的段直接并入该话题、不提问；其余段在“开放话题（值为话题 ID）、更早的段（`S<n>`）、`new`”里出 `choice`，选项不足 2 个不出；`hints` 非空的段只列这些话题
  - `body` 的 `input` 只含别名（`C<n>` 会话、`L<n>` 行、`S<n>` 段），外发前对每行再套 `rules`；`layout` 记行序、段与题号，供 `sort` 对号入座
- `sort`（`triage/sort.rs`）：`{layout, response, min_confidence, threshold}` → `{placements[{key,probability,topic}], topics[{topic,label,last_ms,probability,urgent}]}`；答案数量须等于 `layout.questions()`，否则请求无效；按位置核对类型，类型不符整批失败，单项 null、缺字段、`refusal`、概率不在 [0,1]、置信度低于下限、选项不在列表只当未作答（无概率或新话题）；新话题 ID 为 `t<批号>-<段序号>`，话题概率取 `layout.probabilities` 的历史分与当前成员分的最大值
- `brief`（`triage/prompt.rs`）：`{rows[{key?,occurred_at_ms,sender,text}], rules, max_chars, template, followup, resume?{until,until_key?}}` → `{prompt, start, until_ms, until_key}`；有 `resume` 时用 `followup` 且只含 `(occurred_at_ms, key)` 位于游标之后的行；`{messages}` 只替换一次
- `notice`：代入 `{topic}`、`{probability}`（两位小数），最后才代入截断后的 `{analysis}`
- `reply`：`{stdout, max_chars}` → `{thread_id, text}`；从 `codex exec --json` 的 JSONL 取首个 `thread.started` 的 `thread_id` 与最后一条 `item.completed` 里 `agent_message` 的文本，缺任一或文本为空按请求无效
- `redact`：`{text, rules}` → `{text}`，供启动自检；`tests/it/redact.rs` 读取真实的 `cli/config.yaml` 里 `triage.redact.patterns` 跑样例表：每条规则至少一例必须被替换、几例明文必须原样；规则是数据，改规则必须同步样例，源码里的中文用 `\u{…}` 转义（`non_ascii_literal`）
- `screen`（`triage/screen.rs`）：`{instructions, lines[{key,text}], model, topics[{topic,label}]}` → `{body}`；OpenAI 兼容的 chat 请求体（`system` 放指令，`user` 放只含别名 `L<n>` 与话题的 JSON，`response_format` 为 `json_object`，温度 0），不含消息键
- `screened`（`triage/screened.rs`）：`{lines, min_chars, placeholder, response, topics}` → `{lines[{key,text,flag,hints}]}`；从 `choices[0].message.content` 解出 `{lines[{line,risk,spans,topics}]}`；片段去空白后须不短于 `min_chars` 且是该行文字的子串才用 `placeholder` 遮盖；话题须在给定话题内且去重；`risk == high` 且无可用片段 `flag = unmasked`，缺该行的判断 `flag = missing`；响应无法解析按请求无效
- `learn`（`triage/learn.rs`，不是路由，供 `review.update`）：粘贴的原文去空白后须不短于 `min_chars` 且是该行文字的子串，转义成字面量正则，否则 `Unprocessable`
- `transcript.rs`：`Row`（`key?`、`occurred_at_ms,sender,text`，`render` 过 `Mask` 并按 `max` 截断）、`Style`、`fit`（整个 JSON 数组超 `max_chars` 就丢最旧的行，至少留最后一条）
- 脱敏（`triage/redact.rs`）：`Mask::new(&[Rule])` 编译全部正则，任一无效返回 `BadRequest`（失败即不外发）；`apply` 按序 `replace_all`，placeholder 里可写 `${1}`；`regex` 只开 `std`、`perf`、`unicode-perl`、`unicode-case`，无环视，规则里数字类用 `[0-9]`、不用 `\b`（CJK 属 word 字符）；每次调用现编译，不缓存
- 提示词模板里的“不可信数据”说明在配置里；domain 只做占位符替换

## review

- `Docket`（`review.rs`，`Debug + Sync`），三个方法失败都返回 `AppError::Storage`：
  - `list(&Listing)` → `Vec<Item>`
  - `settle(&Ruling)` → 裁决后的当前状态，只裁决待审核项，键不存在为 `None`
  - `source(key)` → 该行当前文字，供漏脱敏拒绝校验粘贴的原文
- `Ruling::approve(key)`、`Ruling::reject(key, reason)` 加 `learned(&Rule)`，不记审核人；`state()` 给 `approved`｜`rejected`｜`leaked`；跨 crate 不 match 变体
- `Reason{code,label,placeholder?}` 来自配置；带 `placeholder` 的是漏脱敏类，拒绝时必须带 `leak`，经 `learn` 生成 `Rule(pattern, placeholder)` 交 `settle`；不带 `placeholder` 的必须不带 `leak`；原因未知或组合不符为 `Unprocessable`
- 路由（`features/review.rs`）：
  - `review.list`（`listed`，`GET /api/reviews`）：查询参数 `limit?, state?`（缺省列 `pending`，条数规则同 `messages.list`）→ `{items, reasons}`；`Item{batch_id,conversation_id,flag,key,occurred_at_ms,reason,sender,state,text}`
  - `review.update`（`keyed`，`PATCH /api/reviews/{key}`）：params `Key{key}`，请求体 `Update`（按 `state` 打标签：`approved`，或 `rejected` 带 `reason`、`leak?`）→ `{state}`
- `settle`：键不存在 `NotFound`；返回状态与本次裁决的 `ruling.state()` 相同回 200（含重复提交），不同即 `Conflict`；请求格式错 `BadRequest`
- 拒绝的日志只记 `redacted:<字节数>`，粘贴的原文不入日志

## 错误

- 错误码由调用方经 `Errors`（从配置反序列化）传入；`UnknownAction` 只在按字符串查不到路由时出现（adapters 与测试），HTTP 不会遇到
- 新增 `AppError` 变体时同步：`Errors` 字段、`spec`、`status`、`summary` 分支（`summary` 写入接口说明）、需要声明的路由的 `failures`、`cli/config.yaml`、`cli/tests/it/support.rs` 的 `YAML`、`domain/tests/it/support.rs` 与 `adapters/tests/it/support.rs` 的 `ERRORS`、`tests/it/error.rs`、README“返回码”表、docs/requirements.md“异常”；错误体为 `{"code":"<配置的 code>"}`，经 serde 序列化

## 日志

- dispatch 打一条 debug 日志：target `dispatch`，字段 transport、session_id、user、feature、action、params、request、response、ok、elapsed_us
  - session_id、user 缺省为空串
  - request 为原始请求体；response 成功时为响应体，失败时为 `AppError::body(errors)`
  - 二者原样入日志，故业务请求与响应不得携带 secret；标 `Trace::Redacted` 的路由，params、request、response 只记 `redacted:<字节数>`

## 测试

- `tests/it/support.rs`：`local` 构造 adapters 来源的 `Call`（需要参数时 `.with_params`）；`Fixture` 持有 `Errors` 与 `Paging`，`dispatch` 用空 reader，`dispatch_with(&reader, &call)` 换 reader，`dispatch_review(&docket, &call)` 换审核台
- 替身：`Rows`（按契约过滤并截断的内存 reader）；`Broken`（`Reader` 与 `Docket` 都恒返回 `Storage`）；`Dock`（内存 `Docket`，按键与状态存项，`source` 返回固定的 `LEAKED` 文字，带规则的裁决只在规则为 `LEARNED` 与 `[x]` 时返回 `leaked`，否则 `mislearned`）
- triage 测试按动作一文件，放 `tests/it/features/triage/`，与 `src/features/triage/` 对应
- 用 scoped subscriber 捕获日志前先确保常驻占位 `Dispatch`（`tests/it/log.rs` 的 `ANCHOR`）：tracing-core 只注册 1 个 dispatcher 时按当前线程算 callsite interest，并行测试中无 subscriber 的线程会把 interest 缓存成 never，日志丢失

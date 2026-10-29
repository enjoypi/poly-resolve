# domain

## 职责

- 业务逻辑；不依赖传输层（axum、clap、tokio 等），tracing-subscriber 只作 dev-dependency

## 调用

- 业务只经 `features::dispatch(&Context, &Call)` 调用；`Call` 由 `Origin`（transport、session_id、user）、`Target`（feature、action）与 request 经各自的 `new` 构造；`Context::new(errors, Rules, Ports)` 由调用方（cli、adapters 编排）构造：`Rules::new(&Paging, &[Reason])`，`Ports::new(reader, docket)`，`reader` 是 `inbox::Reader`（存储读取）、`docket` 是 `review::Docket`（审核台读写），均由 adapters 实现
- handler 有四种签名：`fn(Req) -> Resp` 经 `typed`，`fn(&Req) -> Result<Resp, AppError>` 经 `fallible`，`fn(&Req, &Context) -> Result<Resp, AppError>` 经 `queried`，`fn(&Req, &Context, &str) -> Result<Resp, AppError>` 经 `signed`（第三个参数是 `Origin.user`，缺失或为空按请求无效），统一 JSON 编解码，共用 `decode`；`Route.call` 为 `fn(&Call, &Context)`，闭包用 `call.request` 取请求体，不需要上下文的路由闭包忽略第二个参数
- `route` 的 `&&` 两侧都要有用例：功能不存在、功能存在而动作不存在、两者都存在
- `catalog::build` 供 cli 拼 OpenAPI：schema 引用路径为 `/components/schemas`，错误体 `ErrorBody` 取自 `Body`

## 新功能

- 新建 `src/features/<f>.rs`
- `features.rs` 加 `mod <f>;`，每个 action 在 `ROUTES` 加一项：`call` 用 `typed` 或 `fallible`，`schema` 用 `schemas::<Req, Resp>`；路由与接口说明共用此表
- `ROUTES` 项另有 `failures`：除请求无效外该路由可能返回的错误（如 `messages.list` 为 `[AppError::Storage]`），供接口说明声明状态码，须与实际一致
- 自定义类型的 Req、Resp 派生 `JsonSchema`，并加 `#[schemars(rename = "<Feature><Action>Request")]`（Resp 同理）避免跨功能重名
- 对外整数可能超过 2^53 时不用 i64（生成的 TS 为 number）
- 新建 `tests/it/features/<f>.rs`，`tests/it/features.rs` 加 `mod <f>;`；`tests/it/catalog.rs` 补条目与 schema 断言
- 跑 `bun scripts/gen.ts` 更新 `api/openapi.json` 与 `web/src/api.gen.ts`
- 更新 docs/requirements.md 的业务规则
- `typed`、`fallible`、`queried`、`decode`、`schemas` 是泛型（见根“测试与覆盖率”），故每个 feature 至少有一条请求格式错误的测试，且 `catalog::build` 会走到其 schema

## ingest

- `parse_event`（钉钉实时事件一行）、`parse_page`（补拉一页）→ `IngestEnvelope`，请求共用 `IngestParseRequest`：account、source、body、received_at_ms（调用方给，保持纯函数）、utc_offset_minutes；`window` 把毫秒区间转成补拉用的 `YYYY-MM-DD HH:MM:SS`（按偏移）
- 字段名按候选列表取首个有值的（`pick`），不用 serde alias：补拉消息可同时含 `messageId` 与 `openMessageId`，alias 会报重复字段
- 时间 `create_time` 或 `createTime`：数字当 UTC 毫秒，字符串先试 RFC3339，再试无时区的 `%Y-%m-%d %H:%M:%S`（按偏移换算）；早于 1970 取 0
- 补拉一页内无法解析的消息计入 `skipped`，不使整页失败；`next_cursor` 仅在 `hasMore` 为真时给出（服务端取完后仍会返回游标，不能只看游标），非空表示未取完
- `key` 为 `<source>:<account>:<message_id>`，实时与补拉共用，供存储去重
- `cipher`（`typed`）：请求 `{envelope, min_length}`，响应 `{cipher}`（是密文则为 `text`，否则 null）；规则同 dws 的 `IsEncrypted`（`ingest/cipher.rs`）：trim 后以 `||版本||类型||长度` 结尾（三段全是数字、非空），其余部分 trim 后不短于 `min_length` 且只含字母数字与 `+/=` 和空白；`min_length` 由 adapters 取自配置
- `reveal`（`typed`）：请求 `{envelope, plaintext}`，响应 envelope，仅把 `text` 换成 `plaintext`，其余字段不变；如何解密由 adapters 决定，domain 只做替换
- `fragments`（`ingest/cipher.rs`）：正文任一行（trim 后）整行只含字母数字与 `+/=` 且不短于 `cipher_min_length`，即视为留有密文片段；`parse_event`、`parse_page` 据此给 envelope 填 `lookup`（消息 ID，否则空），请求带 `cipher_min_length`
- `assemble`（`typed`）：请求 `{envelope, children: [{sender, text}]}`，正文重组为“原正文首行 + 每条子消息的 `sender:text`”，`lookup` 置空；子消息由 adapters 向钉钉工具查询（解密后的）
- `cipher`、`reveal`、`assemble` 都带消息内容，路由标 `Trace::Redacted`
- 对外结构里不放 bool 字段（`struct_excessive_bools`），用可空游标等代替

## messages

- `messages.list`：请求 `{cursor?, limit?, from_ms?, to_ms?}`，响应 `{items, next_cursor}`；`items` 为 `inbox::Row`（不含 raw），路由标 `Trace::Redacted`
- `inbox::Reader` 是 domain 定义、adapters 实现的读取接口（`Debug + Sync`：`Context` 持有 `&dyn Reader` 跨 await 时须 Send）；契约：按 `occurred_at_ms` 降序、同值按 `key` 降序，返回严格小于游标的至多 `limit` 条；读失败返回 `AppError::Storage`
- 分页规则在 domain：`limit` 缺省取 `Paging.default`，为 0 或超过 `Paging.max` 拒绝；向 reader 多取 1 条判断是否有下一页，有才给 `next_cursor`
- 时间范围 `from_ms`、`to_ms`（UTC 毫秒）是半开区间 `[from_ms, to_ms)`，可只给一端；两端都给且 `from_ms >= to_ms` 按请求无效；经 `inbox::Window`（字段 `since`、`until`，`min_ident_chars` 不允许 `to`）放进 `Query`，由 reader 与游标条件叠加过滤
- 游标是不透明字符串 `<occurred_at_ms>:<key>`，按第一个冒号切分（key 自带冒号）；解析失败按请求无效处理
- `Paging` 字段名不带相同后缀（`struct_field_names`），故为 `default`、`max`

## triage

- 九个路由，请求都带消息内容，均标 `Trace::Redacted`；`pack`/`line`/`sort`/`brief`/`reply`/`redact`/`screened` 为 `fallible`，`notice`/`screen` 为 `typed`。`screen`（`triage/screen.rs`）`{instructions, lines[{key,text}], model, topics[{topic,label}]}` → `{body}`：OpenAI 兼容的 chat 请求体（`system` 放指令，`user` 放只含别名 `L<n>` 与话题的 JSON，`response_format` 为 `json_object`，温度 0），外发不含消息键；`screened`（`triage/screened.rs`）`{lines, min_chars, placeholder, response, topics}` → `{lines[{key,text,flag,hints}]}`：从 `choices[0].message.content` 解出 `{lines[{line,risk,spans,topics}]}`，片段去空白后须不短于 `min_chars` 且是该行文字的子串才用 `placeholder` 遮盖，话题须在给定话题内且去重，`risk == high` 且无可用片段 `flag = unmasked`，缺该行的判断 `flag = missing`，响应无法解析按请求无效。其余路由见下：，均标 `Trace::Redacted`，全部 `fallible`（`notice` 为 `typed`）：`line`（`triage/line.rs`）`{message{conversation_id,sender_id,sender_name,text}, rules, max_chars, unknown_sender, self_sender_id, notify_conversation_id, cipher_min_length}` → `{sender, text, skip}`——依次判本人发送（`own`）、通知会话（`notice`）、空正文（`empty`）、仍是密文（`sealed`）则只给 `skip`（此时不编译规则），否则发送人与正文过 `redact::Mask`，正文按 `max_chars` 截断；`pack`（`triage/pack.rs`）`{lines[{key,conversation_id,occurred_at_ms,sender,text}], attach[{conversation_id,last_ms,topic_id}], topics[{topic,label}], batch_id, gap_ms, label_chars, max_chars, model, new_label, questions{urgent,topic}, rules}` → `{body, layout}`——把行按会话、时间切段（相邻间隔 `<= gap_ms`），再按首行时间排序；每行一个 `predicate`（`name` 为 `<urgent.name>_<行号>`），`attach` 里同会话且间隔内的段直接并入该话题、不提问，其余段在“开放话题（值为话题 ID）、更早的段（`S<n>`）、`new`”里出 `choice`（选项不足 2 个不出），`body` 里 `input` 只含别名（`C<n>` 会话、`L<n>` 行、`S<n>` 段），外发前对每行再套 `rules`；`layout` 记行序、段与题号，供 `sort` 对号入座；`sort`（`triage/sort.rs`）`{layout, response, min_confidence, threshold}` → `{placements[{key,probability,topic}], topics[{topic,label,last_ms,probability,urgent}]}`——答案数量须等于 `layout.questions()`，否则请求无效；按位置核对类型，`refusal`、类型不符、概率不在 [0,1]、置信度低于下限、选项不在列表都当未作答（无概率或新话题），新话题 ID 为 `t<批号>-<段序号>`，话题概率取成员最大；`learn`（`triage/learn.rs`，不是路由，供 `review.reject`）把粘贴的原文去空白后须不短于 `min_chars` 且是该行文字的子串，转义成字面量正则；`brief`（`triage/prompt.rs`）`{rows[{occurred_at_ms,sender,text}], rules, max_chars, template, followup, resume?{until}}` → `{prompt, start, until_ms}`，有 `resume` 时用 `followup` 且只含 `occurred_at_ms > until` 的行，`{messages}` 只替换一次；`notice` 代入 `{topic}`、`{probability}`（两位小数）、最后才代入截断后的 `{analysis}`；`redact` `{text, rules}` → `{text}`，供 `cli call` 试规则与启动自检；`reply` `{stdout, max_chars}` → `{thread_id, text}`，从 `codex exec --json` 的 JSONL 取首个 `thread.started` 的 `thread_id` 与最后一条 `item.completed` 里 `agent_message` 的文本，缺任一或文本为空按请求无效
- `transcript.rs` 只放 `Row`（`occurred_at_ms,sender,text`，`render` 过 `Mask` 并按 `max` 截断）、`Style`、`fit`（整个 JSON 数组超 `max_chars` 就丢最旧的行，至少留最后一条）；不再有会话切段
- 脱敏（`triage/redact.rs`）：`Mask::new(&[Rule])` 编译全部正则，任一无效返回 `BadRequest`（失败即不外发）；`apply` 按序 `replace_all`，placeholder 里可写 `${1}`；`regex` 只开 `std`、`perf`、`unicode-perl`、`unicode-case`，无环视，规则里数字类用 `[0-9]`、不用 `\b`（CJK 属 word 字符）；每次调用现编译，不缓存
- 提示词模板里的“不可信数据”说明在配置里；domain 只做占位符替换
- `notice` 按值收请求以满足 `needless_pass_by_value`，用解构消耗；其余 triage handler 按引用收
- 对外结构里不放 bool 字段（`struct_excessive_bools` 阈值 0）、结构体字段不全带同一后缀；`regex` 在 domain 使用 `alloc::collections`（`std_instead_of_alloc`），`lib.rs` 须 `extern crate alloc`
- 审核（`review.rs`、`features/review.rs`）：`Docket`（`Debug + Sync`）`list(&Listing) -> Vec<Item>`、`settle(&Ruling) -> Option<String>`（只裁决待审核项，返回裁决后的当前状态，键不存在为 `None`）、`source(key) -> Option<String>`（取该行当前文字，供 `reject` 校验粘贴的原文）；`Ruling::approve(key)`、`Ruling::reject(key, reason)`（不记审核人） 加 `learned(&Rule)`，`state()` 给 `approved`｜`rejected`｜`leaked`，跨 crate 不 match 变体；`Reason{code,label,placeholder?}` 来自配置，带 `placeholder` 的是漏脱敏类：`review.reject` 必须带 `leak`，经 `learn` 生成 `Rule(pattern, placeholder)` 交 `settle`，不带 `placeholder` 的必须不带 `leak`，其余组合都是请求无效；`review.approve` `{keys}` → `{states}` 逐条裁决；`review.list`（`queried`，`{limit?, state?}`，缺省列 `pending`，条数规则同 `messages.list`）→ `{items, reasons}`；`Item{batch_id,conversation_id,key,occurred_at_ms,reason,sender,state,text}`；`Rules::new(&Paging, &[Reason], learn_min_chars)`；原因未知、键不存在、请求格式错都是请求无效；`failures` 为 `[Storage]`；`reject` 的日志只记 `redacted:<字节数>`，粘贴的原文不入日志
- `inbox::Row` 增 `priority`（该消息的决策概率）、`topic`（话题 ID）、`analysis`（话题最近一次转交的分析）三个可空字段，由 adapters 连表填入；其他 reader 缺省为空
- `ingest::clock`、`ingest/cipher::sealed` 对 `features` 内可见，供 triage 复用

## 错误

- 错误码与退出码由调用方经 `Errors`（从配置反序列化）传入；退出码用 `NonZeroU8`，拒绝 0
- 新增 `AppError` 变体时同步：`Errors` 字段、`spec`、`status`、`summary` 分支（`summary` 写入接口说明）、需要声明的路由的 `failures`、`cli/config.yaml`、`cli/tests/it/support.rs` 的 `YAML`、`domain/tests/it/support.rs` 的 `ERRORS`、`tests/it/error.rs`、README“返回码”表、docs/requirements.md“异常”；错误体为 `{"code":"<配置的 code>"}`，经 serde 序列化

## 日志

- dispatch 打一条 debug 日志：target `dispatch`，字段 transport、session_id、user、feature、action、request、response、ok、elapsed_us
  - session_id、user 缺省为空串
  - request 为原始请求体；response 成功时为响应体，失败时为 `AppError::body(errors)`
  - 二者原样入日志，故业务请求与响应不得携带 secret；含消息正文的路由在 `ROUTES` 标 `Trace::Redacted`，日志里 request、response 只记 `redacted:<字节数>`

## 测试

- 用 `tests/it/support.rs` 的 `cli` 构造 CLI 来源的 `Call`；`Fixture` 持有 `Errors` 与 `Paging`，`dispatch` 用空 reader，`dispatch_with(&reader, &call)` 换 reader；`Rows`（按契约过滤并截断的内存 reader）与 `Broken`（恒返回 `Storage`）供 messages 测试
- 审核测试：`Dock`（内存 `Docket`，按键与状态存项，`source` 返回固定的 `LEAKED` 文字，带规则的裁决返回 `leaked:<pattern>:<placeholder>`）、`Broken`（`Reader` 与 `Docket` 都恒返回 `Storage`）；`Fixture::dispatch_review(&docket, &call)`；`signed(action, request, user)` 构造带使用者的 `Call`；triage 测试按动作拆在 `tests/it/features/triage/`（`line.rs`、`pack.rs`、`prompt.rs`、`redact.rs`、`reply.rs`、`sort.rs`）
- 用 scoped subscriber 捕获日志前先确保常驻占位 `Dispatch`（`tests/it/log.rs` 的 `ANCHOR`）：tracing-core 只注册 1 个 dispatcher 时按当前线程算 callsite interest，并行测试中无 subscriber 的线程会把 interest 缓存成 never，日志丢失

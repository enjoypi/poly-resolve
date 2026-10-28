# domain

## 职责

- 业务逻辑；不依赖传输层（axum、clap、tokio 等），tracing-subscriber 只作 dev-dependency

## 调用

- 业务只经 `features::dispatch(&Context, &Call)` 调用；`Call` 由 `Origin`（transport、session_id、user）、`Target`（feature、action）与 request 经各自的 `new` 构造；`Context::new(errors, paging, reader)` 由调用方（cli、adapters 编排）构造，`reader` 是 `inbox::Reader`（存储读取，由 adapters 实现）
- handler 有三种签名：`fn(Req) -> Resp` 经 `typed`，`fn(&Req) -> Result<Resp, AppError>` 经 `fallible`，`fn(&Req, &Context) -> Result<Resp, AppError>` 经 `queried`，统一 JSON 编解码，共用 `decode`；`Route.call` 为 `fn(&str, &Context)`，不需要上下文的路由闭包忽略第二个参数
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

- `messages.list`：请求 `{cursor?, limit?}`，响应 `{items, next_cursor}`；`items` 为 `inbox::Row`（不含 raw），路由标 `Trace::Redacted`
- `inbox::Reader` 是 domain 定义、adapters 实现的读取接口（`Debug + Sync`：`Context` 持有 `&dyn Reader` 跨 await 时须 Send）；契约：按 `occurred_at_ms` 降序、同值按 `key` 降序，返回严格小于游标的至多 `limit` 条；读失败返回 `AppError::Storage`
- 分页规则在 domain：`limit` 缺省取 `Paging.default`，为 0 或超过 `Paging.max` 拒绝；向 reader 多取 1 条判断是否有下一页，有才给 `next_cursor`
- 游标是不透明字符串 `<occurred_at_ms>:<key>`，按第一个冒号切分（key 自带冒号）；解析失败按请求无效处理
- `Paging` 字段名不带相同后缀（`struct_field_names`），故为 `default`、`max`

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
- 用 scoped subscriber 捕获日志前先确保常驻占位 `Dispatch`（`tests/it/log.rs` 的 `ANCHOR`）：tracing-core 只注册 1 个 dispatcher 时按当前线程算 callsite interest，并行测试中无 subscriber 的线程会把 interest 缓存成 never，日志丢失

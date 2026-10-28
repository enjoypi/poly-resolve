# domain

## 职责

- 业务逻辑；不依赖传输层（axum、clap、tokio 等），tracing-subscriber 只作 dev-dependency

## 调用

- 业务只经 `features::dispatch(errors, &Call)` 调用；`Call` 由 `Origin`（transport、session_id、user）、`Target`（feature、action）与 request 经各自的 `new` 构造；handler 签名 `fn(Req) -> Resp`，由 `typed` 统一 JSON 编解码
- `route` 的 `&&` 两侧都要有用例：功能不存在、功能存在而动作不存在、两者都存在
- `catalog::build` 供 cli 拼 OpenAPI：schema 引用路径为 `/components/schemas`，错误体 `ErrorBody` 取自 `Body`

## 新功能

- 新建 `src/features/<f>.rs`
- `features.rs` 加 `mod <f>;`，每个 action 在 `ROUTES` 加一项：`call` 用 `typed`，`schema` 用 `schemas::<Req, Resp>`；路由与接口说明共用此表
- 自定义类型的 Req、Resp 派生 `JsonSchema`，并加 `#[schemars(rename = "<Feature><Action>Request")]`（Resp 同理）避免跨功能重名
- 对外整数可能超过 2^53 时不用 i64（生成的 TS 为 number）
- 新建 `tests/it/features/<f>.rs`，`tests/it/features.rs` 加 `mod <f>;`；`tests/it/catalog.rs` 补条目与 schema 断言
- 跑 `bun scripts/gen.ts` 更新 `api/openapi.json` 与 `web/src/api.gen.ts`
- 更新 docs/requirements.md 的业务规则
- `typed`、`schemas` 是泛型（见根“测试与覆盖率”），故每个 feature 至少有一条请求格式错误的测试，且 `catalog::build` 会走到其 schema

## 错误

- 错误码与退出码由调用方经 `Errors`（从配置反序列化）传入；退出码用 `NonZeroU8`，拒绝 0
- 新增 `AppError` 变体时同步：`Errors` 字段、`spec` 与 `status` 分支、`cli/config.yaml`、`cli/tests/it/support.rs` 的 `YAML`、`domain/tests/it/support.rs` 的 `ERRORS`、`tests/it/error.rs`、README“返回码”表、docs/requirements.md“异常”；错误体为 `{"code":"<配置的 code>"}`，经 serde 序列化

## 日志

- dispatch 打一条 debug 日志：target `dispatch`，字段 transport、session_id、user、feature、action、request、response、ok、elapsed_us
  - session_id、user 缺省为空串
  - request 为原始请求体；response 成功时为响应体，失败时为 `AppError::body(errors)`
  - 二者原样入日志，故业务请求与响应不得携带 secret

## 测试

- 用 `tests/it/support.rs` 的 `cli` 构造 CLI 来源的 `Call`
- 用 scoped subscriber 捕获日志前先确保常驻占位 `Dispatch`（`tests/it/log.rs` 的 `ANCHOR`）：tracing-core 只注册 1 个 dispatcher 时按当前线程算 callsite interest，并行测试中无 subscriber 的线程会把 interest 缓存成 never，日志丢失

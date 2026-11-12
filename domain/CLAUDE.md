# domain

## 职责

- 业务逻辑；不依赖传输层（axum、clap、tokio 等），tracing-subscriber 只作 dev-dependency

## 调用

- 业务只经 `features::dispatch(&Context, &Call)` 调用（唯一例外：`redact::Redactor::new(&[redact::Rule])`、`apply(text)` 供 cli 的日志写出口脱敏，与 `triage.redact` 共用 `Mask`；日志写出口不能走 dispatch，否则 dispatch 自身的日志会递归）；`Call` 由 `Origin`（transport、session_id、user）、`Target`（feature、action）与 request（JSON 请求体）经各自的 `new` 构造，HTTP 的路径与查询参数经 `with_params` 以 urlencoded 字符串传入
- `redact::Seal::new(密钥材料, hash 位数)`：`blake3::derive_key` 得密钥后存成带密钥的 `Hasher`（其 `Debug` 不含密钥，故 `Seal` 可直接 derive，不手写 `fmt`：`renamed_function_params` 与 `min_ident_chars` 对参数名 `f` 互斥）；`tag(值)` 取 trim、小写后的带密钥 hash 前 N 位，`digest(正文)` 把空白折叠后取 32 位；`Context::with_seal(&Seal)` 由 adapters 的 `Desk` 每次调用时附上，密钥不进请求体与日志
- `Context::new(errors, Rules, Ports)` 由调用方（cli、adapters）构造：`Rules::new(&Paging)`，可选 `with_placeholders(&[String])`（缺省空）；`Ports::new(reader, outbox, roster)`，`reader` 是 `inbox::Reader`（存储读取），`outbox` 是 `outbound::Outbox`（外发记录读取），`roster` 是 `batches::Roster`（失败批读写），均由 adapters 实现；可选 `with_library(&dyn prompts::Library)`（提示词读写，缺省为空实现：列表为空、保存得 `None`）
- handler 包装：`typed`（`fn(Req) -> Resp`）、`fallible`（`fn(&Req) -> Result`）解 JSON 请求体，供内部动作；`listed`（`fn(&Params, &Context) -> Result`）只解 params；`keyed`（`fn(&Params, Req, &Context) -> Result`）解 params 与请求体；`features::limit(requested, context)` 是各列表接口共用的条数校验；JSON 共用 `decode`，urlencoded 共用 `params`（`serde_urlencoded`）；health 不解码；`Route.call` 为 `fn(&Call, &Context)`，不需要上下文的路由闭包忽略第二个参数
- `route` 的 `&&` 两侧都要有用例：功能不存在、功能存在而动作不存在、两者都存在
- `catalog::build` 供 cli 拼路由与 OpenAPI：只列 `Http::Bound(方法, 路径模板, 描述)` 的路由，`Entry` 给出 `method`、`path`、`params`、`body`、`response`；描述函数 `bare`（无参数无请求体）、`asked`（只有 params）、`changed`（params 加请求体）；schema 引用路径为 `/components/schemas`，错误体 `ErrorBody` 取自 `Body`

## 新功能

- 新建 `src/features/<f>.rs`
- `features.rs` 加 `mod <f>;`，每个 action 在 `features/routes.rs` 的 `ROUTES` 加一项（按 feature 字母序）：`call` 按签名选包装，`http` 对外为 `Http::Bound("get"|"patch", "/api/<资源>", 描述)`、内部为 `Http::Hidden`；路由与接口说明共用此表
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

- `ingest` feature 含钉钉格式与邮件（`ingest/mail.rs`）；新数据源产出同样的 envelope
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
- `parse_mail`（`fallible`，`Trace::Redacted`）：`{account, source, raw: 字节数组, internal, uid, uid_validity, received_at_ms, limits{chars,depth,parts}, style{attachment,quote_starts,quote_ends}}` → `{envelope, skip}`；用 mail-parser（`full_encoding`，GBK 等）；`occurred_at_ms` 取 `internal`（`%d-%b-%Y %H:%M:%S %z`），解析不了用 `received_at_ms`；`key` 为 `<source>:<account>:<Message-ID>`，无则 `uid:<uid_validity>:<uid>`；`conversation_id` 为 `mail:<account>:thread:<根 id>`（根 id 依次取 `References` 首项、`In-Reply-To`、本封标识）；正文为主题 + `body_text(0)`（只有 html 时转文字），删 `>` 行，遇 `quote_starts` 前缀或 `quote_ends` 结尾的行截断，附件按模板追加，去控制字符后截断到 `chars`；`skip` 只有一种：首个 `Authentication-Results` 含 `dmarc=fail` 为 `dmarc`；自动与群发邮件照收，在 `raw.bulk` 里标 `auto`（`Auto-Submitted` 非 `no`）或 `list`（有 `List-Id` 或 `Precedence` 为 bulk、junk、list）；`raw` 只存标识、日期、发件地址、主题、回复关系、`internal`、大小与 UID；邮件解析失败为 `BadRequest`
- `since`（`typed`）：`{at_ms}` → `{date}`，IMAP `SEARCH SINCE` 用的 `%-d-%b-%Y`（UTC）

## messages

- `messages.list`（`listed`，`GET /api/messages`）：查询参数 `cursor?, limit?, from_ms?, to_ms?` → `{items, next_cursor}`；`items` 为 `inbox::Row`（不含 raw），标 `Trace::Redacted`
- `inbox::Row` 的 `priority`（该消息的决策概率）、`topic`（话题 ID）、`analysis`（话题最近一次转交的分析）、`state` 与 `reason`（处理状态及原因）可空，由 adapters 连表填入；其他 reader 留空
- `inbox::Reader` 是 domain 定义、adapters 实现的读取接口（`Debug + Sync`：`Context` 持有 `&dyn Reader` 跨 await 时须 Send）；契约：按 `occurred_at_ms` 降序、同值按 `key` 降序，返回严格小于游标的至多 `limit` 条；读失败返回 `AppError::Storage`
- 分页规则在 domain：`limit` 缺省取 `Paging.default`，为 0 或超过 `Paging.max` 拒绝；向 reader 多取 1 条判断是否有下一页，有才给 `next_cursor`
- 时间范围 `from_ms`、`to_ms`（UTC 毫秒）是半开区间 `[from_ms, to_ms)`，可只给一端；两端都给且 `from_ms >= to_ms` 按请求无效；经 `inbox::Window`（字段 `since`、`until`，`min_ident_chars` 不允许 `to`）放进 `Query`，由 reader 与游标条件叠加过滤
- 游标是不透明字符串 `<occurred_at_ms>:<key>`，按第一个冒号切分（key 自带冒号）；解析失败按请求无效处理

## triage

- 十三个路由，请求都带消息内容，均标 `Trace::Redacted`；`notice`、`screen`、`people` 为 `typed`，要脱敏的 `line`、`pack`、`brief`、`redact` 为 `sealed`（`fn(&Req, Option<&Seal>)`，密钥取自 `Context`），其余为 `fallible`；`notice` 按值收请求以满足 `needless_pass_by_value`，用解构消耗
- `line`（`triage/line.rs`）：`{message{bulk?,conversation_id,sender_id,sender_name,text}, rules, max_chars, unknown_sender, self_sender_ids, notify_conversation_id, cipher_min_length}` → `{role, sender, skip, state, text}`；`self_sender_ids` 为空、后两项为 null（数据源无此概念）即不做该项判定；`role`：发送人与 `self_sender_ids` 任一项不区分大小写相等为 `own`，否则 `bulk` 非空为 `auto`，否则 null；`digest`：请求带 `digest_min_chars`、有密钥且原文不短于它时为 `Seal::digest(原文)`，否则空串（被跳过的行恒为空）；`skip` 依次判通知会话（`notice`，`state` 为 `skipped`）、空正文（`empty`）、仍是密文（`sealed`），后两者 `state` 为 `withheld`，都不编译规则、不给文字；其余 `state` 为 `queued`，发送人与正文过 `redact::Mask`，正文按 `max_chars` 截断
- `clues`（`triage/clue.rs`，`fallible`）：`{message{raw?,text}, patterns[{kind,pattern}], subject{ignore,min_chars,prefixes}}` → `{clues[{kind,value}]}`（去重、按 kind 与 value 排序）；`raw` 里的 `message_id`、`in_reply_to`、`references` 各出一条 `mail`；`raw.subject` 去首尾空白转小写、反复去掉 `prefixes` 后不短于 `min_chars` 且不在 `ignore` 里出一条 `subject`；每条 `patterns` 在原文上取全部匹配（有第 1 分组取分组）；`raw` 不是对象时不取邮件线索；正则无效为 `BadRequest`；线索只留在本机，不外发
- `pack`（`triage/pack.rs`）：`{lines[{key,conversation_id,occurred_at_ms,role?,sender,text,hints?}], attach[{conversation_id,last_ms,probability?,topic_id}], linked[{key,topic}]?, topics[{topic,label,probability?}], batch_id, gap_ms, label_chars, max_chars, model, new_label, none_label, questions{urgent,topic}, rules}` → `{body, layout, questions}`（`questions` 为问题总数，0 时 adapters 不外发）；布局类型 `Choice`、`Layout`、`Placed`、`Segment` 在 `triage/pack/layout.rs`，旧布局缺 `role` 按空串读
  - 行按会话、时间切段（相邻间隔 `<= gap_ms`），再按首行时间排序
  - `role` 为空的行各一个 `predicate`（`name` 为 `<urgent.name>_<行号>`）；带 `role` 的行不出题、外发时带 `role` 字段
  - 行可带 `digest`：同批里 `digest` 相同（非空）的行只外发最早的一条并带 `copies`（处数），其余只发 `{line, repeat: <首条别名>}`、不出紧急题，`layout` 里 `role` 记 `copy`
  - `attach` 里同会话且间隔内的段直接并入该话题、不提问；`linked`（强线索命中的话题）里含本段任一行的，段的 `linked` 记下全部话题（去重排序）、同样不提问；其余段在“开放话题（值为话题 ID）、更早的段（`S<n>`）、`new`”里出 `choice`，选项不足 2 个不出；`hints` 非空的段只列这些话题；全是 `auto` 行的段把 `new` 换成 `none`（说明取 `none_label`），段的 `role` 记 `auto`
  - `body` 的 `input` 只含别名（`C<n>` 会话、`L<n>` 行、`S<n>` 段），外发前对每行再套 `rules`；`layout` 记行序、段与题号，供 `sort` 对号入座
- `sort`（`triage/sort.rs`）：`{layout, response, min_confidence}` → `{placements[{key,probability,topic?,via?}], relations[{fresh,known}], topics[{topic,label,last_ms,probability}]}`；一行可有多条归属（段的 `attach` 为 `adjacent`，`linked` 为 `clue`，选中已有话题或更早的段为 `choice`，另起为 `new`）；选了已有话题但置信度不足或缺失时另起话题，并在 `relations` 记“新话题可能与所选话题相关”；紧急度与优先级不在这里算（adapters 的 SQL 按成员重算）；答案数量须等于 `layout.questions()`（紧急题只数 `role` 为空的行，按这些行的顺序对号），否则请求无效；按位置核对类型，类型不符整批失败，单项 null、缺字段、`refusal`、概率不在 [0,1]、置信度低于下限、选项不在列表只当未作答（无概率或新话题）；新话题 ID 为 `t<批号>-<段序号>`，`auto` 段没选到话题时 `topic` 为 null（不新建话题，adapters 记为无关），话题概率取 `layout.probabilities` 的历史分与当前成员分的最大值
- `brief`（`triage/prompt.rs`）：`{rows[{key?,occurred_at_ms,omitted?,role?,sender,source?,text}], rules, max_chars, omitted, template, followup, thread?}` → `{prompt, start}`；`thread` 非空用 `followup`（`start` 为 `resume`），否则用 `template`；两种都给全部行，不截断总长，`max_chars` 只限单行；`{messages}` 只替换一次；带 `source` 的行多一个 `source` 字段；`digest` 相同（非空）的可外发行只留最早一条并带 `copies`；带 `omitted`（未外发原因）的行不渲染，相邻且原因相同的合成一项 `{"note": …}`，文字取 `omitted` 模板（`{count}`、`{reason}`）；带 `role` 的行多一个 `role` 字段
- `notice`：代入 `{topic}`、`{score}` 与 `{urgency}`（两位小数，缺失为空）、`{people}` `{conversations}` `{sources}`（取自 `reach`，缺失为 0）、`{state}`（按 `states` 映射，没有则原样）、`{late}`（有 `late_ms` 时为 `late` 模板代入 `{time}`，格式 `%Y-%m-%d %H:%M:%S UTC`，否则空）、`{label}`、`{changes}`，最后才代入截断后的 `{analysis}`
- `reply`：`{stdout, max_chars, state?}` → `{thread_id, text, status, title, changes, outcome}`；从 `codex exec --json` 的 JSONL 取首个 `thread.started` 的 `thread_id` 与最后一条 `item.completed` 里 `agent_message` 的文本；文本第一行须是 JSON `{status, title?, changes?, changed?}`，`status` 须为 `open`、`resolved`、`cancelled`，其后为分析正文；`changed` 为 `false` 且 `status` 等于请求里的 `state`（话题此前的状态）时 `outcome` 为 `quiet`，否则 `notify`；缺会话 ID、缺回复、没有第二行、首行不是合规 JSON 都按请求无效（adapters 当作分析失败重试）
- `redact`：`{text, rules}` → `{text}`，供启动自检；`tests/it/redact.rs` 读取真实的 `cli/config.yaml` 里 `triage.redact.patterns` 跑样例表：每条规则至少一例必须被替换、几例明文必须原样；规则是数据，改规则必须同步样例，源码里的中文用 `\u{…}` 转义（`non_ascii_literal`）
- `screen`（`triage/screen.rs`）：`{instructions, lines[{key,text}], max_tokens, model, topics[{topic,label}]}` → `{body}`；OpenAI 兼容的 chat 请求体（`system` 放指令，第一条 `user` 只放话题 JSON，第二条 `user` 只放别名 `L<n>` 的行 JSON，让指令加话题成为可缓存的公共前缀；`response_format` 为 `json_object`，温度 0，带 `max_tokens`），不含消息键；adapters 每次只传一行，探测模型是否可用时传空 `lines`
- `screened`（`triage/screened.rs`）：`{lines, min_chars, placeholder, response, topics}` → `{lines[{key,text,flag,hints,spans}]}`（`spans` 为通过校验并用于遮盖的片段，供 adapters 存漏判样本）；从 `choices[0].message.content` 解出 `{lines[{line,risk,spans,topics}]}`；片段去空白后须不短于 `min_chars` 且是该行文字的子串才用 `placeholder` 遮盖；话题须在给定话题内且去重；`risk == high` 且无可用片段 `flag = unmasked`，缺该行的判断 `flag = missing`；响应无法解析按请求无效
- `transcript.rs`：`Row`（`key?`、`occurred_at_ms,sender,text`，可选 `omitted`、`role`、`source`；`render` 过 `Mask` 并按 `max` 截断）、`Style`
- `people`（`triage/people.rs`，`typed`）：`{senders[{source,sender_id,name}]}` → `{identities[{source,sender_id,person_id,name}]}`；姓名折叠空白转小写；同一姓名出现在至少两个来源、且在每个来源里都只对应一个 `sender_id` 时 `person_id` 为 `name:<姓名>`，否则为 `<source>:<sender_id>`（同一邮箱在多个邮箱账号下 `source` 与 `sender_id` 相同，自然是同一人）；空名不对应
- 脱敏（`triage/redact.rs`）：`Mask::new(&[Rule], Option<&Seal>)` 编译全部正则，任一无效、或有规则的 placeholder 含 `{hash}` 却没有密钥，返回 `BadRequest`（失败即不外发）；`apply` 按序 `replace_all`，placeholder 里可写 `${1}`；含 `{hash}` 的规则逐个匹配展开后把 `{hash}` 换成 `Seal::tag(值)`，值取 `Rule.group` 指定的分组（缺省整个匹配）；值本身已是这条规则产出的占位（`{hash}` 前后的字面量之间恰为 N 位十六进制）时原样保留，保证在已脱敏的文字上重复套用结果不变；日志用的 `Redactor` 没有密钥，把 `{hash}` 换成 `-`；`regex` 只开 `std`、`perf`、`unicode-perl`、`unicode-case`，无环视，规则里数字类用 `[0-9]`、不用 `\b`（CJK 属 word 字符）；每次调用现编译，不缓存：实测 `cli/config.yaml` 的 25 条规则 release 下每次约 3 ms，intake 每轮至多 `intake` 条，暂不改批量；学到的规则多到每轮耗时明显再改
- 复核问题：`pack` 请求可带 `reviews[{left,right,tag}]` 与 `questions.review{instructions,name}`，每项出一道是非题（`{review}` 换成序号，两段文字按当前规则脱敏后放进输入），题目计入 `Layout::scored()`；`sort` 把各题概率连同原样的 `tag` 放回 `reviews[{probability,tag}]`，判定阈值与计数在 adapters
- `regroup`（`triage/regroup.rs`，`typed`）：`{candidates[{from,into,keys,kind,size}]}` → `{changes[{from,into,keys,kind,reason}]}`；`apart` 且 `keys` 非空、少于 `size` 为 split，`clue` 且 `keys` 不到 `size` 一半为 move，其余为 merge（`keys` 清空）；一个话题一次只参与一条变更，按输入顺序先到先得
- `recap`（`triage/prompt.rs`，`typed`）：`{item, items[{label,score?,state}], states, template}` → `{text}`，`{count}` 与 `{items}`（每项一行）代入汇总模板；`notice` 另代入 `{origin}`（按 `origins` 映射，没有为空）
- 提示词模板里的“不可信数据”说明在配置里；domain 只做占位符替换

## outbound

- `Outbox`（`outbound.rs`，`Debug + Sync`）：`list(&Listing)` → `Vec<Item>`，失败返回 `AppError::Storage`；`Listing{state, limit}` 也供 `batches::Roster`
- `Item{account,batch_id,conversation_id,flag,key,occurred_at_ms,reason,role,sender,source,state,text}`
- `redact::marks(&[&str])`：占位符去掉 `$name`、`${name}` 分组引用（`$` 为 `$$`），去空、去重、长的在前，供外发记录页标红；`{hash}` 原样留在结果里，由页面换成十六进制的匹配；`redact::Rule` 是日志脱敏用的规则
- 路由（`features/outbound.rs`）：`outbound.list`（`listed`，`GET /api/outbound`，`Trace::Redacted`）：查询参数 `limit?, state?`（缺省列 `approved`，条数规则同 `messages.list`）→ `{items, placeholders}`；只读，没有裁决

## batches

- `Roster`（`batches.rs`，`Debug + Sync`）：`list(&Listing)` → `Vec<Item>`；`revive(batch_id)` → 该批当前状态，不存在为 `None`；失败返回 `AppError::Storage`
- `Item{attempts,batch_id,lines,reason,state,updated_at_ms}`，`lines` 为已通过的行数
- 路由（`features/batches.rs`）：
  - `batches.list`（`listed`，`GET /api/batches`）：查询参数 `limit?, state?`（缺省列 `failed`，条数规则同 `outbound.list`，共用 `features::limit`）→ `{items}`
  - `batches.update`（`keyed`，`PATCH /api/batches/{batch_id}`）：请求体 `{state:"ready"}` → `{state}`；返回状态为 `ready` 回 200（含重复提交），否则 `Conflict`，不存在 `NotFound`
- 不含消息内容，标 `Trace::Full`

## topics

- `Board`（`topics.rs`，`Debug + Sync`）：`overview(&Listing)` → `Overview{items,lines,topics}`（话题摘要、消息与话题各状态的计数 `Tally{count,reason,state}`），`detail(topic_id)` → `Option<Detail{changes,timeline,topic}>`；失败返回 `AppError::Storage`；缺省实现 `Blank`（空列表、查无），由 `Context::with_board` 换成真实实现（`Ports` 按值传已到 64 字节上限，故不放进 `Ports`）
- 路由（`features/topics.rs`），含消息内容，均标 `Trace::Redacted`：`topics.list`（`GET /api/topics`，查询参数 `state?, limit?`，`limit` 规则同 messages）、`topics.get`（`GET /api/topics/{topic_id}`，查无为 `NotFound`）

## prompts

- `Library`（`prompts.rs`，`Debug + Sync`）：`list()` → `Vec<Prompt>`；`save(&Prompt)` → 未启用的名字为 `None`；失败返回 `AppError::Storage`；`Prompt{name,text}`
- 名字与必含占位符只在 `features/prompts.rs` 的 `TABLE`：`session`、`followup` 为 `{messages}`，`topic` 为 `{segment}`，`urgent` 为 `{line}`，`review` 为 `{review}`，`screen` 无；占位符常量取自 `triage/prompt.rs`、`triage/pack.rs`（`pub(crate)`）；`verify`：名字不在表里 `NotFound`，正文 trim 后为空或任一占位符出现次数不为 1 为 `Unprocessable`（`brief` 只替换一次 `{messages}`，多出的会原样外发）
- 路由，均标 `Trace::Redacted`：
  - `prompts.list`（`GET /api/prompts`，`bare`，闭包直接调）→ `{items[{name,required,text}]}`；不在表里的名字 `required` 为空
  - `prompts.update`（`keyed`，`PATCH /api/prompts/{name}`）：请求体 `{text}`，先 `verify` 再 `save`，`save` 得 `None` 为 `NotFound` → `{name}`
  - `prompts.check`（`fallible`，`Http::Hidden`）：`{name,text}` → `{name}`，供 adapters 每次读文件后与启动自检复用同一校验

## 错误

- 错误码由调用方经 `Errors`（从配置反序列化）传入；`UnknownAction` 只在按字符串查不到路由时出现（adapters 与测试），HTTP 不会遇到
- 新增 `AppError` 变体时同步：`Errors` 字段、`spec`、`status`、`summary` 分支（`summary` 写入接口说明）、需要声明的路由的 `failures`、`cli/config.yaml`、`cli/tests/it/support.rs` 的 `YAML`、`domain/tests/it/support.rs` 与 `adapters/tests/it/support.rs` 的 `ERRORS`、`tests/it/error.rs`、README“返回码”表、docs/requirements.md“异常”；错误体为 `{"code":"<配置的 code>"}`，经 serde 序列化

## 日志

- dispatch 打一条 debug 日志：target `dispatch`，字段 transport、session_id、user、feature、action、params、request、response、ok、elapsed_us
  - session_id、user 缺省为空串
  - request 为原始请求体；response 成功时为响应体，失败时为 `AppError::body(errors)`
  - 二者原样入日志，故业务请求与响应不得携带 secret；标 `Trace::Redacted` 的路由，params、request、response 只记 `redacted:<字节数>`

## 测试

- `tests/it/support.rs`：`local` 构造 adapters 来源的 `Call`（需要参数时 `.with_params`）；`Fixture` 持有 `Errors`、`Paging`、占位符 `[x]` 与一个 `Seal`（各 `dispatch*` 都附上），`dispatch_unsealed` 不带密钥，`dispatch` 用空 reader，`dispatch_with(&reader, &call)` 换 reader，`dispatch_outbox(&outbox, &call)` 换外发记录，`dispatch_prompts(&library, &call)` 换提示词存储
- 替身：`Rows`（按契约过滤并截断的内存 reader）；`Broken`（`Reader`、`Outbox`、`Roster` 都恒返回 `Storage`）；`Pool`（内存 `Roster`，`failed` 重发得 `ready`，其余返回原状态）；`dispatch_roster(&roster, &call)` 换失败批端口；`Desk`（内存 `Board`，只认话题 `KNOWN`），经 `dispatch_board(&board, &call)` 接入；`Sent`（内存 `Outbox`，按键与状态存项）
- triage 测试按动作一文件，放 `tests/it/features/triage/`，与 `src/features/triage/` 对应
- 用 scoped subscriber 捕获日志前先确保常驻占位 `Dispatch`（`tests/it/log.rs` 的 `ANCHOR`）：tracing-core 只注册 1 个 dispatcher 时按当前线程算 callsite interest，并行测试中无 subscriber 的线程会把 interest 缓存成 never，日志丢失

# web

## 工程

- Bun + TypeScript；依赖版本写 `*` 由 `bun.lock` 锁定；包只在 web/ 内
- tsconfig 开 strict 与 `noUncheckedIndexedAccess`，`bun x tsc --noEmit` 须通过
- TypeScript 7 无编程式 API，openapi-typescript 需 `ts.factory`，故 `typescript` 别名为 `npm:@typescript/typescript6`（库用 6.x API），`@typescript/native` 别名为 `npm:typescript` 提供 7.x 的 `tsc`；openapi-typescript 支持 7 后去掉两个别名
- `api.gen.ts`、`api/openapi.json` 是生成物，不受 512 行限制

## 生成与调用

- `gen.ts` 用 openapi-typescript 的 Node API 由 `api/openapi.json` 生成 `src/api.gen.ts`，并删去生成器写的注释；`generate()` 同时供 `test/contract.test.ts` 比对
- 只经 `src/client.ts` 的 `connect(baseUrl, fetch?)`（内部 `createClient<paths>`）调用接口，禁手写 fetch 与请求、响应类型；路径字面量由 tsc 对照 `paths` 校验；`fetch` 可注入供测试；业务代码收 `ReturnType<typeof connect>`

## 入口与模块

- `index.html` 是入口，`<link>` 引入 `src/style.css` 与 `src/favicon.svg`；`src/main.ts` 用 `connect(location.origin)` 按 `location.hash` 调 `renderMessages`、（`OUTBOUND_HASH`）`renderOutbound` 或（`FAILED_HASH`）`renderFailed`、（`PROMPTS_HASH`）`renderPrompts`、（`TOPICS_HASH`）`renderTopics`、（以 `TOPIC_HASH` 开头，其后为百分号编码的话题 ID）`renderTopic`；消息页 `.bar` 追加话题页、外发记录页与提示词页链接，外发记录页追加失败批页链接，`hashchange` 时整页重载
- `bun run build` 把产物平铺写到 `dist/`（已忽略），由 `cli serve` 的 `web_dir` 托管，不清旧产物；`bun build` 会按哈希重命名资源并改写引用
- `src/` 保持平铺（contract 测试只扫这一层；改成子目录须同步改扫描范围）；一个模块一件事，文件名即职责，以下几个需注明：
  - `messages.ts`：`renderMessages(client, root, timeZone?)`，只构造 `Board`
  - `board.ts`：`Board`，状态、加载、重绘、事件接线
  - `table.ts`：表头、表体、tip 内容
  - `columns.ts`：列推断与显示值
  - `view.ts`：排序、筛选，纯函数
  - `outbound.ts`：`renderOutbound(client, root, timeZone?)`，只读的外发记录页
  - `failed.ts`：`renderFailed(client, root, timeZone?)`，失败批页
  - `prompts.ts`：`renderPrompts(client, root)`，提示词页
  - `topics.ts`：`renderTopics(client, root, timeZone?)`，话题列表页
  - `topic.ts`：`renderTopic(client, root, topic, timeZone?)`，话题详情页；`brief` 生成话题摘要（标题链接与一行说明），列表页复用
  - `dom.ts`：`area`、`button`，各卡片页共用
- `config.ts` 是前端唯一的常量源：字段呈现表 `FIELDS`（标签、类型、默认隐藏、回退字段、占位、`look`、`primary`、`values` 显示值映射）、`DETAIL`、`PIN`、`TEXT` 及各项上限与时长；`PAGE_LIMIT` 须不超过 `cli/config.yaml` 的 `messages.max`
- 字段：新增字段不必改 `config.ts`；已登记 `priority`（数字，默认显示）、`state`（处理状态，按 `STATES` 映射，默认显示）、`reason`、`topic` 与 `analysis`（默认隐藏）；未登记的字段按值推断（键以 `_ms` 结尾的整数为时间，全数值为数字，其余为文本）并默认显示
- 正文、发送人等一律 `textContent`、`createElement`、`createTextNode` 写入，禁 `innerHTML`；出错只显示错误 `code`，网络失败显示 `unreachable`；空值占位在 `FIELDS` 里

## 消息页

- 数据流：`Board` 持有已加载的行（按 `key` 去重、至多 `MAX_ROWS` 条），每次变化重推断列，列集合变化才重建表头（否则筛选框会丢焦点），其余只重绘表体与状态栏；时间范围经请求的 `from_ms`、`to_ms` 交给服务端，排序与筛选只在已加载行上做；默认顺序即服务端返回的时间降序，不额外排序；排序必须稳定、空值始终在最后、文本用 `Intl.Collator("zh-CN", { numeric: true })`
- `.bar` 里追加指向外发记录页的链接
- 来源列按 `SOURCES` 映射显示（`dingtalk` 钉钉、`imap` 邮箱，未登记原样），映射在 `columns.ts` 的 `content` 里做，筛选、排序、点选都按显示值；来源与账号默认显示
- 点选筛选：`PIN` 里的列（会话、发送人、来源、账号）单元格带 `pick` 类，点击不开详情；`Board.narrow` 只把被点列的筛选设为 `view.ts` 的 `pin` 结果（显示值加英文双引号），该列筛选已等于它时再点则清空；其余列的筛选与搜索保留；`narrow`、`erase` 都经 `refilter` 清空 `headKey` 重建表头，筛选框才显示新值；键盘走行的 Enter 开详情，不触发此筛选
- 筛选词被英文双引号包住时按整格精确匹配（区分大小写），否则为包含匹配
- 清除筛选：每个列标题里有 `×` 按钮（`table.ts` 的 `eraseBox`，类 `clear`），只清该列筛选；该列筛选非空才显示，由 `markFilters` 维护（`buildHead` 与 `Board.filter` 调用，与 `markSort` 同理）；没有全局清除按钮，搜索框用原生 `type=search` 的清除
- `copy.ts`：正文列（`DETAIL.body`）每个单元格的“复制”按钮，写入 `content()` 的完整原文，经 `navigator.clipboard`；成功、失败分别显示 `TEXT.copied`、`TEXT.copyFailed` 持续 `COPY_FEEDBACK_MS`；空正文不放按钮；按钮在前、正文放 `span.text`（单元格靠省略号截断，按钮放后面会被截掉）；点击 `stopPropagation`，行的 Enter 处理跳过来自按钮的事件，二者都不开详情
- `markdown.ts`：`plain`（单元格用，去标记压成一行，只取前 `PLAIN_LIMIT` 字符）、`blocks`（详情用，标题、引用、围栏代码块、段落）、`inline`（粗体、行内代码、链接）；超过 `MD_LIMIT` 的正文不格式化；正则全部带长度上限，避免病态输入的二次方耗时；图片写法按链接显示，不加载外链；只认 http、https
- `links.ts`：`anchor` 生成 `<a>`（`target=_blank`、`rel=noopener noreferrer`，`title` 只含主机+路径、不含查询参数，因地址里常带 token；`href` 保留完整地址），`new URL` 解析失败保持纯文本；地址里含 `)` 会在第一个 `)` 处截断，属已知限制
- `stamp.ts`：`stamp`（带时区）、`clock`（不带时区，用于单元格与筛选）、`zone`（`UTC+8`）、`parse`（`datetime-local` 文本→UTC 毫秒，`clock` 的逆运算）；用 `en-US` 区域（`sv-SE` 的负号是 U+2212）；缺省取浏览器时区；根规则“支持 TZ 环境变量”只适用于命令行与后端
- `badge.ts`：会话 id 经 FNV-1a 加混合生成 5×5 左右对称图案，用单个 span 的 SVG data URL 背景绘制；data URL 里不能出现 `'`、`(`、`)`（happy-dom 会丢弃整条样式），故 SVG 属性用双引号并手动转义括号
- tip：仅鼠标（`pointerType === "mouse"`），悬停 `TIP_DELAY_MS` 后显示共享的 `role="tooltip"`；只在单元格被截断（`scrollWidth > clientWidth`）或带 `data-always`（时间、会话徽记）时出现；触屏与键盘走详情 `<dialog>`（点击行或 Enter，`showModal`，关闭后焦点回到该行）

## 外发记录页

- 只读：`GET /api/outbound` 取 `CARD_LIMIT`（200）条，`state` 取下拉框的值（`OUTBOUND_STATES`：已外发 `approved`、未外发 `withheld`），切换或点“刷新”重新请求；没有通过、拒绝、取批，不自动轮询
- 标红：返回的 `placeholders` 转义后拼成带捕获组的正则切分正文，占位符放 `span.masked`（`--mask` 红色），其余用文本节点
- 每行一张卡片：批号、来源（`SOURCES` 映射）与账号、会话、发送人、时间、角色（`ROLES`：本人、自动邮件、重复转发）、脱敏文字；未外发的加 `flagged` 类并写明原因（`HELD` 映射，未登记原样）
- 安全：文字只经文本节点与 `span` 的 `textContent` 写入 `<pre>`，不渲染
- happy-dom 无全局 `Option`，下拉项用 `createElement("option")`

## 话题页

- 只读，不自动轮询；列表 `GET /api/topics` 取 `CARD_LIMIT` 条，`state` 取下拉框（空为全部，其余为 `TOPIC_STATES`），页首三行：话题各状态数量、消息各状态数量（原因按 `HELD` 映射）、已知盲区 `TEXT.blind`
- 每个话题一张卡片（类 `review topic`）：标题（空则用话题 ID）链到 `TOPIC_HASH` + 编码后的 ID，说明行为状态、优先级与紧急度（缺失显示 `TEXT.none`）、人数与会话数、来源、消息数、最近进展时间，再是最新分析（`<pre>` 文本）
- 详情 `GET /api/topics/{topic_id}`：话题卡片、时间线（每条一张卡片：时间、来源、会话、发送人、角色、归入方式 `VIAS`；状态不是 `approved` 的加 `flagged` 并写状态与原因，没有正文的不放 `<pre>`）、自动调整记录（类 `review change`：时间、类型 `CHANGE_KINDS`、起止话题与条数、原因、状态）
- 消息详情“全部字段”里 `DETAIL.topic` 的值是指向话题详情的链接（`detail.ts` 的 `jump`）

## 失败批页

- `GET /api/batches` 取 `CARD_LIMIT` 条失败批，每批一张卡片（样式类 `review`，各卡片页共用）：批号、时间、`TEXT.failedMeta`（尝试次数、已通过条数、原因）与“重发”；重发 `PATCH /api/batches/{batch_id}` 请求体 `{state:"ready"}`，成功移除卡片，失败显示错误码；不自动轮询

## 提示词页

- `GET /api/prompts` 取全部提示词，每项一张卡片（样式类 `review`，外加 `prompt`）：标签与说明取 `PROMPTS`（未登记的名字以名字作标签）、必含占位符（`required`，缺失或不止一次时加 `missing` 标红）、`textarea`（只经 `.value` 读写）、“保存”、“撤销修改”与状态文字
- 未改动或不合规时禁用保存；保存 `PATCH /api/prompts/{name}` 请求体 `{text}`，成功后成为新的基准；失败只显示错误码；不做并发冲突检测
- 有未保存修改时 `onbeforeunload` 拦截离开（赋值而非 `addEventListener`，重复渲染不叠加）

## 样式与视觉

- `style.css`：深色单主题
  - 字号只有 `--fs-1`…`--fs-4` 四个变量（桌面 12／14／16／20，手机 13／15／16／18，输入框在手机取 16 以免 iOS 聚焦放大页面），所有 `font-size` 都写成 `var(--fs-N)`，不用 `font` 简写
  - 装饰：无阴影、无渐变，圆角只有 2px；动效只有详情面板滑入，`prefers-reduced-motion` 下取消
  - 布局：表格用 `table-layout: auto`，正文列 `max-width: 0; width: 100%` 吃掉剩余宽度并省略；`.app` 的网格列必须是 `minmax(0, 1fr)`，否则单行不折行的内容会撑宽整页
  - 手机（≤640px）：`tr` 改成 flex 两行，`thead` 默认隐藏，`.panel-open` 时作为筛选面板
- `favicon.svg`：“PR” 字母图标，纯路径不依赖字体，配色取自 `style.css` 的 `--bg`、`--ion`；后端须把 svg 的 content-type 配成 `image/svg+xml`（见 cli/CLAUDE.md 的“页面”）
- 视觉验收没有截图测试：用无头 Chrome（`--headless=new --screenshot --window-size`）配合临时假数据服务截图；无头 Chrome 的窗口最小宽 500，看窄屏要把页面放进指定宽度的 iframe；假数据与脚本不入库，也不要用真实消息截图

## 测试

- `test/rig.ts` 是消息页测试共用的假接口与取值函数（`messages.test.ts`、`picks.test.ts`、`topics.test.ts` 引用），不含用例
- 测试放 `test/`，用 happy-dom 断言 DOM 与数据，不截图（`style.test.ts` 读 `style.css` 文本断言字号规则）；在测试文件内 `if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register()`（多个文件共用同一进程），不用 bunfig.toml；点击后用轮询 `until` 等待异步完成，测试里的时间显式传 `timeZone`；请求路径取自 `api/openapi.json`，不手写
- `test/contract.test.ts` 断言：生成物与 spec 同步、生成物无注释、`src/*.ts`（不含 `api.gen.ts`，不递归子目录）无 `fetch(`、`XMLHttpRequest`、`axios`

# web

## 工程

- Bun + TypeScript；依赖版本写 `*` 由 `bun.lock` 锁定；包只在 web/ 内
- tsconfig 开 strict 与 `noUncheckedIndexedAccess`，`bun x tsc --noEmit` 须通过
- TypeScript 7 无编程式 API，openapi-typescript 需 `ts.factory`，故 `typescript` 别名为 `npm:@typescript/typescript6`（库用 6.x API），`@typescript/native` 别名为 `npm:typescript` 提供 7.x 的 `tsc`；openapi-typescript 支持 7 后去掉两个别名

## 生成与调用

- `gen.ts` 用 openapi-typescript 的 Node API 由 `api/openapi.json` 生成 `src/api.gen.ts`，并删去生成器写的注释；`generate()` 同时供 `test/contract.test.ts` 比对
- 只经 `src/client.ts` 的 `connect(baseUrl, fetch?)`（内部 `createClient<paths>`）调用接口，禁手写 fetch 与请求、响应类型；路径字面量由 tsc 对照 `paths` 校验；`fetch` 可注入供测试；业务代码收 `ReturnType<typeof connect>`

## 页面

- `index.html` 是入口，`<link>` 引入 `src/style.css`，`src/main.ts` 用 `connect(location.origin)` 调 `renderMessages`；`bun run build` 把产物（含平铺的 css、js）写到 `dist/`（已忽略），由 `cli serve` 的 `web_dir` 托管，不会清旧产物；`scripts/check.ts` 的 web 步骤会跑 build
- `src/` 保持平铺（contract 测试只扫这一层；改成子目录须同步改扫描范围）；一个模块一件事：`messages.ts`（`renderMessages(client, root, timeZone?)`，只构造 `Board`）、`board.ts`（`Board`：状态、加载、重绘、事件接线）、`table.ts`（表头、表体、tip 内容）、`toolbar.ts`、`status.ts`、`detail.ts`、`tip.ts`、`copy.ts`、`columns.ts`（列推断与显示值）、`view.ts`（排序、筛选，纯函数）、`range.ts`、`markdown.ts`、`links.ts`、`badge.ts`、`stamp.ts`、`config.ts`
- `config.ts` 是前端唯一的常量源：字段呈现表 `FIELDS`（标签、类型、默认隐藏、回退字段、占位、`look`、`primary`）、详情用的列 `DETAIL`、可点击筛选的列 `PIN`、文案 `TEXT`、`PAGE_LIMIT`（须不超过 `cli/config.yaml` 的 `messages.max`）、`MAX_ROWS`、`MD_LIMIT`、`PLAIN_LIMIT`、`TIP_DELAY_MS`、`COPY_FEEDBACK_MS`、时间范围预设；新增字段不必改它，未登记的字段按值推断（键以 `_ms` 结尾的整数为时间，全数值为数字，其余为文本）并默认显示
- 数据流：`Board` 持有已加载的行（按 `key` 去重、至多 `MAX_ROWS` 条），每次变化重推断列，列集合变化才重建表头（否则筛选框会丢焦点），其余只重绘表体与状态栏；时间范围经请求的 `from_ms`、`to_ms` 交给服务端，排序与筛选只在已加载行上做；默认顺序即服务端返回的时间降序，不额外排序，排序必须稳定、空值始终在最后、文本用 `Intl.Collator("zh-CN", { numeric: true })`
- 正文、发送人等一律 `textContent`、`createElement`、`createTextNode` 写入，禁 `innerHTML`；出错只显示错误 `code`，网络失败显示 `unreachable`；空值占位在 `FIELDS` 配置里
- `markdown.ts`：`plain`（单元格用，去标记压成一行，只取前 `PLAIN_LIMIT` 字符）、`blocks`（详情用，标题、引用、围栏代码块、段落）、`inline`（粗体、行内代码、链接）；超过 `MD_LIMIT` 的正文不格式化；正则全部带长度上限，避免病态输入的二次方耗时；图片写法按链接显示，不加载外链；只认 http、https
- `links.ts`：`anchor` 生成 `<a>`（`target=_blank`、`rel=noopener noreferrer`，`title` 只含主机+路径、不含查询参数——地址里常带 token，`href` 保留完整地址），`new URL` 解析失败保持纯文本；地址里含 `)` 会在第一个 `)` 处截断，属已知限制
- `stamp.ts`：`stamp`（带时区）、`clock`（不带时区，用于单元格与筛选）、`zone`（`UTC+8`）、`parse`（`datetime-local` 文本→UTC 毫秒，`clock` 的逆运算）；用 `en-US` 区域（`sv-SE` 的负号是 U+2212）；缺省取浏览器时区。项目“支持 TZ 环境变量”的规则只适用于命令行与后端，浏览器读不到该变量
- 点击会话、发送人：`config.ts` 的 `PIN` 里的列，单元格带 `pick` 类，点击不开详情，`Board.narrow` 只把被点列的筛选设为 `view.ts` 的 `pin` 结果（显示值加英文双引号），该列筛选已等于它时再点则清空（切换）；其余列的筛选与搜索保留；筛选词被英文双引号包住时按整格精确匹配（区分大小写），否则为包含匹配；`narrow`、`erase` 都经 `refilter` 清空 `headKey` 重建表头，筛选框才显示新值；键盘走行的 Enter 开详情，不触发此筛选
- 清除筛选：每个列标题里有 `×` 按钮（`table.ts` 的 `eraseBox`，类 `clear`），只清该列筛选；该列筛选非空才显示，由 `markFilters` 维护（`buildHead` 与 `Board.filter` 调用，与 `markSort` 同理）；没有全局清除按钮，搜索框用原生 `type=search` 的清除
- `copy.ts`：正文列（`DETAIL.body`）每个单元格的“复制”按钮，写入 `content()` 的完整原文（不是占位或压缩后的文本），经 `navigator.clipboard`；成功、失败分别显示 `TEXT.copied`、`TEXT.copyFailed` 持续 `COPY_FEEDBACK_MS` 后恢复；空正文不放按钮；按钮在前、正文放 `span.text`（单元格靠省略号截断，按钮放后面会被截掉）；点击 `stopPropagation`，行的 Enter 处理跳过来自按钮的事件，二者都不开详情
- `favicon.svg`：“PR” 字母图标，纯路径不依赖字体，配色取自 `style.css` 的 `--bg`、`--ion`；`index.html` 用 `<link rel="icon">` 引用，`bun build` 会按哈希重命名并改写引用；后端须把 svg 的 content-type 配成 `image/svg+xml`（见 cli/CLAUDE.md 的页面）
- `badge.ts`：会话 id 经 FNV-1a 加混合生成 5×5 左右对称图案，用单个 span 的 SVG data URL 背景绘制；data URL 里不能出现 `'`、`(`、`)`（happy-dom 会丢弃整条样式），故 SVG 属性用双引号并手动转义括号
- tip：仅鼠标（`pointerType === "mouse"`），悬停 `TIP_DELAY_MS` 后显示共享的 `role="tooltip"`；只在单元格被截断（`scrollWidth > clientWidth`）或带 `data-always`（时间、会话徽记）时出现；触屏与键盘走详情 `<dialog>`（点击行或 Enter，`showModal`，关闭后焦点回到该行）
- `style.css`：深色单主题；字号只有 `--fs-1`…`--fs-4` 四个变量（桌面 12／14／16／20，手机 13／15／16／18，输入框在手机取 16 以免 iOS 聚焦放大页面），所有 `font-size` 都写成 `var(--fs-N)`，不用 `font` 简写；无阴影、无渐变，圆角只有 2px；动效只有详情面板滑入，`prefers-reduced-motion` 下取消；表格用 `table-layout: auto`，正文列 `max-width: 0; width: 100%` 吃掉剩余宽度并省略；手机（≤640px）把 `tr` 改成 flex 两行，`thead` 默认隐藏，`.panel-open` 时作为筛选面板；`.app` 的网格列必须是 `minmax(0, 1fr)`，否则单行不折行的内容会撑宽整页
- 视觉验收没有截图测试：用无头 Chrome（`--headless=new --screenshot --window-size`）配合临时假数据服务截图；无头 Chrome 的窗口最小宽 500，看窄屏要把页面放进指定宽度的 iframe；假数据与脚本不入库，也不要用真实消息截图

## 测试

- 测试放 `test/`，用 happy-dom 断言 DOM 与数据，不截图（`style.test.ts` 读 `style.css` 文本断言字号规则）；在测试文件内 `if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register()`（多个文件共用同一进程），不用 bunfig.toml；点击后用轮询 `until` 等待异步完成，测试里的时间显式传 `timeZone`；请求路径取自 `api/openapi.json`，不手写
- `test/contract.test.ts` 断言：生成物与 spec 同步、生成物无注释、`src/*.ts`（不含 `api.gen.ts`，不递归子目录）无 `fetch(`、`XMLHttpRequest`、`axios`

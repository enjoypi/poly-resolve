# web

## 工程

- Bun + TypeScript；依赖版本写 `*` 由 `bun.lock` 锁定；包只在 web/ 内
- tsconfig 开 strict 与 `noUncheckedIndexedAccess`，`bun x tsc --noEmit` 须通过
- TypeScript 7 无编程式 API，openapi-typescript 需 `ts.factory`，故 `typescript` 别名为 `npm:@typescript/typescript6`（库用 6.x API），`@typescript/native` 别名为 `npm:typescript` 提供 7.x 的 `tsc`；openapi-typescript 支持 7 后去掉两个别名

## 生成与调用

- `gen.ts` 用 openapi-typescript 的 Node API 由 `api/openapi.json` 生成 `src/api.gen.ts`，并删去生成器写的注释；`generate()` 同时供 `test/contract.test.ts` 比对
- 只经 `src/client.ts` 的 `connect(baseUrl, fetch?)`（内部 `createClient<paths>`）调用接口，禁手写 fetch 与请求、响应类型；路径字面量由 tsc 对照 `paths` 校验；`fetch` 可注入供测试；业务代码收 `ReturnType<typeof connect>`

## 页面

- `index.html` 是入口，`src/main.ts` 用 `connect(location.origin)` 调 `renderMessages`；`bun run build` 把产物写到 `dist/`（已忽略），由 `cli serve` 的 `web_dir` 托管；`scripts/check.ts` 的 web 步骤会跑 build
- `src/messages.ts`：`renderMessages(client, root, timeZone?)` 返回首次加载的 Promise；正文、发送人等一律 `textContent` 写入，禁 `innerHTML`；出错只显示错误 `code`，网络失败显示 `unreachable`；占位文案 `[非文本消息]` 常量在此文件
- `src/stamp.ts`：`stamp(millis, timeZone?)` 输出 `2026-10-28 09:20:09 UTC+8`；用 `en-US` 区域（`sv-SE` 的负号是 U+2212）；缺省取浏览器时区。项目“支持 TZ 环境变量”的规则只适用于命令行与后端，浏览器读不到该变量

## 测试

- 测试放 `test/`，用 happy-dom 断言 DOM 与数据，不截图；在测试文件内 `if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register()`（多个文件共用同一进程），不用 bunfig.toml；点击后用轮询 `until` 等待异步完成，测试里的时间显式传 `timeZone`；请求路径取自 `api/openapi.json`，不手写
- `test/contract.test.ts` 断言：生成物与 spec 同步、生成物无注释、`src/*.ts`（不含 `api.gen.ts`，不递归子目录）无 `fetch(`、`XMLHttpRequest`、`axios`

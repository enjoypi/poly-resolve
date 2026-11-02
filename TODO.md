# TODO

- 按真实流量调 `triage.questions.topic.min_confidence`，观察决策接口限速；无积压后逐步放大 `triage.limits` 的 `batch`、`max_handoffs_per_hour` 等限额，限速则调小 `batch`；观察期间通知改发本人测试会话
- 用真实密文验证 dws 并发解密：同时跑多个 `dws chat crypto decrypt`（含跨过令牌过期点），查 `~/.local/bin/.dws-runtime.lock` 与报错；无冲突再调大 `ingest.sources[].dingtalk.dws.parallel`（垃圾输入只得到 `key request already in progress`，无法判断）
- IMAP 长连接无新信时被断开按 `Retry::Failed` 递增退避，可涨到 `backoff_max_ms`；完成过一轮 IDLE 的会话结束应复位退避（`adapters/src/imap.rs` 的 `session`）
- `adapters/src/imap/wire.rs` 的 `more()` 把读错误归为 `imap:eof`，区分不出连接被重置与正常关闭；改为返回 `io:<kind>`
- `web/test/badge.test.ts` 用了真实钉钉会话 ID（自 `bf21600` 起公开）：换成假值，改写历史并强推


## 用户角度

- 前端增加修改发给 AI 的 prompt 的配置页面
- 前端增加来源类型列（也可以筛选、排序）
- 深入思考下还可以支持哪些数据源？twitter是否可以？

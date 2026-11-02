# TODO

- 按真实流量调 `triage.questions.topic.min_confidence`，观察决策接口限速；无积压后逐步放大 `triage.limits` 的 `batch`、`max_handoffs_per_hour` 等限额，限速则调小 `batch`；观察期间通知改发本人测试会话
- 用真实密文验证 dws 并发解密：同时跑多个 `dws chat crypto decrypt`（含跨过令牌过期点），查 `~/.local/bin/.dws-runtime.lock` 与报错；无冲突再调大 `ingest.sources[].dingtalk.dws.parallel`（垃圾输入只得到 `key request already in progress`，无法判断）
- `web/test/badge.test.ts` 用了真实钉钉会话 ID（自 `bf21600` 起公开）：换成假值，改写历史并强推

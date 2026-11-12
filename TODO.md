# TODO

- 线上已于 2026-11-12 部署并切自动通过加本地模型筛查；筛查地址暂以字面量写在线上配置的 `triage.screen.http.url`，把 `SCREEN_URL` 加进 pm3 的加密环境后改回 `${SCREEN_URL}`
- 抽查 `misses` 表：`unmasked`、`unscreened` 样本里误判多则改 `screen.md`；`rules` 样本里反复出现的模式写成 `triage.redact.patterns` 规则
- 运行一周后按真实流量复核 `questions.threshold`（现 0.85）、`max_handoffs_per_hour`（现 3）与 `triage.questions.topic.min_confidence`，观察决策接口限速；无积压后逐步放大 `triage.limits` 的 `batch` 等限额，限速则调小 `batch`；观察期间通知改发本人测试会话

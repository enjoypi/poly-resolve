# TODO

- 按真实流量调 `triage.questions.topic.min_confidence`，观察决策接口限速；无积压后逐步放大 `triage.limits` 的 `batch`、`max_handoffs_per_hour` 等限额，限速则调小 `batch`；观察期间通知改发本人测试会话

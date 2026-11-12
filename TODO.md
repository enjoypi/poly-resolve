# TODO

- 线上已于 2026-11-12 部署并切自动通过加本地模型筛查；筛查地址暂以字面量写在线上配置的 `triage.screen.http.url`，把 `SCREEN_URL` 加进 pm3 的加密环境后改回 `${SCREEN_URL}`
- 抽查 `misses` 表：`unmasked`、`unscreened` 样本里误判多则改 `screen.md`；`rules` 样本里反复出现的模式写成 `triage.redact.patterns` 规则
- 运行一周后按真实流量复核 `questions.threshold`（现 0.85）与 `triage.questions.topic.min_confidence`，观察决策接口限速；限速则调小 `batch`；观察期间通知改发本人测试会话
- 上线话题第 ① 期前改线上配置：删 `triage.review` 与 `limits` 的 `backlog_max_ms`、`handoff_cooldown_ms`、`max_decisions_per_hour`、`max_handoffs_per_hour`；加 `questions.topic.none_label`、`session.omitted`、`screen.min_chars`；各邮箱加发件文件夹数据源；授权范围补上本人消息与自动邮件
- 上线后首次启动会重新处理约 1500 条历史消息（此前跳过的本人、过期与积压上限外的消息），只归入话题不通知；观察决策接口是否限速
- 话题第 ②③④ 期按根目录 `mighty-sauteeing-babbage.md` 继续

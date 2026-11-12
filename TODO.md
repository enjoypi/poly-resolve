# TODO

- 线上已于 2026-11-12 部署并切自动通过加本地模型筛查；筛查地址暂以字面量写在线上配置的 `triage.screen.http.url`，把 `SCREEN_URL` 加进 pm3 的加密环境后改回 `${SCREEN_URL}`
- 抽查 `misses` 表：`unmasked`、`unscreened` 样本里误判多则改 `screen.md`；`rules` 样本里反复出现的模式写成 `triage.redact.patterns` 规则
- 运行一周后按真实流量复核 `questions.threshold`（现 0.85）与 `triage.questions.topic.min_confidence`，观察决策接口限速；限速则调小 `batch`；观察期间通知改发本人测试会话
- 上线话题第 ① 期前改线上配置：删 `triage.review` 与 `limits` 的 `backlog_max_ms`、`handoff_cooldown_ms`、`max_decisions_per_hour`、`max_handoffs_per_hour`；加 `questions.topic.none_label`、`session.omitted`、`screen.min_chars`；各邮箱加发件文件夹数据源；授权范围补上本人消息与自动邮件
- 上线后首次启动会重新处理约 1500 条历史消息（此前跳过的本人、过期与积压上限外的消息），只归入话题不通知；观察决策接口是否限速
- 上线话题第 ② 期前改线上配置：`triage.notice` 改成 `{late, late_after_ms, states, template}`（模板里 `{probability}` 换成 `{score}`）；删 `sizes.session_max_ms`、`sizes.topics_max_open`；加 `triage.topics`（照 `cli/config.yaml`）；线上提示词目录的 `session.md`、`followup.md` 换成仓库里的新版（回复第一行为 JSON，否则分析一律失败）
- 第 ② 期上线后首次启动会把全部历史行补建线索索引，旧邮件的会话标识改成回复链；观察 `subject` 线索是否把无关邮件并进同一话题，是则加进 `topics.clues.subject.ignore`
- 话题第 ③④ 期按根目录 `mighty-sauteeing-babbage.md` 继续

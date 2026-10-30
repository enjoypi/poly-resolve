# TODO

- 按真实流量调 `triage.questions.topic.min_confidence`，观察决策接口限速；无积压后逐步放大 `triage.limits` 的 `batch`、`max_handoffs_per_hour` 等限额，限速则调小 `batch`
- 观察学到的字面量规则的实际命中情况（`learned_rules`），把反复出现的同类漏脱敏提炼成正则写进 `cli/config.yaml` 的 `triage.redact.patterns`，并补 `cli/tests/it/redact.rs` 的样例
- 主链路稳定后真实验证本地模型筛查：下载 GGUF 权重、启动 `llama-server`，在 `cli/config.yaml` 加 `triage.screen`（`exec` 用 `curl` 调 `http://localhost:<端口>/v1/chat/completions`），用含假密钥的合成消息核对返回 `choices[0].message.content` 是 `{lines[{line,risk,spans,topics}]}`，并在 `log stream` 里按 `deny(1)` 记录补全 `cli/sandbox/screen.sb`；与 `domain/src/features/triage/screened.rs` 的假设不符则改该文件与 `fake-triage.sh`
- intake 只取 `lookback_ms` 内的消息，停用超过该时长后积压的消息永不分级，与需求“不遗漏”冲突：补上停用期间的积压
- 限额触顶、冷却压掉的转交、卡住的批都不写日志，只能查库发现：补日志
- `failed` 的批不自动重试，其中已通过的行没有优先级：补重试或人工重发入口
- 用真实密文验证 dws 并发解密：同时跑多个 `dws chat crypto decrypt`（含跨过令牌过期点），查 `~/.local/bin/.dws-runtime.lock` 与报错；无冲突再调大 `ingest.sources[].dingtalk.dws.parallel`（垃圾输入只得到 `key request already in progress`，无法判断）
- 脱敏正则每条消息现编译一遍：`triage.line` 改批量，每轮只编译一次

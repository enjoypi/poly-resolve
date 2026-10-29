# TODO

- 决策接口的返回格式已验证（`answers[]` 同序、`choice` 为选项 `value`、带 `confidence`）；开启后按真实流量调 `triage.questions.topic.min_confidence`，并观察速率限制，不稳就调小 `triage.limits.batch`
- 观察学到的字面量规则的实际命中情况（`learned_rules`），把反复出现的同类漏脱敏提炼成正则写进 `cli/config.yaml` 的 `triage.redact.patterns`，并补 `cli/tests/it/redact.rs` 的样例
- 真实验证本地模型筛查：下载 GGUF 权重、启动 `llama-server`，在 `cli/config.yaml` 加 `triage.screen`（`exec` 用 `curl` 调 `http://localhost:<端口>/v1/chat/completions`），用含假密钥的合成消息核对返回 `choices[0].message.content` 是 `{lines[{line,risk,spans,topics}]}`，并在 `log stream` 里按 `deny(1)` 记录补全 `cli/sandbox/screen.sb`；与 `domain/src/features/triage/screened.rs` 的假设不符则改该文件与 `fake-triage.sh`

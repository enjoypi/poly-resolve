# TODO

- 真实验证 codex：`sops exec-env secrets.enc.yaml 'cd .debug/codex-work && CODEX_HOME=../codex-home codex exec --json --skip-git-repo-check -s read-only "回复OK"'`，核对首个 `thread.started` 带 `thread_id`、回复在 `item.completed` 的 `agent_message`；再用该 ID 跑 `codex exec … resume <ID> "回复OK2"`，核对 `-s`、`-C` 写在 `resume` 前同样生效（不生效改用 `-c sandbox_mode="read-only"`，必要时给 `Exec` 加 cwd）；与 `domain/src/features/triage/reply.rs` 的假设不符则改该文件与 `fake-triage.sh`
- 真实验证决策接口的话题归并：用合成的几条跨会话消息调一次含 `predicate` 与 `choice` 的请求，核对 `answers[]` 与问题同序、`choice` 取值是选项的 `value`、`confidence` 的量级；据此调 `triage.questions.topic.min_confidence`，并观察速率限制，不稳就调小 `triage.limits.batch`；与 `domain/src/features/triage/sort.rs` 的假设不符则改该文件与 `fake-triage.sh`
- 观察学到的字面量规则的实际命中情况（`learned_rules`），把反复出现的同类漏脱敏提炼成正则写进 `cli/config.yaml` 的 `triage.redact.patterns`，并补 `cli/tests/it/redact.rs` 的样例
- 真实验证本地模型筛查：下载 GGUF 权重、启动 `llama-server`，在 `cli/config.yaml` 加 `triage.screen`（`exec` 用 `curl` 调 `http://localhost:<端口>/v1/chat/completions`），用含假密钥的合成消息核对返回 `choices[0].message.content` 是 `{lines[{line,risk,spans,topics}]}`，并在 `log stream` 里按 `deny(1)` 记录补全 `cli/sandbox/screen.sb`；与 `domain/src/features/triage/screened.rs` 的假设不符则改该文件与 `fake-triage.sh`

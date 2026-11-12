# TODO

- 线上切自动通过并用本地模型兜底（改 `~/.config/poly-resolve/` 下的配置与提示词，每步重启 `pm3 restart poly-resolve`）：
  1. 部署新版本：`cargo +stable test --release --locked && cargo +stable build --release --locked`，替换 `~/.local/bin/poly-resolve`
  2. 本地模型（`llama-server` 已由 pm3 托管为 `llama-screen`，`screen.md` 已放入线上提示词目录）：pm3 环境加 `SCREEN_URL=http://127.0.0.1:8089/v1/chat/completions`；配置加 `triage.screen`（`parallel: 4`、`timeout_ms: 300000`）与 `limits.retry_max_ms: 900000`；`screen.md` 换成 README 里的新提示词
  3. 提示词与模板：`urgent.md`、`session.md`、`followup.md` 与 `triage.notice` 同步仓内 `cli/prompts/`、`cli/config.yaml` 的新内容
  4. 排空积压：`questions.threshold` 设 1.01、`review.mode` 设 `auto`；在 `#reviews` 把 11-06 取的 100 条待审整批通过；等 `lines` 的 `queued` 降到接近 0
  5. `questions.threshold` 改为 0.85
- 运行一周后按真实流量复核 `questions.threshold` 与 `triage.questions.topic.min_confidence`，观察决策接口限速；无积压后逐步放大 `triage.limits` 的 `batch`、`max_handoffs_per_hour` 等限额，限速则调小 `batch`；观察期间通知改发本人测试会话

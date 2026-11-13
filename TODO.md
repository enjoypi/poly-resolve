# TODO

- 各邮箱加发件文件夹数据源：需要两个邮箱发件文件夹的准确名字；`account` 另取（如 `t4f-sent`），`initial_backfill_ms` 先用 1 天；上线后看日志里新账号是否反复出现 `imap session ended`（文件夹名写错时如此）
- 2026-11-14 起（上线次日）：
  - `misses` 表按 `kind` 与日期计数；`rules` 里同一片段出现 3 次以上，请用户看原文后写成 `triage.redact.patterns` 规则；`unscreened` 超过同期筛查行数 5% 再调大 `screen.max_tokens`（现 512）；`unmasked` 超过 2% 改 `screen.md`
  - `changes` 表按类型、原因、日期计数，随机抽 10 条在话题详情页核对；误合并 2 条以上把 `topics.merge_after` 调到 3 或改 `review.md`，误拆 2 条以上把 `topics.split_after` 调到 3
  - `subject` 线索：跨 3 个以上会话而互不相关的主题加进 `topics.clues.subject.ignore`
- 2026-11-20 起（上线一周后）：看话题分数分布、每日通知数、决策接口非 200 的次数，复核 `questions.threshold`（现 0.85）、`questions.topic.min_confidence`、`topics.clues.max_conversations`；决策接口限速则调小 `limits.batch`
- 上线本次改动：`cli/config.yaml` 已加 `triage.limits.daily_tokens`、`down_after_ms`，`triage.notice.alarm`，`triage.session.purge`，`triage.topics.retain`，重启前确认 `codex delete` 在沙箱内能跑；`urgent.md` 于 2026-11-13 10:51 UTC 起换成填好关注点的版本（决策提示词标识 `80a4b691de93`，此前为 `19ddd7cd968c`），紧急度分布会变，2026-11-20 复核 `questions.threshold` 时按 `calls.version` 分开统计；观察一周每日 token 用量后定 `daily_tokens` 初值（现为 0 不检查）
- 线上备份 `~/.local/share/poly-resolve/backup-20261113/` 含明文消息，观察期结束（2026-11-20）后删除

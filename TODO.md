# TODO

- 各邮箱加发件文件夹数据源：需要两个邮箱发件文件夹的准确名字；`account` 另取（如 `t4f-sent`），`initial_backfill_ms` 先用 1 天；上线后看日志里新账号是否反复出现 `imap session ended`（文件夹名写错时如此）
- 2026-11-14 起（上线次日）：
  - `misses` 表按 `kind` 与日期计数；`rules` 里同一片段出现 3 次以上，请用户看原文后写成 `triage.redact.patterns` 规则；`unscreened` 超过同期筛查行数 5% 再调大 `screen.max_tokens`（现 512）；`unmasked` 超过 2% 改 `screen.md`
  - `changes` 表按类型、原因、日期计数，随机抽 10 条在话题详情页核对；误合并只数新决策提示词生效（2026-11-13 13:56 UTC）之后的，2 条以上把 `topics.merge_after` 调到 3 或再改 `review.md`，误拆 2 条以上把 `topics.split_after` 调到 3
  - `subject` 线索：跨 3 个以上会话而互不相关的主题加进 `topics.clues.subject.ignore`
- 2026-11-20 起（上线一周后）：看话题分数分布、每日通知数、决策接口非 200 的次数，复核 `questions.threshold`（现 0.85）、`questions.topic.min_confidence`、`topics.clues.max_conversations`；决策接口限速则调小 `limits.batch`
- 提示词标识：决策提示词 2026-11-13 10:51 UTC 起为 `80a4b691de93`（此前 `19ddd7cd968c`），13:56 UTC 起五个提示词与通知模板全部更换，决策与分析（此前 `80f7ded3e669`）的新标识待首次调用后从 `calls.version` 查出补记于此；紧急度分布会变，2026-11-20 复核 `questions.threshold` 时按三个决策标识分开统计，每日通知数按分析标识分开统计；首条新版通知请用户确认钉钉里的分段效果，不理想则调 `triage.notice.template` 与 `session.md`、`followup.md` 的正文格式；观察一周每日 token 用量（来源页，上线首日分析输入约 446 万、决策约 3 万）后定 `daily_tokens` 初值（现为 0 不检查）
- 用量与多余通知的根因（提示词治不了，须改代码或配置）；`daily_tokens` 初值待处理后再定：
  - 冷却期内合并的通知只带最新一条的变化要点
  - 话题已有 codex 会话时每批新消息都再分析，不看分数、无冷却（首日两个话题各分析 45、34 次）
  - 追加分析既续写旧会话又重发全量消息，单次输入每次多一份全量（告警群话题每次约多 2.2 万 token）
  - 同会话 `session_gap_ms`（2 小时）内的消息不经判断直接并入，告警群与闲聊群的话题断不开
  - 给 codex 的消息时间只有 UTC（`transcript.rs` 偏移写死 0），换算 UTC+8 靠模型；通知的晚到说明也是 UTC
- 审查修复（同话题只留一个待分析项、已结束话题不在落结果时重开、旧 codex 会话随保留期清除等）上线后在此补记上线时间（UTC），2026-11-20 统计用量与每日通知数时按它分段；升级前分析失败留下的，以及此后超时或停机打断时已建的 codex 会话，库里没有记录，按 `CODEX_HOME` 手工清
- 2026-11-20 起：按 `messages.raw` 是否带 `event_id` 统计钉钉本人消息经实时通道与补拉到达的条数（2026-11-14 时 270 条里 229 条为实时），查明 11-06 至 11-11 本人消息为 0 的原因，再议是否定期补拉与补拉起点（现起点为库内水位减重叠，水位被他人消息推前，较早的本人消息补不回）
- 钉钉重连后补拉失败且 `down_after_ms` 内没有任何消息时会误报一次“采集异常”；待确认真实 dws 监听的连接状态输出后改按它记 `ok`
- 线上备份 `~/.local/share/poly-resolve/backup-20261113/`、`backup-20261113-2/`、`backup-20261113-135602/` 含明文消息，观察期结束（2026-11-20）后删除

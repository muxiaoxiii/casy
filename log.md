# Loop Log — 文档审查：离线提醒决策收口

日期：2026-08-14

## 完成内容

### 1. 现状核验（源码 vs 文档）
- 提醒系统现状：`reminder_rules`/`reminder_log` 已落库（schema v5），`commands/reminder.rs` 命令已注册；但 `start_reminder_engine` **未在应用启动时拉起**，飞书消息/任务通道在 `dispatch_reminder` 中是**占位**（只写日志、不真正调 API，`send_feishu_reminder_async`/`create_feishu_task_reminder_async` 为 dead code）；前端无提醒 UI；飞书 `AutoPushManager` 是数据同步而非提醒。
- 补偿扫描不健壮：`deadline_before/deadline_on/deadline_after/hearing_before` 用 `days_diff == trigger_days` 严格相等，离线错过的 T-N 天无法补发；`task_due/task_overdue` 用 `<=` 可补。
- 据此修正了两份文档中"多通道提醒已可用"等过度承诺。

### 2. docs/architecture.md（v2.9 → v2.10）
- 新增**第九章「离线提醒与准时送达决策」**：四层语义（计算正确/进入发送队列/服务端接受/终端实际送达与用户确认）、当前能力边界、方案 A/B/C 比较、M0/M1/移动端推荐、`reminder_jobs` 作业数据结构与状态机、桌面端与云端职责与冲突规则、凭据/隐私/高敏案件、端到端时序与故障恢复、SLO/验收、飞书 API 接受 ≠ 用户已读。
- 后续章节重排：九→十、十→十一、十一→十二、十二→十三、十三→十四（含子节号）。
- §2.2/§4.1/§5.2/§8.x/§10.2/§12.2/§14 补充提醒与凭据边界、幂等、崩溃恢复、回滚、验收条款；§3.3 登记目标态未实现项清单（DomainCommand/ContextPolicy/ai_runs/双路径路由表/事件总线/reminder_jobs）。

### 3. docs/casy-design-philosophy.md（v2.1 → v2.2）
- 修复失效交叉引用：§5.1→§5、§4.7→§5.2、§3.2→§3.3、§3.1→§6.3、§3.14→§3.3、§3.0→§3.3；移除"早期遗留建表 SQL 段落"过期说明。
- 11.1 改写为按源码核验的真实现状；11.2 提醒等级 **L1-L4 → R1-R4**（避免与确认等级 L1-L3 混淆），新增 `#### 离线提醒与送达语义（v2.2 收口）`。
- 11.12 表修正"多通道提醒"过度承诺；13.1 M0 验收新增"提醒尽力而为"项；P2/P3/P4 路线图新增提醒引擎启动拉起+启动补偿、最小云端提醒中继 v1、移动端（条件立项）行及 v2.2 路线说明。

### 4. 一致性
- 两份文档统一术语：四层语义、R1-R4、`reminder_jobs` 状态机、飞书 API 接受 ≠ 用户已读、M0（尽力而为）/M1（云端中继）/移动端（非前置条件）、高敏案件三档通知策略。
- 明确回答：**不需要为离线准时推送开发移动端**；移动端仅服务独立需求。

## 检查
- `git diff --check -- docs/casy-design-philosophy.md docs/architecture.md` 通过。
- 两份文档交叉引用、版本/日期、现状标注已核对；仅修改了这两份文档与 log.md（prompt.md 保留原任务指令）。

## 主审补充修正
- 修复“云端不保存任何通道凭据却可在桌面端离线时代发”的矛盾：明确托管应用与用户自管通道两种凭据模式，发送能力进入 Secret Manager/KMS，并与作业、日志隔离。
- 作业 ID 改为不可变；改期采用“取消旧作业 + 新 ID + `supersedes_id`”，避免复用幂等键。
- 增加 `executor=local/cloud` 互斥执行、IANA `timezone`、`lease_expires_at`、`next_attempt_at`、`version` 等落地字段，消除双发、夏令时和乱序更新风险。
- SLO 改为可监控的月度服务目标，并明确飞书通道接受、终端送达和用户已读均不是绝对保证。
- 明确承诺从“云端确认入队”开始；未同步的本地变更和 Casy 从未接入的新数据不在承诺范围。增加 `delivery_unknown`，避免飞书超时后盲目重试造成双发。

## Cycle 01
### run (2026-08-14T09:52:35Z)
Now let me examine the actual source code to verify implementation boundaries, especially reminders, autoPush, and feishu sync.
Now let me search both target documents for all reminder/notification-related content to find conflicts and over-claims.
Let me check the working-tree diffs of the two target files to avoid clobbering existing changes.
I now have a complete picture. Let me set up the task list and begin.
Starting with architecture.md. First the header and the new chapter.
Now insert the new chapter 九 (offline reminder decision) and renumber the following chapters.
Now let me update architecture.md §3.3 目标态边界 so the philosophy doc's cross-references have valid targets.
architecture.md is complete. Now the philosophy doc. Starting with the header and cross-references.
Now the key part: rewrite §11.1/§11.2 with accurate status and add the offline reminder subsection. Let me read the current section.
Now let me update log.md and prompt.md, then run final checks.
All checks pass. Task complete.

DONE: 收口离线提醒决策——architecture.md 新增第九章（四层语义/A/B/C 方案、M0 本地尽力而为 + M1 飞书接收端 + 最小云端中继、移动端非前置条件、reminder_jobs 状态机、时序/故障/SLO/隐私、飞书 API 接受≠用户已读）；philosophy v2.2 修正现状表述、提醒等级改 R1-R4、修复全部失效交叉引用并同步路线图；两文档术语一致，`git diff --check` 通过。

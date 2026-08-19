# Casy 架构设计

> 版本: v2.11
> 日期: 2026-08-14
> 状态: 按当前仓库实现校准
> 关联: `Casy-SPEC.md` / `Casy-STATUS.md` / `README.md` / `docs/modules/`（模块设计文档，见 `modules/00-README.md`）/ `docs/casy-design-philosophy.md`
>
> **v2.11（离线提醒·日历同步收口）**：第九章在 v2.10 基础上修订——本地进程确实无法在关机/休眠/退出/断网时触发提醒（技术事实），但"离线准时提醒"不依赖 Casy 自己实现：**新增方案 D「日历/日程同步（CalDAV + ICS via Email）」作为 M1 首选**，把提醒固化为日程事件同步到用户已有的 Google/Apple/Outlook 日历，由日历服务商 24/7 在线的推送基础设施负责准时送达。原方案 B「自建云端中继」降级为备选。`reminder_jobs.executor` 从 local/cloud 改为 local/calendar；凭据（CalDAV/OAuth/SMTP）只存本地 OS keyring，不存在 Casy 自建云端。
>
> **v2.10（离线提醒语义）**：明确 Casy 是本地优先桌面应用、非全天运行，本地定时器/托盘/飞书自动 PUSH 都无法在关机、休眠、退出、断网时保证准时提醒；区分「计算正确 / 进入发送队列 / 服务端接受 / 终端实际送达与用户确认」四层语义；`docs/casy-design-philosophy.md` 的提醒等级统一改名 R1-R4，避免与 AI 确认等级 L1-L3 混淆。

## 一、文档定位与边界

本文档是 Casy 当前仓库的顶层技术架构说明，目标是把三件事说清楚：

1. 当前源码已经实现了什么，哪些能力只能写成“现状”。
2. 接下来允许继续演进到什么目标态，哪些仍然只是设计约束。
3. 设计文档之间必须共享哪些统一术语、数据模型、命令名和状态机。

判定优先级如下：

1. 运行中的源码与数据库迁移是实现边界的最终依据。
2. 产品与模块承诺优先参考 `Casy-SPEC.md`、`Casy-STATUS.md`、`README.md`。
3. 本文档与专题设计文档必须服从前两者，不能把未实现能力写成既成事实。

本轮核验结论：

- 当前仓库已经包含前端、Tauri 命令、SQLite 迁移、同步、AI、收件箱批处理等真实实现，不再是“规格先行、尚未落地”的纯文档仓库。
- 当前主架构仍然是“Vue/Pinia + Tauri command + Rust 模块 + SQLite/文件系统/外部服务”。
- 不存在已落地的 `DomainCommand` 统一写入口、MCP Server、Harness、外置记忆蒸馏、AI 审计表 `ai_runs`/`ai_context_items` 等体系；这些内容不能写成当前架构事实。

## 二、现状总览

### 2.1 当前可确认的实现范围

截至 2026-08-14，仓库中可以直接核验到的主要实现包括：

- Vue 3 + Pinia + Vue Router 前端，覆盖案件、任务、日历、收件箱、文书工坊、知识库、同步、设置等模块。
- Tauri 2 + Rust 后端，`src-tauri/src/commands/mod.rs` 当前注册 `137` 个命令。
- SQLite + SQLCipher 数据层，`PRAGMA user_version = 8`，带增量迁移。
- WebDAV 同步、飞书同步、IMAP 监听、文件夹 watcher、系统托盘、全局快捷键。
- AI 分类/提取/写作辅助，以及收件箱批量处理队列的第一版实现。
- 双轨/三轨状态机字段与 `case_track_history` 审级历程表。

### 2.2 当前尚未形成统一平台层的部分

以下能力有局部实现，但还没有形成本文档此前声称的“统一平台层”：

- 后端写入目前并非统一经由领域服务，大量命令仍直接 `open_db()` 并执行 SQL。
- 批处理、Watcher、IMAP、飞书自动推送等后台能力以各自模块维护，没有统一 job runtime。
- 提醒系统处于“数据层已有、调度未接通”的中间态：`reminder_rules` / `reminder_log` 表与 `commands/reminder.rs` 的命令已存在，但 `start_reminder_engine` 并未在应用启动时拉起，飞书消息/任务通道在 `dispatch_reminder` 中只是占位（仅写日志、不真正调用飞书 API），前端也没有提醒规则/日志界面（详见第四章、第九章）。
- 可观测性以日志和少量前端事件为主，还没有统一 metrics、trace、审计事件总线。
- 隐私分级与脱敏策略没有落实为通用策略引擎，主要依赖“本地优先 + 用户配置 + 模块内判断”。

## 三、系统分层

### 3.1 当前实际分层

```text
Vue 页面 / 组件
  -> Pinia stores + tauriBridge
  -> Tauri commands
  -> Rust 模块（db / deadline / formula / sync / ai / parse / files / email / watcher / docsy_engine）
  -> SQLite(SQLCipher) / 本地文件系统 / Feishu / WebDAV / AI API / IMAP
```

### 3.2 各层职责

| 层 | 当前职责 | 当前限制 |
|---|---|---|
| 前端表现层 | 页面、表单、筛选、批量操作入口、轮询进度、事件监听 | 仍有部分页面直接依赖具体命令名，缺少统一任务/作业抽象 |
| Store/Bridge | 聚合 UI 状态，统一 `tauriCallSafe` 调用 | 仅对前端调用做封装，不是后端边界 |
| Tauri Commands | 输入校验、调度后端模块、返回 JSON/结构体 | 仍承载不少业务分支，不是纯薄壳 |
| Rust 模块 | 数据访问、公式、期限、同步、AI、收件箱处理、文件归档 | 模块边界清晰度不一，很多写路径仍是“命令 + SQL”直连 |
| 持久化/外部适配层 | SQLite、文件系统、WebDAV、Feishu、AI、IMAP | 缺少统一幂等/审计/任务运行时 |

### 3.3 目标态边界

本文档认可的目标态是“在现有分层基础上逐步收口”，不是重造另一套平台。允许推进的方向：

- 将关键写入路径逐步收口到可复用的 Rust 服务函数。
- 将后台批处理、同步、提醒逐步抽象为一致的作业状态模型。
- 增补审计、日志、恢复与验收标准。

以下设计哲学文档中提到的机制，本文档统一登记为**目标态（未实现）**，任何情况下不得写成现状：

- **DomainCommand 统一写入口**：所有写操作走领域事务、事件在事务内发出（设计哲学 §11.11 引用）；当前仍以 `open_db()` + 命令直连为主（见 §6.2）。
- **ContextPolicy（上下文治理）**：required_sources / max_depth / max_items / token_budget / sensitivity_scope / snapshot_version 的通用框架未实现，当前为模块内判断。
- **AI 审计表 `ai_runs` / `ai_context_items` / `audit_events`**：未落库，设计草案。
- **双路径路由表（Rule/AI）**：未实现，设计哲学 P0 项。
- **事件/审计总线**：未实现，`task_events` 等表未落库。
- **日历日程同步 / `reminder_jobs`**：未实现，见第九章（M1）。

本文档不把以下内容纳入当前已批准范围：

- 新增独立 MCP 平台层。
- 用 AI 代理替代确定性规则引擎。
- 把所有模块重写为事件溯源系统。

## 四、模块地图

### 4.1 已实现模块

> 每个模块的详细设计见 `docs/modules/`（编号见 `modules/00-README.md`）。

| 模块 | 现状 | 关键入口 | 模块文档 |
|---|---|---|---|
| 案件管理 | 已实现 | `commands/cases.rs`, `db/cases.rs`, `src/modules/cases/` | `01-cases.md` |
| 双轨/三轨状态机 | 已实现基础字段、筛选、看板、手动切换 | `schema.rs` v8, `commands/cases.rs:update_case_status`, `CaseDetailView.vue`, `KanbanView.vue` | `02-status-machine.md` |
| 时间线 | 已实现 | `commands/timeline.rs` | `01-cases.md` |
| 任务 | 已实现 | `commands/tasks.rs`, `src/modules/tasks/` | `03-tasks.md` |
| 日历/期限 | 已实现 | `deadline/engine.rs`, `commands/calendar.rs`, `commands/cases.rs:get_dashboard_stats` | `06-calendar-deadline.md` |
| 收件箱 | 已实现 | `commands/inbox.rs`, `watcher.rs`, `src/modules/inbox/` | `04-inbox.md` |
| 收件箱批处理 | 已实现第一版 | `start_inbox_batch` / `pause_inbox_batch` / `resume_inbox_batch` / `cancel_inbox_batch` / `get_inbox_progress` | `05-inbox-batch.md` |
| 文书工坊/Docsy | 已实现 | `commands/docs.rs`, `docsy_engine/`, `src/modules/docs/` | `08-docsy.md` |
| 知识库 | 已实现 | `commands/knowledge.rs`, `db/search.rs`, `src/modules/knowledge/` | `07-knowledge.md` |
| WebDAV 同步 | 已实现 | `commands/sync.rs`, `sync/webdav.rs` | `10-sync.md` |
| 飞书同步 | 已实现 | `sync/feishu.rs`, `commands/sync.rs` | `10-sync.md` |
| IMAP 邮件监听 | 已实现 | `email/mod.rs` | `11-email.md` |
| 提醒规则 | 已实现基础版（规则表 + 命令层） | `commands/reminder.rs`, `schema.rs` v5（`reminder_rules`/`reminder_log`） | `12-reminder.md` |
| 提醒引擎调度 | 未接通（见第九章） | `start_reminder_engine` 已注册命令但未在启动拉起；飞书消息/任务通道为占位 | `12-reminder.md` |
| 托盘/快捷键/文件夹监听 | 已实现 | `tray.rs`, `lib.rs`, `watcher.rs` | `09-files.md` / `15-observability-settings.md` |

### 4.2 当前不应误写为“已实现”的能力

| 能力 | 正确表述 |
|---|---|
| 统一领域服务写入口 | 目标态，未形成统一框架 |
| AI 审计与可重放上下文 | 未实现 |
| ContextPolicy/上下文治理 | 未实现为通用框架 |
| 后台任务统一总线 | 未实现 |
| 批处理按案件串行 | 设计目标，当前实现未做到 |
| 批处理事件推送流 | 当前只有轮询进度接口，未向前端公开事件流 |

## 五、核心数据模型

### 5.1 数据库基线

- 数据库入口：`src-tauri/src/db/mod.rs`
- Schema 版本：`8`
- 模式：SQLCipher + WAL + `busy_timeout=5000` + 外键开启
- 迁移方式：启动时按 `MIGRATIONS` 顺序执行增量迁移

### 5.2 关键表

| 表 | 用途 | 当前备注 |
|---|---|---|
| `cases` | 案件主表 | 同时承载旧 `track/case_status` 与新 `case_route/*_status` 字段 |
| `clients` | 客户主数据 | 已落表并在建案时 `INSERT OR IGNORE` |
| `case_logs` / `hearings` / `tasks` | 时间线与执行事项 | 为看板、日历、仪表盘提供来源 |
| `case_deadlines` / `deadline_rules` | 期限规则与实例 | 期限计算是确定性路径 |
| `inbox_items` | 收件箱主表 | 同时服务单件处理与批处理队列 |
| `case_files` / `email_records` | 文件与邮件归档 | 与收件箱、文书、时间线联动 |
| `knowledge_items` / `knowledge_versions` / `knowledge_relations` / `knowledge_embeddings` | 知识库与检索 | 支持 FTS 和 embedding |
| `drafts` | 文书草稿 | 与 Docsy 模板/导出相关 |
| `sync_map` / `sync_queue` | 同步映射与同步队列 | 飞书/WebDAV 共用的持久化同步辅助模型 |
| `imap_accounts` | IMAP 配置 | 当前密码字段为 `password_enc` |
| `case_track_history` | 三轨状态变更历史 | v8 新增，已被前后端使用 |
| `reminder_rules` / `reminder_log` | 提醒规则与提醒日志 | v5 落表，规则有种子数据；`reminder_log.status` 取值 `sent/failed/snoozed`，表示“本地声称已发出”，不等于服务端接受或用户已读（见第九章） |

> 字段语义与表名以本章为口径。设计哲学文档中的目标态新表（`task_events`、`decisions`、`ai_runs`/`ai_context_items`、`audit_events`、`memory_entries`、`provenance` 等）与第九章的 `reminder_jobs` 均为**设计草案，尚未落库**，不得写成现状。

### 5.3 状态机口径

案件状态需要同时保留两套口径：

1. 兼容口径：`track` + `case_status`
2. 新口径：`case_route` + `civil_status` + `invalidation_status` + `admin_status`

统一要求：

- `case_status` 只作为聚合筛选和统计字段。
- 轨道内细分状态以 `docs/modules/02-status-machine.md` 的枚举为准。
- 新文档不得再把 `case_progress` 当作唯一状态机来源；它更接近自由文本进展字段。

## 六、命令与写入边界

### 6.1 当前命令边界

当前命令分组以 `commands/mod.rs` 为准，典型类别包括：

- 案件：`list_cases`、`create_case`、`update_case`、`update_case_status`
- 同步：`webdav_*`、`sync_feishu_*`、`feishu_*`
- 收件箱：`add_inbox_item`、`process_inbox_item`、`file_inbox_item`、`start_inbox_batch` 等
- 文书：`list_docsy_templates`、`render_docsy_template`、`export_docx`
- 诊断：`get_log_dir`、`get_recent_logs`、`search_logs`

### 6.2 当前真实写入路径

必须明确：当前多数写入路径仍是下面这种模式，而不是统一领域事务框架。

```text
前端 -> Tauri command -> open_db() -> SQL / 模块函数 -> 返回结果
```

这意味着本阶段文档设计必须遵守两个现实：

- 不能假设所有写操作天然共享统一事务编排器。
- 任何新增复杂业务如果需要跨表不变量，必须在具体 Rust 模块内明确事务边界。

### 6.3 后续收口原则

未来继续演进时，优先收口这些高风险写路径：

1. `update_case_status` 及其聚合状态更新。
2. 收件箱“处理 -> 归档 -> 生成任务/知识/日志”的复合写入。
3. WebDAV/飞书同步完成后的映射与元数据回写。
4. 批处理队列的状态推进与恢复。

## 七、关键运行流

### 7.1 案件创建

当前流程：

1. 前端提交案件数据。
2. `create_case` 写入 `cases`。
3. 若客户不存在，则写入 `clients`。
4. 自动创建案件文件夹。
5. 更新 `folder_path`。
6. 通知飞书自动推送管理器。

设计约束：

- 文件夹创建失败不能导致案件主记录回滚，但必须记录日志。
- 后续若将其改为单事务/补偿流程，需要明确“数据库成功、文件夹失败”的补偿策略。

### 7.2 收件箱单件处理

当前 `process_inbox_item` 的流程：

1. 读取 `inbox_items.content_text`。
2. 有 AI 时调用 `process_inbox_with_ai`，无 AI 时回退到规则分类。
3. 回写 `ai_category` / `ai_confidence` / `ai_extracted` / `ai_suggested_case_id`。
4. 依据分类和置信度执行一部分自动路由。

约束：

- 分类与路由不是同一回事；批处理只覆盖“分类/提取”，不等于完成归档。
- 自动路由失败要记录日志，但不能回滚整个分类结果。

### 7.3 收件箱批量处理

现状与目标的详细设计见 `docs/modules/05-inbox-batch.md`。架构层只保留统一约束：

- 队列主数据复用 `inbox_items`，不允许另建与之脱节的第二套收件箱实体。
- 状态字段、进度口径、取消/恢复语义必须与 `modules/05-inbox-batch.md` 一致。
- 批处理输出只负责推进到“已分类/待归档”这一层，不应隐式扩大为全自动办案。

### 7.4 WebDAV 启动同步

当前流程：

1. 命令层读取数据库路径和已保存的 ETag。
2. 调用 `sync::startup_sync` 或手动 push/pull。
3. 成功后回写 `webdav_last_etag`、`webdav_last_sync_at` 等设置。

约束：

- 冲突解决目前是“保留本地 / 保留远程”两档，不是字段级合并。
- 任何文档不得把 WebDAV 现状描述为“增量多记录同步”；当前同步对象仍是数据库文件。

## 八、并发、幂等性与崩溃恢复

### 8.1 当前并发模型

| 能力 | 当前并发模型 |
|---|---|
| 普通命令 | 多数通过 `run_blocking` 在线程池执行阻塞任务 |
| IMAP 监听 | 常驻异步任务 + 29 分钟重连 |
| 文件夹监听 | 独立线程监听文件创建事件 |
| 飞书自动推送 | 后台 watcher + 防抖（数据同步，非提醒；应用在线才运行） |
| 提醒引擎 | 进程内周期检查（`start_reminder_engine`，默认 5 分钟）；**未在启动时拉起**，当前不运行（见第九章） |
| 收件箱批处理 | 进程内 `InboxProcessor` + `Semaphore` 并发控制 |

### 8.2 幂等性约束

当前已具备或应继续坚持的幂等约束：

- `sync_map`、`sync_queue`、飞书导入等路径需要稳定主键或唯一约束。
- 批处理重试只允许重复更新 AI 分类字段，不允许重复创建案件、任务、知识库记录。
- 案件状态切换写入 `case_track_history` 时，后续若增加自动状态推进，应避免同一变更被重复写入。
- 提醒去重当前靠 `reminder_log` 的 `rule_id + date(sent_at) + case_id + task_id` 组合查询（`already_sent`），无唯一约束；它只是“本地同日去重”，**不是端到端幂等键**。第九章 M1 要求每条云端提醒作业带独立幂等键，保证重试/断网追赶不产生重复发送。

### 8.3 崩溃恢复约束

当前事实：

- 数据库迁移和主数据依赖 SQLite 落盘恢复。
- 批处理运行态主要在内存中；应用进程退出后，`running/paused/cancel` 等状态丢失。
- `inbox_items.status`、`retry_count`、`last_error`、`processing_started_at` 可用于恢复未完成项。

因此，文档统一口径为：

- 当前支持“基于 `inbox_items` 状态的粗粒度恢复”，不支持恢复到精确的任务句柄或进度百分比。
- 启动后如发现残留 `processing` 项，后续实现应把它们重置为 `pending` 或做超时回收；在代码真正补上前，本文档只把它记为待完善项。
- 提醒的崩溃恢复是“尽力而为”：重启后若提醒引擎运行，`check_and_trigger` 会按 `already_sent` 去重补查，但 `deadline_before/deadline_on/deadline_after/hearing_before` 用 `days_diff == trigger_days` 严格相等触发，离线数天后错过的 T-N 天预警**无法自动补发**；`task_due/task_overdue` 用 `<=` 可补。M1 日历作业模型（见第九章）要求“已同步到日历的作业不依赖本地恢复；未同步的在恢复/追赶时补齐”。

## 九、离线提醒与准时送达决策

> 本章是 v2.10 收口的核心架构决策：Casy 是**本地优先桌面应用，可能并非全天运行**。应用退出、设备关机、休眠或断网时，任何“进程内定时器”都不会触发。因此本文档明确区分“计算正确 / 进入发送队列 / 服务端接受 / 终端实际送达与用户确认”四层语义，并给出 M0/M1 推荐与“是否需要移动端”的明确回答。

### 9.1 问题本质：本地定时器无法离线准时

本地提醒依赖三个环节，全部要求“应用进程在线”：

1. **定时触发**：`deadline_recalc_scheduler`（每日 00:01 重算）、`start_reminder_engine`（周期检查）都是应用进程内的 tokio 任务。
2. **托盘/通知**：系统托盘与 macOS 通知由应用进程维护；应用退出即失效。
3. **飞书自动 PUSH**：`AutoPushManager`（数据同步，5 秒防抖）与提醒通道都是进程内 watcher，断网或退出即停止。

结论：**Casy 本地进程本身无法在关机/休眠/退出/断网时触发提醒**——这是技术事实，不因文档写法而改变。**但"离线准时提醒"不必由 Casy 自己实现**：把提醒固化为日程事件、同步到用户的日历/邮箱生态（Google Calendar / Apple iCloud / Microsoft Outlook，均支持 CalDAV，且通常就是用户的邮箱账号），由日历服务商 24/7 在线的推送基础设施负责准时送达。本文档的口径从"不保证准时"改为：**M0 本地尽力而为；M1 通过日历/日程同步实现离线准时（方案 D，见 9.4）**。

### 9.2 四层语义（统一口径）

| 层 | 含义 | 当前可实现性 | 证据/示例 |
|---|---|---|---|
| **① 计算正确** | 期限/开庭/任务的日期与触发条件由本地规则引擎确定性地算对 | ✅ 离线可算 | `deadline/engine.rs` 期限引擎；`commands/reminder.rs` 的 `days_diff` 判断 |
| **② 进入发送队列** | 提醒被固化为可持久化、可重试、可对账的作业（job）进入队列 | ⚠️ 部分 | 现状只有 `reminder_log` 事后日志；目标态用 `reminder_jobs`（9.6） |
| **③ 服务端接受** | 通道（日历服务 CalDAV / email SMTP）收到并确认事件 | ⚠️ 未接通 | M1 接通 CalDAV/ICS 同步；日历服务返回 ETag/事件 ID 即"服务端接受" |
| **④ 终端实际送达 / 用户确认** | 提醒真正到达用户设备、用户已读/已处理 | ❌ 无法保证 | 日历服务推送 ≠ 已读；`delivered/read` 需回执或用户确认（9.11） |

> **统一口径**：① 是确定性地步，不依赖任何在线能力；② 需要作业模型（M1 才有真正持久化队列，M0 以 `reminder_log` 为近似）；③ 只有日历服务/邮件服务确认接受才写 `sent`；④ Casy 永不主动声称"用户已读"，除非拿到回执。**注意：日历同步让 ③ 在 M1 成立且不再要求 Casy 进程在线——因为执行方是日历服务，不是 Casy 本地。**

### 9.3 当前能力边界（现状，不得写成已实现）

- 提醒引擎命令已注册但**未在启动时拉起**；本地/系统通道会写 `reminder_log`，飞书消息/任务通道是**占位**（`dispatch_reminder` 对 `feishu_message`/`feishu_task` 直接返回成功并写 `sent`，实际未调用 `send_feishu_reminder_async` / `create_feishu_task_reminder_async`）。
- 重启补偿扫描：`task_due/task_overdue` 可补发；`deadline_before` 等严格相等的触发项无法补发错过的 T-N 天（见 8.3）。
- 以上两点均记为**待完善项**，本文档不把“多通道提醒已端到端可用”写成现状。

### 9.4 方案比较（四种）

**方案 A：纯本地（原生通知 + 启动补偿，尽力而为）**

- 做法：应用在线时由本地定时器/托盘触发 macOS 通知；离线期错过的提醒在下次启动时补偿扫描，在应用内列出“离线期间错过的提醒清单”。
- 优点：零额外基础设施、零隐私外泄、实现成本最低；与“本地优先”哲学一致。
- 缺点：设备关机/休眠/退出/断网时不产生任何外部通知；补偿扫描是“事后可见”，不是“准时送达”；`deadline_before` 等严格触发项需改为区间触发（`0 <= days_diff <= trigger_days`）才能真正补偿。
- 定位：**M0 的默认能力**，始终保留作为兜底。

**方案 D：日历 / 日程同步（CalDAV + ICS via Email）——推荐 M1**

- 做法：Casy 不自己“推送”，而是把提醒固化为**日程事件**（iCalendar .ics），通过标准协议同步到用户已有的日历服务，由日历服务商 24/7 在线的推送基础设施负责准时提醒。两条同步通道：
  - **CalDAV**（首选）：Casy 作为 CalDAV 客户端 PUT/UPDATE/DELETE 事件到用户的日历（Google Calendar / Apple iCloud / Microsoft Outlook，三者均支持 CalDAV，且通常就是用户的邮箱账号）。标准协议、双向同步、可改期可取消。
  - **Email 邀请（ICS）**：通过 SMTP 把 .ics 邀请发送到用户自己的邮箱（日历服务自动归入日历），也支持**对外场景**（把开庭通知/日程邀请发给当事人、同事）。
- 核心机制：本地规则引擎算出触发时间（确定性）→ 生成日程事件（summary/description/dtstart + alarm，映射 R1-R4 等级）→ 同步到日历 → 日历服务在到期前按 alarm 推送（移动端本地通知 + 邮件提醒 + 桌面端通知，跨设备）。Casy 离线、关机、退出均不影响——事件已在云端日历。
- 优点：**零自建基础设施**（不持有通道凭据、不运营常驻服务、无 SLO 运维）；可靠性远超自建（Google/Apple/Microsoft 的推送基础设施 24/7 在线，跨桌面+移动+手表）；复用用户已有账号；与"本地优先"一致（本地仍是权威数据源，日历只是送达通道）；天然支持对外日程（当事人/同事）。
- 缺点：事件内容存在于日历服务商服务器 → 必须脱敏 + 案件级“不出端”策略（9.8）；依赖日历服务推送（同样不是法律级送达证明，与飞书同级“通知 ≠ 已读”）；需用户配置一次日历账号（CalDAV 凭据 / OAuth 存 OS keyring）。
- 定位：**M1 的目标能力**，是“离线准时提醒”的推荐路径。**替代原方案 B 的自建云端中继。**

**方案 B：最小云端提醒中继/调度器（降级为备选）**

- 做法：Casy 只把**最少的 reminder job**（作业信封，见 9.6）同步到常驻云端服务；云端在目标时刻调用飞书 Bot 等通道发送，不依赖本地应用在线。同步规则：只同步作业本身，不同步案件全文。
- 必须内置的可靠性机制：幂等键、租约/claim、重试、死信、送达回执、撤销/改期、时区与设备时钟校验、断网追赶、健康监控、隐私最小化（见 9.6-9.9）。
- 优点：通道可控（可选飞书等），作业元数据最小化上云。
- 缺点：**需要自建常驻服务与运维、需要托管飞书发送凭据、需要实现全套 SLO 机制**——成本远高于方案 D；对 Casy 这类单律师工具是明显的过度建设。
- 定位：**备选**。仅当出现明确需求“用户不配置日历、且必须飞书渠道、且接受自建服务”时才评估；不默认立项。

**方案 C：移动端伴侣**

- 事实澄清：**移动端后台同样受系统限制**——iOS 后台任务、Android 厂商省电策略都会冻结后台进程；若要可靠推送，移动端仍依赖 APNs / FCM 等平台级常驻通道，与桌面端或云端中继并无本质区别。
- 因此：**移动端不是“离线准时推送”的前置条件**。它解决的是另一组独立需求（移动办公、移动录入/查阅、移动审批）；只有在这些独立需求成立时才立项。方案 D 已经通过日历生态覆盖了移动端提醒（日历 App 自带推送），无需为提醒开发移动端。

### 9.5 推荐与分阶段路线

| 阶段 | 决策 | 说明 |
|---|---|---|
| **M0** | 保留本地提醒 + 启动补偿（尽力而为） | 接通 `start_reminder_engine` 启动拉起；补偿扫描改为区间触发（能补发 T-N 天）；设置/日志明示“M0 本地提醒为尽力而为，M1 起由日历同步保证离线准时” |
| **M1** | **日历 / 日程同步（方案 D）** | 接通 CalDAV（Google/Apple/Outlook）+ ICS email 邀请；提醒固化为日程事件由日历服务准时提醒；案件级通知策略（完整/脱敏/不出端）；`reminder_jobs.executor=calendar`（9.6）；本地在线时飞书通道作为补充渠道 |
| **移动端** | 仅当移动办公 / 移动录入/查阅等独立需求成立时立项 | 提醒的移动端需求已由日历生态覆盖，不因“推送不准时”立项 |
| **备选** | 自建云端中继（方案 B） | 仅当“不配置日历 + 必须飞书渠道 + 接受自建服务”的明确需求出现时评估 |

M1 的承诺从“**日历事件已确认同步**”开始：案件、任务或期限每次变更，应在同一本地事务中更新业务数据并生成提醒作业（`reminder_jobs`），随后把日程事件同步到日历；只有收到日历服务确认（ETag / 事件 ID）后，UI 才显示“日历提醒已就绪”。若作业尚未同步就退出/断网，只能显示“待同步”（本地尽力而为）。Casy 离线期间在**其他系统**中新产生、但 Casy 从未获取的数据，日历同样无法凭空提醒——该场景仍需把外部日历（如对方系统）接入，不属于提醒同步范围。

### 9.6 提醒作业的数据结构与状态机（目标态，M1）

作业表 `reminder_jobs`（设计草案，未落库）：

| 字段 | 语义 |
|---|---|
| `id` | 不可变作业 ID / 幂等键（UUID），云端按此去重 |
| `rule_id` / `entity_type` / `entity_id` | 来源规则与触发对象（case/task/hearing/deadline） |
| `channel` | `local` / `system` / `calendar` / `email_ics` / `feishu_message` / `feishu_task` |
| `executor` | `local` / `calendar`；每条作业只能有一个执行方，避免双发。`executor=calendar` 表示由日历服务商在 `scheduled_at` 触发推送 |
| `scheduled_at` | 已换算完成的目标触发时刻（UTC），同时作为日程事件的 dtstart（含提前提醒 alarm） |
| `timezone` / `offset_snapshot` | IANA 时区（如 `Asia/Shanghai`）与生成时偏移快照；不能只存固定 offset，否则无法正确处理夏令时 |
| `calendar_account` | 目标日历账号别名（CalDAV 配置 id）；本地通道为空 |
| `calendar_event_id` / `calendar_etag` | 日历服务确认后返回的事件 ID 与 ETag——`sent` 的锚点；为空时 UI 显示“待同步” |
| `content` / `masked_content` | 完整内容（本地）与脱敏内容（同步到日历/邮件，见 9.8） |
| `due_snapshot` | 触发时的期限/开庭信息快照，避免事后字段变化导致误发/漏发 |
| `status` | `pending → synced → sent → delivered → read`；`sync_failed → (retry) → dead_lettered`；另含 `delivery_unknown`、`cancelled` |
| `attempts` / `last_error` / `next_attempt_at` | 重试计数、最后错误、下一次重试时间 |
| `supersedes_id` / `version` | 改期关联的旧作业 ID 与同步版本 |
| `server_msg_id` | 通道侧事件/消息 ID（日历事件 UID / 邮件 Message-ID），幂等与回执的锚点 |

状态机（至少一次 + 幂等，避免重复发送）：

```text
pending ──同步到日历──> synced(日历确认 ETag) ──到点──> sent(日历服务触发推送) ──回执──> delivered ──已读──> read
   │                      │
   │                      └─同步失败/超时─> sync_failed ──重试(<=N 次)──> pending
   │                                                   └─超限─> dead_lettered(人工接管)
   └─撤销/改期/完成─> cancelled（改期：取消旧作业 + 生成新 ID，并以 supersedes_id 关联旧作业）
```

> 这里采用“至少一次调度 + 尽可能去重”，不宣称端到端 exactly-once。`sent` 只写日历服务/邮件服务确认接受（ETag / 事件 ID）；若同步请求结果不明（超时/断连）进入 `delivery_unknown`，先查询或人工对账，禁止盲目重试造成重复日程。

### 9.7 桌面端与日历服务职责、同步与冲突规则

| 职责 | 桌面端（权威） | 日历服务商（执行方） |
|---|---|---|
| 计算 | 规则引擎算出触发条件并生成作业 | — |
| 作业生命周期 | 本地持久化 `reminder_jobs`，是“提醒意图”（创建/撤销/改期）的权威源 | 保存日程事件（Calendar），是“到点触发”的执行方 |
| 定时触发 | 只执行 `executor=local` 的作业 | 对 `executor=calendar` 的作业，在事件 dtstart 前按 alarm 触发推送（移动通知/邮件/桌面） |
| 状态回传 | 记录同步结果（ETag/事件 ID）；拉取/接收回执对账 | 返回事件写入确认（CalDAV PUT 响应 / 邮件接受） |
| 撤销/改期 | 生成 cancel/reschedule → DELETE/UPDATE 日历事件或发 ICS CANCEL | 在到期前删除/更新事件；错过撤销由日历服务按最终状态处理 |

同步与冲突规则：

- 桌面端是“提醒意图”的权威源：作业只允许由客户端创建/撤销/改期；`sent` 状态只能来自日历/邮件服务确认（ETag / 事件 ID），客户端不得自行推断。
- 幂等键为 `id`（即 ICS 的 UID）：CalDAV PUT 同一 UID 覆盖更新，天然幂等；email 邀请以 UID 标识同一日程，重发不产生重复事件。结果不明时按 9.6 的 `delivery_unknown` 处理，不承诺绝对不重复。
- 执行方互斥：`executor=calendar` 的作业同步成功后，本地不得再次发送；本地只负责展示/对账。若尚未同步成功，UI 显示“待同步”，不能悄悄降级双发。
- 撤销语义：撤销请求必须在事件到期前到达才能保证不触发；CalDAV DELETE 或 ICS CANCEL 均可。错过撤销由日历服务最终一致处理。
- 改期语义：事件 UID（作业 ID）不变，更新 dtstart 即可（CalDAV PUT 覆盖）；大版本变更可通过 `supersedes_id` 关联旧作业。

### 9.8 凭据与密钥边界、隐私最小化、高敏案件

- **方案 D 的核心优势：不存在 Casy 自建云端，因此没有任何 Casy 服务端凭据托管问题。** 执行方是用户的日历/邮箱服务商（Google/Apple/Microsoft），Casy 只作为客户端。
- **CalDAV 凭据**：用户的日历账号凭据（或 OAuth token）只存 **OS keyring**（本地），永不上传。OAuth 流程（Google/Apple/Microsoft 授权码）在本地完成，刷新 token 存 keyring。`calendar_account` 是本地配置 id，不携带凭据。
- **Email ICS 邀请凭据**：SMTP 凭据 / OAuth 存 keyring（本地）；若用户愿意，也可委托日历服务的"发邮件邀请"能力。
- **隐私最小化（同步到日历/邮件的内容）**：日程事件只含调度与提醒必需字段——`summary`（脱敏标题）、`dtstart`、`alarm`、`description`（可选脱敏备注）；**不含案件全文、客户敏感字段**。默认 `masked_content`；用户可整体关闭“日历同步”退回 M0。
- **高敏案件**：案件级配置 `通知策略 ∈ {完整, 脱敏摘要, 不出端}`，默认脱敏摘要。开启前弹风险提示："日程内容会出现在你的日历应用（及关联设备）中，可能被他人看到；选择'不出端'则提醒只在应用内展示。"
- 数据保留：`reminder_jobs` 在 `read`/`dead_lettered` 后按保留策略清理；日历事件由用户自行管理（可在日历中删除）。

### 9.9 端到端时序与故障恢复

端到端时序（M1，CalDAV 同步为例）：

```text
t-7d  本地规则引擎计算 → 生成 reminder_job(pending, scheduled_at=用户时区 T-7d 09:00)
       → 生成 iCalendar 事件（UID=job id, summary=脱敏标题, dtstart, alarm）
       → CalDAV PUT 到用户日历
       → 日历返回 ETag → status=synced（服务端接受）→ 记录 calendar_event_id
T-7d 09:00  日历服务商按 alarm 触发推送（移动端本地通知 / 邮件 / 桌面端）
       → 用户打开/处理 → 应用内标记 done（可选：日历回执极少，不依赖）
本地在线时：对账 reminder_jobs 状态、展示“日历提醒已就绪”
本地离线时：日历服务照常触发（不需本地在线）；恢复后客户端对账
```

故障恢复矩阵：

| 故障 | 恢复机制 |
|---|---|
| 本地应用退出/关机 | M0：错过（尽力而为），重启补偿扫描；M1：事件已在日历，日历服务照常触发 |
| 本地断网 | M1：事件已同步到日历则不受影响；未同步的在恢复后补同步 |
| 日历服务临时不可用 | CalDAV PUT 失败 → `sync_failed` → 指数退避重试（≤N 次）→ 期间本地 M0 提醒仍尽力触发 |
| 同步请求超时/响应丢失 | 标记 `delivery_unknown`；以 UID 幂等重查（CalDAV GET by UID），不盲目重复 PUT |
| 设备时钟或时区错误 | 生成时以 IANA `timezone` + UTC `scheduled_at` 写入 ICS；日历服务按自身时区规则触发，发现时区变化时告警 |
| 撤销/改期未及时同步 | 到期前 DELETE/UPDATE 事件；错过撤销由日历服务按最终状态处理（最终一致） |
| 用户未配置日历账号 | 整体退回 M0 本地提醒（尽力而为），设置中引导配置 |

### 9.10 SLO / 验收标准（按方案分档）

| 档位 | SLO | 说明 |
|---|---|---|
| M0（纯本地） | 规则测试用例必须 100% 通过；仅在应用在线且系统允许通知时尽力触发，离线期不承诺 | 验收：在线触发、重启补偿扫描、设置中明示“M0 尽力而为，配置日历同步后离线准时” |
| M1（日历同步） | Casy 侧承诺：**“已同步到日历”的有效作业，事件内容与 alarm 正确、且在用户日历中可见**（以 ETag 确认 + 本地审计为准）。准时就绪由日历服务商承担（其推送基础设施 24/7 在线，跨设备），Casy 不重复计量 | 验收：模拟桌面端离线 48h，事件按 alarm 正常触发；同一 UID 不产生重复事件；撤销后事件删除；仪表盘区分“同步失败 / 日历拒绝 / 结果不明” |
| 送达/已读 | Casy 不承诺；送达取决于日历服务商推送，已读取决于回执 | 验收：`delivered`/`read` 只有取得对应回执或用户确认时才写入，不虚报 |

> SLO 是可监控、可复盘的服务目标，不是绝对保证。若需要法律期限级别的风险控制，仍应保留人工复核、纸质/邮件留痕或其他独立通道作为兜底，不能把单一日历推送当作完成法律行为的证明。

### 9.11 日历/邮件服务接受 ≠ 用户已读（统一口径）

- 日历服务返回 ETag / 邮件服务接受（Message-ID）只证明“事件已入日历/邮件已发”，**不等于已送达、更不等于用户已读**。
- `reminder_log.status` 只允许写 `sent/failed/snoozed`（现状语义）；`delivered/read` 属于 M1 的 `reminder_jobs` 增强状态，必须来自回执或用户确认，禁止从同步成功直接推断。
- UI 口径：提醒记录区分“日历已就绪 / 已发送 / 已送达 / 用户已处理”多档；产品文案不得使用“已提醒（默认用户看到）”。

## 十、资源预算与安全隐私

### 10.1 资源预算

当前仓库中已经有明确预算或硬编码上限的部分：

- AI 每日调用预算：`AiConfig.daily_limit`，默认 `50`。
- 收件箱批处理并发：默认 `8`，动态下调最小 `1`。
- WebDAV：连接超时 `30s`，请求超时 `300s`。
- 收件箱列表默认只取最近 `100` 条。

设计要求：

- 新增后台任务必须写明默认并发、超时、重试上限和人工接管点。
- 未经明确设计，不要把全文内容、整库数据或全量客户数据无界送入模型上下文。

### 10.2 隐私与凭据

当前状态必须如实描述：

- 数据库加密已落地：优先 OS keychain，失败回退本地密钥文件。
- 飞书凭据使用 keyring。
- IMAP 密码当前是 `base64` 编码保存在 `imap_accounts.password_enc`，这不是安全加密，只是临时实现。

因此后续文档口径统一为：

- “本地优先 + 数据库加密”可以写成现状。
- “所有外部凭据均已安全存入系统钥匙串”不能写成现状。
- IMAP 凭据迁移到 keychain 是安全补强项，不是已完成事实。
- 日历日程同步（M1，见第九章）只同步日程事件的最小字段（脱敏 `summary`、`dtstart`、`alarm`），案件全文与客户敏感字段不得进入日历/邮件。日历账号凭据（CalDAV/OAuth/SMTP）只存 OS keyring，不存在 Casy 自建云端，因此也没有服务端凭据托管问题（9.8）；关闭日历同步即退回 M0 纯本地。

## 十一、可观测性

当前可确认的可观测性能力：

- `app_log` 提供按天轮转日志，默认保留 7 天。
- 前端可通过 `get_log_dir`、`get_recent_logs`、`search_logs` 读取日志。
- 文件导入、托盘、部分文件复制流程会通过 Tauri event 向前端发消息。

当前缺口：

- 没有统一指标面板。
- 批处理虽然内部有 `broadcast::Sender<ProcessingProgress>`，但前端公开接口仍以轮询 `get_inbox_progress` 为主。
- 没有覆盖 AI、同步、批处理的统一审计事件模型。

## 十二、迁移、回滚与发布约束

### 12.1 数据库迁移

- 迁移入口：应用启动 `init_db()`。
- 当前版本：v8。
- 允许前滚，不承诺通用自动降级回滚。

### 12.2 必须明确的回滚策略

| 领域 | 当前可用回滚 |
|---|---|
| 数据库加密迁移 | 自动生成 `.bak` 明文备份 |
| WebDAV 冲突 | 保留本地 / 保留远程 |
| 收件箱批处理 | 依赖单项重试和重新入队，不支持整批回滚 |
| 飞书同步 | 依赖 `sync_map`/日志人工排查，不支持事务式双边回滚 |
| 日历日程同步（M1） | 设置里一键禁用日历同步即退回 M0 纯本地；已同步到日历的事件由用户自行管理，本地 `reminder_log` 不受影响 |

设计要求：

- 未来若新增“重建表迁移”，文档必须同时写数据迁移步骤、失败时的备份策略和用户可见症状。
- 不允许在没有备份策略的前提下，把重建表操作写成“无风险升级”。

## 十三、与 `modules/05-inbox-batch.md` 的统一约束

两份文档必须共享以下口径：

- 收件箱队列主表是 `inbox_items`。
- 批处理相关状态最少包括 `pending`、`processing`、`processed`、`failed`，并与归档相关状态 `filed`、`archived`、`ignored` 区分。
- 当前批处理是“分类/提取队列”，不是“自动归档/自动办案引擎”。
- 取消、暂停、恢复、重试的接口名称以当前已注册命令为准：`start_inbox_batch`、`pause_inbox_batch`、`resume_inbox_batch`、`cancel_inbox_batch`、`get_inbox_progress`、`retry_inbox_item`、`retry_inbox_case`。

## 十四、验收标准

本文档的架构口径只有在满足以下条件时才算有效：

1. 所有“已实现”表述都能在当前源码或迁移中找到对应证据。
2. 所有“目标态”表述都明确标注为待实现，不冒充现状。
3. 模块名、命令名、表名、字段名、状态值与当前代码一致。
4. 与 `docs/modules/05-inbox-batch.md` 不存在状态机、命令名、并发语义上的冲突。
5. 不无依据扩大产品范围，不把未落地的平台层写成既成事实。
6. 离线提醒口径一致：任何文档不得声称“离线准时送达”；四层语义、`reminder_jobs` 状态机、飞书 API 接受 ≠ 用户已读 等术语与第九章一致。
7. 与 `docs/casy-design-philosophy.md` 的提醒等级（R1-R4）、作业状态机、M0/M1/移动端路线不冲突。

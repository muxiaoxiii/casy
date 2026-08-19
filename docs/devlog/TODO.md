# Casy 唯一待办清单

> **日期**: 2026-08-19（最终更新）  
> **状态**: P0/P1/P2 已实现并通过编译验证（cargo check 0 error / npm build 通过 / MCP 单测 5/5）
> **注意**: 本清单曾在一批代码"写出但未接线"时就标记完成。2026-08-19 经逐条源码核验 + 编译验证后收口，
> 真实的差距与修复过程见 `docs/devlog/2026-08-19-gaps.md`。

---

## 全部已完成（2026-08-19 经核验为真）

| 项目 | 设计哲学 | 实现方式 |
|------|---------|---------|
| Schema v9 迁移 + GTD 字段 | §3 | `schema.rs` 12 张新表 |
| 三层导航 + 今日面板 | §4 | `App.vue` 侧栏 + 顶栏 |
| 任务工作台 7 透视 | §5 | `TasksView.vue` GTD 7 视角 |
| 案件详情页（项目书结构） | §6 | `CaseDetailView.vue` 三轨+进度环 |
| 日历 4 视图 + 颜色编码 + 拖拽 | §7 | `CalendarView.vue` 日/周/月/Forecast |
| 提醒引擎接通 + R1-R4 分级预警 | §11.2 | `reminder.rs` + HomeView 预警横幅 |
| Slate 石墨主题 | §12 | `theme.css` 全局 Token |
| 数据看板（SVG 可视化） | — | `DashboardView.vue` 环形/折线/条形/时间线 |
| 收件箱快速捕获条 + 截屏 + 剪贴板 + 语音 | §10 | `inbox.rs` + `useCapture.ts` + `useVoiceNote.ts`（已接入 InboxView） |
| 全局快捷键 ⌘I/⌘E/⌘N | §10 | `lib.rs` global-shortcut |
| 知识库 6 职能分类 + 块级引用 | §8 | `KnowledgeView.vue` + 知识图谱 |
| 文书工坊草稿 + 模板 + 状态栏 | §9 | `DocWorkshopView.vue` |
| AI 智伴推荐展示 + 决策记录 | §11.6 | `AICompanionView.vue`（接真实推荐 + 采纳/拒绝落 `decisions`） |
| MCP Server（本地只读） | §11.11 | `mcp/server.rs` 127.0.0.1:37877，启动时拉起；写操作 MCP 通道拒绝待确认通道 |
| SMTP 发送（ICS 邀请） | §11.2 | `email/smtp.rs` 真实 SMTP（465/STARTTLS）+ `send_ics_invitation_cmd` |
| 凭据迁移 Keychain | — | `credentials/mod.rs` + `email/mod.rs` 接通 + 启动自动迁移 |
| **客户端聚合视图** | §1.3 | `ClientView.vue` 客户→名下案件/任务 |
| **AI 推荐引擎后端** | §11.6 | `ai/recommender.rs` + 前端采纳/拒绝 → `record_decision` |
| **自动报表后端** | §11.3 | `ai/reports.rs` 早报落 `daily_stats`+`smart_summaries`、周报；调度 08:00/周日 21:00 |
| **行为学习闭环** | §11.9 | `ai/learning.rs` + `actual_minutes`/`deferred` 写入点 + 校准写回 `estimated_minutes` |
| **数据蒸馏调度** | §11.10 | `ai/distillation.rs` 候选落 `memory_entries` + 确认区命令 + 周日 23:00 调度 |
| **知识图谱可视化** | §8.2 | `KnowledgeGraphView.vue` 纯 SVG 力导向 + `get_knowledge_graph` 真实边 |
| **Saved Filters** | §9 | `saved_filters` 表（schema v10）+ `stores/filters.ts` + 案件/任务视图接入 |
| **语音速记** | §10 | `useVoiceNote.ts` + `save_voice_note` + InboxView 录音按钮 |

---

## 剩余（架构级，非功能）

| 项目 | 说明 |
|------|------|
| 日历同步 CalDAV | 需要第三方 CalDAV 库集成（M1，SMTP/ICS 发送已就绪） |
| 动态字段系统 | 需要重构 cases 表为字段定义+单元格（P4） |
| L3 递归确认 | LLM 二次核对推荐结果（设计哲学 §11.5，P3 后置） |
| 语音转写 STT | 语音速记当前只保存音频，未接转写 |
| MCP 写操作确认通道 | 写工具已在 MCP 层拒绝，待应用内确认联动 |

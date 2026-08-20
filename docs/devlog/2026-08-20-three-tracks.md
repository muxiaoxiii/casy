# 三线并行完成：AI 兜底增强 + 视图迁移清零 + 日历/知识库对标

> **日期**: 2026-08-20
> **背景**: 用户要求三线并行、完成后再汇报：
> ① AI 分析增强（本地规则为基线，AI 兜底）；
> ② 剩余视图组件全量迁移到 ctx 服务通路（tauriCallSafe 直调清零）；
> ③ 日历对标 Fantastical（自然语言建日程 + Forecast 双栏）、知识库对标 Notion/Obsidian（双链 + 快速捕获）。

---

## ① AI 分析增强（src-tauri/src/commands/inbox.rs）

**本地规则基线（不开 AI 也完整处理）**：
- 文本意图：笔记/任务/期限/提醒/知识/新案件（已有）
- **法院送达短信**：detect_service_delivery（zxfw.court.gov.cn 链接）→ 新 `service_delivery` 意图，推荐"抓取送达文书"
- 文件类型：auto_classify（传票/判决等分类，已有）
- **confirm 分派新增 service_delivery**：block_on download_service_delivery → 下载 PDF 到 Casy/inbox + 关联案件 + 留痕

**AI 兜底**：`ai_analyze_inbox_item` 新增 `intent` 返回字段（`ai_category_to_intent`）：
- 法律文书类（summons/hearing_notice/judgment/complaint/defense/correspondence/opposing_counsel）→ file_to_case
- 委托指示（client_instruction）→ create_task
- 前端 `getRecommendations()`：本地规则优先，fallback 时用 AI 意图渲染推荐按钮

## ② 视图迁移清零（43 个文件 / 186 处直调）

**subagent 分工（6 个并行 + 父代理）**：
| 分工 | 文件 | 结果 |
|---|---|---|
| A 业务视图 | cases 群/tasks/sync/files/dashboard/home/clients/reminder（12 文件） | 0 残留 |
| D 设置组件 | settings 7 组件（45 处）→ 补 35 个服务方法 | 0 残留 |
| E AI 模块 | AICompanionView/DecisionsView/AIAuditView → 新建 AiService（17 方法） | 0 残留 |
| F docs 模块 | DocWorkshop/Writing/DocumentGen/LegalEditor → 新建 DocsService | 0 残留 |
| 父代理 | InboxView（19 处）+ App.vue（3 处）→ 补 InboxService 批处理/转写方法 | 0 残留 |

**服务层扩充**：AiService / DocsService 新建；cases/tasks/sync/calendar/files/settings/reminder/inbox 补约 50 个方法。
**最终状态**：全部 .vue 视图 tauriCallSafe 直调 = 0（仅服务层内部封装，写入口唯一）。

## ③ 对标（日历 + 知识库）

**日历对标 Fantastical（CalendarView.vue +720 行）**：
- 自然语言建日程：顶部输入条解析 "今天/明天/周X/X月X日" + "上午/下午/X点/HH:MM" → 创建日程/任务
- Forecast 双栏：左栏按日分组的日程/期限列表 + 右栏当日硬性/弹性任务时间轴（语义色分区）

**知识库对标 Notion/Obsidian（KnowledgeView 等）**：
- 双链展示：列表"关联"列（案件名 → /cases/:id）+ 详情关联区块（案件/任务/父块/子块，任务可跳 /tasks?edit=:id）
- 快速捕获：顶部输入条（文本 + 6 职能分类 + 可选关联案件）→ 回车创建
- 子块下钻：块树逐级展开；图谱节点悬停显示类型

## 验证
- `npm run build` ✅（多轮，含全部 subagent 合并态）
- `cargo check` ✅（18 warning 存量）
- `cargo test --lib` ✅ 103/103

## 遗留
- composables（useDocsyBridge/useCopilot/useKnowledgeCapture/useCapture/useVoiceNote）仍封装 tauriCallSafe——属组合式函数层，服务方法已就绪，可按需迁移
- AI 意图判断目前为"本地规则 + 分类映射"两级；更深的 AI 直接意图生成（重写 prompt）可后续增强

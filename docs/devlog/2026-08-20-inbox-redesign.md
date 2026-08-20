# Inbox 回归原始设计 + 视图迁移（store 全量收口）

> **日期**: 2026-08-20
> **背景**: 用户指出 Inbox 偏离了原始设计——"捕获任意信息，软件判断拿过来的文本/文件是要干嘛的，给出推荐按钮，然后自行推送"。
> 现状 QuickJudge 只做文件归档（file_to_case 一种动作），文本/任务/期限/知识零意图判断。本轮回归设计 + 视图迁移收口。

---

## 一、Inbox 回归原始设计（设计哲学 §10）

### 1.1 后端（src-tauri/src/commands/inbox.rs，+560 行）

**QuickRecommendation 扩展**：新增 `intent` 字段（动作参数 JSON），action 支持六类：
`file_to_case`（归入案件）/ `create_task`（转为任务）/ `create_deadline`（记为期限）/
`save_knowledge`（存入知识库）/ `create_case`（新建案件）/ `set_reminder`（设置提醒）

**quick_judge_inbox_item 文本意图判断**（新增 `quick_judge_text`，纯本地规则）：
- 无源文件（纯文本）→ 意图规则：期限词→create_deadline、行动词→create_task、
  提醒词→set_reminder、知识词→save_knowledge、收案词→create_case、兜底（关联案件→任务/否则→知识）
- 案号/当事人命中本地案件 → 推荐自动携带 caseId
- 日期提示提取（YYYY-MM-DD / MM-DD / 明天 / 后天）→ dueDate/remindAt

**confirm_inbox_action 动作分派**（向后兼容，缺省 file_to_case）：
- file_to_case：原有安全拷贝归档
- create_task / create_deadline / set_reminder：写 tasks（insert_task_from_intent，含 task_events 记录）
- save_knowledge：写 knowledge_items
- create_case：写 cases（最小字段）
- 每个动作后：更新收件项状态 + 写 inbox_recommendations 采纳记录

### 1.2 前端（src/modules/inbox/views/InboxView.vue + 服务层）

- **ACTION_META**：六类动作的 标签/图标/描述 映射（推荐按钮按意图渲染）
- **推荐列表**：文案按动作显示（"创建任务：xxx" / "记期限：xxx（2026-08-25）" / "存入知识库：xxx" ...）
- **Strong 一键推送**：确认后按意图自行落位（不再写死"确认归档"）
- **确认弹窗**：推荐动作单选列表 + 仅 file_to_case 显示案件/目录选择；按钮文案动态
- **InboxService 扩展**：quickJudge / aiAnalyze / confirmAction（数据通路）

## 二、视图迁移（store 全量收口到 ctx 服务）

**subagent（37a06b63）完成 6 个 store**：cases/tasks/settings/filters/profile/calendar，
tauriCallSafe 引用 0；补充 settings 服务方法（savedFilters/saveFilter/deleteFilter/profile/saveProfile）。
**父代理完成 inbox store**：list/add/process/file/dismiss 全部走 ctx.inbox。

至此 **7 个 store 全部迁移**：视图 → ctx 服务 → tauriBridge → Rust 命令（数据通路收口）。

## 三、验证

- `npm run build` ✅
- `cargo check` ✅（18 warning 均为存量）
- `cargo test --lib` ✅ 103/103
- git 工作区干净

## 四、遗留

- AI 分析（ai_analyze）尚未输出意图推荐（当前意图判断为本地规则；AI 增强可后续加）
- inbox store addItem 的 title 参数暂被服务 add(sourceType, contentText, sourcePath) 简化（title 可选，后端默认 null）

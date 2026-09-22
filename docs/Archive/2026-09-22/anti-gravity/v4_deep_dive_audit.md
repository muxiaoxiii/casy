# Casy v4.0 深度核查与架构升级计划 (Deep Dive Audit & Upgrade Plan)

> 本文档旨在对 Casy 软件从前端到后端进行一次商业级的深度核查，解决所有流于表面的“半成品”功能，并重点攻克 **无缝跨设备数据同步 (WebDAV + 冲突解决)**、**AI 自我进化反馈环** 以及 **飞书级多维案件创建流** 等硬核痛点。
> 
> *副本已同步保存至 `docs/anti-gravity/v4_deep_dive_audit.md` 供随时检查验证。*

---

## 🔍 第一部分：核心痛点核查与诊断

### 1. 真实数据闭环 (Real Data Integrity)
- **现状诊断**：之前收件箱和文书工坊已接入真实数据，但仪表盘（Dashboard）中部分卡片的数据结构及状态（如“今日待办”、“AI 推荐策略”）缺乏完善的闭环写入。
- **解决方案**：梳理 `App.vue`、`HomeView.vue`、`Dashboard`，确保所有组件直接绑定 Pinia `stores/cases.ts` 和 `stores/tasks.ts`。清理所有残留的 `mock` 占位符。

### 2. 自我进化系统 (Self-Evolving AI)
- **现状诊断**：AI 智伴和推荐模块目前是单向的（输出建议给用户），用户点击“采纳/拒绝”后，并没有数据沉淀，下一次依然是同样的推荐策略。
- **解决方案**：新建一张 `ai_feedback_loop` 或 `user_preferences` 表。记录用户采纳的提示词、模板和策略偏好。在下一次发送 AI 请求时，将这些偏好作为 System Prompt 的上下文注入（RAG 机制雏形）。

### 3. AI 模块提示词系统 (Prompt Engineering & External APIs)
- **现状诊断**：虽然已支持 Ollama 和 OpenAI API，但提示词系统过于单一。侧边栏状态写着 `Local Model Active` 属于硬编码，未能真实反映当前使用的外部 API。
- **解决方案**：
  1. 解除 `Local Model Active` 硬编码，动态显示当前后端的模型（如 `GPT-4o Active` 或 `Qwen 14B Active`）。
  2. 在 `prompts.ts` 中针对：收件箱结构化、文书润色、早报生成、案件策略推演 分别建立**高精度独立 Prompt 模板库**。

### 4. 统一捕获与收件箱 (Big Pocket & Inbox UI)
- **现状诊断**：毛玻璃和圆角（Stitch UI v4.0）已初步实现，但“展开后的详情页”以及图标交互动效仍有割裂感，细节不够精致。
- **解决方案**：针对 `InboxView.vue` 和 `UnifiedCaptureDialog.vue`，优化毛玻璃的 CSS filter 性能问题（解决 `html2canvas` 导出白屏/黑块），重绘微交互动效（Hover 过渡、按键阴影）。

### 5. 知识库与文书工坊 (Knowledge Vault & Doc Workshop)
- **现状诊断**：已完成 Typora 沉浸式编辑器（LegalEditor）和 Word 导出。但在文书工坊中直接无缝调用知识库内容（引用）的交互缺失。
- **解决方案**：在 LegalEditor 的 `@` 快捷菜单中，新增 `/引用知识库` 的 TipTap 扩展，实现一边写文书一边右侧抽屉检索知识。

### 6. 多语言与国际化 (i18n Polish)
- **现状诊断**：虽然修复了收件箱和设置的硬编码，但侧边栏 (`App.vue` 中的 navGroups) 仍是英文底座加中文 Sublabel，没有与 `$t` 绑定；部分多语言超长文本会出现截断。
- **解决方案**：全面抽离 `App.vue` 中的导航配置，统一改用 Computed 响应式 i18n。增加 CSS `text-overflow: ellipsis` 并在关键按钮引入 `el-tooltip` 防止长文本无法显示。

### 7. 设置界面重构 (Settings UI Overhaul)
- **现状诊断**：目前的 `SettingsView.vue` 只有简单的 `el-tabs`，缺乏层级和说明，模块功能显得零散。
- **解决方案**：重新设计为 **Mac 系统偏好设置风格** 或 **Notion 风格**，分为 `Account`, `Appearance`, `Sync (WebDAV/Feishu)`, `AI Copilot` 等清晰模块，每个选项配以精准说明，并保证每个测试按钮（Test Connection） 100% 连通真实后端逻辑。

### 8. 飞书级案件创建流 (Feishu-style Case Creation)
- **现状诊断**：创建案件仅仅弹出一个小弹窗，无法满足复杂的律所案件信息（案号、当事人多方、标的额、管辖法院、收费方式等）输入需求。
- **解决方案**：重构 `CreateCaseDialog.vue`，改用全屏沉浸式向导（Wizard）或类似多维表格的侧边抽屉。提供结构化表单（原告/被告动态增减、案由级联选择器、智能解析输入块）。

### 9. 商业级跨设备数据同步 (Lossless WebDAV Sync & Conflict Checking)
- **现状诊断**：目前的 WebDAV 同步只比较 ETag，有冲突时直接抛错，不支持无缝导入/合并；用户在两台设备上工作容易产生丢失。
- **解决方案**：
  1. **无缝导入**：提供“导出完整快照（包含 SQLite DB、本地附件、Key文件）”以及“恢复快照”的一键功能。
  2. **冲突检查 UI**：在 `SyncStatusView.vue` 开发冲突解决界面。如果 WebDAV 出现 ETag 不匹配，弹窗展示：`[云端修改时间]` vs `[本地修改时间]`，由用户选择 `强制覆盖云端`、`强制覆盖本地` 或 `取消并手动备份`。

---

## 🛠️ 第二部分：执行计划 (Execution Plan)

我们将分为三个 Sprint 逐个攻破：

### Sprint 1: 基础设施核心重构 (Data Sync, AI & Settings)
1. **重写 WebDAV 同步与冲突解决机制**：在 Rust 侧增加快照导出接口，在 Vue 侧增加冲突解决交互 (`SyncStatusView.vue`)。
2. **Settings UI 重构**：基于最新设计哲学，彻底翻新设置页面，连接实时多语言引擎，修复 `Local Model Active` 硬编码。
3. **AI 自我进化反馈环**：建表 `ai_preferences`，实现采纳记录入库机制，更新提示词库。

### Sprint 2: 飞书级体验升级 (Case Creation & Vault Integration)
4. **重构多维案件创建流**：开发全屏的 `CaseWizard.vue`，支持结构化复杂信息录入。
5. **知识库引用打通**：在 `LegalEditor.vue` 增加知识库调用命令（`/知识`），支持从侧边栏拖拽知识条目到编辑器。

### Sprint 3: 极致打磨与全局审查 (UI/UX & Bug Squashing)
6. **全局遍历核查**：排查所有按钮对齐、文字截断（特别是在英文状态下）。
7. **修复遗留毛玻璃导出 Bug**：替换导致白屏的 CSS filter 为渐变 fallback 或调整 html2canvas 渲染层级。
8. **编写验收报告**：完成所有修复后，在 `docs/anti-gravity/` 输出最终验收单。

---

## 💬 开放问题与用户审批 (Open Questions)

针对 **第八项：飞书级案件创建流**，如果要录入大量信息，您更倾向于哪种交互形式？
- **选项 A**：类似 Notion 的**右侧滑出超大抽屉**（Slide-over Panel），不离开当前页面。
- **选项 B**：类似飞书审批流的**全屏沉浸式向导**（Step-by-step Wizard），分步引导填写（基础信息 -> 当事人 -> 费用）。
*(个人推荐选项 A，因为操作更加流畅且能随时参考当前页面其他内容。您可以直接回复偏好。)*

请审阅上述诊断报告与实施计划，如果您确认该深度和方向正确，请点击 **Proceed**，我将立即按照 Sprint 1 开始硬核编码工作。

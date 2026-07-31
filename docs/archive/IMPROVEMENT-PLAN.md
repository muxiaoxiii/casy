# Casy 改进计划

> 基于 2026-07-31 讨论，在 Phase 9-14 完成基础上的 Next Steps

---

## 核心理念

**先解决自己每天实际遇到的痛点，再做锦上添花。**

Casy 是个人工具，不是 SaaS 产品。改进方向应该聚焦：

1. 每天手动做的事 → 自动化（文档分类、案件筛选）
2. 写过的内容 → 可复用（知识沉淀、模板检索）
3. 缺失的核心流程 → 补齐（看板、Copilot 写作）

---

## 一、AI 文档分类 + 收件箱路由（P0）

### 目标

收到一份文件（PDF/图片/邮件），自动判断：属于哪个案件、是什么文书类型、路由到对应收件箱。

### 场景覆盖

| 输入 | 期望输出 |
|------|---------|
| 法院传票 | 案件: 浦项 v NSC, 类型: 传票, 关联庭审日期 |
| 对方代理词 | 案件: 钛金 v 高德, 类型: 对方文书, 标签: 需要答辩 |
| 判决书 | 案件: 自动匹配, 类型: 判决, 触发期限计算 |
| 审查意见通知书 | 案件: 专利号匹配, 类型: 官方通知, 触发答复期限 |
| 新案起诉状 | 案件: 新建案件, 类型: 起诉状, 提示补充案件信息 |

### 实现方案

```
文件进入收件箱
    │
    ├── AI 分类 (一次 LLM 调用)
    │   ├── 识别文书类型 (transmit/verdict/complaint/defence/notice...)
    │   ├── 提取关键字段 (案号/当事人/法院/日期/专利号)
    │   └── 置信度评分
    │
    ├── 路由决策
    │   ├── 置信度高 + 匹配已有案件 → 直接关联
    │   ├── 置信度中 → 列出候选案件让用户确认
    │   └── 置信度低或新案件 → 标记待处理，提示用户
    │
    └── 更新案件时间线 + 触发期限计算
```

### Prompt 设计要点

- 注入**活跃案件列表**（案号、当事人）作为匹配候选
- 注入**法律文书类型特征词**（判决书→"本院认为"、起诉状→"诉讼请求"、传票→"开庭时间"）
- 输出固定 JSON schema，含 `doc_type`、`matched_case_id`、`confidence`、`extracted_fields`
- OCR 后的文本先做清洗再送 LLM（去除扫描件噪声）

### 当前基础

- AI backend trait 已就位（OllamaBackend / OpenAiBackend / NoOpBackend）
- 收件箱 (inbox_items) 表已有
- case_files + case_logs 表可存分类结果

---

## 二、多维表格（P0-P1）

### 2.1 多条件筛选（P0，半天）

后端 `cases` 表已有 filter，只需要前端 UI：

- 条件组合：轨道（侵权/无效/行诉）+ 状态（进行中/已结案）+ 客户 + 日期范围
- 筛选条件持久化到 URL query，可分享
- 已保存的筛选方案（如"我的待办"、"本月到期案件"）

### 2.2 字段引用 / 下拉搜索（P1，一天）

核心交互：创建案件时选客户、庭审关联案件、文件关联案件，都用带搜索的下拉组件，而非手动输入 ID。

- 已有外键：cases↔clients, hearings↔cases, case_relations
- 前端：基于 Element Plus `el-select` + `filterable` + `remote` 做远程搜索下拉
- 搜索支持名称/案号模糊匹配

### 2.3 看板视图（P1，一到两天）

按案件状态分列，支持拖拽切换阶段：

```
待处理  │  证据交换  │  庭审准备  │  等待判决  │  已结案
────────┼───────────┼───────────┼───────────┼─────────
 案件A  │  案件C    │  案件E    │  案件B    │  案件F
 案件D  │           │           │           │
```

- 拖拽库：`vue-draggable-plus`
- 卡片展示：案号、当事人、下个期限日期、优先级标记
- 拖拽到新列 → 更新 `case_status` + 记录到 `case_logs`
- 依赖筛选和引用先就位——没有引用关系的卡片信息是空的

### 2.4 画册/表单视图（P1，合计一天）

- 画册：卡片网格，按客户/类型分组，适合浏览
- 表单：新建/编辑案件的模板，已有编辑页，按需补充快捷入口

---

## 三、知识库实质化（P1-P2）

### 定位

**不是自动沉淀的知识图谱，而是你自己主动标注的内容仓库。** 核心价值在于：检索源可控、内容经过人工校准、AI 只在检索和匹配时辅助。

### 3.1 知识入库（P1，一天）

工作流：**在写作或阅读文档时，选中文本 → 一键入库，不是批量导入。**

```
写作中选中一段文字 (TipTap 编辑器内)
    │
    ├── "标记为常用段落" → 存入 knowledge_items
    │   标签: 起诉状-损害赔偿-计算方式
    │   来源: 2025-浦项-起诉状.md
    │
    ├── "提取法条引用"  → 关联到法条内容
    │   存入 knowledge_relations
    │   链接: 段落 #id123 ↔ 法条 专利法第65条
    │
    ├── "记录判例要点"  → 总结裁判逻辑
    │   标签: 判例-直接侵权-禁令
    │   关联案件、法条、法官
    │
    └── "标注写作风格"  → 添加风格标签
        起诉状 / 代理词 / 法律意见 / 律师函

阅读已有文档时 (文档查看页)
    │
    ├── 选中判决书段落 → "记录为判例摘要"
    └── 选中法条 → "存入常用法条"
```

### 3.2 混合检索（P1，一天）

关键词 (FTS5) + 语义向量 (Ollama embedding)，两路融合排序。

```
用户查询: "类似专利侵权案件怎么主张损害赔偿"

关键词检索 (FTS5)
  → 匹配: "专利侵权"、"损害赔偿"、"主张"
  → BM25 排序

语义检索 (embedding)
  → 向量相似度匹配
  → 余弦距离排序

融合排序 (RRF)
  → 顶部结果返回给 Copilot sidebar
```

技术选型：
- FTS5：已有 `knowledge_fts` 表 ✅
- Embedding：Ollama 本地跑 nomic-embed-text（无需外部 API，免费）
- 向量存储：SQLite 扩展或独立表存 embedding binary

### 3.3 风格标注体系（P2，一天）

定义 5-6 种文书风格的标签模板：

| 风格标签 | 特征 | 适用场景 |
|---------|------|---------|
| 起诉状 | 诉求明确、法条引用精准、事实陈述简洁 | 民事起诉、行政起诉 |
| 代理词 | 论证层次清晰、证据引用规范、反驳有力 | 庭审代理、书面辩论 |
| 法律意见 | 结论先行、风险分析、建议具体 | 客户咨询、内部汇报 |
| 律师函 | 立场明确、期限警告、法律依据充分 | 侵权警告、催告函 |
| 答辩状 | 逐项反驳、证据清单、时效抗辩 | 应诉答辩 |

每条知识点入库时绑定风格标签。后续 AI 生成时，根据目标风格注入对应的段落范例作为 few-shot 示例。

### 当前基础

- `knowledge_items` / `knowledge_relations` 表 ✅
- `knowledge_fts` 全文搜索 ✅
- AI backend trait (Ollama / OpenAI) ✅

---

## 四、文书工坊 · Copilot Sidebar（P2）

### 目标

TipTap 编辑器右侧增加一个面板，实时从你的知识库里检索相关内容，写作时随手查、随手引用。

### 架构

```
┌─── TipTap 主编辑器 ──────────┐  ┌─── Copilot Sidebar ────────┐
│                              │  │                             │
│ 正在起草：起诉状-20260731    │  │ 🔍 搜索我的知识库            │
│                              │  │ [在专利侵权案件中如何计___]  │
│ ┌────────────────────────┐   │  │                             │
│ │ 原告张三诉被告李四侵害  │   │  │ 📋 相关段落                 │
│ │ 发明专利权纠纷一案...   │   │  │ 损害赔偿计算方式 (起诉状)   │
│ │                        │   │  │ 来源: 2025-浦项-起诉状.md   │
│ │ 根据《专利法》第65条...│   │  │ 引用 ────────────── 插入 ✨  │
│ │    └── 选中文本 ───────┼───┼──│                             │
│ │                        │   │  │ 专利侵权举证责任 (代理词)   │
│ └────────────────────────┘   │  │ 来源: 2025-钛金-代理词.md   │
│                              │  │                             │
│ Ctrl+K → AI 辅助写作         │  │ 📜 相关法条                  │
│                              │  │ 专利法第65条（赔偿数额）     │
│                              │  │ 查看全文 ── 复制引用        │
│                              │  │                             │
│                              │  │ ⚖️ 相关判例                  │
│                              │  │ 钛金 v 高德 (UPC_CFI_365)  │
│                              │  │ 裁判要点: 禁令+赔偿         │
│                              │  │                             │
│                              │  │ ✨ AI 写作辅助               │
│                              │  │ 基于以上知识生成损害赔偿     │
│                              │  │ 计算的事实与理由段落...     │
│                              │  │                             │
└──────────────────────────────┘  └─────────────────────────────┘
```

### 交互设计

| 用户操作 | Sidebar 响应 |
|---------|-------------|
| 编辑器中光标移动 / 选中文本 | 根据当前上下文（段落主题、选中的法条号）自动检索相关知识 |
| 在 Sidebar 搜索框输入 | 混合检索（关键词 + 语义）返回排序结果 |
| 点击知识条目的"插入" | 将段落内容插入编辑器光标位置 |
| 点击知识条目的"引用" | 插入引用脚注（来源 + 案号） |
| 按 Ctrl+K | AI 辅助写作：根据选中的文本 + 检索到的相关知识 + 当前文书风格，生成写作建议 |
| 选中一段 AI 生成内容 | Sidebar 显示生成来源（基于哪些知识点），支持"接受"/"修改"/"拒绝" |

### 技术要点

- Sidebar 作为 TipTap 的 `NodeView` 或独立 Vue 组件，与编辑器共享 Pinia store
- 搜索 debounce 300ms，混合检索结果缓存 30 秒
- AI 生成的 writing suggestion 以 TipTap `decoration` 方式嵌入（灰色下划线），不直接修改文档
- 用户接受后才转为正式文本

### 当前基础

- TipTap 编辑器 ✅ (Phase 10.1)
- Docsy 桥接 + 模板引擎 ✅ (Phase 10.2)
- 知识库表结构 ✅ (Phase 9.4)
- AI backend ✅ (Phase 13.2)

---

## 五、UI 主题定制（P3）

### 原则

**不搞全局主题工程，功能做到哪，风格定到哪。**

### 参考方向

- 左侧窄导航（40-48px），图标 + 悬浮展开标签
- 内容区最大化，克制色彩（主色 1 个 + 中性灰阶）
- 信息密度高但不杂乱：卡片留白统一 16px，字体阶差 13px/15px/18px
- 图标系统统一（推荐 Lucide，Element Plus 图标不够用）

### 实施策略

| 阶段 | 范围 | 做法 |
|------|------|------|
| 先用 | 全局 | 定义 5 个 CSS 变量（主色、文字色、背景色、边框色、圆角），覆盖 Element Plus 默认值 |
| 做看板时 | 看板页 | 定义卡片/拖拽列/状态的组件样式 |
| 做 Sidebar 时 | 编辑器页 | 定义双栏布局、面板背景、搜索结果样式 |
| 最后 | 全站 | 统一间距/字号/动效，做 dark mode |

---

## 六、AI 提示词矩阵

### 已规划场景

| 场景 | 当前状态 | 实现难度 | 预估效果 |
|------|---------|---------|---------|
| 文档分类 + 收件箱路由 | 待做（P0） | 中 | ⭐⭐⭐⭐⭐ 立竿见影 |
| 信息提取（OCR 后结构化） | 飞书导入已有部分 | 中 | ⭐⭐⭐ |
| 知识检索增强 | 待做（P0-P1） | 中 | ⭐⭐⭐⭐ |
| 写作辅助（Copilot） | 待做（P2） | 高 | ⭐⭐⭐⭐ |
| 期限建议 | deadline_rules 表已有规则引擎 | 低 | ⭐⭐ |

### 不在范围内的场景

- **案件胜诉分析**：LLM 做不准法律推理，不应交给个人工具
- **自动生成完整文书**：法律文书需要律师逐字确认，AI 只能做段落级辅助
- **批量处理**：个人工具无需 pipeline，每次一份文件按需处理

---

## 七、执行路线图

### Phase A：让 AI 帮你干活（1-2 天）

| 序号 | 任务 | 工作量 | 依赖 | 状态 |
|------|------|-------|------|------|
| A1 | 文档分类 prompt + 收件箱路由 | 一天 | 无 | ✅ 已完成 |
| A2 | 多条件筛选前端 UI | 半天 | 无 | ✅ 已完成 |
| A3 | 信息提取 prompt（OCR 后结构化） | 一天 | A1 | ✅ 已完成 |

**里程碑**：收到新文件 → AI 自动分类并关联案件 → 筛选面板快速定位案件 ✅

#### Phase A 完成内容

**A1: AI 文档分类 prompt + 收件箱路由**
- ✅ 创建 `src-tauri/src/ai/prompts/classify_document.md` — 文档分类 prompt 模板
- ✅ 创建 `src-tauri/src/ai/prompts/extract_info.md` — 信息提取 prompt 模板
- ✅ 完善 `ai/mod.rs`：
  - 实现 `classify_document_with_prompt` 函数
  - 实现 `extract_info_with_prompt` 函数
  - 实现 `process_inbox_with_ai` 函数
  - 实现 `route_by_confidence` 路由逻辑
- ✅ 更新收件箱处理流程：
  - `add_inbox_item` 调用 AI 分类并写入 ai_category、ai_confidence、ai_extracted 字段
  - `process_inbox_item` 使用 prompt 增强分类并自动匹配案件
  - `list_inbox_items` 返回 ai_extracted 和 ai_suggested_case_id 字段
- ✅ 更新 InboxView.vue：
  - 显示 AI 提取的结构化信息（案号、法院、专利号、当事人、庭审日期、期限）
  - 添加"接受关联"按钮处理 AI 建议的案件关联

**A2: 多条件筛选前端 UI**
- ✅ 完善 CaseFilterBar.vue：
  - 状态筛选（进行中/已结案/全部）
  - 客户筛选（远程搜索下拉）
  - 日期范围筛选
  - 保存筛选方案按钮
  - 清除筛选按钮
- ✅ 创建 `src/shared/components/ReferenceSelect.vue`：
  - 基于 el-select + filterable + remote 的远程搜索下拉组件
  - 支持案件/客户/法官的模糊搜索
- ✅ 更新 cases store 和后端以支持日期范围筛选

**A3: 信息提取 prompt**
- ✅ 完善 `extract_info.md` prompt：
  - 支持传票、口审通知书、判决书、起诉状、审查意见通知书
  - 输出包含：案号、当事人、法院、日期、专利号等
  - OCR 文本预清洗逻辑

### Phase B：知识库开始运转（2-3 天）

| 序号 | 任务 | 工作量 | 依赖 |
|------|------|-------|------|
| B1 | 知识入库：选中文本 → 标注 → 存入 knowledge_items | 一天 | 无 | ✅ 已完成 |
| B2 | 混合检索：FTS5 + Ollama embedding | 一天 | B1 | ✅ 已完成 |
| B3 | 风格标注体系 + 标签模板 | 一天 | B1 | ✅ 已完成 |

**里程碑**：主动标注 30+ 条知识 → 语义搜索可用 → 风格标签就绪 ✅

#### Phase B 完成内容

**B1: 知识入库（选中文本→标注→存入）**
- ✅ 创建 `src/modules/knowledge/composables/useKnowledgeCapture.js`：
  - `captureFromSelection(text, source, tags, category)` — 从选中文本创建知识条目
  - `linkToLawArticle(knowledgeId, lawName, articleNo)` — 关联法条
  - `linkToCase(knowledgeId, caseId, relationType)` — 关联案件
  - `startCapture(selectedText, title)` — 弹窗式捕获流程
- ✅ 更新 `LegalEditor.vue`：右键菜单（标记常用段落/提取法条引用/记录判例要点/5种风格标注）+ 标签输入弹窗
- ✅ 更新 `WritingView.vue`：同上右键菜单和弹窗
- ✅ 后端命令 `create_knowledge_from_selection`、`link_knowledge_to_case`、`link_knowledge_to_law`

**B2: 混合检索**
- ✅ 创建 `src-tauri/src/db/search.rs`：
  - `hybrid_search(query, limit)` — FTS5 关键词检索 + Ollama nomic-embed-text 语义检索
  - RRF (Reciprocal Rank Fusion) 融合排序，k=60
  - `embed_knowledge_item` — 单条知识 embedding 生成
  - `embed_all_knowledge` — 批量 embedding 生成
- ✅ 新增 `knowledge_embeddings` 表存储向量（BLOB 格式，768 维）
- ✅ Tauri 命令 `hybrid_search_knowledge`、`embed_knowledge`、`embed_all_knowledge`

**B3: 风格标注体系**
- ✅ 扩展 `knowledge_items.category` CHECK 约束：新增 `complaint`、`defense_brief`、`legal_opinion`、`lawyer_letter`、`reply_brief`、`common_paragraph`、`law_reference`
- ✅ 创建 `src/modules/knowledge/views/KnowledgeStyleGuide.vue`：
  - 5 种文书风格（起诉状/代理词/法律意见/律师函/答辩状）的特征描述和适用场景
  - 每种风格下的知识条目列表
  - 路由：`/knowledge/style-guide`

### Phase C：文书工坊 · Copilot（2-3 天）

| 序号 | 任务 | 工作量 | 依赖 | 状态 |
|------|------|-------|------|------|
| C1 | Copilot Sidebar 框架 + 搜索框 | 一天 | B2 | ✅ 已完成 |
| C2 | 检索结果展示 + "插入/引用"交互 | 一天 | C1 | ✅ 已完成 |
| C3 | AI 写作辅助（Ctrl+K） | 一天 | C2, B3 | ✅ 已完成 |

**里程碑**：编辑器中随时检索自己的知识库 → AI 基于上下文生成段落建议 ✅

#### Phase C 完成内容

**C1: Copilot Sidebar 框架 + 搜索框**
- ✅ 创建 `src/modules/docs/components/CopilotSidebar.vue`：
  - 右侧面板，与 TipTap 编辑器并列（双栏布局）
  - 顶部搜索框（debounce 300ms）
  - 四个可折叠区域：相关段落、相关法条、相关判例、AI 写作辅助
  - 使用 `hybrid_search_knowledge` 混合检索（FTS5 + 语义向量）
- ✅ 在 `WritingView.vue` 中集成 CopilotSidebar：
  - 双栏布局：左编辑器 + 右 Sidebar（340px）
  - 编辑器中光标移动时自动检索上下文相关知识（500ms 防抖）
  - 工具栏新增"✨ AI 辅助"按钮

**C2: 检索结果展示 + 插入/引用交互**
- ✅ 每个检索结果显示：
  - 标题、来源分类标签、相关度分数、检索源（fts/semantic/hybrid）
  - 插入按钮 → 将内容插入编辑器光标位置
  - 引用按钮 → 插入引用脚注（来源 + 案号）
  - 复制按钮 → 复制内容到剪贴板
- ✅ 点击知识条目可展开查看完整内容

**C3: AI 写作辅助（Ctrl+K）**
- ✅ 实现 Ctrl+K 快捷键弹出 AI 写作辅助对话框
- ✅ 对话框支持：
  - 输入写作意图
  - 选择文书风格（起诉状/代理词/法律意见/律师函/答辩状/通用）
  - 快捷操作按钮（损害赔偿/技术比对/诉讼请求/案件事实）
- ✅ 调用 AI 后端 `generate_writing_suggestion`，注入当前上下文 + 检索到的相关知识 + 文书风格
- ✅ 生成结果以高亮标记方式嵌入编辑器
- ✅ 创建 `src/modules/docs/composables/useCopilot.js`：
  - `searchContext(editorContent)` — 根据编辑器内容自动检索相关知识
  - `searchKnowledge(query)` — 混合检索知识库
  - `generateWriting(intent, context, style)` — 调用 AI 生成写作建议
  - `insertSuggestion(text)` — 将建议插入编辑器
  - `insertToEditor(item)` / `insertCitation(item)` / `copyContent(item)`
- ✅ 后端新增 `generate_writing_suggestion` Tauri 命令（`src-tauri/src/ai/mod.rs`）：
  - 根据 intent/context/knowledge/style 调用 Ollama 或 OpenAI 生成写作建议
  - 集成 TokenBudget 配额保护

### Phase D：看板 + UI（2-3 天）

| 序号 | 任务 | 工作量 | 依赖 | 状态 |
|------|------|-------|------|------|
| D1 | 字段引用组件（下拉搜索） | 一天 | A1 | ✅ 已完成 |
| D2 | 看板视图（拖拽 + 卡片） | 一到两天 | D1 | ✅ 已完成 |
| D3 | UI 主题统一 | 一天 | D2 | ✅ 已完成 |

**里程碑**：看板工作流可用 → 全站风格统一 ✅

#### Phase D 完成内容

**D1: 字段引用组件（下拉搜索）**
- ✅ 完善 `src/shared/components/ReferenceSelect.vue`：
  - 新增 `knowledge` 类型支持（调用 `hybrid_search_knowledge`）
  - 新增 `judge` 类型（原 `official` 重命名）
  - 多选模式完善（缓存已选项标签、正确传递选中对象数组）
  - 搜索结果显示摘要标签（案件状态 tag、知识条目分类 tag）
  - 选项布局优化（主标签 + tag + 副标签三行结构）
- ✅ 在 `CaseInfoPanel.vue` 中集成：
  - 客户字段从手动输入改为 ReferenceSelect（type=client，远程搜索）
- ✅ 在 `CaseDetailView.vue` 中集成：
  - 关联案件弹窗：手动搜索替换为 ReferenceSelect（type=case）
  - 添加庭审弹窗：新增关联案件字段（ReferenceSelect type=case）
  - 清理废弃的手动搜索 CSS 代码

**D2: 看板视图（拖拽 + 卡片）**
- ✅ 安装 `vue-draggable-plus` 依赖
- ✅ 创建 `src/modules/cases/views/KanbanView.vue`：
  - 五列看板：待处理、证据交换、庭审准备、等待判决、已结案
  - 案件卡片展示：案号、当事人、下个期限日期、优先级标记（🔴🟠🟡）、轨道标签
  - 拖拽卡片到新列 → 自动更新 case_status + 记录到 case_logs
  - 卡片点击 → 跳转到案件详情
  - 每列显示案件数量 badge
- ✅ 添加路由 `/cases/kanban`（name: case-kanban）
- ✅ 在 App.vue 侧边栏添加看板入口

**D3: UI 主题统一**
- ✅ 创建 `src/assets/theme.css`：
  - 5 个核心 CSS 变量：`--c-primary`、`--c-text`、`--c-bg`、`--c-border`、`--c-radius`
  - 覆盖 Element Plus 默认值（颜色、圆角、字号）
  - 统一字体阶差：13px / 15px / 18px
  - 统一间距系统：16px 基础（4/8/16/24/32）
  - 侧边栏变量：48px 收起 / 180px 展开
- ✅ 在 `main.js` 中引入 theme.css
- ✅ 重构 App.vue 侧边栏：
  - 左侧窄导航（48px），图标居中
  - 悬浮展开标签（transition 动画）
  - 活跃项左侧蓝色指示条
  - 品牌标识简化为字母 "C"
  - 知识库入口添加到导航菜单

---

## 八、当前基础核查

| 已就位 | 状态 | 位置 |
|--------|------|------|
| TipTap 编辑器 | ✅ | `src/modules/documents/` |
| Docsy 模板引擎 | ✅ | `src-tauri/src/docsy_engine/` |
| AI backend (Ollama/OpenAI) | ✅ | `src-tauri/src/ai/` |
| 知识库表 (items + relations + FTS) | ✅ | `src-tauri/src/db/schema.rs` |
| 案件表 (含 filter 逻辑) | ✅ | `src-tauri/src/db/schema.rs` |
| 收件箱 | ✅ | `src/modules/inbox/` |
| 飞书导入 | ✅ | `src-tauri/src/sync/` |

| 缺失 | 需新建 |
|------|-------|
| ~~AI 文档分类 prompt 设计~~ | ~~`src-tauri/src/ai/prompts/classify.md`~~ ✅ 已创建 |
| ~~字段引用搜索组件~~ | ~~`src/shared/components/ReferenceSelect.vue`~~ ✅ 已创建 |
| ~~Copilot Sidebar 组件~~ | ~~`src/modules/docs/components/CopilotSidebar.vue`~~ ✅ 已创建 |
| ~~混合检索 (embedding + FTS5)~~ | ~~`src-tauri/src/db/search.rs`~~ ✅ 已创建 |
| ~~看板视图~~ | ~~`src/modules/cases/views/KanbanView.vue`~~ ✅ 已创建 |
| ~~UI 主题~~ | ~~`src/assets/theme.css`~~ ✅ 已创建 |

---

> 最后更新：2026-07-31（Phase A + Phase B + Phase C + Phase D 完成）

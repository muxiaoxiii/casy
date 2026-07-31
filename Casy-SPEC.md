# Casy 综合技术规格文档

> **版本**: v2.0 (consolidated)  
> **日期**: 2026-07-31  
> **定位**: 本地优先的专利律师案件管理 + 文书生成 + 飞书同步 + AI 收件箱 + 案卷管理  
> **栈**: Tauri 2 + Vue 3 + SQLite (SQLCipher) + Rust  
> **说明**: 本文档合并了原 `Casy-完整规划文档.md`、`Casy-补充规格.md`、五份实现规格文档及 `期限规则修正方案.md` 的全部内容。

---

## 目录

1. [技术选型与架构](#一技术选型与架构)
2. [数据层](#二数据层)
3. [核心模块](#三核心模块)
4. [UI 组件](#四ui-组件)
5. [收件箱与解析](#五收件箱与解析)
6. [同步引擎](#六同步引擎)
7. [文书工坊](#七文书工坊)
8. [AI 模式](#八ai-模式)
9. [安全与隐私](#九安全与隐私)
10. [错误处理与测试](#十错误处理与测试)
11. [部署](#十一部署)
12. [扩展性设计](#十二扩展性设计)

---

## 一、技术选型与架构

### 1.1 技术选型总表

| 技术 | 可行性 | 风险 | 推荐方案 |
|------|--------|------|---------|
| Tauri 2 桌面框架 | ✅ 确定可行 | 无 | 与 Docsy 一致 |
| Vue 3 + Element Plus | ✅ 确定可行 | 无 | 与 Docsy 一致 |
| SQLite + FTS5 + SQLCipher | ✅ 确定可行 | 无 | rusqlite bundled-sqlcipher |
| WebDAV 同步 | ✅ 确定可行 | ETag 因服务器而异 | VACUUM INTO + 临时路径上传 |
| 飞书 Bitable API | ✅ 可行 | 限流 | 令牌桶 + 队列 |
| TipTap 编辑器 | ✅ 可行 | 中文 IME 在 WebKit 有已知问题 | isComposing 守卫 + 防抖 |
| Tesseract OCR | ⚠ 有限 | 中文精度 70-85% | `rusty-tesseract` CLI 调用 |
| IMAP 邮件监听 | ✅ 确定可行 | 连接断开需重连 | async-imap + IDLE + 重连循环 |
| DOCX 导出 | ✅ 确定可行 | 复杂格式需定制 | html-to-docx 或 docx npm |
| Ollama 本地 AI | ✅ 可行 | 大模型需 16GB+ 内存 | qwen2.5:14b 或更小模型 |

### 1.2 关键技术决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| IMAP crate | `async-imap` (非 `imap`) | 原生 async + tokio，IDLE 推送通知 |
| OCR 引擎 | Tesseract 为主 + Vision LLM 补充 | 本地免费，复杂文档用 LLM 补充 |
| SQLite 安全拷贝 | `VACUUM INTO` | 原子操作，单文件输出 |
| WebDAV 上传 | 临时路径 → MOVE | 防止上传中断导致文件损坏 |
| 编辑器 | TipTap v3.29+ | Vue 3 原生支持，Suggestion 扩展 |
| AI 后端 | Ollama / OpenAI 兼容 API | 双模式，用户可选 |
| 节假日数据 | 内置当年 + JSON 文件更新 | 不依赖网络 |

### 1.3 分层架构 [已实现]

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Casy 桌面应用                               │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    前端层 (Vue 3 + Element Plus)               │  │
│  │  案件管理 | 时间线 | 任务 | 日历 | 收件箱 | 文书工坊          │  │
│  │  案卷文件 | 设置 | 同步状态 | 知识库 | 邮件记录 | 看板        │  │
│  │  Pinia Stores: cases | tasks | inbox | files | settings       │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕ Tauri IPC                            │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    命令层 (70 个 Tauri Commands)               │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    业务层 (Rust Modules)                       │  │
│  │  db/ | formula/ | sync/ | parse/ | ai/ | files/ | docsy/     │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  存储层: SQLite (WAL + SQLCipher) + 文件系统 + 外部 API       │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.4 模块间数据流

```
收件箱 → AI 分类 + 案件匹配 → 案件
客户 ← 案件 → 办案日志 / 庭审 / 任务 / 案卷文件 / 知识库 / 官方人员 / 案件关系
期限引擎 ← cases + deadline_rules → DeadlineResult
日历 ← hearings + deadlines + tasks → CalendarEvent
时间线 ← case_logs + hearings + tasks → TimelineEvent
文书工坊 ← cases → Docsy → case_files + case_logs
```

### 1.5 事件系统 [已实现]

模块间通过 Pinia store watch 和 Tauri emit/listen 通信：
- 案件创建 → 自动创建文件夹 + 初始化关联
- 庭审创建 → 自动生成准备任务
- 收件箱归档 → 自动记录日志

### 1.6 SQLite 性能预估

数据量 < 1 MB，全量 WebDAV 同步 < 1秒（宽带）。

---

## 二、数据层 [已实现]

### 2.1 Cargo.toml 依赖 [已实现]

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
keyring = "3"
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio = { version = "1", features = ["full"] }
regex = "1"
uuid = { version = "1", features = ["v4"] }
quick-xml = "0.37"
calamine = { version = "0.26", features = ["dates"] }
mailparse = "0.15"
async-imap = { version = "0.11", default-features = false, features = ["runtime-tokio"] }
async-native-tls = "0.5"
rusty-tesseract = "1.1"
```

### 2.2 SQLite Schema [已实现]

18 张表 + FTS5 + 触发器 + 种子数据，含完整迁移框架 (CURRENT_SCHEMA_VERSION + MIGRATIONS)。

#### 核心表

| 表名 | 用途 | 关键字段 |
|------|------|---------|
| `cases` | 案件 | id, track, case_name, case_no, client_name, 10个专利无效专属字段 |
| `clients` | 客户 | id, name, type |
| `case_logs` | 办案日志 | case_id, event_type, event_date |
| `hearings` | 庭审 | case_id, hearing_date, judges |
| `tasks` | 任务 | case_id, priority, deadline, completed |
| `officials` | 官方人员 | name, role, court |
| `case_relations` | 案件关系 | source/target_case_id, relation_type |
| `deadline_rules` | 期限规则 | track, trigger_field, offset_days, calc_method |
| `case_deadlines` | 案件期限 | case_id, rule_id, due_date |
| `inbox_items` | 收件箱 | source_type, ai_category, ai_confidence |
| `case_files` | 案卷文件 | case_id, category, file_path |
| `email_records` | 邮件 | subject, from_address, linked_case_id |
| `knowledge_items` | 知识库 | title, category, content, tags |
| `knowledge_relations` | 知识关联 | source/target_id |
| `drafts` | 草稿 | case_id, content_html, version |
| `sync_map` | 同步映射 | feishu_record_id ↔ local_id |
| `sync_queue` | 同步队列 | action, status |
| `imap_accounts` | IMAP 配置 | server, username |
| `skills` | 技能/工作流 | name, description |
| `knowledge_embeddings` | 知识向量 | item_id, embedding (BLOB 768维) |

#### FTS5 全文搜索 [已实现]

- `cases_fts` — 案件名称/案号/当事人/专利名
- `files_fts` — 文件名/摘要/关键词
- `knowledge_fts` — 知识库全文搜索

#### 触发器 [已实现]

- `trg_cases_updated` — updated_at 自动更新
- `trg_cases_status_insert/update` — case_status 自动计算
- `trg_clients_updated`, `trg_case_deadlines_updated`, `trg_drafts_updated`
- FTS 同步触发器 (cases_fts, files_fts, knowledge_fts 各 3 个)

### 2.3 期限规则种子数据 [已实现]

**专利无效 (patent_invalidation)** — 5 条规则：
- 专利权人陈述意见：收到受理通知书之日起1个月
- 请求人答复意见：收到专利权人陈述意见之日起1个月
- 专利权人补充意见：收到请求人补充意见之日起1个月
- 请求人补充意见：收到专利权人陈述意见之日起1个月
- 预估审限（无效）：立案之日起5个月

**行政诉讼 (admin_litigation)** — 5 条规则：
- 提交答辩状：15日（日历日）— 行政诉讼法第67条
- 预估审限（简易）：3个月 + procedure_type='简易' — 行政诉讼法第84条
- 预估审限（普通）：6个月 + procedure_type='普通' — 行政诉讼法第81条
- 判决上诉期：15日 + verdict_type='判决' — 行政诉讼法第85条
- 裁定上诉期：10日 + verdict_type='裁定' — 行政诉讼法第85条

**民事诉讼 (civil_tort)** — 5 条规则：
- 提交答辩状：15日（日历日）— 民事诉讼法第128条
- 预估审限（简易）：3个月 + procedure_type='简易' — 民事诉讼法第164条
- 预估审限（普通）：6个月 + procedure_type='普通' — 民事诉讼法第152条
- 判决上诉期：15日 + verdict_type='判决' — 民事诉讼法第171条
- 裁定上诉期：10日 + verdict_type='裁定' — 民事诉讼法第171条

### 2.4 2026 年节假日数据 [已实现]

已修正（经 timor.tech 交叉验证）：
- 元旦 1/1-3, 春节 2/15-23（9天）, 清明 4/4-6, 劳动节 5/1-5, 端午 6/19-21, 中秋 9/25-27, 国庆 10/1-7
- 调休：1/4, 2/14, 2/28, 5/9, 9/20, 10/10
- 11 个单元测试全部通过
- 支持 JSON 导入覆盖（`import_holidays_json` 命令）

### 2.5 飞书数据导入 [已实现]

- 事务包裹，幂等导入（INSERT OR REPLACE）
- 5 张表完整导入：cases, hearings, tasks, officials, case_relations
- 10 个专利无效专属字段完整导入
- DuplexLink 关联字段解析
- 自动检测案件关系（同专利/同客户/审级关联）

---

## 三、核心模块 [已实现]

### 3.1 案件管理 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| CRUD | ✅ | 7 个 Tauri command + rusqlite |
| 列表 | ✅ | 分页(50/页) + 多条件筛选 + 排序 + 分组 |
| 全文搜索 | ✅ | FTS5 MATCH 查询 |
| 详情 | ✅ | 三栏布局(信息+时间线+关联) + 自动保存(2秒防抖) |
| 行颜色 | ✅ | 🔴3天内 🟡14天内 ⬜已完结 |
| 文件夹 | ✅ | 创建案件时自动建 7 个子文件夹 |
| 关系网 | ✅ | add/get/remove_relation + 自动检测 |
| 导出 | ✅ | CSV 格式，带筛选条件 |
| 删除 | ✅ | 物理文件移入系统回收站 (trash crate) |

**Rust CRUD**: `db/cases.rs` — list_cases, get_case, insert_case, update_case, delete_case, search_cases, case_counts_by_client/track, active_cases

**Tauri 命令**: list_cases, get_case, create_case, update_case, delete_case, search_cases, case_stats

### 3.2 时间线 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| 事件合并 | ✅ | UNION case_logs + hearings + tasks |
| 颜色图标 | ✅ | submitted=📤绿 received=📥蓝 record=📝灰 hearing=📅蓝 task=📌紫 |
| 按月分组 | ✅ | YYYY-MM 分组 + 蓝色月份标题 + 分隔线 |
| 添加/删除 | ✅ | 弹窗操作 |

**Rust**: `db/timeline.rs` — TimelineEvent 结构体 + case_timeline 函数

### 3.3 任务管理 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| 四象限 | ✅ | urgent_important / important / urgent / normal |
| 截止日倒计时 | ✅ | 过期标红（红色边框 + 粉红背景 + "已逾期N天"） |
| 自动任务 | ✅ | 创建庭审时自动生成 6 个准备任务 |
| 编辑 | ✅ | el-drawer 抽屉 + 案件搜索下拉 |
| 日历事件颜色 | ✅ | 紫色 |

### 3.4 日历 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| 月视图 | ✅ | 7列网格 |
| 五色事件 | ✅ | 蓝=口审 红=开庭 黄=二审 橙=期限 紫=任务 |
| 点击交互 | ✅ | 点击日期展开事件列表，点击事件跳转案件 |
| 图例 | ✅ | 完整图例显示 |

### 3.5 期限引擎 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| 规则表 | ✅ | deadline_rules 声明式配置，按 track 匹配 |
| 工作日 | ✅ | 中国法定节假日 + 调休，JSON 可更新 |
| 计算 | ✅ | day / workday / month 三种偏移，含条件判断 |
| 预警 | ✅ | 🔴≤3天 🟡4-14天 🟢>14天 |
| 每日重算 | ✅ | 启动时 + 每天 00:01 |
| Dashboard 统计 | ✅ | get_dashboard_stats 命令 |

**修正要点**（专利法实施细则第5条）：
- 起算日不计入，从次日起算
- 休假日顺延到下一个工作日
- 月末钳制（clamping）

### 3.6 案件关系 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| 关系类型 | ✅ | same_patent / same_party / appeal_of / cross_reference |
| 双向存储 | ✅ | A→B 和 B→A 同时写入 |
| 自动检测 | ✅ | 按专利号/客户名自动建议关联 |
| 可视化 | ✅ | CaseNetworkView 按关系类型分层展示 |

### 3.7 知识库 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| CRUD | ✅ | knowledge_items + knowledge_relations |
| FTS5 搜索 | ✅ | knowledge_fts 全文搜索 |
| 混合检索 | ✅ | FTS5 关键词 + Ollama nomic-embed-text 语义向量 + RRF 融合 |
| 知识入库 | ✅ | 选中文本 → 右键菜单 → 标注 → 存入 |
| 风格标注 | ✅ | 5 种文书风格标签 |
| 法条专用字段 | ✅ | law_name / article_no / effective_date / status |
| 版本追踪 | ✅ | knowledge_versions 表 |

### 3.8 案卷文件管理 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| 7 子目录 | ✅ | 传票/证据/交文/收文/内部/通信/其他 |
| 上传/删除 | ✅ | CaseFilesView + files.rs |
| 自动命名 | ✅ | {日期}_{分类}_{案号}.{扩展名} + SHA-256 防覆盖 |
| FTS5 索引 | ✅ | files_fts |

---

## 四、UI 组件 [已实现]

### 4.1 路由 [已实现]

| 路由 | 组件 | 说明 |
|------|------|------|
| `/` | HomeView | 首页 Dashboard（统计+期限预警+最近活动） |
| `/cases` | CaseListView | 案件列表（分组+筛选+排序+分页+导出） |
| `/cases/:id` | CaseDetailView | 案件详情（三栏布局） |
| `/cases/kanban` | KanbanView | 看板视图（五列拖拽） |
| `/cases/network` | CaseNetworkView | 关系网络 |
| `/calendar` | CalendarView | 日历月视图 |
| `/tasks` | TasksView | 任务四象限 |
| `/inbox` | InboxView | 收件箱（待处理/已归档） |
| `/documents` | DocumentGenView | 文书生成 |
| `/write/:caseId?` | WritingView | TipTap 编辑器 + Copilot Sidebar |
| `/files/:caseId` | CaseFilesView | 案卷文件管理 |
| `/sync` | SyncStatusView | 同步状态（WebDAV+飞书+冲突解决） |
| `/settings` | SettingsView | 设置（5 标签页） |
| `/knowledge/style-guide` | KnowledgeStyleGuide | 知识库风格指南 |

### 4.2 Pinia Stores [已实现]

| Store | 文件 | 功能 |
|-------|------|------|
| `cases` | `src/stores/cases.js` | 案件 CRUD + 筛选 + 分组 |
| `tasks` | `src/stores/tasks.js` | 任务 CRUD + 四象限 |
| `inbox` | `src/stores/inbox.js` | 收件箱 CRUD + 处理 |
| `files` | `src/stores/files.js` | 文件 CRUD |
| `settings` | `src/stores/settings.js` | WebDAV/飞书/AI/IMAP/通用配置 |

### 4.3 Vue 组件清单 [已实现]

**已实现 (25 个组件)**：

| 组件 | 用途 |
|------|------|
| `CaseFilterBar.vue` | 案件列表顶部多条件筛选栏 |
| `CaseGroupPanel.vue` | 可折叠分组面板 |
| `CaseInfoPanel.vue` | 案件详情左栏（可编辑字段） |
| `CaseTimelinePanel.vue` | 案件详情中栏（合并时间线） |
| `CaseRelatedPanel.vue` | 案件详情右栏（期限+关联+快捷操作） |
| `CaseNetworkView.vue` | 关系网络可视化 |
| `DeadlinePanel.vue` | 全局期限预警面板 |
| `TaskDetailPanel.vue` | 任务编辑抽屉 |
| `InboxView.vue` | 统一收件箱 |
| `SyncStatusView.vue` | 同步状态页 |
| `ConflictResolver.vue` | 同步冲突并排对比解决器 |
| `TemplateBrowser.vue` | 文书模板列表浏览 |
| `DocumentGenView.vue` | 文书生成页 |
| `WritingView.vue` | TipTap 编辑器 + 案件侧栏 + Copilot |
| `LegalEditor.vue` | TipTap 编辑器核心组件 |
| `CopilotSidebar.vue` | 知识检索 + AI 写作辅助侧栏 |
| `ReferenceSelect.vue` | 远程搜索下拉组件 |
| `KanbanView.vue` | 看板视图（五列拖拽） |
| `KnowledgeStyleGuide.vue` | 知识库风格指南 |
| `SettingsView.vue` | 设置页（5 标签页） |
| `CaseListView.vue` | 案件列表页 |
| `CaseDetailView.vue` | 案件详情页 |
| `CalendarView.vue` | 日历月视图 |
| `TasksView.vue` | 任务四象限 |
| `HomeView.vue` | 首页 Dashboard |

### 4.4 UI 主题 [已实现]

- `src/assets/theme.css`：5 个核心 CSS 变量 (`--c-primary`, `--c-text`, `--c-bg`, `--c-border`, `--c-radius`)
- 左侧窄导航（48px），图标居中，悬浮展开标签
- 统一字体阶差：13px / 15px / 18px
- 统一间距系统：16px 基础 (4/8/16/24/32)

### 4.5 全局交互 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| 全局错误处理 | ✅ | `tauriCall()` + `tauriCallSafe()` |
| Loading Skeleton | ✅ | CaseListView / InboxView / TasksView |
| 自动保存 | ✅ | 2 秒防抖 + toast |
| 系统托盘 | ✅ | TrayIconBuilder + 菜单 |
| 全局热键 | ✅ | Cmd+Shift+V → 剪贴板到收件箱 |
| 文件夹监听 | ✅ | ~/Documents/Casy/inbox/ 新文件自动导入 |
| 拖拽上传 | ✅ | 收件箱拖拽上传区 |

---

## 五、收件箱与解析 [已实现]

### 5.1 文本提取 [已实现]

| 格式 | 实现 | 依赖 |
|------|------|------|
| PDF | `pdf_extract::extract_text_from_mem` | pdf-extract |
| PDF OCR | `rusty_tesseract::tesseract` | 系统 Tesseract |
| DOCX | zip 解压 + quick-xml 提取 `<w:t>` 文本 | zip + quick-xml |
| .eml | `mailparse::parse_mail` | mailparse |
| 图片 OCR | rusty-tesseract | 系统 Tesseract |
| 剪贴板 | 直接读取 | — |

### 5.2 文档解析 [已实现]

| 文档类型 | 解析方案 | 提取字段 |
|---------|---------|---------|
| 传票 | 正则 + AI 增强 | 案号/日期/法院/法官/书记员 |
| 口审通知书 | 正则 + AI 增强 | 案件编号/专利号/请求人/合议组 |
| 判决书 | 正则 + AI 增强 | 案号/当事人/判决结果 |
| 起诉状 | AI 增强 | 案号/当事人/诉讼请求 |
| 审查意见 | AI 增强 | 专利号/审查意见/期限 |
| 通用 | Vision LLM 回退 | 全文 |

### 5.3 收件箱处理管道 [已实现]

```
文件进入收件箱
    ├── 文本提取（PDF/DOCX/OCR/邮件/剪贴板）
    ├── AI 分类（11 种类型 + 置信度评分）
    ├── 案件匹配（案号精确 → 专利号 → 当事人模糊）
    └── 路由决策
        ├── 高置信度 + 匹配 → 直接关联
        ├── 中置信度 → 列出候选让用户确认
        └── 低置信度 → 标记待处理
```

**InboxProcessor**:
- `process()` — 提取文本 → AI 分类 → 案件匹配
- `file_to_case()` — 归档到案件（文件拷贝 + 数据库事务）

**入口** [已实现]:
- UI 拖拽上传
- 系统托盘菜单
- 全局快捷键 Cmd+Shift+V
- 文件夹监听 ~/Documents/Casy/inbox/
- IMAP 邮件监听

### 5.4 IMAP 邮件监听 [已实现]

- `ImapWatcher` — 每个账号一个 tokio task
- IDLE 推送 + 29 分钟超时重连
- 白名单过滤（发件人/主题关键词）
- 自动解析 → 收件箱

---

## 六、同步引擎 [已实现]

### 6.1 WebDAV 同步 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| WebDavClient | ✅ | PUT/GET/HEAD/MKCOL/DELETE/MOVE |
| VACUUM INTO | ✅ | 安全拷贝数据库 |
| ETag 检测 | ✅ | HEAD 检查 + If-Match 条件 PUT |
| startup_sync | ✅ | 启动时 ETag 比较 + 冲突检测 |
| manual_sync | ✅ | 推送/拉取按钮 |
| 冲突解决 | ✅ | 并排对比 + 本地/远程选择 |

**同步协议**：
```
启动: HEAD → ETag 比较 → PUSH/PULL/冲突
PUSH: VACUUM INTO → PUT 临时路径 → MOVE 原子操作
PULL: GET → 完整性验证 → 替换本地
冲突: 用户选择（本地覆盖/远程覆盖）
```

**策略**: WebDAV 不自动推送，仅手动/启动/关闭时上传。

### 6.2 飞书同步 [已实现]

| 功能 | 状态 | 实现 |
|------|------|------|
| Auth | ✅ | FeishuAuth + keychain 存储 + 自动刷新（提前 60 秒） |
| PULL | ✅ | 分页拉取 → 时间戳对比 → INSERT/UPDATE + sync_map 映射 |
| PUSH | ✅ | local_newer 检测 → 字段转换 → POST/PUT |
| 限流 | ✅ | RateLimiter 令牌桶（5 req/s）+ 429 Retry-After 自动重试 |
| 设置 UI | ✅ | App ID/Secret 输入 + 测试连接 |

**策略**: 飞书 5 秒防抖自动 PUSH + 15 分钟定时 PULL。

### 6.3 防抖层级 [已实现]

| 层级 | 触发 | 延迟 | 目标 |
|------|------|------|------|
| L1 自动保存 | 用户停止输入 | 2 秒 | 写入本地 SQLite |
| L2 同步推送 | L1 保存完成 | 5 秒 | 推送到远程 |
| L3 安全拷贝 | 手动同步/启动 | 无延迟 | VACUUM INTO + 上传 |

---

## 七、文书工坊 [已实现]

### 7.1 TipTap 编辑器 [已实现]

**LegalEditor 组件**：
- StarterKit + Underline + Placeholder
- 3 个 Suggestion 扩展：
  - `{` → 案件字段补全（案号/客户/法院/日期等 13 个字段）
  - `【` → 法条补全（专利法常用条款）
  - `@` → 当事人补全（从 cases store 获取）
- 自动保存 + 2 秒 debounce

### 7.2 Copilot Sidebar [已实现]

**CopilotSidebar 组件**：
- 右侧面板（340px），与编辑器并列
- 顶部搜索框（debounce 300ms）
- 四个可折叠区域：相关段落 / 相关法条 / 相关判例 / AI 写作辅助
- 混合检索（FTS5 + 语义向量）
- 光标移动时自动检索上下文相关知识（500ms 防抖）

**交互**：
| 用户操作 | Sidebar 响应 |
|---------|-------------|
| 光标移动/选中文本 | 自动检索相关知识 |
| 搜索框输入 | 混合检索返回排序结果 |
| 点击"插入" | 插入内容到光标位置 |
| 点击"引用" | 插入引用脚注 |
| Ctrl+K | AI 辅助写作对话框 |

### 7.3 Docsy 桥接 [已实现]

- IPC bridge 调用 Docsy 模板引擎
- `mapCaseToTemplate` — 40+ 字段映射（文本/日期/party_list/reference/checkbox/radio）
- TemplateBrowser + DocumentGenView
- DOCX 导出

### 7.4 草稿管理 [已实现]

- drafts 表 CRUD
- 版本历史（version 字段自动递增）

---

## 八、AI 模式 [已实现]

### 8.1 后端架构 [已实现]

```rust
pub trait AiBackend: Send + Sync {
    async fn classify(&self, text: &str) -> Result<DocumentClassification>;
    async fn extract(&self, text: &str, doc_type: &str) -> Result<serde_json::Value>;
    async fn summarize(&self, text: &str) -> Result<String>;
    async fn call_raw(&self, prompt: &str) -> Result<String>;
}
```

**已实现后端**：
- `OllamaBackend` — 本地 Ollama 调用
- `OpenAiBackend` — OpenAI 兼容 API（远程）
- `NoOpBackend` — 无 AI 模式

### 8.2 用途 [已实现]

| 场景 | 实现 |
|------|------|
| 收件箱分类 | classify_document + extract_info prompt |
| 文档解析 | AI 增强提取 |
| 知识沉淀 | embedding 生成 (nomic-embed-text) |
| 写作辅助 | generate_writing_suggestion |

### 8.3 保护 [已实现]

- TokenBudget 日限额（默认 50 次）
- 用量统计
- PII 脱敏（发送前替换当事人姓名）
- 本地优先，远程 AI 需手动开启

---

## 九、安全与隐私 [已实现]

### 9.1 数据加密 [已实现]

| 数据 | 加密方案 |
|------|---------|
| SQLite 数据库 | SQLCipher (AES-256)，密钥从 OS keychain 读取 |
| IMAP/WebDAV/AI/飞书密码 | OS Keychain (keyring crate) |

**迁移**: `migrate_to_encrypted` + `sqlcipher_export` + 自动备份 .bak

### 9.2 数据保留

| 数据 | 策略 |
|------|------|
| 案件数据 | 永久（用户手动删除） |
| 办案日志 | 随案件级联删除 |
| 收件箱 | 已归档 30 天后可自动清理 |
| 邮件/知识库 | 永久 |

---

## 十、错误处理与测试 [已实现]

### 10.1 统一错误类型 [已实现]

```rust
#[derive(Debug, thiserror::Error)]
pub enum CasyError {
    Database(#[from] rusqlite::Error),
    Io(#[from] std::io::Error),
    Network(#[from] reqwest::Error),
    Ai(String),
    SyncConflict { local: String, remote: String },
    ParseError(String),
    TemplateError(String),
}
```

### 10.2 前端错误处理 [已实现]

- `tauriCall()` — Tauri 命令失败时自动显示 ElMessage.error
- `tauriCallSafe()` — 返回 `{ ok, data/error }` 供手动处理

### 10.3 测试 [已实现]

| 层 | 工具 | 状态 |
|----|------|------|
| Rust 单元测试 | `#[test]` | 16 个全部通过 |
| Rust 集成测试 | rusqlite in-memory DB | ✅ |
| 编译 | cargo check | 0 errors, 1 warning |

---

## 十一、部署 [待实现]

```bash
# macOS
npm run tauri build -- --bundles dmg

# Windows
npm run tauri build -- --target x86_64-pc-windows-msvc --bundles nsis
```

**外部依赖**（可选）：
- Tesseract + 中文语言包（OCR）
- Ollama（AI 模式）

---

## 十二、扩展性设计

### 12.1 新增案件类型 [已实现可扩展]

只需在 `deadline_rules` 表添加新规则，前端轨道筛选器动态读取。

### 12.2 新增文档解析类型 [已实现可扩展]

实现 `DocumentParser` trait，注册到解析器列表即可自动识别。

### 12.3 新增 AI 后端 [已实现可扩展]

实现 `AiBackend` trait。

### 12.4 新增同步目标 [已实现可扩展]

实现 `SyncTarget` trait。

---

## 附录：当前项目指标

| 指标 | 数值 |
|------|------|
| 代码行数 | 19,595 行 |
| Rust 命令 | 70 个 |
| Vue 组件 | 25 个 |
| Pinia Stores | 5 个 |
| 路由 | 14 个 |
| 测试 | 16 个全部通过 |
| 编译错误 | 0 |
| 编译警告 | 1（预留 API） |

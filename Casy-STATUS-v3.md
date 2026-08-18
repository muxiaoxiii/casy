# Casy 项目状态 v3.0

> **最后更新**: 2026-08-18  
> **版本**: v3.0  
> **架构**: 插件化架构（借鉴 Cordis + DSH）

---

## 一、当前项目状态

| 指标 | v0.2.0 | v3.0 |
|------|--------|------|
| 架构 | 传统分层 | 插件化 |
| 业务插件 | 0 | 9 |
| 注册工具 | 0 | 38 |
| AI 提供商 | 1 (Ollama) | 3+ (Ollama/OpenAI/DeepSeek) |
| 事件系统 | 无 | 统一事件系统 |
| 确认机制 | 分散 | 统一 L1/L2/L3 |
| Rust 命令 | 70 | 148 |
| Vue 组件 | 30 | 44 |
| 代码行数 | ~23,500 | ~43,300 |
| 路由 | 16 | 18 |

### v3.0 新增完成项

- ✅ **AI 审计日志接通** — `ai_routes.rs` 的 `log_ai_run` / `log_ai_context_item` 已接入 `process_inbox_with_ai` 与 `generate_writing_suggestion` 调用路径，SHA256 脱敏入库
- ✅ **R1-R4 后端分级** — `reminder_log` 表新增 `level` 字段，`dispatch_reminder` 在派发时计算 R1(温和 T>1)/R2(明确 T=1)/R3(强提醒 T=0)/R4(逾期 T<0) 并写入，前端 `ReminderView` 直接消费后端 level

---

## 二、插件系统

### 2.1 核心容器（CasyContext）

- ✅ 插件管理（use/unuse）
- ✅ 工具注册（registerTool/executeTool）
- ✅ 技能注册（registerSkill/executeSkill）
- ✅ AI 管理（registerProvider/getModels）
- ✅ 事件系统（on/emit）
- ✅ 画像管理（setProfile/getProfile）
- ✅ 确认机制（calculateEffectiveLevel/requestConfirm）

### 2.2 已注册插件（9 个）

| 插件 | 工具数 | 状态 |
|------|--------|------|
| CasesPlugin | 6 | ✅ |
| TasksPlugin | 5 | ✅ |
| KnowledgePlugin | 5 | ✅ |
| CalendarPlugin | 3 | ✅ |
| InboxPlugin | 5 | ✅ |
| ReminderPlugin | 4 | ✅ |
| FilesPlugin | 3 | ✅ |
| SyncPlugin | 4 | ✅ |
| SettingsPlugin | 3 | ✅ |
| **总计** | **38** | |

### 2.3 工具清单

#### 案件管理（6 个）
- `list_cases` — 获取案件列表
- `get_case` — 获取案件详情
- `create_case` — 创建案件
- `update_case` — 更新案件
- `delete_case` — 删除案件（需 L3 确认）
- `search_cases` — 搜索案件

#### 任务管理（5 个）
- `list_tasks` — 获取任务列表
- `create_task` — 创建任务
- `toggle_task` — 切换完成状态
- `update_task` — 更新任务
- `delete_task` — 删除任务

#### 知识库（5 个）
- `list_knowledge` — 获取知识列表
- `search_knowledge` — 搜索知识
- `create_knowledge` — 创建知识
- `update_knowledge` — 更新知识
- `delete_knowledge` — 删除知识

#### 日历与期限（3 个）
- `get_calendar_events` — 获取日历事件
- `get_deadline_warnings` — 获取期限预警
- `get_dashboard_stats` — 获取仪表盘统计

#### 收件箱（5 个）
- `list_inbox_items` — 获取收件箱列表
- `add_inbox_item` — 添加收件箱条目
- `process_inbox_item` — 处理收件箱条目
- `file_inbox_item` — 归档到案件
- `dismiss_inbox_item` — 忽略收件箱条目

#### 提醒（4 个）
- `list_reminder_rules` — 获取提醒规则
- `create_reminder_rule` — 创建提醒规则
- `get_reminder_log` — 获取提醒日志
- `start_reminder_engine` — 启动提醒引擎

#### 文件管理（3 个）
- `list_case_files` — 获取案件文件列表
- `add_case_file` — 上传文件到案件
- `delete_case_file` — 删除案件文件

#### 同步（4 个）
- `get_sync_status` — 获取同步状态
- `test_webdav_connection` — 测试 WebDAV 连接
- `manual_sync_push` — 手动推送同步
- `manual_sync_pull` — 手动拉取同步

#### 设置（3 个）
- `get_settings` — 获取设置
- `save_settings` — 保存设置
- `configure_ai` — 配置 AI 后端

---

## 三、AI 系统

### 3.1 模型提供商

| 提供商 | 状态 | 说明 |
|--------|------|------|
| Ollama | ✅ | 本地模型，自动发现 |
| OpenAI | ✅ | GPT 系列 |
| DeepSeek | ✅ | DeepSeek 系列 |
| 其他 | ✅ | 任何 OpenAI 兼容 API |

### 3.2 AI 工具调用

- ✅ AI 工具调用服务（AIToolCaller）
- ✅ 自动工具选择
- ✅ 确认策略检查
- ✅ 工具执行结果返回

### 3.3 AI 对话面板

- ✅ 模型选择
- ✅ 工具调用展示
- ✅ 确认对话框
- ✅ 审计日志

---

## 四、前端模块

### 4.1 已实现模块

| 模块 | 路由 | 状态 |
|------|------|------|
| 首页（今日工作台） | `/` | ✅ |
| 案件列表 | `/cases` | ✅ |
| 案件详情 | `/cases/:id` | ✅ |
| 案件看板 | `/cases/kanban` | ✅ |
| 案件关系网络 | `/cases/network` | ✅ |
| 任务工作台 | `/tasks` | ✅ |
| 日历 | `/calendar` | ✅ |
| 收件箱 | `/inbox` | ✅ |
| 知识库 | `/knowledge` | ✅ |
| 文书工坊 | `/docs` | ✅ |
| AI 智伴 | `/ai` | ✅ |
| 提醒 | `/reminder` | ✅ |
| 设置 | `/settings` | ✅ |

### 4.2 共享组件

| 组件 | 说明 |
|------|------|
| AIStatusBadge | AI 状态徽标 |
| DegradedBanner | 降级态提示条 |
| EmptyState | 空状态引导 |
| SkeletonCard | 骨架屏 |
| ReminderBanner | 提醒横幅 |
| OverdueMorningBrief | 逾期早报 |

---

## 五、数据层

### 5.1 Schema v9

- ✅ tasks 表 GTD 字段
- ✅ cases 表项目化字段
- ✅ 12 张新表（task_events, decisions, ai_runs, ai_context_items, audit_events, smart_summaries, daily_stats, memory_entries, provenance, reminder_jobs, ai_insights, command_routes）

### 5.2 数据库版本

- Schema 版本：9
- 迁移方式：增量迁移

---

## 六、文档清单

| 文档 | 说明 |
|------|------|
| `docs/architecture-v3.md` | v3.0 架构设计文档 |
| `docs/architecture-plugin-system.md` | 插件系统详细设计 |
| `docs/architecture.md` | v2.11 架构文档（历史） |
| `docs/casy-design-philosophy.md` | 设计哲学 |
| `docs/modules/` | 模块设计文档（16 个） |
| `docs/devlog/` | 开发日志 |

---

## 七、后续计划

### 7.1 短期（P2）

1. **实现技能系统** — 合同审查、诉讼分析等法律技能
2. **完善律师画像** — 首次使用引导
3. **实现 MCP 接口** — 对外暴露 Casy 的能力

### 7.2 中期（P3）

1. **AI 推荐引擎** — 今日任务推荐、优先级排序
2. **自动报表** — 日/周/月报表生成
3. **数据蒸馏** — L2 蒸馏 + 确认区

### 7.3 长期（P4）

1. **双向开放** — MCP Server + Skill Runner
2. **移动端伴侣** — 仅当移动办公需求成立时立项

---

## 八、验收标准

### 8.1 插件系统

- [x] 核心容器实现
- [x] 9 个业务插件注册
- [x] 38 个工具注册
- [x] 事件系统实现
- [x] 确认机制实现

### 8.2 AI 系统

- [x] 多模型提供商支持
- [x] AI 工具调用流程
- [x] AI 对话面板
- [x] 审计日志记录

### 8.3 文档

- [x] 架构设计文档
- [x] 插件系统文档
- [x] 开发日志记录
- [x] 项目状态文档
# Casy 项目状态

> **最后更新**: 2026-07-31  
> **说明**: 本文档合并了原 `PROGRESS.md`、`DEVELOPMENT-PLAN.md`、`IMPROVEMENT-PLAN.md`、`Casy-OPTIMIZATION.md` 的全部内容。

---

## 一、当前项目状态

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
| cargo check | ✅ 0 errors |
| npm run build | ✅ 成功 |

---

## 二、已完成阶段

### Phase 1-8：基础功能 ✅

| Phase | 内容 | 状态 |
|-------|------|------|
| 1 | 数据层 + 案件 CRUD | ✅ |
| 2 | 核心 UI（路由/Store/侧边栏/Dashboard/列表/详情/任务/日历/设置） | ✅ |
| 3 | 时间线（命令 + 集成 + 弹窗） | ✅ |
| 4 | 同步基础（WebDAV 客户端 + 状态查询 + 连接测试） | ✅ |
| 5 | 文件管理（文件夹自动创建 + 智能命名 + 自动分类） | ✅ |
| 6 | 文档解析（规则分类 + 传票/口审/判决解析） | ✅ |
| 7 | 知识库（CRUD + 版本追踪 + FTS5 + 法条字段） | ✅ |
| 8 | 收件箱（CRUD + 自动分类 + 案件匹配 + 节假日解析 + UI） | ✅ |

### Phase 9：修基础 + 关系网 ✅ (2026-07-31)

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 9.1 节假日数据修正 | ✅ | 端午/中秋/国庆/调休修正，11 个测试通过 |
| 9.2 期限规则种子数据 | ✅ | 区分判决/裁定上诉期，增加 verdict_type 条件 |
| 9.3 前端崩溃级 Bug | ✅ | InboxView typo, router history, style.css 残留 |
| 9.4 Schema 补全 | ✅ | 22+ CHECK, 11 索引, 3 触发器, FTS5, 4 张表, 迁移框架 |
| 9.5 飞书导入修复 | ✅ | 事务包裹 + 10 个专利无效字段 + judge_panel/clerk/attorneys |
| 9.6 日历/Bug 修复 | ✅ | calendar.rs 月末, case_stats, TimelineEvent files, 首页统计 |
| 9.7 关系网 | ✅ | add/get/remove_relation + 自动检测 + CaseRelatedPanel |

### Phase 10：文书工坊 ✅ (2026-07-31)

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 10.1 TipTap 编辑器 | ✅ | LegalEditor + 3 个 Suggestion 扩展 + 自动保存 |
| 10.2 Docsy 桥接 | ✅ | IPC bridge + mapCaseToTemplate(40+ 字段) + DOCX 导出 |
| 10.3 草稿管理 | ✅ | drafts 表 CRUD + 版本历史 |

### Phase 11：大口袋多入口 ✅ (2026-07-31)

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 11.1 系统托盘 | ✅ | TrayIconBuilder + 菜单 + 角标 |
| 11.2 全局热键 | ✅ | Cmd+Shift+V → 剪贴板到收件箱 |
| 11.3 文件夹监听 | ✅ | ~/Documents/Casy/inbox/ 新文件自动导入 |
| 11.4 UI 增强 | ✅ | 拖拽上传 + 补充 stores + 日历文字 + 快捷按钮绑定 |

### Phase 12：飞书双向同步 ✅ (2026-07-31)

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 12.1 Auth + Token | ✅ | FeishuAuth + keychain + 自动刷新 + 设置 UI |
| 12.2 Sync Map + Push/Pull | ✅ | sync_map + PULL + PUSH + 字段映射 |
| 12.3 限流 + 错误处理 | ✅ | RateLimiter(5 req/s) + 429 Retry-After |

### Phase 13：邮件 + AI ✅ (2026-07-31)

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 13.1 IMAP 监听 | ✅ | async-imap + IDLE + 29 分钟重连 + 白名单 |
| 13.2 AI 后端 | ✅ | AiBackend trait + Ollama/OpenAI/NoOp + TokenBudget + 设置 UI |

### Phase 14：打磨 + 安全 ✅ (2026-07-31)

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 14.1 SQLCipher 迁移 | ✅ | bundled-sqlcipher + OS keychain + 自动备份 |
| 14.2 统计面板 | ✅ | Dashboard + get_dashboard_stats |
| 14.3 导出 | ✅ | CSV 导出 + 筛选条件 |
| 14.4 组件重构 | ✅ | CaseFilterBar/CaseGroupPanel/CaseInfoPanel/CaseTimelinePanel |

### 改进计划 Phase A-D ✅ (2026-07-31)

| Phase | 内容 | 状态 |
|-------|------|------|
| A | AI 文档分类 + 收件箱路由 + 多条件筛选 + 信息提取 prompt | ✅ |
| B | 知识入库 + 混合检索(FTS5+Ollama embedding) + 风格标注 | ✅ |
| C | Copilot Sidebar + 检索结果展示 + AI 写作辅助(Ctrl+K) | ✅ |
| D | 字段引用组件 + 看板视图(拖拽) + UI 主题统一 | ✅ |

### 优化任务 P0-P3 ✅ (2026-07-31)

| 优先级 | 内容 | 状态 |
|--------|------|------|
| P0 | Settings Store/命令, WritingView, CaseFilesView, SyncStatusView, 文件夹创建, 物理删除, 字段补全, 行颜色 | ✅ |
| P1 | 编译 warnings 29→0, SettingsView 重构, Loading Skeleton, 全局错误处理, 时间线分组, 日历五色, 任务过期, AI 置信度, 节假日导入, 任务编辑 | ✅ |
| P2 | 庭审自动生成任务, WebDAV 同步核心/冲突处理, CaseNetworkView | ✅ |
| P3 | IMAP 监听, AI 后端, 每日重算, 飞书限流/PUSH, 知识库版本, SettingsView 拆分 | ✅ |

---

## 三、后端命令清单 (70 个 Tauri Commands)

### 案件管理 (10)
list_cases, get_case, create_case, update_case, delete_case, search_cases, case_stats, add_relation, get_relations, remove_relation

### 任务管理 (4)
list_tasks, create_task, toggle_task, delete_task, update_task, generate_hearing_prep_tasks

### 时间线 (3)
get_case_timeline, add_case_log, delete_case_log

### 日历 (1)
get_calendar_events

### 期限 (2)
get_deadline_warnings, get_dashboard_stats

### 收件箱 (6)
add_inbox_item, list_inbox_items, process_inbox_item, file_inbox_item, dismiss_inbox_item, parse_holiday_notice

### 知识库 (8)
list_knowledge, create_knowledge, update_knowledge, delete_knowledge, search_knowledge, knowledge_stats, hybrid_search_knowledge, embed_knowledge, embed_all_knowledge, create_knowledge_from_selection, link_knowledge_to_case, link_knowledge_to_law

### 文件管理 (4)
list_case_files, upload_case_file, delete_case_file, ensure_case_folder

### 同步 (6)
get_sync_status, test_webdav_connection, manual_sync_push, manual_sync_pull, resolve_keep_local, resolve_keep_remote

### 飞书 (4)
import_feishu_data, save_feishu_credentials, test_feishu_connection, sync_feishu_push, sync_feishu_pull

### AI (5)
configure_ai, test_ai, get_ai_config, generate_writing_suggestion, classify_document_with_prompt, extract_info_with_prompt

### 设置 (3)
get_settings, save_settings, import_holidays_json, get_holidays_summary

### 邮件 (4)
configure_imap, start_imap_watch, stop_imap_watch, imap_status

### 其他 (5)
export_cases, list_docsy_templates, render_docsy_template, save_draft, list_drafts

---

## 四、待做事项 / 剩余工作

### 未实现功能 (详见 docs/todo-features.md)

| # | 功能 | 优先级 | 预估工时 | 说明 |
|---|------|--------|---------|------|
| 1 | WebDAV SyncCoordinator 完整 startup_sync | P2 | 已部分实现 | VACUUM INTO + ETag 已有，需完善调度 |
| 2 | ConflictResolver 字段级选择 | P2 | 已部分实现 | 并排对比已有，逐字段选择待完善 |
| 3 | CaseNetworkView 递归深度 | P3 | 2h | 当前仅 2 层，设计支持 depth 参数 |
| 4 | DeadlinePanel 全局独立组件 | P3 | 1h | 已嵌入 HomeView，可提取为独立组件 |
| 5 | 每日期限重算定时器 | P3 | 1h | 启动时重算已有，00:01 定时器待实现 |
| 6 | 批量操作（任务） | P3 | 2h | 批量完成/删除 |
| 7 | 知识库版本差异对比 UI | P3 | 2h | knowledge_versions 表已有，对比 UI 待做 |
| 8 | SettingsView 子组件化 | P3 | 2h | 当前 630 行单文件，应拆分 |
| 9 | 自动保存最后时间显示 | P3 | 0.5h | 当前仅 toast |
| 10 | 自动更新（Tauri Updater） | P4 | 1天 | 需配置 endpoint + 签名密钥 |

### 技术债务

| # | 债务 | 位置 | 建议 |
|---|------|------|------|
| 1 | ConversionState 未使用 | `lib.rs:24-38` | 移除 |
| 2 | 每命令独立 open_db | 所有 commands | 考虑 tauri::State<DbPool> |
| 3 | store 路径不统一 | stores/ vs composables/ | 统一到 stores/ |

---

## 五、风险提示

| 风险 | 说明 | 缓解措施 |
|------|------|---------|
| SQLCipher 迁移 | 首次使用需确保 keyring 可用 | 已有 plaintext→encrypted 迁移逻辑 + .bak 备份 |
| Docsy 依赖 | 需确认 .docsytpl 模板路径 | — |
| TipTap IME | 中文输入法在 WebKit 有已知问题 | isComposing 守卫 |
| 2026 节假日 | 已交叉验证 | 正式使用前建议再次核实 |
| 飞书限流 | 已有 RateLimiter | 5 req/s + 429 自动重试 |

---

## 六、设计文档索引

| 文档 | 位置 | 说明 |
|------|------|------|
| 综合技术规格 | `Casy-SPEC.md` | 合并全部规格文档 |
| 本文档 | `Casy-STATUS.md` | 项目状态与进度 |
| 收件箱设计 | `docs/inbox-system-design.md` | 大口袋完整设计 |
| 待办功能 | `docs/todo-features.md` | 未实现功能清单 |
| 归档文档 | `docs/archive/` | 历史规格文档存档 |

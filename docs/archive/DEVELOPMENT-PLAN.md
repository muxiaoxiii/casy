# Casy 开发工作计划

> 最后更新：2026-07-31
> 基于全面审计后的修订版计划（Phase 12 飞书双向同步已完成）

## 审计发现汇总

### 🔴 严重问题
1. `InboxView.vue:170` — `fileedItems` 拼写错误，已归档 tab 必崩
2. `router/index.js:50` — `createWebHistory()` 在 Tauri 下刷新白屏，应为 `createWebHashHistory()`
3. `style.css` — Vite 模板残留 CSS 污染布局
4. `holidays.rs` — 2026 年节假日数据错误（端午/中秋/国庆/调休）
5. ~~飞书导入未开事务，中断后数据残缺~~ ✅ 已修复
6. `calendar.rs` 硬编码 31 号，月末查询有误
7. ~~飞书导入只覆盖 ~60% 字段，10 个专利无效专属期限字段完全未导入~~ ✅ 已修复

### 🟡 UI/UX 问题
8. 案件详情页 5 个快捷按钮没有绑定事件
9. 首页 3 个统计卡片功能重复
10. 日历只显示色点无文字
11. 自动保存每 2 秒弹 toast
12. 无 loading/error 骨架屏

### 🟡 架构缺陷
13. Schema 缺失 40+ CHECK 约束、10+ 索引、3 触发器、2 FTS5 表、4 张完整表
14. 前端只有 1 个 Pinia store
15. spec 定义了 14 个 Vue 组件，实际只有 7 个单体视图
16. WebDAV 同步只有 HTTP 原语，无 SyncCoordinator
17. 缺少迁移框架

---

## 开发计划

### Phase 9: 修基础 + 关系网（5 天）

#### 9.1 修复节假日数据（0.5 天）
- [x] 更新 `holidays.rs` 中 2026 年节假日：端午改为 6/19-21，增加中秋 9/25-27，国庆改为 10/1-7
- [x] 修正调休日：1/4, 2/14, 2/28, 5/9, 9/20, 10/10
- [x] 增加单元测试覆盖所有节假日边界（11 个测试全部通过）

#### 9.2 修复期限规则种子数据（0.5 天）
- [x] 核对 `schema.rs` 中 deadline_rules 种子数据与 `期限规则修正方案.md`
- [x] 区分判决上诉期 15 天 vs 裁定上诉期 10 天
- [x] 增加 verdict_type 条件规则（procedure_types 字段存储 JSON 对象）
- [x] 修正专利权人补充意见触发字段（petitioner_submit_date → petitioner_received_date）

#### 9.3 修复前端崩溃级 Bug（0.5 天）
- [x] InboxView.vue:170 `fileedItems` → `filedItems`
- [x] router/index.js `createWebHistory` → `createWebHashHistory`
- [x] 替换 style.css 为项目实际样式（移除 Vite 模板残留）
- [x] CaseListView.vue 导入 Loading 图标

#### 9.4 补全 Schema 缺失项（1 天）
- [x] 补齐 22+ CHECK 约束（cases×2, hearings, tasks×2, officials, inbox×3, case_files×3, knowledge_items, knowledge_relations, sync_map×2, sync_queue×3, drafts, email_records×2）
- [x] 补齐 11 个索引（idx_cases_progress, idx_logs_type, idx_tasks_priority, idx_tasks_completed, idx_officials_role, idx_inbox_category, idx_inbox_case, idx_files_category, idx_files_knowledge, idx_case_deadlines_source, idx_sync_status）
- [x] 补齐 3 个触发器（trg_clients_updated, trg_case_deadlines_updated, trg_drafts_updated）
- [x] 补齐 files_fts + knowledge_fts 全文搜索表及 6 个同步触发器
- [x] 补齐 4 张缺失表（email_records, skills, sync_queue, imap_accounts）
- [x] 增加 schema 迁移框架（CURRENT_SCHEMA_VERSION + MIGRATIONS 数组 + run_migrations 函数）

#### 9.5 修复飞书导入（0.5 天）
- [x] 包裹事务
- [x] 导入 10 个专利无效专属期限字段
- [x] 导入 judge_panel / clerk / attorneys
- [x] 修复 hearing 中 court/level 硬编码问题

#### 9.6 修复日历和其他 Bug（0.5 天）
- [x] calendar.rs 月末日期用 `last_day_of_month` 替代硬编码 31
- [x] case_stats 补充 upcoming_hearings / overdue_deadlines
- [x] TimelineEvent 补充 files 字段
- [x] 首页统计卡片接入正确的状态过滤

#### 9.7 Case Relations 关系网（1.5 天）
- [x] 增加 add_relation / get_relations / remove_relation 命令
- [x] 自动检测：同专利号 → same_patent，同客户 → same_party，审级关联 → appeal_of
- [x] CaseRelatedPanel 组件（案件详情右栏）
- [x] 点击关联案件可跳转

---

### Phase 10: 文书工坊（5 天）

#### 10.1 TipTap 编辑器（2 天）
- [x] 安装依赖 + LegalEditor 组件
- [x] 3 个 Suggestion 扩展：{field、【law、@party
- [x] 自动保存 + 2s debounce

#### 10.2 Docsy 桥接（2 天）
- [x] IPC bridge 调用 Docsy 模板引擎
- [x] 完整 mapCaseToTemplate（40+ 字段映射）
- [x] TemplateBrowser + DocumentGenView
- [x] DOCX 导出

#### 10.3 草稿管理（1 天）
- [x] drafts 表 CRUD 命令
- [x] 版本历史（version 字段自动递增）

---

### Phase 11: 大口袋多入口（3 天）

#### 11.1 系统托盘（1 天）
- [x] TrayIconBuilder + 菜单（添加文件/笔记/剪贴板/打开/退出）
- [x] 托盘角标显示待处理数量（事件通知前端）

#### 11.2 全局热键（0.5 天）
- [x] Cmd+Shift+V → 剪贴板到收件箱
- [x] global-shortcut 插件

#### 11.3 文件夹监听（0.5 天）
- [x] 监听 ~/Documents/Casy/inbox/ 新文件
- [x] 自动导入

#### 11.4 UI 增强（1 天）
- [x] 收件箱拖拽上传区
- [x] 补充缺失的前端 store（tasks, inbox, calendar）
- [x] 日历事件显示文字
- [x] CaseDetailView 快捷按钮绑定事件（添加日志/庭审/任务/生成文书/打开文件夹）

---

### Phase 12: 飞书双向同步（3 天）

#### 12.1 Auth + Token 管理（1 天）
- [x] FeishuAuth + keychain 存储（keyring crate，com.casy.feishu 服务）
- [x] tenant_access_token 自动刷新（提前 60 秒过期刷新）
- [x] 设置页凭证配置 UI（App ID/Secret 输入 + 测试连接按钮）

#### 12.2 Sync Map + Push/Pull（1.5 天）
- [x] sync_map 表（feishu_record_id ↔ local_id，已有 schema）
- [x] PULL: 分页拉取 → 时间戳对比 → INSERT/UPDATE + sync_map 映射
- [x] PUSH: local_newer 检测 → 字段转换 → POST/PUT + 限流

#### 12.3 限流 + 错误处理（0.5 天）
- [x] 令牌桶限流（RateLimiter，5 req/s）+ 429 Retry-After 自动重试

---

### Phase 13: 邮件 + AI（4 天）

#### 13.1 IMAP 监听（2 天）
- [x] async-imap + IDLE + 29 分钟重连
- [x] 白名单过滤
- [x] 自动解析 → 收件箱

#### 13.2 AI 后端（2 天）
- [x] AiBackend trait: classify/extract/summarize
- [x] OllamaBackend + OpenAiBackend + NoOpBackend
- [x] TokenBudget 日限额
- [x] 设置页 AI 配置
- [x] 接入收件箱处理 + 文档解析 fallback

---

### Phase 14: 打磨 + 安全（3 天）

#### 14.1 SQLCipher 迁移（1 天）
- [x] 切换 bundled-sqlcipher（Cargo.toml features）
- [x] OS keychain 读取密钥（keyring crate, com.casy.db 服务）
- [x] 加密现有 DB（migrate_to_encrypted + sqlcipher_export + 自动备份 .bak）

#### 14.2 统计面板（0.5 天）
- [x] HomeView 增强：活跃案件数、期限预警（红/黄/绿标记）、最近 7 天活动
- [x] 新增 get_dashboard_stats Tauri 命令返回聚合数据（DeadlineResult + RecentActivity）

#### 14.3 导出（0.5 天）
- [x] 新增 export_cases Tauri 命令（CSV 格式，导出到下载目录）
- [x] CaseListView 工具栏添加导出按钮（带当前筛选条件）

#### 14.4 组件重构（1 天）
- [x] 从 CaseListView.vue 提取 CaseFilterBar 组件（工具栏筛选 + 搜索 + 导出）
- [x] 从 CaseListView.vue 提取 CaseGroupPanel 组件（分组列表 + 表格）
- [x] 从 CaseDetailView.vue 提取 CaseInfoPanel 组件（案件信息表单 + 当事人 + 审理 + 专利 + 备注）
- [x] 从 CaseDetailView.vue 提取 CaseTimelinePanel 组件（时间线列表 + 添加/删除事件）

---

## 当前进度

| Phase | 状态 | 开始 | 完成 |
|-------|------|------|------|
| 9.1 节假日 | ✅ | 2026-07-31 | 2026-07-31 |
| 9.2 期限规则 | ✅ | 2026-07-31 | 2026-07-31 |
| 9.3 前端Bug | ✅ | 2026-07-31 | 2026-07-31 |
| 9.4 Schema | ✅ | 2026-07-31 | 2026-07-31 |
| 9.5 飞书导入 | ✅ | 2026-07-31 | 2026-07-31 |
| 9.6 日历/Bug | ✅ | 2026-07-31 | 2026-07-31 |
| 9.7 关系网 | ✅ | 2026-07-31 | 2026-07-31 |
| 10 文书工坊 | ✅ | 2026-07-31 | 2026-07-31 |
| 11 大口袋 | ✅ | 2026-07-31 | 2026-07-31 |
| 12 飞书同步 | ✅ | 2026-07-31 | 2026-07-31 |
| 13 邮件+AI | ✅ | 2026-07-31 | 2026-07-31 |
| 14 打磨 | ✅ | 2026-07-31 | 2026-07-31 |

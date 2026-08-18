# 模块 14 · 数据层

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `architecture.md` §5 / §8 / §10 / §12

---

## 一、职责边界

### 1.1 做什么

- 数据库初始化、加密（SQLCipher）、WAL、外键、busy_timeout。
- Schema 版本管理与增量迁移（`MIGRATIONS`，当前 v8）。
- 业务表 DDL 与索引（`schema.rs`）。
- 全文搜索（FTS5 虚拟表 + 同步触发器）。
- 数据访问封装（`db/cases.rs`、`db/search.rs`）。
- 数据库密钥管理（优先 OS keychain，回退本地密钥文件）。
- 审计/备份（加密迁移自动生成 `.bak` 明文备份）。

### 1.2 不做什么

- **不负责**具体业务逻辑（各模块自身的事务边界在各自 Rust 模块内）。
- **不负责**统一领域写入口（目标态，现状仍是命令直连 SQL）。

---

## 二、架构基线

| 项 | 值 | 说明 |
|---|---|---|
| 数据库入口 | `src-tauri/src/db/mod.rs` | 启动时 `init_db()` |
| Schema 版本 | 8 | 前滚迁移，不承诺自动降级回滚 |
| 加密 | SQLCipher | keychain 优先，失败回退本地密钥文件 |
| WAL | 开启 | 提升并发读 |
| busy_timeout | 5000ms | 写锁等待 |
| 外键 | 开启 | `PRAGMA foreign_keys = ON` |
| 迁移方式 | 启动时按 `MIGRATIONS` 顺序执行 | 每次迁移后 `PRAGMA user_version = v` |

## 三、表清单

### 3.1 业务表

| 域 | 表 |
|---|---|
| 案件 | `cases` / `clients` / `case_logs` / `hearings` / `case_relations` / `officials` / `case_officials` / `case_track_history` |
| 任务 | `tasks` / `task_templates` |
| 收件箱 | `inbox_items` / `inbox_recommendations` / `case_files` |
| 期限 | `deadline_rules` / `case_deadlines` |
| 知识 | `knowledge_items` / `knowledge_versions` / `knowledge_relations` / `knowledge_embeddings` |
| 文书 | `drafts` |
| 文件 | `file_naming_rules` / `case_folder_templates` |
| 同步 | `sync_map` / `sync_queue` / `feishu_*` |
| 邮件 | `imap_accounts` / `email_records` |
| 提醒 | `reminder_rules` / `reminder_log` |
| 系统 | `settings` / `skills` |

### 3.2 目标态新表（设计草案，未落库）

- `task_events` / `decisions` / `ai_runs` / `ai_context_items` / `audit_events`
- `smart_summaries` / `daily_stats` / `ai_insights` / `memory_entries` / `provenance`
- `reminder_jobs`（见 12）

> 以上均为设计草案，任何文档不得写成现状（architecture.md §3.3、§5.2 末尾注）。

---

## 四、关键约束

### 4.1 幂等性

- `sync_map` / `sync_queue` / 飞书导入路径需要稳定主键或唯一约束。
- 批处理重试只允许重复更新 AI 分类字段，不允许重复创建案件/任务/知识记录。
- 提醒同日去重是本地组合查询，**不是端到端幂等键**（M1 `reminder_jobs` 用独立幂等键）。

### 4.2 崩溃恢复

- 基于 `inbox_items.status` 粗粒度恢复（`processing_started_at` 超时 → 重置 `pending`）。
- 不支持恢复到精确任务句柄或进度百分比。
- 提醒 `deadline_before` 等严格相等触发项离线后无法补发（M0 缺陷，M1 日历作业解决）。

### 4.3 迁移与回滚

| 领域 | 当前可用回滚 |
|---|---|
| 数据库加密迁移 | 自动生成 `.bak` 明文备份 |
| WebDAV 冲突 | 保留本地 / 保留远程 |
| 收件箱批处理 | 单项重试 + 重新入队，不支持整批回滚 |
| 飞书同步 | `sync_map` / 日志人工排查 |
| 日历同步（M1） | 一键禁用退回 M0 |

**设计要求**：未来"重建表迁移"必须同时写数据迁移步骤、失败备份策略、用户可见症状。

---

## 五、演进方向（目标态）

1. **统一领域写入口（DomainCommand）**：所有写操作走领域事务，事件在事务内发出（architecture.md §6.3）。
2. **ContextPolicy / 事件审计表落库**：`task_events` / `ai_runs` / `audit_events` 三分离。
3. **IMAP 凭据迁移 keychain**：`password_enc` 从 base64 升级（architecture.md §10.2）。
4. **动态字段系统（长期）**：从固定列演进为"字段定义 + 单元格"（设计哲学 §9）。

---

## 六、验收标准

1. 数据库迁移前滚可验证（user_version 正确推进）。
2. 加密迁移失败有 `.bak` 兜底。
3. 外键/索引完整，无孤儿数据。
4. 新表必须先在本文档登记，再写 schema。

# 模块 10 · 同步（WebDAV + 飞书）

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `architecture.md` §7.4 / `14-data-layer.md`（密钥/存储）

---

## 一、职责边界

### 1.1 做什么

- WebDAV 数据库文件同步（push / pull / 启动同步 / 冲突解决）。
- 飞书多维表格同步（表发现 / 字段映射 / 导入 / 双向推送）。
- 飞书凭据与表配置管理（`feishu_connections` / `feishu_tables` / `feishu_fields` / `feishu_field_mappings` / `feishu_base_config`）。
- 同步状态查询与自动推送（`AutoPushManager`，数据同步 5 秒防抖）。

### 1.2 不做什么

- **不负责**提醒送达（见 12；飞书自动 PUSH 是数据同步，不是提醒）。
- **不负责**案件/任务等业务数据本身的正确性（同步只搬运）。
- **不负责**日历日程同步（M1 的 CalDAV 属于 12 提醒的送达通道，见 architecture.md §9）。

---

## 二、数据模型

| 表 | 用途 | 关键约束 |
|---|---|---|
| `sync_map` / `sync_queue` | 同步映射与同步队列 | WebDAV/飞书共用持久化辅助模型 |
| `feishu_connections` | 飞书应用连接 | 凭据走 keyring，表内不存明文 token |
| `feishu_tables` / `feishu_fields` | 表与字段结构发现 | `feishu_list_tables` / `feishu_list_fields` |
| `feishu_field_mappings` | Casy ↔ 飞书字段映射 | `feishu_save_mappings` / `feishu_get_mappings` |
| `feishu_base_config` / `feishu_field_meta` / `feishu_link_cache` | 配置与缓存 | — |
| `imap_accounts` | IMAP 配置（见 11） | 密码 `password_enc`（临时实现） |

---

## 三、命令接口

**WebDAV**：`get_sync_status` / `test_webdav_connection` / `webdav_startup_sync` / `webdav_push` / `webdav_pull` / `webdav_resolve_keep_local` / `webdav_resolve_keep_remote`

**飞书连接**：`configure_feishu` / `test_feishu_connection` / `get_feishu_sync_info` / `configure_feishu_table` / `set_feishu_auto_push` / `get_feishu_auto_push_status` / `trigger_feishu_push`

**飞书结构**：`feishu_list_tables` / `feishu_list_fields` / `feishu_list_records` / `feishu_compare_table` / `feishu_compare_records`

**飞书导入/同步**：`feishu_save_mappings` / `feishu_get_mappings` / `feishu_import_all` / `feishu_import_selected` / `feishu_import_incremental` / `feishu_sync_pull` / `feishu_sync_push` / `import_feishu_data`（JSON dump 导入）/ `import_feishu_dump`

---

## 四、关键流程

### 4.1 WebDAV 启动同步

```text
启动时
  → 读取数据库路径 + 已保存 ETag
  → sync::startup_sync（自动 PULL）
  → 成功回写 webdav_last_etag / webdav_last_sync_at
```

**约束**：冲突解决目前是"保留本地 / 保留远程"两档，不是字段级合并；同步对象是数据库文件（VACUUM INTO），不是增量多记录（architecture.md §7.4）。

### 4.2 飞书双向同步

```text
配置连接（configure_feishu）→ 表发现（list_tables）→ 字段映射（save_mappings）
  → import（全量/选中/增量） 或 sync_pull/sync_push
  → AutoPushManager 5 秒防抖自动推送案件数据到 Bitable
```

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 01 案件 | 案件数据推送/拉取 | 同步不改业务语义，只映射字段 |
| 06 期限 | 期限字段映射 | 同步不触发期限重算 |
| 12 提醒 | 飞书通道（数据同步 ≠ 提醒） | 提醒的飞书消息/任务通道是 12 的占位项 |
| 14 数据层 | 密钥存储 / `settings` | 凭据走 keyring，DB 只存配置 id |

---

## 六、演进方向（目标态）

1. **冲突解决字段级合并**：从"本地/远程两档"升级为逐字段选择合并（技术债务）。
2. **CalDAV 日历同步（M1）**：`reminder_jobs.executor=calendar`，提醒固化为日程事件同步到 Google/Apple/Outlook 日历（architecture.md §9，属于 12 的送达通道，但同步链路在本模块实现）。
3. **飞书消息通道接通**：`send_feishu_message` / `create_feishu_task` 从 dead_code 变为真实调用（见 12）。

---

## 七、验收标准

1. WebDAV 启动同步自动执行且不阻塞启动。
2. 飞书字段映射可保存、可复用、可回滚。
3. 凭据不落库（keyring）。
4. 同步状态可在 UI 清晰查看（SyncStatusView）。

# 模块 15 · 可观测性与设置

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `architecture.md` §11（可观测性）/ §10（资源预算）

---

## 一、职责边界

### 1.1 做什么

- 日志系统（`app_log.rs`）：按天轮转，默认保留 7 天。
- 日志查询命令（`get_log_dir` / `get_recent_logs` / `search_logs`）。
- 系统设置（`settings` 表 + `get_settings` / `save_settings`）。
- 节假日导入（`import_holidays_json` / `get_holidays_summary`）。
- 系统托盘（`tray.rs`）与全局快捷键。
- 前端事件推送（文件导入/托盘/复制流程的 Tauri event）。

### 1.2 不做什么

- **不负责**统一 metrics/trace/审计事件总线（现状缺口，目标态）。
- **不负责**业务模块的逻辑（只提供横切能力）。

---

## 二、数据模型

| 表 | 用途 | 关键约束 |
|---|---|---|
| `settings` | 键值配置 | 含 AI 配置、WebDAV 配置、外观等 |

---

## 三、命令接口

| 命令 | 说明 |
|---|---|
| `get_settings` / `save_settings` | 设置读写 |
| `import_holidays_json` / `get_holidays_summary` | 节假日导入与汇总 |
| `get_log_dir` / `get_recent_logs` / `search_logs` | 日志查询 |
| `list_folder_templates` / `get_folder_template` / `save_folder_template` / `delete_folder_template` | 目录模板（09） |
| `get_folder_naming_settings` / `save_folder_naming_settings` | 命名规则（09） |

---

## 四、当前缺口

| 缺口 | 说明 |
|---|---|
| 统一指标面板 | 无 metrics 聚合视图 |
| 统一审计事件模型 | 未覆盖 AI/同步/批处理的统一 audit 模型 |
| 批处理事件流 | 前端仍以轮询 `get_inbox_progress` 为主 |
| 日志格式统一 | 部分模块仍用旧 `log::` 宏（app_log 已存在） |

---

## 五、演进方向（目标态）

1. **统一事件/审计总线**：`audit_events`（append-only）+ 前端事件流。
2. **降级态设计**：AI 不可用时各功能显示"可用的替代"（骨架屏/规则版/提示条，设计哲学 §12.5）。
3. **全局 AI 状态徽标**：绿=可用 / 灰=关闭 / 琥珀=降级。
4. **资源预算可见**：AI 每日调用预算、批处理并发等可在设置页查看。

---

## 六、验收标准

1. 日志按天轮转且保留策略生效。
2. 设置读写幂等、可迁移。
3. 前端可查询日志定位问题。
4. AI 状态可在全局徽标体现。

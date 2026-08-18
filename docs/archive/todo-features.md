# Casy 待办功能清单

> **日期**: 2026-07-31  
> **说明**: 本文档列出在综合技术规格中已设计但尚未完整实现的功能。

---

## 一、部分实现（需完善）

### 1.1 WebDAV SyncCoordinator 完整调度 [P2]

**设计规格**: `Casy-SPEC.md` §六 同步引擎  
**当前状态**: WebDavClient + VACUUM INTO + ETag + 手动同步已实现  
**缺失**: 
- 完整的 `startup_sync` 自动调度（应用启动时检测远程变化 → 自动 PULL）
- `schedule_push` 在应用关闭时自动触发

### 1.2 ConflictResolver 字段级选择 [P2]

**设计规格**: `Casy-补充规格.md` §1.11  
**当前状态**: SyncStatusView 已有并排对比 + 本地/远程选择  
**缺失**:
- 逐字段单选（local/remote）合并
- "应用合并"按钮的实际合并逻辑

### 1.3 案件详情三栏布局 [P3]

**设计规格**: `Casy-实现规格-核心模块.md` §2.1  
**当前状态**: CaseInfoPanel + CaseTimelinePanel 已有  
**缺失**:
- CaseRelatedPanel 作为独立右栏组件（当前期限/关联信息嵌入在其他面板中）

---

## 二、完全未实现

### 2.1 CaseNetworkView 递归深度 [P3]

**设计规格**: `Casy-补充规格.md` §1.6  
**预估**: 2h  
**说明**: 当前 CaseNetworkView 仅展示 2 层关系，设计支持 `depth` 参数递归展示。

### 2.2 DeadlinePanel 全局独立组件 [P3]

**设计规格**: `Casy-补充规格.md` §1.7  
**预估**: 1h  
**说明**: 期限预警当前嵌入 HomeView Dashboard，设计要求为可复用的全局组件。

### 2.3 每日期限重算定时器 [P3]

**设计规格**: `Casy-实现规格-同步与公式.md` §3.2  
**预估**: 1h  
**说明**: 设计要求"启动时 + 每天 00:01"重算。启动时已有，00:01 tokio 定时器未实现。

### 2.4 任务批量操作 [P3]

**设计规格**: `Casy-实现规格-核心模块.md` §3.3  
**预估**: 2h  
**说明**: 批量完成/删除任务的 UI 和后端命令。

### 2.5 知识库版本差异对比 UI [P3]

**设计规格**: `Casy-补充规格.md`  
**预估**: 2h  
**说明**: `knowledge_versions` 表已存在，但无版本差异对比的 UI 界面。

### 2.6 SettingsView 子组件化 [P3]

**当前状态**: SettingsView.vue 630 行单文件  
**预估**: 2h  
**说明**: 应拆分为 5 个子组件对应 5 个标签页（飞书/WebDAV/AI/IMAP/通用）。

### 2.7 自动保存最后时间显示 [P3]

**设计规格**: `Casy-补充规格.md` §1.14  
**预估**: 0.5h  
**说明**: WritingView 底部状态栏应显示"最后保存: HH:MM"，当前仅有 toast。

### 2.8 TipTap 字数统计 + 导出 Word 状态栏 [P3]

**设计规格**: `Casy-补充规格.md` §1.14  
**预估**: 1h  
**说明**: WritingView 底部状态栏应显示字数统计和导出 Word 按钮。

### 2.9 自动更新 (Tauri Updater) [P4]

**设计规格**: `Casy-完整规划文档.md` §八  
**预估**: 1 天  
**说明**: 需配置 endpoint + 签名密钥 + CI 集成。

---

## 三、技术债务

| # | 债务 | 位置 | 建议 |
|---|------|------|------|
| 1 | ConversionState 未使用 | `lib.rs:24-38` | 移除，疑似 Docsy 残留 |
| 2 | 每命令独立 open_db | 所有 commands/*.rs | 改为 tauri::State<DbPool> 全局连接池 |
| 3 | store 路径不统一 | `src/stores/` vs 设计的 `composables/` | 已统一到 stores/，清理 composables 引用 |
| 4 | 缺少批量操作 | tasks | 批量完成/删除 |
| 5 | insert_case 字段覆盖 | `db/cases.rs` | 确保 Case 全字段已覆盖 |

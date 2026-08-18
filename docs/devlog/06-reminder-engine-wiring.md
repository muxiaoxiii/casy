# 06-提醒引擎接通 + 编译修复

> **日期**: 2026-08-18  
> **状态**: 已完成  
> **影响范围**: 后端 `reminder.rs` / `lib.rs` / `areas.rs` / `tasks.rs`；前端 `ReminderSettings.vue` / `ReminderToast.vue` / `SettingsView.vue` / `App.vue` / `types/index.ts`

---

## 一、任务背景

基于设计哲学文档、架构文档与模块设计文档（16 个模块）继续完成系统实现。第一步：**检查项目现状并修复编译问题**，识别出：

1. **后端编译失败**（5 个 error）：areas.rs / tasks.rs 在 `run_blocking` 闭包（返回 `anyhow::Result`）内使用了 `ok_or("...")?`（`&str` 错误）与 `return Err(String)`，`?` 无法将 `&str`/`String` 转为 `anyhow::Error`
2. **前端类型错误**（23 个）：`src/types/index.ts` 的 `Task` 接口仍是旧版（taskName/deadline/linkedCaseId/completedAt），未同步 M0 实施的 GTD 新字段（taskType/startBucket/blocked/caseId 等）
3. **前端构建失败**：`@element-plus/icons-vue` 中不存在 `Inbox` 图标（3 处引用）
4. **模块缺口**：架构文档 4.1 标注「提醒引擎调度未接通」——`start_reminder_engine` 已注册命令但未在启动时拉起；local 通道仅打日志、飞书通道为占位

---

## 二、修复内容

### 2.1 后端编译修复

| 文件 | 修复 |
|------|------|
| `areas.rs:51/145` | `.map_err(\|e\| e.to_string())?` → `.map_err(\|e\| anyhow::anyhow!(e))?`（String 无法转 anyhow::Error） |
| `areas.rs:70` | `ok_or("Missing area name")?` → `ok_or_else(\|\| anyhow::anyhow!("Missing area name"))?` |
| `areas.rs:126` | `return Err(format!(...))` → `return Err(anyhow::anyhow!(...))` |
| `tasks.rs:203` | `ok_or("Missing task id")?` → `ok_or_else(\|\| anyhow::anyhow!("Missing task id"))?` |

> 根因：`run_blocking` 闭包签名 `FnOnce() -> anyhow::Result<T>`，闭包内 `?` 的 From 转换只支持 StdError。`&str`/`String` 需显式包 `anyhow::anyhow!`。

### 2.2 前端类型修复

`src/types/index.ts` 重写 `Task` 接口，对齐后端 `tasks.rs` 返回的 JSON 字段（31 个字段）：

- 基础：`taskName` / `description` / `createdDate` / `deadline` / `priority` / `completed`(0/1) / `assignee` / `finishNote`
- GTD：`taskType` / `startDate` / `dueDate` / `waitingFor` / `followUpDate` / `context` / `flagged` / `sequential` / `blocked` / `sequenceOrder` / `startBucket` / `todayIndex` / `estimatedMinutes` / `actualMinutes` / `isOverdue` / `dueSoon` / `lastReviewDate` / `nextReviewDate` / `areaId` / `knowledgeId` / `caseId`

`src/stores/tasks.ts` 的 `GTDTask extends Task` 与 Task 已有字段冲突，改为 `export type GTDTask = Task`（字段统一在 types 定义）。

### 2.3 图标修复

`Inbox` 图标在 `@element-plus/icons-vue@2.x` 中不存在（已移除），3 处（App.vue / HomeView.vue / TasksView.vue）替换为 `Box` 图标。

### 2.4 提醒引擎接通（核心）

**后端**（`src-tauri/src/commands/reminder.rs`）：
- `send_local_notification`：从"仅打日志"改为 → **emit 前端事件 `reminder:triggered`**（含 message + 时间戳）+ 调系统通知
- `send_system_notification`：macOS 用 osascript 发系统通知（原有）
- `dispatch_reminder` 飞书通道：从占位改为**真正异步发送**——`feishu_message` 调 `send_feishu_reminder_async_generic`（飞书卡片消息），`feishu_task` 调 `send_feishu_task_async_generic`（创建飞书任务），均通过 `tauri::async_runtime::spawn` 异步执行不阻塞引擎循环
- 新增 `send_feishu_reminder_async_generic` / `send_feishu_task_async_generic`：从 settings 读取 `feishu_reminder_receive_id`，未配置则警告跳过
- `start_reminder_engine`：加 `OnceLock<Arc<AtomicBool>>` **防重复启动**

**启动接线**（`src-tauri/src/lib.rs`）：setup 中 spawn 调用 `start_reminder_engine(Some(300))`，每 5 分钟检查期限/开庭/任务规则

**前端**：
- 新增 `src/modules/settings/components/ReminderSettings.vue`：提醒规则管理（列表 + 新建/编辑弹窗 + 测试 + 删除 + 引擎状态徽标 + 最近 20 条触发日志）
- 新增 `src/shared/components/ReminderToast.vue`：全局提醒浮层，`listen('reminder:triggered')` 监听事件，队列化展示 + 稍后提醒/知道了
- `SettingsView.vue` 增加「提醒」Tab；`App.vue` 挂载 ReminderToast

---

## 三、验证结果

| 检查项 | 结果 |
|--------|------|
| `cargo check` | ✅ 通过（0 error，4 warning 均为既有 dead_code） |
| `cargo test` | ✅ 63/63 通过（含 reminder 2 个测试） |
| `vue-tsc --noEmit` | ✅ 0 error |
| `vite build` | ✅ 构建成功（1711 modules） |

---

## 四、设计原则遵循

- **原则六：主动智伴** — 提醒不是"到点弹一下"，而是按规则（期限前 7/3 天、当天、开庭前、任务到期）在正确时机提醒正确的事；多通道（本地/系统/飞书）分发
- **原则八：双向开放** — 飞书通道真正打通（消息 + 任务）
- **12.5 降级态** — 飞书未配置 receive_id 时警告跳过，不影响本地通道
- **幂等性**（架构 §8.2）— 引擎防重复启动，重复调用直接返回

---

## 五、已知限制与后续

1. **飞书接收人配置**：需在设置中配置 `feishu_reminder_receive_id`（当前无 UI，可后续在 FeishuSettings 增加字段）
2. **离线准时提醒**（架构第九章）：本地引擎仅"应用在线 + 重启补偿"尽力而为；M1 需接日历同步（CalDAV + ICS via Email）实现离线准时——**未实现，属 P3**
3. **提醒事件流推送**（架构 4.2）：当前前端通过事件监听获取，批处理场景尚未实现服务端事件流
4. 前端 `ReminderToast` 的"稍后提醒"为本地占位（无 snooze 持久化）

# 05 · 外部服务集成审计（飞书 / WebDAV / CalDAV / IMAP-SMTP / MCP / AI 网关）

审计日期：2026-09-30
范围：`src-tauri/src/sync/**`、`src-tauri/src/commands/{sync,caldav,import_feishu,feishu_snapshot,portable_backup}.rs`、`src-tauri/src/email/**`、`src-tauri/src/ai/**`、`src-tauri/src/mcp/**`、`src-tauri/src/credentials/mod.rs`、前端 `src/modules/sync/**` 与设置页外部服务组件。

---

## 概述

本轮的核心结论与 Codex 的"没有问题"相反：**六条外部服务链路里，AI 网关是唯一一条端到端可用的；WebDAV 完整备份可用但同步能力被高估；飞书的自动推送与同步中心入口是彻底不可达的剧场代码；CalDAV 与 IMAP/SMTP 邮件是"能编译、有界面、从未跑过真实服务器"的半成品。**

**用户最想知道的问题（"哪些数据链路没在工作"）有一个确定答案：飞书的 App Token 和 Table ID 从来没有被保存过。** 详见 S-0 —— 这不是推测，是三条独立代码路径共同验证的结果。

文档反复出现的"需显式配置和真实服务验收"和"无自动多主合并"并不是免责声明，而是对以下事实的准确描述：

0. **飞书的 `feishu_app_token` / `feishu_table_id` 永远为 NULL。** 唯一的后端写入命令 `configure_feishu_table` 在前端完全没有调用点，导致同步中心的飞书拉取/推送按钮、自动推送 watcher 两条链路**永久不可达**，且失败完全静默。见 S-0。
1. **飞书 pull 路径有一个确定性的类型错误，导致本地修改 100% 被远程覆盖。** 不是"可能丢数据"，是"每次拉取必丢"。见 S-1。
2. **失败重试计数被硬编码为 0，`sync_status` 永远不会变成 `push_failed`。** 重试机制是剧场代码。见 S-2。
3. **飞书导入在事务内吞掉 `hearings` / `clients` 的插入失败，却仍然上报"导入成功"。** 见 S-3。
4. **WebDAV 的冲突检测原语 `put_if_match` 从未被调用。** 用户点"保留本地"就是无条件覆盖远程，没有并发保护。见 S-4。
5. **MCP 的写操作确认门禁被一个已注册的 IPC 命令绕过。** 见 S-5。
6. **SMTP 客户端生成的邮件没有 `Date:` 和 `Message-ID:` 头；ICS 没有时区标识、没有 75 字节折行、`DTSTAMP` 格式非法。** 真实邮件服务器上大概率进垃圾箱或被拒。见 S-6。
7. **邮件监听不会自动启动，且"启动"不验证任何连接。** 见 S-7。

同时必须说清楚的是，凭据处理这一层**做得比预期好**：没有任何密钥被写入日志、stdout 或返回给前端；WebDAV/飞书上传路径确实不上传 `.key`；AI 错误消息刻意不回显响应体（避免服务商把 `Authorization` 头回显）。这部分不需要重写，需要修的是四条例外路径（§4）。

---

## 可用度评估表

| 集成 | 代码量（Rust 行） | 有无测试 | 是否需真实服务 | 实际可用度评估 |
| --- | --- | --- | --- | --- |
| **飞书 · 凭据与鉴权** | `feishu.rs` 2286（其中约 200 行鉴权/限流） | 鉴权与限流**无单测** | 是（自建应用 + 多维表格权限） | **半成品** — token 刷新、令牌桶、429 处理都写对了，但密钥同时落钥匙串与 DB 明文 |
| **飞书 · 案件双向同步** | `sync_feishu_pull_inner/push_inner` + `sync_table_pull/push` 约 1100 | **无**（`pull_one_record` / `get_push_items` 零覆盖） | 是 | **剧场代码** — 入口因 S-0 不可达；且 pull 必丢本地修改、失败计数恒为 0、冲突无检测 |
| **飞书 · 表结构发现与导入** | `import_feishu.rs` 1281 + `feishu_snapshot.rs` 984 | 有：`feishu_import_test`(3)、`feishu_snapshot_test`(5) —— 都是纯函数/本地 DB | 是 | **半成品** — 这是飞书唯一真正能用的路径（token 作为参数传入，不依赖持久化），但事务内吞错后仍报成功（S-3）、分页 cap 静默截断、字段名硬编码 |
| **飞书 · 自动推送 watcher** | `AutoPushManager` + watcher 约 145 | **无** | 是 | **剧场代码** — 后端防抖逻辑写对了，但 app_token 永为 NULL（S-0）+ `notify_change` 三重失效，**永久静默失败** |
| **飞书 · 消息/任务提醒** | `send_feishu_message` / `create_feishu_task` 约 115 | **无** | 是（需额外开通机器人权限） | **半成品** — `#[allow(dead_code)]`，由 `reminder.rs` 调用；不依赖 app_token，是飞书侧唯一可能真跑通的推送通道 |
| **WebDAV · 加密快照同步** | `sync/mod.rs` 247 + `webdav.rs` 345 | 有：`production_validation_test`(1)、`webdav_full_backup_test`(1)、`webdav.rs` 内联 2 | 是（需 PUT/GET/MOVE/HEAD） | **半成品** — 不上传密钥 ✅，恢复走维护锁 ✅，但冲突检测未接线、无重试、允许 http 明文 Basic Auth |
| **WebDAV · 完整加密备份** | `portable_backup.rs` 932 + `commands/sync.rs` 的 2 个命令 | 有：`webdav_full_backup_test`、`delivery_lifecycle_test` | 是 | **真实** — 六条链路里工程最扎实的一条（age 加密、临时名+MOVE、哈希校验、恢复前保护副本），前端也有 3 个测试 |
| **CalDAV** | `sync/caldav.rs` 431 + `commands/caldav.rs` 409 = 840 | 很少：内联 3（ICS 生成、错误分类）+ `calendar_delivery_test`(1) | 是（Google/iCloud/Outlook 各家实现不同） | **半成品偏剧场** — 单向 PUT/DELETE，无回读、无 sync-token、无重试；ICS 不带时区且不折行；`calendar_sync_enabled` 默认关 |
| **IMAP 监听** | `email/mod.rs` 762 | **零** | 是 | **剧场代码** — 不自动启动、启动不验证连接、白名单邮件导致无限重扫、写入无事务、keychain 失败降级 base64 |
| **SMTP 发送** | `email/smtp.rs` 601 | **零** | 是 | **剧场代码** — MIME 缺 `Date:`/`Message-ID:`；`use_tls`/`from_address` 是死配置；无重试无队列 |
| **MCP 本地服务** | `mcp/mod.rs` 1131 + `server.rs` 429 = 1560 | 有：`server.rs` 内联 7（鉴权 401、写工具 pending、431/413/405） | 否（本地回环），但**需真实 MCP 客户端验收** | **真实但有洞** — 绑定 127.0.0.1 ✅、Bearer 鉴权 ✅、写操作确认队列 ✅；但确认门禁被 IPC 命令绕过（S-5），且默认开启 |
| **AI 网关** | `ai/mod.rs` 1278 + `gateway.rs` 414 + `profiles.rs` 322 + `embeddings.rs` 308 + `usage.rs` 93 + `retrieval.rs` 337 = 2752 | 有：`ai_profiles_test`(5，含真实 TCP mock)、`ai_gateway_test`(4)、`local_retrieval_test`(1) | 是 | **真实** — 唯一有端到端网络断言、唯一有前端测试（`aiSettings.test.ts` 4 项）的链路。缺陷是无重试/无 429、`system_prompt` 只在 1/8 调用点生效、NoOp 后端报"成功"（S-12） |
| **凭据层**（横切） | `credentials/mod.rs` 299 | **零** | 否（可用 keychain 手动验证） | **真实** — `keychain:v1:<uuid>` 引用 + 写后读回校验；六条例外见 §4 |

**一句话结论**：**真实可用 = AI 网关、WebDAV 完整备份、MCP（带一个洞）；半成品 = 飞书凭据/导入、CalDAV；剧场代码 = 飞书案件同步、飞书自动推送、IMAP、SMTP。**

---

## 严重问题

### S-0 【致命 · CONFIRMED】飞书 App Token / Table ID 从未被持久化，自动推送与同步中心入口永久不可达

这是本轮最重要的发现，直接回答用户"哪些数据链路没在工作"。

**唯一的后端写入点**（`src-tauri/src/commands/sync.rs:263-272`）：
```rust
#[tauri::command]
pub async fn configure_feishu_table(app_token: String, table_id: String) -> Result<String, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        sync::feishu::update_sync_metadata(&conn, "feishu_app_token", &app_token)?;
        sync::feishu::update_sync_metadata(&conn, "feishu_table_id", &table_id)?;
        Ok("飞书表格配置已保存".to_string())
    }).await
}
```

**核实结果**：`grep -rn 'configure_feishu_table' src/` 与 `grep -n 'configure_feishu_table' src/types/commandMap.ts` **双双返回空**。这个命令在 Rust handler 里注册了（`commands/mod.rs:308`），但**前端既没有调用点，也没有 CommandMap 契约声明**。

`FeishuSettings.vue` 里的 App Token 与 Table ID 是纯本地 ref：
- `src/modules/settings/components/FeishuSettings.vue:36` — `const feishuAppToken = ref('')`
- `src/modules/settings/components/FeishuSettings.vue:54` — `const selectedTableId = ref('')`
- `:507` — `<el-input v-model="feishuAppToken" placeholder="多维表格的 App Token（URL 中获取）" />`

页面关闭即丢失。`:337-341` 那段看着像保存，实际只是把 `selectedTableId` 塞进字段映射的 payload 里（`feishuTableId: selectedTableId.value`），不是持久化。

**因此 `settings.feishu_app_token` 与 `settings.feishu_table_id` 永远是 NULL。** 三条链路因此不可达：

**(1) 同步中心的飞书拉取/推送按钮 —— 渲染为可用，点击永远失败**

`src/modules/sync/views/SyncStatusView.vue:224` / `:245`
```js
if (!feishuSyncInfo.value.appToken || !feishuSyncInfo.value.tableId) {
    ElMessage.warning('请先在设置中配置飞书表格')   // 永久走到这里
    return
}
```

而按钮的禁用条件用的是另一个字段（`SyncStatusView.vue:382` / `:390`）：
```html
:disabled="!feishuSyncInfo.configured"
```
`configured` 来自 `feishu_configuration()`（`sync/feishu.rs:1481-1489`），**只检查 app_id + app_secret，不检查 app_token / table_id**。徽章（`:352-355`）也因此显示"已配置"。

**可观察的错误结果**：用户配好凭据 → 同步中心显示飞书"已配置"、按钮可点 → 点下去弹"请先在设置中配置飞书表格" → 但设置页里那个输入框填了没用、没保存提示、也没有任何"保存表格配置"的按钮。**这是一个没有出口的死胡同。**

**(2) 后端自动推送 watcher —— 每次触发都 bail**

`src-tauri/src/sync/feishu.rs:2153-2161`
```rust
async fn execute_auto_push() -> Result<FeishuSyncReport> {
    let conn = crate::db::open_db()?;
    let app_token = get_sync_metadata(&conn, "feishu_app_token")?.unwrap_or_default();
    let table_id = get_sync_metadata(&conn, "feishu_table_id")?.unwrap_or_default();
    if app_token.is_empty() || table_id.is_empty() {
        anyhow::bail!("飞书表格未配置");
    }
    ...
}
```

`sync/feishu.rs:2144-2146` 把这个错误降级成一行 `log::warn!("飞书自动推送失败: {}", e)`。

**(3) 案件每次写入都触发一次注定失败的推送，且失败完全不可见**

`src/core/autoPush.ts:55-63`
```js
export async function notifyDataChange(): Promise<void> {
  try {
    await tauriCallSafe('trigger_feishu_push')
  } catch (e) {
    console.debug('飞书自动推送通知失败:', e)
  }
}
```

这段代码有两处独立的失效：

- `tauriCallSafe` **从不抛异常** —— `src/core/tauriBridge.ts:71-76` 返回 `{ ok: false, error }`。所以 `catch` 是死代码，返回值从未被检查。`trigger_feishu_push` 失败时函数静默返回，调用方（`src/stores/cases.ts:165, 179, 189`，即新建/更新案件的三个入口）照常继续。
- 即使它抛了，`console.debug` 在用户侧不可见。

再叠加 S-11（`trigger_feishu_push` 在未启用时直接 return 但仍回"已触发"）与 S-1（`notify_change` 第一件事就是检查 `enabled`），**案件写入 → 自动推送这条链路上有三重失效，用户侧的观感是：自动推送开关打开着、设置显示已配置、案件一直在改，但飞书里什么都不会变，且没有任何提示。**

`src/core/ai/toolAudit.ts:35-37` 有完全相同的模式（`.catch` 在 `tauriCallSafe` 上永不触发，审计写入失败被丢弃），根因是代码作者以为 `tauriCallSafe` 会 reject。

**修法**（约 30 行）：
1. `commandMap.ts` 增加 `configure_feishu_table` 契约，`services/sync.ts` 增加 `configureFeishuTable(appToken, tableId)`。
2. `FeishuSettings.vue` 在选表（`:198` `selectedTableId.value = tableId`）后立即调用它，或在 pull/push 按钮前调用。
3. `SyncStatusView.vue` 的按钮禁用条件改为 `!configured || !appToken || !tableId`。
4. `autoPush.ts` 改为 `const r = await tauriCallSafe(...); if (!r.ok) console.warn(...)`，或让 `tauriCallSafe` 在调用方需要时提供 throw 变体。

---

### S-1 【致命 · CONFIRMED】飞书案件 pull 每次都覆盖本地修改，本地编辑 100% 丢失

`src-tauri/src/sync/feishu.rs:976-986`

```rust
let new_remote_updated = item["last_modified_time"]
    .as_str()                      // ← 飞书返回的是 i64 毫秒时间戳，as_str() 恒为 None
    .or_else(|| item["created_time"].as_str())
    .unwrap_or("")
    .to_string();

if old_remote_updated.as_deref() == Some(new_remote_updated.as_str())
    && !new_remote_updated.is_empty()      // ← 恒为 false
{
    return Ok("skipped".to_string());
}
```

飞书 Bitable 记录的 `last_modified_time` 是 JSON 数字。`.as_str()` 对数字恒返回 `None`，于是 `new_remote_updated` **恒为空字符串**，`&& !new_remote_updated.is_empty()` 恒为 `false`，`skipped` 分支**永远进不去**。每一条已映射记录都会走 `update_local_case()`，无条件用飞书的值覆盖本地。

同一文件 100 行之外的通用 pull 写的是对的，可作反证：

`src-tauri/src/sync/feishu.rs:1605-1607`
```rust
let last_modified = item["last_modified_time"]
    .as_i64()                      // ← 正确
    .map(|t| t.to_string())
    .unwrap_or_default();
```

**触发路径**（无需任何异常条件，正常使用即触发）：
1. 用户在飞书多维表格与 Casy 之间建立案件映射，执行一次 pull，记录 `sync_map.remote_updated = ""`。
2. 律师在 Casy 里修改案件的案号、对方代理人、承办律师、备注，保存。
3. 任何人（同事、或自动拉取流程）再执行一次 pull。
4. 第 2 步的全部修改被飞书旧值覆盖，`report.updated` 显示 `updated: N`，**UI 显示同步成功**。

**可观察的错误结果**：案件详情的修改无声消失；`FeishuSyncReport.skipped` 恒为 0（可作为快速自检）；`sync_map.remote_updated` 列全部为空串。

**同时暴露的设计缺陷**：即使修好类型，两个方向的冲突策略都是"远端永远赢"。`sync_table_pull`（`feishu.rs:1630-1680`）在更新已有映射时**不检查 `sync_status = 'local_newer'`**，也不比较 `local_updated` 与 `remote_updated`。本地的未推送修改没有任何保护。

---

### S-2 【高 · CONFIRMED】失败重试计数被硬编码为 0，`push_failed` 状态永不出现

`src-tauri/src/sync/feishu.rs:1315-1325`

```rust
let mut stmt = conn.prepare(
    "SELECT id, local_id, remote_id, 0 FROM sync_map        // ← attempts 恒为字面量 0
     WHERE remote_source = 'feishu' AND sync_status = 'local_newer'
     UNION ALL
     SELECT sm.id, sm.local_id, sm.remote_id, 0             // ← 同上
     FROM sync_map sm JOIN cases c ON c.id = sm.local_id
     WHERE ...")?;
```

`PushItem.attempts` 字段（`feishu.rs:1311`）被声明并在失败分支里使用，但查询从不读它，永远给 `0`。同一硬编码重复出现在 `sync_table_push`（`feishu.rs:1861` `attempts: 0`）。

于是失败分支：

`src-tauri/src/sync/feishu.rs:1230-1239`
```rust
let attempts = item.attempts + 1;      // 永远 = 1
let status = if attempts >= 3 { "push_failed" } else { "local_newer" };
```

`1 >= 3` 恒为假 → **永远写回 `local_newer`**。

**后果**：一条因为字段类型不匹配（飞书字段是 `SingleSelect` 但本地存了它不接受的值）而永远推不上去的记录，会在**每一次**同步、每一次自动推送防抖中被重新 PUT 一次，永久重试、无退避、无告警、无死信。同时 `push_failed` 这个状态在 `get_push_items`（`:1318`）里被查询，永远查不到任何数据 —— 用户在看板上永远看不到失败队列。

---

### S-3 【高 · CONFIRMED】飞书导入吞掉开庭记录与委托人插入失败，仍上报"导入成功"

`src-tauri/src/commands/import_feishu.rs:839` 与 `:857`

```rust
Ok(_) => {
    for (h_idx, h_date) in raw_trial_dates_collected.iter().enumerate() {
        let h_id = db::new_id();
        let h_name = format!("第 {} 次开庭/口审", h_idx + 1);
        let judges_json = extracted.get("judgePanel").and_then(|s| clean_array_to_json(s));
        let _ = tx.execute(                                       // ← 结果被丢弃
            "INSERT INTO hearings (...) VALUES (?1, ?2, ...)",
            rusqlite::params![ ... ],
        );
    }
    if !final_client_name.is_empty() && final_client_name != "待补充委托人" {
        let _ = tx.execute(                                       // ← 结果被丢弃
            "INSERT OR IGNORE INTO clients (...) VALUES (?1, ?2, ...)",
            rusqlite::params![db::new_id(), final_client_name],
        );
    }

    report.created_count += 1;                                    // ← 仍然 +1
    report.imported_case_ids.push(new_case_id);                   // ← 仍然记为成功
}
```

`hearings` 与 `clients` 的 INSERT 失败（外键约束、`judges_json` 绑定类型不匹配、`datetime()` 参数个数错误等）被 `let _ =` 完全吞掉，随后 `report.created_count` 照常自增。`:723` 处对已有案件的同一分支也是同样写法。

**触发**：任何一条 `hearings` 写入失败的记录。字段过多或 `judges_json` 为 `None` 时最容易触发。

**可观察的错误结果**：`ImportResult` 报 `created: 120`，UI 显示导入成功，但 120 个案件的**开庭时间全部丢失** —— 对一个以法定期限为核心的产品，这是最坏的一类静默数据丢失：期限引擎算不出开庭提醒，用户也不会收到任何提示。

同样的模式在 `:1264`（`import_feishu_dump`）重演：cases 插入被吞，但 `report.cases += 1` 照常执行。

---

### S-4 【高 · CONFIRMED】WebDAV 冲突检测原语从未接线，"解决冲突"就是无条件覆盖

`src-tauri/src/sync/webdav.rs:225` 定义了带 `If-Match` 的条件 PUT：

```rust
/// 带 If-Match 条件的 PUT（冲突检测）
pub async fn put_if_match(&self, path: &str, data: &[u8], etag: &str) -> Result<String> {
    ... .header("If-Match", etag) ...
    if resp.status() == 412 { anyhow::bail!("ETag conflict: remote file has been modified"); }
```

全仓库 grep 结果：**唯一命中就是它自己的定义处**，零调用方。

实际使用的路径（`sync/mod.rs:170-171`）是无条件 PUT + `Overwrite: T` 的 MOVE：

```rust
client.put(&remote_temp, &data).await?;
client.move_resource(&remote_temp, "casy.db").await?;
```

`resolve_keep_local` / `resolve_keep_remote`（`sync/mod.rs:229`、`:240`）只是 `manual_sync_push` / `manual_sync_pull` 的别名，没有在覆盖前后做任何 ETag 比对。

**后果**：两台设备（或同一台设备的两个窗口）同时操作时，`startup_sync` 可能正确报告 `conflict: true`，但用户一旦点"保留本地"，就直接 `Overwrite: T` 覆盖远程，另一台设备的下一次启动会**用本地库覆盖掉刚刚上传的数据**，且用户以为自己"解决"了冲突。**远端最新的案件数据被无声销毁。**

文档 [DATA_AND_SECURITY.md:40] 写的"冲突由用户决定"成立，但"决定"的执行是单向覆盖，没有任何保护。这是典型的"看起来有冲突处理、实际没有"。

---

### S-5 【高 · CONFIRMED】MCP 写操作确认门禁被一个已注册的 IPC 命令完整绕过

设计意图（`mcp/mod.rs:5-6`、`server.rs:6-7`）：写工具不直接执行，进入 `mcp_pending_writes` 队列，用户在应用内确认后才执行。HTTP 通道上这个门禁是有效的：

`mcp/mod.rs:480`
```rust
if !self.allow_write && WRITE_TOOLS.contains(&call.name.as_str()) { /* 入队，返回 pending_confirmation */ }
```
`server.rs:101` 传入 `McpServer::new_readonly()`（`allow_write = false`）。

但自由函数绕过了整个门禁：

`mcp/mod.rs:910-912`
```rust
pub async fn execute_tool(call: McpToolCall) -> Result<serde_json::Value, String> {
    execute_tool_by_name(&call.tool, call.arguments).await   // ← 不检查 allow_write
}
```

而它被注册为 Tauri 命令：

`src/commands/mod.rs:557-565`
```rust
#[tauri::command]
pub async fn mcp_execute_tool(tool: String, arguments: serde_json::Value)
    -> Result<serde_json::Value, String> {
    let call = crate::mcp::McpToolCall { tool, arguments };
    let result = crate::mcp::execute_tool(call).await;
    ...
}
```
注册于 `commands/mod.rs:499` 的 invoke handler。

**触发**：webview 中任意 JS 执行
```js
await invoke('mcp_execute_tool', { tool: 'case_create_task', arguments: { ... } })
```

**可观察的错误结果**：真实写入 `tasks` 表，**没有 `mcp_pending_writes` 记录、没有 `audit_events` 审计行、没有用户确认**。Tauri v2 的自定义命令不需要 `capabilities/default.json` 授权条目（该文件只有 `core:*`、`dialog`、`notification`、`global-shortcut`），所以这条路径是活的。前端当前无调用方（grep 零命中），但任何 XSS 或被投毒的依赖都能直接用。

---

### S-6 【高 · CONFIRMED】SMTP/ICS 生成物在真实邮件服务器上大概率被拒或进垃圾箱

`src-tauri/src/email/smtp.rs:208-240` 构造的 MIME 报文：

**缺失 `Date:` 头**。RFC 5322 §3.6 要求 `Date` 是必填 origination date；Outlook、Exchange、部分企业网关会直接拒收无 Date 的邮件。代码里全文没有任何 Date 头的构造。

**缺失 `Message-ID:` 头**。同 §3.6.1 要求。缺失时客户端线程归并、去重、防伪造全部失效。

`src-tauri/src/email/smtp.rs:71`
```rust
let now = chrono::Local::now().format("%Y%m%dT%H%M%S");   // 本地时间，无 'Z'
```
ICS 的 `DTSTAMP` 按 RFC 5545 §3.3.5 必须是 UTC DATE-TIME，**必须以 `Z` 结尾**。这里输出的是本地浮动时间且无 `Z`，是格式非法的 DATE-TIME。CalDAV 侧写对了（`sync/caldav.rs:346` `chrono::Utc::now().format("%Y%m%dT%H%M%SZ")`），SMTP 侧写错了。

**两处 ICS 生成都没有时区**。`DTSTART`/`DTEND` 输出 `20260930T090000` 这样的浮动时间，既无 `Z` 也无 `TZID`，且 `VCALENDAR` 里没有 `VTIMEZONE` 组件。收件人在不同时区看到的提醒时间会偏移。CalDAV 场景更糟：Casy 推过去的提醒是给 Google/Outlook **在服务器侧**触发推送的，服务端按自己的时区解释浮动时间，北京时间 09:00 的截止提醒可能在 UTC 服务器上变成 01:00。

**两处 ICS 生成都没有 75 字节折行**（RFC 5545 §3.1 强制要求）。`escape_ics` 只处理 `\;`, `\,`, `\n`，不折行。中文案件摘要几乎必然产生超过 75 字节的行（30 个中文字符 = 90 字节），严格解析器会拒绝整个 `.ics`。

**死配置**：`SmtpConfig.use_tls`（`smtp.rs:30`）在全文件**从未被读取** —— `smtp_session` 的 `implicit_tls` 只看 `config.smtp_port == 465`（`smtp.rs:359`）。`load_smtp_config` 在 `smtp.rs:543` 硬编码 `use_tls: true`，这个字段是纯装饰。

**缺失的发件人配置**：`load_smtp_config` 在 `smtp.rs:544-545` 把 `from_address` 和 `from_name` **都设成 `smtp_user`**。没有任何设置项可以指定发件地址。SMTP 登录账号与对外发件地址不同的场景（律所常见）**无法配置**。

---

### S-7 【高 · CONFIRMED】邮件监听不自动启动，"已启动"不代表能收到信

**不自动启动**。全仓库对 `start_email_monitor` 的引用只有一处：

`src/commands/mod.rs:386`
```rust
crate::email::start_email_monitor,
```
这是 invoke handler 的注册项。`lib.rs` 的 `setup()` 里**没有任何启动邮件监听的代码** —— 对比同文件 `:213` 的 `start_auto_push_watcher()` 和 `:186` 的 MCP spawn。每次重启应用，IMAP 监听都处于停止状态，必须由用户手动点"开始监听"。

**启动不验证任何连通性**：

`src-tauri/src/email/mod.rs:433-467`
```rust
pub fn start(&mut self) -> Result<()> {
    if self.running.load(Ordering::SeqCst) { return Ok(()); }
    let accounts = load_enabled_accounts()?;
    if accounts.is_empty() { anyhow::bail!("没有启用的 IMAP 账号，请先配置"); }
    self.running.store(true, Ordering::SeqCst);
    let handle = tokio::spawn(async move {
        for account in accounts {
            tokio::spawn(async move {
                if let Err(e) = watch_account(account, running_clone).await {
                    crate::processing::service("email","邮件监听","failed","邮件监听异常",Some(&e.to_string()));
                    log::error!("IMAP 监听错误: {}", e);
                }
            });
        }
        ...
    });
    self.handle = Some(handle);
    crate::processing::service("email","邮件监听","waiting","等待新邮件通知",None);
    Ok(())
}
```

TCP 连接、登录、选文件夹都在**子任务**里异步发生，主函数立刻返回 `Ok`。命令返回 `"邮件监听已启动"`，状态灯显示"等待新邮件通知"。服务器地址错、密码错、端口被墙 —— 用户看到的一切都是正常的，只有日志里有一条 `IMAP 连接错误: ..., 30 秒后重试`，然后每 30 秒重试一次，UI 上的"等待新邮件通知"**永远不会改变**。

**`start()` 后的健康状态不可观测**：`get_email_monitor_status`（`email/mod.rs:747`）只返回 `running`（本地 flag）、账号数量和账号列表，**不返回连接状态或已收取邮件数**。用户无法区分"已连接在监听"和"连不上"。

---

### S-8 【中 · CONFIRMED】白名单过滤的邮件导致无限重扫，且每次都要重新解析

`src-tauri/src/email/mod.rs:282-285` 与 `:365-371`

```rust
// 白名单过滤
if !passes_whitelist(config, &from, &subject) {
    log::debug!("邮件被白名单过滤: from={}, subject={}", from, subject);
    return Ok(false);                       // ← 在更新 last_sync_uid 之前返回
}
...
// 更新 IMAP 账号的 last_sync_uid
if let Some(ref account_id) = config.id {
    conn.execute("UPDATE imap_accounts SET last_sync_uid = ?1 WHERE id = ?2", ...)?;
}
```

被白名单过滤掉的邮件**不推进 `last_sync_uid` 高水位**。而拉取查询是基于高水位的：

`src-tauri/src/email/mod.rs:619-623`
```rust
let search_query = if last_uid > 0 { format!("UID {}:*", last_uid + 1) } else { "ALL".to_string() };
```

**触发**：邮箱里有 N 封不匹配白名单的邮件（律所邮箱里广告、订阅、法院群发的无关通知轻松上万封）。

**可观察的错误结果**：每一次 IDLE 通知、每一次 29 分钟超时重连，都会重新 SEARCH、重新 FETCH、重新 `mailparse::parse_mail` 解析**同样这 N 封邮件**。运行一整天就是几万次重复解析和 FETCH 往返，其中大部分被 UID 去重检查（`:325-331`）丢弃。更糟的是这些邮件会持续触发飞书/AI 相关的下游开销预期 —— 用户会以为"邮件功能很耗资源"。

正确做法是在过滤时也推进高水位（或维护一个独立的 `filtered_until_uid`）。

---

### S-9 【中 · CONFIRMED】`email_records` 与 `inbox_items` 两条写入不在同一事务

`src-tauri/src/email/mod.rs:333` 与 `:352`

```rust
conn.execute("INSERT INTO email_records (...) VALUES (...)", params![...])?;   // 第一条

// 同时添加到收件箱
let inbox_id = new_id();
conn.execute("INSERT INTO inbox_items (...) VALUES (...)", params![...])?;      // 第二条
```

两条独立语句，`process_email` 持有的是 `&Connection` 而非 `Transaction`。第二条失败时第一条已提交。

**可观察的错误结果**：`email_records` 里有这封邮件（`message_id` 去重会在下次拉取时命中并跳过，`:302-314`），但**收件箱里永远没有它**。法院传票进了"已读邮件"列表却不出现在收件箱待处理队列 —— 且没有任何错误提示，因为整个调用链在 `fetch_new_emails_inner` 里被 `if let Err(e) = process_email(...) { log::error!(...) }`（`:644-646`）吞掉。

这与 [production-remediation-2026-09-30.md:35] 记录的"其他模块仍存在静默行错误过滤"是同一类问题，但邮件路径**连测试都没有**。

---

### S-10 【中 · CONFIRMED】`import_feishu` / 快照命令的分页上限静默截断

`src-tauri/src/sync/feishu.rs:499-518`

```rust
pub async fn list_all_bitable_records(app_token: &str, table_id: &str, max_pages: usize)
    -> Result<Vec<BitableRecordInfo>> {
    let mut all_records = Vec::new();
    let mut page_token = String::new();
    for _ in 0..max_pages {
        let page = list_bitable_records(app_token, table_id, &page_token).await?;
        let has_more = page.has_more;
        all_records.extend(page.items);
        if !has_more { break; }
        page_token = page.page_token.unwrap_or_default();   // ← 缺失则回到第一页
    }
    Ok(all_records)      // ← 无任何"已截断"标记
}
```

调用方：`commands/sync.rs:578`（`feishu_compare_records`，20 页 × 500 = **1 万条上限**）、`:821` / `:860` / `:911`（导入，100 页 = **5 万条上限**）。

**两个缺陷**：
1. 达到 `max_pages` 后直接返回，**没有任何字段告诉调用方结果被截断**。用户在有 3 万条记录的表上执行"比对"，UI 显示"远端 10000 条，本地 X 条，全部匹配"—— 远端另外 2 万条根本没被下载。
2. 若 `page_token` 缺失而 `has_more == true`（服务端异常），`unwrap_or_default()` 回到空串 → 重新拉第一页 → 重复记录直到耗尽页数。`feishu_import_bitable_cases`（`import_feishu.rs:429-433`）的同类循环有完全相同的问题，且是 `loop {}` 无界循环 —— 服务端异常时**真的死循环**。

---

### S-11 【中 · CONFIRMED】`trigger_feishu_push` 在自动推送关闭时是空操作，但返回"已触发"

`src-tauri/src/commands/sync.rs:304-308`

```rust
#[tauri::command]
pub async fn trigger_feishu_push() -> Result<String, String> {
    let manager = sync::feishu::get_auto_push_manager();
    manager.notify_change();
    Ok("已触发飞书推送（5 秒后执行）".into())
}
```

而 `notify_change` 的第一件事就是提前返回：

`src-tauri/src/sync/feishu.rs:2048-2054`
```rust
pub fn notify_change(&self) {
    if !self.enabled.load(Ordering::SeqCst) { return; }      // ← 默认关闭
    if !is_feishu_configured() { return; }
    ...
}
```

**可观察的错误结果**：自动推送关闭时点"手动触发推送"，提示"已触发飞书推送（5 秒后执行）"，5 秒后什么也没发生，没有第二次提示。默认 `enabled` 是 `false`（`feishu.rs:2032`），所以**在用户主动打开设置之前，这个"用于测试"的按钮永远是假的**。

---

### S-12 【中 · CONFIRMED】AI 后端初始化失败降级为"成功的规则分类"

`src-tauri/src/ai/mod.rs:912-921`（`create_backend`）

```rust
match OpenAiBackend::new(url, key, model) {
    Ok(backend) => Box::new(backend),
    Err(error) => {
        log::error!("OpenAI 后端初始化失败，回退到 noop: {error}");
        Box::new(NoOpBackend)
    }
}
```

`NoOpBackend::classify_document`（`mod.rs:426-435`）**不返回错误**，而是用纯 Rust 规则解析器返回 `Ok(AiClassifyResult{..})`。于是 `process_inbox_with_ai`（`mod.rs:847-887`）把这次调用记为：

```rust
Ok(r) => { ... ("completed", Some(output_hash)) }
```

**可观察的错误结果**：AI 接口地址写错、模型名不存在、密钥失效 —— `ai_runs.status = "completed"`，审计页显示"成功"，收件箱里出现了看起来合理的分类结果，但没有任何模型参与过。用户和律师都会基于这些"分类"做分流决策。

叠加 `mod.rs:206-210` 的密钥链静默降级（`profiles::resolve(...).unwrap_or_else(|e| AiConfig::default())` → `mode: "noop"`），一个锁定了钥匙串的用户会看到全套规则分类 + 全绿的审计日志。

---

### S-13 【中 · CONFIRMED】`hybrid_search_knowledge` 丢弃语义检索降级状态，@ 引用与文书助手静默退化为关键词匹配

`src-tauri/src/db/search.rs:214-223`

```rust
#[tauri::command]
pub async fn hybrid_search_knowledge(query: String, limit: Option<usize>)
    -> Result<Vec<SearchResult>, String> {
    search(&query, limit.unwrap_or(20), true).await
        .map(|r| r.results)          // ← semantic_status 和 warning 被丢弃
        .map_err(|e| e.to_string())
}
```

`search` 本身是正确的：它会把 `semantic_status`（`"unavailable"`）和 `warning` 一起返回（`search.rs:206-210`），`search_knowledge_index` 命令也如实透传。但这个命令把两者扔掉，只留结果列表。TS 签名（`src/types/commandMap.ts:516`）返回 `SearchResult[]`，调用方无从得知。

**前端两处在用**：`src/shared/components/ReferenceSelect.vue:121`（@ 引用选择器）、`src/modules/docs/composables/useCopilot.js:53`（文书助手）。

**可观察的错误结果**：嵌入接口超时、返回 401、指纹变更（换模型）、维度不匹配 —— 用户在 @ 引用里看到一份"看起来相关"的关键词结果列表，完全不知道语义检索根本没运行。撰稿助手基于这份降级结果给出建议。

---

### S-14 【中 · CONFIRMED】`system_prompt` 是 8 个 AI 调用点里唯一生效的那个

`AiProfiles.system_prompt`（`profiles.rs:31`，默认 `"你是一个专业的法律 AI 助手。"`）是用户可编辑的设置项。全仓库读取它的地方只有一处：

`src-tauri/src/ai/mod.rs:1104-1107`
```rust
let system_prompt = profiles::read(&*crate::db::open_db()...).system_prompt;
let mut policies = vec![system_prompt];
```

（仅 `ai_chat`。）

其余全部硬编码各自的 prompt：`call_llm_json`（`mod.rs:298`）、`classify_document`（`mod.rs:505`、`mod.rs:673`）、`summarize`（`mod.rs:565`、`mod.rs:732`）、`generate_writing_with_ai`（`mod.rs:975`）、`reports.rs:339`、`insights.rs:211`、`recursive_check.rs:57`。

**可观察的错误结果**：用户在设置里把系统提示词改成"请用粤语回答，只输出 JSON"，聊天面板照做，文书生成、分类、摘要、报表、洞察、检查全部无视。设置项存了、UI 显示了、1/8 生效。

---

### S-15 【中 · CONFIRMED】MCP 默认开启，且前端与后端的默认值来源不一致

`src-tauri/src/lib.rs:183-187`
```rust
let mcp_enabled = db::get_setting(&conn, "mcp_server_enabled")
    .ok()                     // ← 读取失败被吞
    .flatten()
    .map(|v| v != "false")
    .unwrap_or(true);        // ← 失败开放
```

`mcp_server_enabled` 在 schema 迁移中**没有种子行**，全新安装 → `get_setting` 返回 `None` → `unwrap_or(true)` → **MCP 服务默认启动**。`.ok()` 使得 DB 读取错误同样走"开启"分支。

前端默认值来自另一个源：`src/stores/settings.ts:39` 写死 `mcp_server_enabled: false`（布尔），而 `SmtpMcpSettings.vue:126` 用 `settingsStore.mcp_server_enabled !== 'false'` 求值得 `true`。写入侧（`:127`）写的是**字符串** `'true'`/`'false'` —— 类型在持久化边界上不一致。

**后果**：一个用户可能看到"已启用"而实际已停、或反之，取决于 settings 表里有没有那行。切换开关需要重启（UI 已如实提示，见 `SmtpMcpSettings.vue:123`），但默认状态的呈现不可信。

---

### S-16 【中 · CONFIRMED】MCP `case_query` 的 filter 解析失败时返回全部案件

`src-tauri/src/mcp/mod.rs:693-696`

```rust
let filter_value = args.get("filter").cloned().unwrap_or(serde_json::json!({}));
let filter: crate::db::cases::CaseFilter =
    serde_json::from_value(filter_value).unwrap_or_default();      // ← 解析失败 = 无过滤
```

客户端发送类型错误的 filter（如 `{"filter":{"page":"not-a-number"}}`）时，得到 `CaseFilter::default()` —— **零过滤**，服务返回**全部案件列表**而不是 JSON-RPC `-32602 Invalid params`。

这是攻击者可控的畸形输入触发的静默数据过度披露。修法：`map_err` 到 `-32602`。

同类：`resources/read` 在后端失败时把错误**序列化成一个成功响应**的内容（`mod.rs:596-599`、`610-613`、`624-627`、`630-633`）：

```rust
Ok(data) => serde_json::to_string_pretty(&data).unwrap_or_default(),
Err(e) => format!("错误: {}", e),        // ← 包在 Ok 里，无 isError 标志
```

MCP 客户端看到 `result.ok == true`，`content` 是字符串 `"错误: ..."`。`handle_tools_call` 走的是 `isError: Some(true)`（`mod.rs:561`）—— 同仓库两条路径行为不一致。

---

## 凭据与密钥

这一层的**主干是可靠的**，以下先记功，再列四处例外。

### 做得对的部分（不要在重构中破坏）

- **无任何密钥进入日志/stdout/前端。** 全量核查 `log::*` / `println!` / `tracing::*` 与 `api_key|token|password|secret|authorization` 的交叉，无一处打印密钥值。`credentials/mod.rs:65` 只打印类型标签与账号名（`凭据已存储到 keychain: IMAP 邮箱密码 (user@example.com)`），这是正确的做法。
- **AI 错误消息刻意不回显响应体**（`ai/mod.rs:311, 348, 494, 591, 658, 766` 只用 `resp.status()`）。理由写在了测试里：`tests/ai_profiles_test.rs:166-182` 断言把 `"secret-echo"` 同时作为 API key 和 401 响应体时，错误消息中不含该字符串。**很多服务商会在错误体里回显提交的 `Authorization` 头**，这个决定是有先见之明的。
- **AI keychain 按需读取**，且显式避免在只读路径解锁：`profiles.rs:166-172` 的 `public_config` 把 `api_key` 设为 `None`，注释写明"Startup/UI metadata must not unlock Keychain just to discard the secret"，并有单测（`profiles.rs:311-321`）。
- **`secure_settings_secret` 失败关闭**（`credentials/mod.rs:290-299`）：每次写入用全新 UUID 账号，写后读回校验，读回不一致就报错且**不修改旧配置**。注释解释了用新条目而非覆盖的原因（"A new immutable entry allows SQLite rollback without overwriting the old credential"）。
- **上传路径不含密钥。** `sync/mod.rs:95` 源码内写明不变量并强制执行；`portable_backup::collect_roots`（`:229-233`）显式排除 backups 目录与点文件，`casy.db.key` 不可能被卷进去；完整档案在 `portable_backup.rs:404` 用 `age::Encryptor::with_user_passphrase` 端到端加密后才上网。飞书推送有表名白名单（`feishu.rs:1505-1507`），凭据表不可达。**文档声称"不上传密钥"，代码核实为真。**
- **前端读不到密钥**：`commands/settings.rs:21-28` 丢弃 `ai_api_key`/`ai_profiles_v1`/`mcp_auth_token`，并把所有 `settings_secret_type` 已知键替换为 `"" + *_configured + *_needs_migration` 标志。写侧还拒绝从前端写入 `keychain:v1:` 引用（`settings.rs:61`）。
- **IMAP 账号列表不返回密码列**（`email/mod.rs:700` 的 SELECT 里根本没有 `password_enc`）。

### 例外 1 · 【高】飞书 App Secret 同时写入钥匙串与 DB 明文，且钥匙串失败完全静默

`src-tauri/src/sync/feishu.rs:24-43`

```rust
pub fn save_feishu_credentials(app_id: &str, app_secret: &str) -> Result<()> {
    // 1. 尝试写入 OS Keychain
    if let Ok(entry_id) = keyring::Entry::new(KEYCHAIN_SERVICE, KEY_APP_ID) {
        let _ = entry_id.set_password(clean_id);           // ← 结果丢弃
    }
    if let Ok(entry_secret) = keyring::Entry::new(KEYCHAIN_SERVICE, KEY_APP_SECRET) {
        let _ = entry_secret.set_password(clean_secret);    // ← 结果丢弃
    }
    // 2. 双重持久化到本地加密 SQLite 设置
    if let Ok(conn) = crate::db::open_db() {
        let _ = crate::db::set_setting(&conn, "feishu_app_id", clean_id);
        let _ = crate::db::set_setting(&conn, "feishu_app_secret", clean_secret);  // ← 明文
    }
    Ok(())
}
```

四个 `let _ =` 丢弃全部结果。函数无条件返回 `Ok(())`，命令 `configure_feishu`（`commands/sync.rs:182-188`）因此永远回"飞书凭证已保存"。

`settings.feishu_app_secret` 是**裸明文**（在 SQLCipher 加密的 DB 内，但拿到 DB 密钥文件即可读，不需要钥匙串的系统解锁提示）。

**这直接抵消了同一代码库里已经做好的工作**：`credentials::settings_secret_type` 已经把 `"feishu_app_secret"` 映射到 `CredentialType::FeishuToken`（`credentials/mod.rs:276`），`resolve_settings_secret`（`:281-289`）也已经写好了处理 `keychain:v1:` 引用 —— 但 `save_feishu_credentials` 从不写这个引用，而且它用的是**自己的钥匙串 service `com.casy.feishu`**（`feishu.rs:19`），而不是凭据模块用的 `casy-feishu`。**两套并存的实现，活着的是不安全的那套。**

降级读取是**刻意为之**而非疏忽（`feishu.rs:1478-1480` 的注释："Passive status checks must not unlock Keychain"），但代价是每次同步都在走裸明文读取路径。

**修法**：`configure_feishu` 改走 `save_settings` / `secure_settings_secret`，删除 `:39` 的 `set_setting`，并把 `KEYCHAIN_SERVICE` 统一到 `credentials` 模块。

### 例外 2 · 【高】IMAP 密码在钥匙串失败时降级为 base64 存进 DB

`src-tauri/src/email/mod.rs:59-72`

```rust
match crate::credentials::store_credential(
    crate::credentials::CredentialType::ImapPassword, &config.email_address, &config.password,
) {
    Ok(()) => "keychain".to_string(),
    Err(e) => {
        log::warn!("keychain 存储失败，回退 base64 存储: {}", e);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, config.password.as_bytes())
    }
}
```

base64 是**编码不是加密**，值落在 `imap_accounts.password_enc`（`schema.rs:657`，`TEXT NOT NULL`）。

这是全代码库**唯一**一处钥匙串失败导致密钥以可还原形式落库的地方，注释甚至写明了这是为了"保证保存不失败（兼容旧逻辑）"。对比 `secure_settings_secret` 是失败关闭的 —— 同一仓库里两种相反的哲学。

**读路径也不对称**：`get_imap_password`（`credentials/mod.rs:228-251`）正确地拒绝把 `"keychain"` 哨兵当 base64 解码，但一次 base64 **写入**之后，这个不安全状态是**粘性的** —— 用户永远不会被提示重新输入密码。

### 例外 3 · 【中】MCP 鉴权 Token 只存 DB 明文，且可被 webview 覆写

`src-tauri/src/mcp/server.rs:39-52` 用 `crate::db::set_setting(&conn, "mcp_auth_token", &fresh)` 存 UUID v4。`credentials::settings_secret_type`（`credentials/mod.rs:271-279`）**不包含** `mcp_auth_token`，所以它不走钥匙串。

这个 token 守护的是**全部案件数据的访问权**，比一个邮箱密码更敏感，却比邮箱密码保护得更弱。

同时 `get_settings` 在读侧过滤了它（`settings.rs:21`），**写侧没有过滤**（`settings.rs:48` 的跳过列表只含 `ai_` 前缀 / `caseFolderBase` / `*_configured` / `*_needs_migration`）。所以任意 webview 脚本可以：

```js
await invoke('save_settings', { settings: { mcp_auth_token: '<攻击者选定的值>' } })
```

当前进程内不生效（`AUTH_TOKEN` 是 `OnceLock`），但**下次启动就生效** —— 全部案件数据访问权被静默转移给攻击者知道的 token。前端目前不发送这个键（grep 无命中），所以这是纵深防御缺口而非活跃攻击面。

### 例外 4 · 【中】旧版明文 settings 密钥没有自动迁移路径

`src-tauri/src/credentials/mod.rs:288`

```rust
Ok(Some(stored)) // legacy credentials remain usable until verified migration succeeds
```

`get_settings` 为 UI 计算了 `*_needs_migration` 标志（`settings.rs:26`），但**没有任何代码执行这个迁移**。grep `secure_settings_secret` 只有两个调用点：`settings.rs:62`（保存新值时）和自身定义。

对比 IMAP 路径**有**真实迁移例程（`migrate_imap_passwords_to_keychain`，`credentials/mod.rs:130-225`，暴露为 `migrate_credentials_to_keychain` 命令）。

在钥匙串工作落地之前保存过 WebDAV / SMTP / CalDAV 密码的用户，会**永久停留在明文**，只有一个 UI 提示在旁敲侧击。

### 例外 5 · 【低】死掉的明文密钥表

`src-tauri/src/db/schema.rs:1663` 的 `feishu_connections.app_secret TEXT NOT NULL` —— grep `feishu_connections` 只在 `schema.rs` 命中，**没有任何代码读写它**。应删除，或加 CHECK/触发器禁止非空值，防止未来迁移不小心开始写入。

### 例外 6 · 【低】两份手工维护的密钥清单

`commands/settings.rs:21` 硬编码 `ai_api_key || ai_profiles_v1 || mcp_auth_token`，而 `settings_secret_type`（`credentials/mod.rs:271-279`）知道的是**另一份**集合。两份清单必须靠人肉保持同步 —— 改一份忘一份就是一次未来泄露。应该由单一表格派生。

### 关于网络传输

- **WebDAV 允许 `http://`**：`WebDavClient::new`（`webdav.rs:92`）不校验 scheme。用户填 `http://` 就会用 HTTP Basic Auth 明文发送密码（`webdav.rs:114` 等 8 处 `.basic_auth`）。应在 `new()` 里对非回环主机强制 https，或至少在 UI 明确警告。
- **`AUTH_TOKEN` 的并发初始化注释是错的**（`server.rs:57`）："并发首次设置时只会有一个生效，两者内容同源无妨"。当 settings 表里**没有**预置 token 时，两个并发首次调用各自生成**不同的** UUID v4，各自写入 DB，最后一次写胜出，而 `OnceLock` 保留的是另一个值 —— 从 settings 表配置的客户端会被 401 拒绝整个进程生命周期。启动时可触发（`auth_token()` 同时被 `run()`（`:64`）和 `get_mcp_server_info`（`:266`）调用）。
- **`log::warn!` 不含 token 值**（`server.rs:51`），启动日志也不打印（`server.rs:68-71`），`get_mcp_server_info` 只返回 4 字符提示（`server.rs:257-279`）。这几处是对的。

---

## 同步正确性

### 冲突策略：三个方向都是"最后写入者赢"，且没有任何检测

| 链路 | 冲突检测 | 实际行为 |
| --- | --- | --- |
| WebDAV 快照 | `startup_sync` 比较 ETag 能报 `conflict: true` | 用户点"解决"→ 无条件 `Overwrite: T`（S-4）。`put_if_match` 死代码 |
| WebDAV 完整备份 | 无 | 临时名 + MOVE 原子替换，只保留远端最新一份，无版本、无条件提交（文档已声明） |
| 飞书 pull（案件） | 有比较，但比较的是空串（S-1） | 恒不跳过，恒覆盖 |
| 飞书 pull（通用表） | 有比较（`as_i64`，正确） | 但不检查 `sync_status='local_newer'`，本地未推送修改无保护 |
| 飞书 push | 无 | 依赖 `c.updated_at > sm.last_synced_at` 启发式，与 1001 类型的 `created_time` 无关 |
| CalDAV | 无 | PUT 同 UID 幂等覆盖，无 `If-Match`；靠 `get_event_etag` 对账"结果不明"（这一条设计是对的） |

飞书 push 的本地变更检测用字符串比较时间戳（`feishu.rs:1324`）：

```sql
AND c.updated_at > COALESCE(sm.last_synced_at, '1970-01-01')
```

`c.updated_at` 由 `update_local_case` 写成 `datetime('now','localtime')`（`feishu.rs:1086`），`sm.last_synced_at` 由 `now_local()` 写成。两个函数若时区/格式不完全一致，比较结果是错的或恒假的 —— 需要一个断言两者格式相同的测试，目前没有。

### 部分失败留下半同步状态

- **飞书 pull**：逐条独立写库，**无事务**。第 N 条失败时前 N-1 条已提交，`report.errors` 收集错误但没有回滚。这本身是可接受的增量语义，但报告与实际状态之间没有任何标记。
- **飞书 push**：同上，S-2 让失败记录永远留在重试队列。
- **飞书导入**：S-3，同一事务内吞错后报成功 —— 这是最糟的一种。
- **CalDAV 作业**：设计得好。`execute_calendar_job`（`commands/caldav.rs:111-181`）有状态机（`pending`/`synced`/`sync_failed`/`delivery_unknown`/`cancelled`）、`attempts` 计数、`next_attempt_at` 五分钟退避、`delivery_unknown` 先 `get_event_etag` 对账再决定是否重 PUT、失败回退本地通知。这是六条链路里唯一做对了失败语义的一条，建议以此为模板重写飞书。
- **IMAP**：S-9，两条写入无事务。

### 重试双应用

CalDAV 有 `next_attempt_at` + `attempts`，逻辑正确。飞书完全没有（S-2）。SMTP 完全没���（`smtp.rs` 无任何队列或重试，一次网络抖动就是一封发不出去的邀请邮件，且 `send_ics_invitation_cmd` 直接把错误返回给用户）。WebDAV 无重试。

### 时间戳与时区

- 飞书 `created_time`/`last_modified_time` 是 i64 毫秒。`pull_one_record` 用 `as_str()` 读（S-1）。通用 pull 用 `as_i64().to_string()` 转字符串存进 `sync_map.remote_updated`（文本列），类型混用。
- CalDAV/SMTP 的 ICS 全部是无时区浮动时间（S-6）。
- `commands/caldav.rs:48-64` 的 `parse_due_morning` / `parse_due_datetime` 把"截止日期"映射为当天 09:00 本地时间 —— 这个假设写在了文档里，但当 `due_date` 带时区信息（`parse_start_time` 在 `smtp.rs:551` 会把 RFC3339 转成本地）时两条路径的语义会分叉。

### IMAP IDLE 生命周期

- 29 分钟超时重连（`email/mod.rs:526`，`IDLE_TIMEOUT_SECS = 29*60`）—— 符合 IMAP RFC 惯例（服务器通常 30 分钟）。
- `tokio::select!` 里用 1 秒轮询 `running` 实现停止（`:573-578`）—— 可行但浪费；`idle.done()` 在停止分支里被 `if let Ok(s) = ...` 吞掉（`:581`），失败则 `session` 保持 `None`，退出循环后 `if let Some(mut s) = session` 不执行，跳过 `logout()` —— 连接泄漏直到 TCP 超时。
- **`stop()` 后快速 `start()` 会产生重复监听任务**：`stop()` 只 `abort()` 外层 handle（`:472-473`），内层 `tokio::spawn` 的账号任务不受父任务 abort 影响，它们靠 `running == false` 退出。若在 1 秒内重新 `start()` 把 `running` 置回 `true`，旧任务不会退出，新任务又已创建 —— **同一邮箱出现两条 IMAP 连接，两份重复邮件**（`message_id` 去重能挡住入库，但 IDLE 通知翻倍）。

---

## 错误吞噬

按危害排序。**"失败被报成成功"在同步场景里比崩溃更糟**，下面每一条都是这个模式。

| 位置 | 模式 | 危害 |
| --- | --- | --- |
| `import_feishu.rs:723, 839, 857, 1264` | `let _ = tx.execute(...)` 后 `report.created_count += 1` | **开庭记录与委托人静默丢失，报告成功**（S-3） |
| `sync/feishu.rs:30-39` | 四个 `let _ =` 丢弃钥匙串与 DB 写入结果 | 飞书凭据"已保存"，实际可能哪都没写进去 |
| `commands/sync.rs:304-308` | `trigger_feishu_push` 无条件返回"已触发" | 假成功提示（S-11） |
| `ai/mod.rs:912-921` + `:426-435` | NoOpBackend 返回 `Ok`，审计记 `completed` | **AI 从未运行但审计全绿**（S-12） |
| `mcp/mod.rs:596-599` 等四处 | `Err(e) => format!("错误: {}", e)` 包进 `Ok` | MCP 客户端看到 `ok=true` |
| `mcp/mod.rs:695` | `from_value(..).unwrap_or_default()` | filter 解析失败 → 返回**全部**案件（S-16） |
| `db/search.rs:214-223` | `.map(\|r\| r.results)` 丢弃降级状态 | 语义检索静默失效（S-13） |
| `email/mod.rs:126` | `get_imap_password(..).unwrap_or_default()` | 钥匙串读失败 → **密码变空串**，账号被静默加载为无密码 |
| `email/mod.rs:644-646` | `if let Err(e) = process_email(...) { log::error!(e) }` | 单封邮件处理失败静默继续，无计数无上报 |
| `email/mod.rs:750` | `load_enabled_accounts().unwrap_or_default()` | 状态接口在 DB 错误时报告"0 个账号"，看起来像未配置 |
| `sync/feishu.rs:1317, 1320, 1861` | `attempts` 硬编码 `0` | 失败计数机制形同虚设（S-2） |
| `sync/feishu.rs:917` | `items.as_array().cloned().unwrap_or_default()` | 响应结构异常 → 空数组 → `break` → **拉取报告"成功，0 条"** |
| `sync/feishu.rs:1427` | `get_sync_metadata` 用 `.ok()` | `feishu_last_pull_at` 读取失败与"从未同步过"无法区分 |
| `sync/mod.rs:172` | `client.head("casy.db").await?.unwrap_or_default()` | 服务器不返 ETag → 存空串，下次比对行为未定义 |
| `ai/profiles.rs:73, 209` | `get_setting(conn, key).ok().flatten()` | **保存路径**上 DB 读错误会被当成"键不存在" → 已有 API key 被丢弃而非保留 |
| `ai/profiles.rs:243` | `let _ = credentials::delete_credential(...)` 且无日志（对比 `:253` 有 `log::warn!`） | 回滚失败会在钥匙串里留下**孤儿 API key**，账号名无人知晓，应用永远不会清理 |
| `ai/gateway.rs:294` | `let _ = conn.execute("UPDATE ai_proposals SET status='expired'...")` | 失败关闭，无实际危害，但仍是吞掉的 DB 错误 |
| `ai/usage.rs:48` | `map_err(\|_\| anyhow!("AI 接口连接失败或超时"))` | 丢掉 `reqwest::Error`，DNS 失败 / TLS 失败 / 超时在 UI 和 `ai_runs` 审计里完全无法区分 |
| `mcp/server.rs:40-43, 50-52` | DB 不可用时生成随机 token 并 `log::warn!` | 用户永远拿不回这个 token（UI 只显示 4 字符），必须重启 —— 自我锁死 |
| `lib.rs:184` | `.ok().flatten().unwrap_or(true)` | DB 读失败 → MCP 服务开启 |
| `feishu_snapshot.rs:930` | `from_str::<Value>(..).unwrap_or(Value::Null)` | 快照记录里的损坏 JSON 变成 `null` 展示给用户，看不出数据已损坏 |

`ai/mod.rs:534` / `:701` 还有一处值得注意：`parsed["confidence"].as_f64().unwrap_or(0.5)` —— 缺字段时**默认 0.5**，而 `route_by_confidence`（`mod.rs:150`）的自动关联阈值正是 `>= 0.5`。模型返回 `{"category":"x"}` 不带 confidence 时，会用一个模型从未给过的值落在自动关联的边界上。

`production-remediation-2026-09-30.md:35` 已经点明"其他模块仍存在静默行错误过滤"。本轮确认：**邮件链路整体（IMAP + SMTP）零测试、零错误处理纪律，是这个问题的重灾区。**

---

## 前端接线与死 UI

### 命令契约：完整，无缺失

抽出前端全部 304 个命令名，与 `commands/mod.rs:139` 的 `generate_handler![...]`（370 项）交叉比对，**差集为 0**。每一条集成命令在前端调用点 → `commandMap.ts` → Rust `#[tauri::command]` → handler 四层齐备。

这个契约由 `tests/contract.commands.test.ts`（8 项）作为门禁强制，已跑通。**唯一的问题不是缺命令，而是多了一个没人调的命令（见 S-0）—— 契约门禁管"前端调了但后端没有"，管不到"后端有但前端不调"。**

### 真正持久化的设置

| 集成 | 入口 | 写入方式 |
| --- | --- | --- |
| WebDAV | `WebDAVSettings.vue:76` | `settingsStore.save(['webdavUrl','webdavUsername','webdavPassword','webdavAutoSync'])` |
| SMTP | `SmtpMcpSettings.vue:17` | `settingsStore.save(['smtp_host','smtp_port','smtp_user','smtp_pass'])` |
| CalDAV | `SmtpMcpSettings.vue:74` | `settingsStore.save(['caldav_url','caldav_user','caldav_pass','calendar_sync_enabled','calendar_mask_case_name'])` |
| MCP 开关 | `SmtpMcpSettings.vue:134` | `settingsStore.save(['mcp_server_enabled'])` |
| IMAP | `ImapSettings.vue:65` | `configureImap(snapshot)` 专用命令 |
| AI profiles | `stores/aiSettings.ts:31` | `save_ai_profiles` |
| 飞书凭据 | `FeishuSettings.vue:101` | `configureFeishu(appId, appSecret)` |
| **飞书表格** | **无** | **无写入点（S-0）** |

密钥处理正确：`stores/settings.ts:94` 从 payload 剔除 `*_configured`、`:101` 保存成功后清空本地 ref、`:117` 的 `clearSecret()` 通过 `:97` 转成 `null`（显式清除）。Rust 侧 `commands/settings.rs:57-64` 处理 null/空/保留三态并走 `secure_settings_secret`。

### 其余死 UI / 死配置

| 位置 | 问题 |
| --- | --- |
| `WebDAVSettings.vue:135` | `webdavAutoSync` 开关只在前端消费（`App.vue:258`），后端无对应读取。是纯客户端约定，能工作，但**且它传的密码是空串**，依赖 `commands/sync.rs:1253-1261` 的钥匙串回退，且要求已保存的 URL/用户名完全一致，否则报"服务器或用户名已变化，请先重新保存密码" |
| `stores/settings.ts:14-19` | 六个 `ai_*` 字段（`ai_mode`/`ai_backend`/`ai_api_url`/`ai_api_key`/`ai_model`/`ai_daily_limit`）在任何 `.vue` 里零引用，且被 `commands/settings.rs:48` 的 `key.starts_with("ai_")` 硬跳过写入。已被独立的 `aiSettings` store 取代，是遗留状态 |
| `stores/settings.ts:27-28` | `feishu_app_secret` / `feishu_app_secret_configured` 同样无组件绑定；`FeishuSettings.vue` 用自己的本地 ref + `configure_feishu` 命令 |
| `SmtpMcpSettings.vue:378` | 文案称 MCP 是"本地只读接口"，但 `approve_mcp_write`（`commands/mod.rs:582`）会走真实写路径。写确认卡片（`:396`）确实存在，所以能力是暴露的，只是文案过时 |
| `SmtpMcpSettings.vue:126`、`:68` | `mcpEnabled` 与 `maskCaseName` 的 getter 用 `!== 'false'` 比对，而 store 默认值是**布尔** `false`（`stores/settings.ts:37, 39`）→ `false !== 'false'` 为 `true`，加载完成前渲染为"开"。行为正确纯属巧合（后端默认也是 true/mask-on），类型不一致是定时炸弹 |
| `SyncStatusView.vue:17-21` | 在 setup 时把 store 快照进本地 `webdavForm`；若 store 在组件挂载后才加载完成，表单永久为空/过期。同步中心的表单是设置页表单的无绑定副本 |
| `mockData.ts:416` | `get_ai_usage` mock 返回 `todayCalls`，而 `AIStatusBadge.vue:53-54` 读 `result.data.usedToday` / `dailyLimit`（`commandMap.ts:660` 的契约）。浏览器模式下徽章显示 `--` 且无错误提示 —— mock 字段漂移掩盖了配额显示问题 |
| `mockData.ts:417-443` | `ai_chat` 的完整 mock 是**死代码** —— `tauriBridge.ts:57` 在 `tryMockCommand` 之前就短路了 `ai_chat` |

### 前端测试覆盖

**已覆盖**（`vitest`，`tests/**/*.test.ts`，`environment: 'node'`）：`webdavSettings.test.ts`(3)、`aiSettings.test.ts`(4)、`settingsSaveScope.test.ts`(3)、`settingsCredentials.test.ts`(1)、`toolPolicySettings.test.ts`(2)、`contract.commands.test.ts`(8)、`tauriBridgeDeadline.test.ts`(8)。已跑通 5 文件 / 19 项。

**零测试**（本轮风险最高的面）：
- `FeishuSettings.vue` —— **一条断言都没有**。S-0 会被"已配置的飞书页能真正完成一次拉取"这样的测试直接抓到。
- `SyncStatusView.vue` —— 零。含 S-0 的"按钮渲染为可用但不可达"死胡同。
- `src/core/autoPush.ts` —— 零。`tauriCallSafe` 返回值未被检查没有任何防护。
- `ImapSettings.vue` —— 零，尽管它有非平凡的表单校验与"失败保留输入"逻辑。
- `SmtpMcpSettings.vue` 的 CalDAV 卡片与 MCP 写确认队列 —— 只测了钥匙串探测，`approveWrite`/`rejectWrite`/`syncNow`/`testCaldav`/`sendTestInvitation` 全未测。
- `src/core/plugins/sync-plugin.ts`（4 个注册工具）与 `core/services/sync.ts`、`calendar.ts` —— 无服务层测试。

### 浏览器 mock 的诚实性

`src/core/tauriBridge.ts:55-67` 的护栏是**对的**：四个 AI 命令在 mock 查找之前被显式拦截（`:57`），正是为了让"坏掉的 AI 集成在开发模式里看起来能用"这件事不可能发生。未 mock 的命令落到 `{ok:false, error:'browser-mode: no mock'}`，`test_webdav_connection`、`webdav_backup_full`、`configure_feishu`、`test_caldav_connection`、`configure_imap` 等都会诚实地失败。

两处缺口：`get_settings`/`save_settings` 被 mock（`mockData.ts:93-94`），而真实后端会为密钥注入 `*_configured` / `*_needs_migration` 标志，mock 不注入 → 浏览器模式下 WebDAV/SMTP/CalDAV 密码占位符恒显示"未设置密码"，清除密码按钮永不渲染，**开发期看到的凭据 UI 与生产不同**。`get_sync_status` mock 写死 `configured: false`，已连接/失败状态在开发期无法触及。

---

## 生产环境前置条件清单

做真实服务验收前，以下每一条都必须先满足，否则实验结果不可信。

### P0 — 会直接导致数据损坏或功能永久不可达，必须先修

- [ ] **S-0** `commandMap.ts` + `services/sync.ts` 补 `configure_feishu_table`；`FeishuSettings.vue` 选表后调用；`SyncStatusView.vue` 按钮禁用条件加入 `!appToken`；`autoPush.ts` 检查 `tauriCallSafe` 返回值。**这是全表投入产出比最高的一项：约 30 行，解锁同步中心与自动推送两条完整链路**
- [ ] **S-1** `pull_one_record` 的 `as_str()` → `as_i64()`；并为"远端未变更则跳过"补一个单测（这是最便宜的回归防护）
- [ ] **S-3** `import_feishu.rs` 四处 `let _ = tx.execute` → 记入 `report.errors`，且**插入失败时不得 `created_count += 1`**
- [ ] **S-4** `webdav_resolve_keep_local` / `manual_sync_push` 改用 `put_if_match`（或 MOVE 前比对 ETag），冲突时中止并提示
- [ ] **S-5** 从 invoke handler 移除 `mcp_execute_tool`（`commands/mod.rs:499`），或让它走 `WRITE_TOOLS` 门禁
- [ ] **S-2** `get_push_items` / `sync_table_push` 改为真实读取 `sync_map` 的 attempts 列（或新增列），并加最大重试次数与死信视图

### P1 — 会让"成功"变成谎言

- [ ] **S-8** IMAP 白名单过滤时推进 `last_sync_uid`
- [ ] **S-9** `process_email` 的两条 INSERT 合并到同一事务
- [ ] **S-10** `list_all_bitable_records` 返回 `(records, truncated)`；`feishu_import_bitable_cases` / `feishu_import_bitable_subtable` 的 `loop` 加最大页数上限
- [ ] **S-11** `trigger_feishu_push` 在未启用时返回明确错误，或改为直接调用 `sync_feishu_push_inner`
- [ ] **S-12** `NoOpBackend` 必须返回错误，或在 `ai_runs` 上打 `backend="noop"` 标记让审计页能识别
- [ ] **S-13** `hybrid_search_knowledge` 返回 `semantic_status` + `warning`，前端两处调用点展示降级提示
- [ ] **S-15** MCP 启用状态改为失败关闭，并统一前后端默认值来源与类型
- [ ] **S-16** `case_query` filter 解析失败返回 `-32602`；`resources/read` 失败置 `isError`

### P2 — 真实邮件服务器上才会暴露

- [ ] **S-6** SMTP 报文补 `Date:` 与 `Message-ID:`；ICS `DTSTAMP` 加 `Z`；DTSTART/DTEND 加 `TZID` + `VTIMEZONE`；两处 ICS 加 75 字节折行
- [ ] **S-6** SMTP 配置补 `from_address` / `from_name` / `use_tls` 真实读取，删除 `smtp.rs:543-545` 的硬编码
- [ ] **S-7** 邮件监听在 `lib.rs setup()` 中按配置自动启动；`start()` 至少做一次连接探测再返回成功；`get_email_monitor_status` 返回连接状态与已收取邮件数
- [ ] **S-10** IMAP `stop()` 后快速 `start()` 的重复任务：用 `JoinHandle` 跟踪内层任务或改用 `CancellationToken`
- [ ] CalDAV：PUT 加 `If-Match`/`If-None-Match`；确认至少在 Google Calendar、iCloud、Outlook.com 三家各跑通一次
- [ ] WebDAV：`WebDavClient::new` 对非回环主机强制 https

### P3 — 一致性收尾

- [ ] **凭据例外 1** `save_feishu_credentials` 改走 `secure_settings_secret`，统一钥匙串 service，删除 DB 明文行
- [ ] **凭据例外 2** IMAP 钥匙串失败改为失败关闭（或至少在 UI 明示"密码未受钥匙串保护"）
- [ ] **凭据例外 3** `mcp_auth_token` 加入 `settings_secret_type`（进钥匙串）+ `save_settings` 跳过列表
- [ ] **凭据例外 4** 实现旧版明文 settings 密钥的迁移入口（IMAP 已有 `migrate_imap_passwords_to_keychain` 可作模板）
- [ ] **凭据例外 6** `settings.rs:21` 与 `settings_secret_type` 由单一表格派生
- [ ] **S-14** `system_prompt` 要么贯通全部 8 个调用点，要么在 UI 上标注"仅对聊天生效"
- [ ] **S-12 附** AI 网络层补 429 处理与重试退避；`usage.rs:48` 保留 `reqwest::Error` 的可诊断信息
- [ ] `mcp/server.rs:57` 的并发注释改为真实现实（用 `OnceLock::get_or_init` 而非 `set`）

### 真实服务验收矩阵（全部尚未执行）

| 集成 | 需准备的真实资源 | 当前状态 |
| --- | --- | --- |
| 飞书 | 自建应用 + `tenant_access_token` 权限 + 多维表格读写权限；一张字段名与 `insert_case_from_feishu`（`feishu.rs:1021-1075`）**完全一致**的表 | 从未执行。**且 S-0 未修前连 app_token 都存不下来。** 字段名硬编码（"案件信息"、"案号"、"三次开庭丨口审" 用的是异体竖线），换一张表就全部落到 `unwrap_or("")` |
| WebDAV | 支持 PUT/GET/MOVE/HEAD 的服务器（nginx / Nextcloud / 群晖） | 仅在 `webdav.rs:280-324` 的本地手写 fixture 上跑过 |
| CalDAV | Google / iCloud / Outlook 各一个账号 | 从未执行 |
| IMAP | 支持 IDLE 的服务器（QQ 163 Gmail 企业 Exchange） | 从未执行 |
| SMTP | 465 或 587 的真实账号 | 从未执行 |
| MCP | 一个真实 MCP 客户端 | 内联测试只覆盖 401/405/413/431 与写工具 pending |
| AI | 已开通的模型服务商账号 | `ai_profiles_test.rs:77-119` 用手写 TcpListener mock 过 wire format，**没有跑过真实服务商** |

**飞书还有一个隐性前置条件**：`insert_case_from_feishu` 的 32 个字段全部按**中文字段名**硬编码（`fields["案件信息"]`、`fields["案由"]`、`fields["三次开庭丨口审"]`）。`update_local_case` 同样。任何列名不完全匹配的表都会静默导入成空案件（`unwrap_or("")`，`feishu.rs:1049-1075`），只有 `案件信息` 为空时才报错（`:1022-1024`）。**这是"用户能拿到的最坏结果：一次看起来成功的导入，产出几百个空案件。**

---

## 修复建议

### 第一优先（本轮就做）

1. **S-0：把 `configure_feishu_table` 接上前端。** 约 30 行，解锁同步中心的飞书拉取/推送与整条自动推送链路。这是本轮**唯一一处"代码写好了但线没插上"**的问题，也是唯一一个改动量极小、收益极大的修复。在此之前，任何关于飞书同步的验收都是无效的。
2. **S-1 的类型修复 + 一条回归测试。** 成本最低、危害最大。测试断言：同一 `last_modified_time` 连续两次 pull，第二次 `report.skipped == 1` 且本地字段不变。
3. **把 `commands/caldav.rs:111-181` 的作业状态机复制到飞书。** `status` 枚举 + `attempts` 真实计数 + `next_attempt_at` 退避 + `delivery_unknown` 对账语义。这一块飞书已经写好了 70%（`sync_map.sync_status`、`last_synced_at`、`local_updated` 三列都在），只是没人用。一次性解决 S-2、S-4 的推送侧、以及部分失败不可见的问题。
4. **S-3 + S-5 + S-11 三个"报成功但没成功"的删除/修正。** 前者要往 `report.errors` 走；后两者是删代码。它们加起来不到 20 行，却覆盖了本轮最伤用户信任的三类问题。
5. **`import_feishu.rs` 的字段名硬编码改为配置化。** 表结构发现代码（`list_bitable_fields`、`FIELD_MAP`，`feishu.rs:728`）已经存在，把 `insert_case_from_feishu` 接到映射表上，而不是继续硬编码 32 个中文字符串。否则"真实服务验收"这一步永远无法进行。

### 第二优先（本轮或下轮）

5. **统一凭据入口。** 把 `configure_feishu`、`configure_imap`、`mcp_auth_token` 三处拉齐到 `secure_settings_secret` 的失败关闭模型，并实现旧明文迁移。这一项纯收尾，没有架构风险。
6. **给网络层加一个共享的"带退避的 HTTP 执行器"**（`usage::send` 已经是 AI 侧的唯一出口，飞书侧是 `call_feishu_api`，SMTP/CalDAV 各自手写）。统一注入：连接超时、总超时、429 + `Retry-After`、指数退避、只对幂等操作重试、错误分类（网络 / 认证 / 限流 / 服务端）。当前飞书只对 429 重试（3 次）、SMTP 与 CalDAV 完全没有 —— 这正是"生产实验会撞墙"的技术原因。
7. **引入"结果可见性"约定。** 所有批量命令的返回类型增加 `truncated: bool` 与 `degraded: Vec<String>`，前端必须渲染。这能一次性消灭 S-10、S-13 和"失败被报成成功"这一整类问题，且不需要改任何业务逻辑。
8. **MCP 纵深防御。** 连接数 `Semaphore`、写工具 ACL、`Host` 头校验、把 `mcp_auth_token` 移出 webview 可写范围。

### 第三优先（工程化）

9. **把邮件链路纳入与案件/任务同等的测试纪律。** `email/mod.rs` 762 行 + `smtp.rs` 601 行，**零测试**。可以照抄 `sync/webdav.rs:280-324` 的手写 TCP fixture 模式 —— 那套已经证明可行，而且顺带能测出 S-6 的 MIME 缺失头问题。
10. **给飞书/SyncStatusView/ImapSettings/autoPush.ts 补前端测试。** 这四个零测试的面正是本轮 S-0、S-11 得以长期存在的原因。`webdavSettings.test.ts` 已经示范了合适的写法（挂载组件 + 断言失败不报成功）。
11. **把契约门禁从单向扩成双向。** `tests/contract.commands.test.ts` 目前只保证"前端调用的命令后端都有"。加一条反向断言：Rust handler 里与集成相关的命令，要么有前端调用点，要么在源码里标注 `@unused` 及原因 —— S-0 就能被这类门禁拦下。
12. **不要在真实服务验收前相信任何"已通过"的结论。** `ai_profiles_test.rs` 的 mock 证明了作者知道怎么做端到端验证，但飞书、CalDAV、IMAP、SMTP、WebDAV 五条链路没有任何等价物。文档里所有"需真实服务验收"的措辞都应该理解为"未验收"，本报告的评估表给出了每一项的具体未覆盖面。

---

### 附：本轮核实为真、可以对外声明的事项

为免过度修复，以下几条经过代码核实与文档声称一致：

- 密钥材料**不上传**到 WebDAV / 飞书（`sync/mod.rs:95` 不变量 + `portable_backup::collect_roots` 排除规则）。
- **无任何密钥进入日志、stdout 或前端**（全量 grep 交叉核查）。
- MCP **仅绑定回环地址**（`mcp/server.rs:16` `127.0.0.1:37877`，全仓库唯一的监听 socket），**且有强制 Bearer 鉴权**（`server.rs:142-146`，在路由之前）。
- WebDAV **恢复路径**（下载 → 校验 → 维护锁 → 恢复前保护副本 → 换库）是六条链路里最严谨的一段，可以作为其他链路的失败处理模板。
- CalDAV 的 `delivery_unknown` 语义（超时后先 `get_event_etag` 对账再决定是否重 PUT）是正确的分布式投递处理，**不要在后续重构中简化掉**。

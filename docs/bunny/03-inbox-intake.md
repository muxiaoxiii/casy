# 收件箱 / 捕获域 深度审计

审计日期：2026-09-30。范围：`src-tauri/src/commands/inbox.rs`（2738 行）、`src-tauri/src/commands/inbox_actions.rs`、`src-tauri/src/watcher.rs`、`src-tauri/src/email/mod.rs`、`src/modules/inbox/**`、`src/shared/components/UnifiedCaptureDialog.vue`、`src/shared/components/HolidayImportReview.vue`、`src/core/services/inbox.ts`、`src/core/plugins/inbox-plugin.ts`。

方法：逐条读源码并把后端返回值追到 Vue 调用点。凡是能一路读完的标 **CONFIRMED**，只追到一半的标 **SUSPECTED**。

---

## 概述

收件箱域有两套并行的动作执行路径：

| 路径 | 入口 | 事务 | 回执 `inbox_action_results` | 反馈 `inbox_feedback` |
| --- | --- | --- | --- | --- |
| `inbox_actions::confirm` | `create_task` / `create_deadline` / `set_reminder` / `create_event` / `save_knowledge` / `create_case` / `create_project` / `update_holidays` | 是（`TransactionBehavior::Immediate`） | 写入 | 写入 accepted=1 |
| `file_inbox_item` / `dismiss_inbox_item` / `service_delivery` | `file_to_case` / `ignore` / `service_delivery` | 是（各自单事务） | **不写入** | 不写入 |

这个分裂是本域大多数"看起来成功、其实没落地"问题的根源。`ARCHITECTURE.md` 声称"甘特保存广播 task:changed 使任务、首页、案件与通知消费者刷新"——我全仓搜索后确认 **`task:changed` 这个事件在 `src-tauri/` 里根本不存在**，收件箱创建的任务/日程/案件同样不广播任何事件。

按用户视角，能直接观察到的断点集中在三处：归档状态可被 AI 工具回退、收件箱没有任何实时刷新、法院送达文书下载后没有进入卷宗。

---

## 严重问题

### S1-1　已归档收件项会被 `process_inbox_item` 复活，且此后无法再归档（假成功）

- **文件行号**：`src-tauri/src/commands/inbox.rs:352-364`、`src-tauri/src/commands/inbox_actions.rs:18-22`、`src/shared/components/UnifiedCaptureDialog.vue:357-366`
- **缺陷（一句话）**：`process_inbox_item` 无条件把 `status` 写回 `'pending'`，而 `confirm()` 命中已有回执时直接 `return` 而不恢复状态，于是用户在统一捕获里点"确认处理"会看到"已完成处理"，收件项却永远停在待处理。
- **触发路径**（CONFIRMED，端到端读完）：
  1. 用户在统一捕获里粘贴文本 → 存入收件箱 → 建议"转为任务" → `confirm_inbox_action` → `inbox_actions::confirm` 写入 `inbox_action_results` 并把 `inbox_items.status` 置为 `'filed'`。
  2. AI 助手（或 `inbox-plugin.ts:87-105` 注册的 L2 写工具 `process_inbox_item`）对该 id 调用 `process_inbox_item`。该命令**没有任何 status 前置检查**，`UPDATE inbox_items SET ... status = 'pending', processed_at = ... WHERE id = ?6`（inbox.rs:354）直接把它改回 pending。
  3. 用户在统一捕获里重新处理这条内容 → `confirm_inbox_action` → `confirm()` 命中 `inbox_action_results` 里那一行，`anyhow::ensure!(previous_action == action, ...)` 通过后**立即 `return Ok(已存回执)`**，既不重放写入，也不执行第 113 行的 `UPDATE ... status='filed'`。
  4. 前端 `UnifiedCaptureDialog.vue:357-360` 判定 `result.ok` 为真 → `ElMessage.success('已完成处理')` → 进入"已保存"页。
- **可观察的错误结果**：收件箱"待处理"计数 +1；点开这条已处理过的内容，再点任何建议按钮，都会反复看到"已完成处理"，但它永远不会被归档。同一文件若再走一次 `file_inbox_item`，`files/mod.rs:463-468` 的 `persist_noclobber` 会在卷宗目录里生成 `xxx_1.pdf`，产生重复卷宗文件。
- **修复方向**：`process_inbox_item` 的 UPDATE 加 `AND status = 'pending'`（或对 `filed` 直接返回错误）；`confirm()` 的回执短路分支要么重放 `status='filed'`，要么明确返回 `already_applied: true` 并让前端区分。

### S1-2　收件箱没有任何实时刷新通道

- **文件行号**：`src-tauri/src/watcher.rs:185`、`src-tauri/src/email/mod.rs:352-361`、`src/modules/inbox/views/InboxView.vue:168,175`
- **缺陷**：后端两处向收件箱写入的路径都会（或本应）通知前端，但**没有任何前端订阅这些事件**。
- **证据**（CONFIRMED）：我枚举了全仓所有 `safeListen(...)` 注册（`src/App.vue:231,236`、`ReminderToast.vue:34`、`ReminderBanner.vue:46`、`DecisionReviewNotice.vue:21`、`FileConversionDialog.vue:56`、`CaseFilesPanel.vue:30`、`KnowledgeNotebookView.vue:433`），共 7 处，事件名是 `global:quick_capture`、`tauri://drag-drop`、`reminder:triggered`、`decision:review-due`、`document-conversion-progress`、`workspace:updated`。**`inbox:new_item` 零监听**（`grep -rn "inbox:new_item" src/` 无结果）。`src-tauri/src/email/` 整个目录没有任何 `emit`。
- **触发**：
  - 把文件拖进被监视的收件目录 → `watcher.rs:144-186` 自动 INSERT `inbox_items` 并 `emit("inbox:new_item")` → 收件箱页面不刷新、待处理计数不变。
  - IMAP 监控收到新邮件 → `email/mod.rs:353` INSERT 收件项且不发任何事件 → 同上。
- **可观察的错误结果**：用户以为文件/邮件没进来，反复拖拽；实际数据已在库里。唯一的刷新手段是手动点收件箱右上角的刷新按钮（`InboxView.vue:368`）。
- **修复方向**：`InboxView` 订阅 `inbox:new_item`；`email/mod.rs` 补发同一个事件（或复用 `workspace:updated`）。

### S1-3　法院送达文书下载后不进入卷宗，收件项仍为待处理

- **文件行号**：`src-tauri/src/commands/inbox.rs:2295-2300`、`inbox.rs:1283-1301`、`src/shared/components/UnifiedCaptureDialog.vue:361-366,463`
- **缺陷**：`confirm_inbox_action` 的 `service_delivery` 分支只下载文件并改写 `source_path`/`source_type`，既不写 `case_files`、不改 `status`、也不触发 OCR/知识索引。
- **触发**（CONFIRMED）：粘贴含 `https://*.court.gov.cn/...` 的短信 → 建议"法院送达" → 确认 → `inbox.rs:1294` 执行 `UPDATE inbox_items SET source_type='file', source_path=<下载路径>`。注意这条路径**绕过了 `inbox_actions::confirm`**，所以：
  - 没有 `inbox_action_results` 回执（`get_inbox_action_result` 返回 null，`UnifiedCaptureDialog.vue:464-467` 的节假日回执分支也不会命中）；
  - 没有 `inbox_feedback`；
  - `status` 仍是 `'pending'`，`linked_case_id` 仍是 NULL。
- **可观察的错误结果**：用户看到"已完成处理"，done 页没有"查看…"按钮（`:463` 只对 create_task/create_event/create_case/save_knowledge 显示），不知道去哪。文书躺在 `Documents/…/inbox/法院送达-<id>.pdf`，卷宗里没有它，全文检索也搜不到。收件项下次打开仍在"待处理"列表里。若快捷捕获时同时拖了附件，`fileCapturedAttachments`（`:386-393`）只会归卷 `capturedIds.slice(1)`，第 0 项（短信本身）永远不会自动归卷。
- **修复方向**：`service_delivery` 分支下载成功后直接调用 `file_inbox_item(inbox_item_id, case_id, "received")`（要求 `intent.caseId` 非空，缺失时明确报错而不是假装成功）。
- **SUSPECTED**：卷宗目录的文件监听（`workspace_sync.rs:509` 发 `workspace:updated`）理论上会对新复制的文书触发 OCR/知识索引，但该链路依赖 `workspace_sync` 选项是否开启且已登记该案件目录，我未追到底，标记为待验证。

---

## 中等问题

### S2-1　`list_inbox_items` 硬编码 `LIMIT 100`，被忽略的旧项会把待处理项挤出窗口

- `inbox.rs:229` `sql.push_str(" ORDER BY created_at DESC LIMIT 100")`；`InboxView.vue:180` 调用 `casyContext.inbox.list()` 不传 status，因此不生成任何 WHERE。
- 结果：第 101 条之后的收件项永远看不到，UI 没有任何分页或"加载更多"。`InboxView.vue:367` 的 `{{ pendingItems.length }} 项待处理` 是**已加载条数**，不是真实待处理数。收件箱自动导入（watcher）+ 邮件监听的场景下 100 条很容易达到，且因为混排了 dismissed/ignored，旧的已忽略项会挤掉新的待处理项。

### S2-2　日期解析：绝对日期优先于相对日期，且接受 `.`/`/` 作月日分隔符

- `inbox.rs:2104-2116` `extract_date_hint_at` **先**调 `parse_absolute_date`（:2034）**再**调 `parse_relative_date`（:1902）；`inbox.rs:2056` 的正则 `(?:^|[^\d])(\d{1,2})\s*[\-/\.月]\s*(\d{1,2})` 把小数点当月日分隔符。
- **可复现的错例**：捕获文本「利率 3.5%，明天还本」→ 第 2 条正则命中 `3.5` → `m=3,d=5` → 今年 3 月 5 日已过 → 走 `inbox.rs:2064-2068` 的 `today.year()+1` 分支 → **dueDate = 明年 3 月 5 日**；"明天"这条相对日期因为排在绝对日期之后被完全忽略。该 `dueDate` 会写进 `create_task`/`create_event` 的 intent（`inbox.rs:1530-1534`、`1583-1589`）。
- 触发条件极广：任何含小数、版本号、"3-5万"之类字样的文本。

### S2-3　六个占位命令静默返回成功（当前无 UI 接线，但已暴露在 commandMap + service）

- `inbox.rs:2401-2416` `pause/resume/cancel_inbox_batch` → `Ok(())`
- `inbox.rs:2429-2437` `retry_inbox_item` / `retry_inbox_case` → `Ok(())`
- 这四个命令已在 `src/types/commandMap.ts:482-485` 声明、`src/core/services/inbox.ts:113-120` 暴露为公开方法。任何一处新增 UI 接线都会得到"点了没反应但没报错"的结果。目前前端确实没有调用点（`grep` 无结果），所以是**尚未引爆的地雷**而不是现行故障。详见占位清单表。

### S2-4　收件箱与卷宗文件之间没有 `source_inbox_id` 关联

- `case_files` 表有 `source_inbox_id TEXT`（`db/schema.rs:388`），但 `inbox.rs:88-100` 的 `register_inbox_case_file` 只写 `id/case_id/file_name/file_path/file_size/file_type/category/source_type`，**从不写 `source_inbox_id`**。
- 结果：卷宗里无法反查某个文件来自哪条收件项，收件项也无法列出它归档后产生的全部卷宗文件（跨类别归档时尤其明显——`file_to_case` 一次只归档一个 source_path）。审计线索断在这里。

### S2-5　法条自动导入的计数是假的

- `inbox.rs:664-674`：没有拆出任何条文时，`let _ = conn.execute(INSERT …)` **忽略错误**，然后无条件 `count = 1`。INSERT 失败（磁盘满、约束冲突）时仍返回 `Ok(1)`，上层 `inbox.rs:414-419` 报出"已自动导入 1 条法条到知识库"。
- `inbox.rs:649-661`：正常分支用 `INSERT OR IGNORE` + `.is_ok()` 计数。`rusqlite::execute` 对被 IGNORE 掉的行返回 `Ok(0)`，所以重复导入同一份法条时 `count` 仍然 +1，消息说"已导入 N 条"，实际新增 0 条。
- `inbox.rs:548` `cause_action_update` 分支同样是 `let _ = insert_knowledge_item(...)`，失败后照样 push 一条成功语义的 action。
- 这条链只能由 AI 工具 `process_inbox_item` 触发（收件箱 UI 不调用它），但一旦触发就是向用户/AI 报告假成功。

### S2-6　`execute_auto_routes` 的错误永远不会传播，`unwrap_or_default` + 注释与实现相反

- `inbox.rs:377` `let actions = route_actions.unwrap_or_default();`，上一行注释写着"路由失败回滚收件项更新"。**实际上不会回滚**：`execute_auto_routes`（:393-596）每个分支都把失败包成 `{"action":"…_failed","error":…}` 然后返回 `Ok(actions)`，所以 `unwrap_or_default()` 实际上永远走不到错误分支，注释描述的回滚语义不存在。
- 影响：当前不是活 bug（因为函数不返回 Err），但这段代码给人"失败会回滚"的错误保障认知；同时 `inbox.rs:564` 的 `&content_text[..content_text.len().min(50)]` 是**按字节**切片，中文文本在 `char` 边界外会被 panic（`execute_auto_routes` 的 note 分支）。`inbox_title`（:134-136）用的是 `.chars().take(120)`，说明作者知道要按字符切，这里漏了。**SUSPECTED**（需要 note 类目 + 非 ASCII 内容 + 走 AI 工具路径才会触发）。

### S2-7　语音速记是死胡同

- `inbox.rs:2368-2386` `save_voice_note` 真实实现（存 webm/ogg/m4a + 写 inbox_items），前端 `useVoiceNote.ts:96-98` 提示"录音原件已存入收件箱，尚未转写"——这句是准确的。
- 但 `inbox.rs:2388-2391` `transcribe_voice_note` 永远 `Err("语音转写功能开发中")`，且 `services/inbox.ts:85-87` 的 `transcribeVoiceNote` **没有任何调用点**。收件箱里那条语音速记永远是一段无法检索、无法转写的音频，`content_text` 只有一句"录音 60 秒；音频原件已保存，尚未转写。"。

### S2-8　快捷录入的案件自动关联使用 2 字包含匹配 + 全表扫描

- `inbox.rs:1493-1515`：没有案号命中时，遍历 `SELECT … FROM cases` 全表，对每个案件的 `client_name/opponent_name/display_name` 做 `text.contains(name)`，只要求 `chars().count() >= 2`。
- `inbox.rs:1507-1509`：当事人名一旦是「王某」「李某」这类两字通用名，几乎任何法律文本都会命中 → `matched_case` 被设为一个**无关案件** → `quick_judge_text` 返回的 `targetCaseId`（:1540）被 `UnifiedCaptureDialog.vue:205` 直接填进「关联案件」选择器 → 用户不核对就把内容归到错案件。
- 同时 `inbox.rs:2159-2224` `extract_parties_from_name` 第 3 步（:2215-2221）把文本里**所有 2-6 字的中文子串**当作"当事人"塞进列表，`inbox.rs:1517-1526` 随后对每一个做一次 `LIKE` 查询。一段 2000 字短信会产生上千个 LIKE 查询。这是性能问题，也会拖慢统一捕获的"存入收件箱→出建议"链路。

---

## 占位功能清单

`inbox.rs:2345-2437`，逐条核对了真实返回值和前端调用点：

| 命令 | 行号 | 实际返回 | 是否报错 | 前端调用点 | 用户实际体验 |
| --- | --- | --- | --- | --- | --- |
| `capture_screenshot` | 2346-2349 | `Err("截图捕获功能开发中，敬请期待")` | 是 | `useCapture.ts:16` | **无 UI 按钮**（`captureScreenshot` 在 `src/` 里零调用）。只有 composable 方法存在，一旦接按钮就是红色 toast，功能不存在 |
| `capture_clipboard` | 2352-2359 | **真实实现**（arboard 读剪贴板 → `add_inbox_item("paste")`） | 否 | `InboxView.vue:43-44` "粘贴文字"按钮 | **可用**。注释写着"（占位）"是错的，文档与实现不符 |
| `start_clipboard_monitor` | 2361-2364 | `Err("剪贴板监听功能开发中，敬请期待")` | 是 | `useCapture.ts:41` | **无 UI 调用点**。功能不存在且无入口 |
| `save_voice_note` | 2367-2386 | **真实实现**（存音频 + 写 inbox_items） | 否 | `useVoiceNote.ts:96`，`InboxView.vue:372` "录音"按钮 | **可用但不完整**：只存音频，永不转写（S2-7） |
| `transcribe_voice_note` | 2388-2391 | `Err("语音转写功能开发中，敬请期待")` | 是 | `services/inbox.ts:86` | **无 UI 调用点**，死代码 |
| `start_inbox_batch` | 2394-2397 | `Err("批量处理功能开发中，敬请期待")` | 是 | `services/inbox.ts:111` | **无 UI 调用点**。会正确报错 |
| `pause_inbox_batch` | 2400-2403 | `Ok(())` | **否（静默成功）** | `services/inbox.ts:114` | **无 UI 调用点**。一旦接线就是"点了暂停，提示成功，实际什么都没发生" |
| `resume_inbox_batch` | 2406-2409 | `Ok(())` | **否** | `services/inbox.ts:117` | 同上 |
| `cancel_inbox_batch` | 2412-2415 | `Ok(())` | **否** | `services/inbox.ts:120` | 同上 |
| `get_inbox_progress` | 2418-2426 | `Ok(InboxProgress{total:0,processed:0,pending:0})` | **否（假数据）** | `services/inbox.ts:123` | **无 UI 调用点**。一旦接到进度条，永远显示 0/0/0，不会报错也不会有进度 |
| `retry_inbox_item` | 2428-2431 | `Ok(())` | **否（静默成功）** | **无**（连 commandMap 都没声明） | 完全死代码 |
| `retry_inbox_case` | 2434-2437 | `Ok(())` | **否** | **无**（同上） | 完全死代码 |

补充：同一区段还有两个**未标注占位**的命令，前端同样零调用：

| 命令 | 行号 | 实际返回 | 说明 |
| --- | --- | --- | --- |
| `download_service_delivery` | 2317-2323 | 恒定 `Err("请把包含法院链接的完整短信粘贴到统一捕获…")` | 旧入口，故意禁用并引导到新流程。注释已说明，**不是隐藏地雷** |
| `process_service_delivery` | 2326-2343 | 真实实现（检测链接 + 下载） | 功能可用但**前端零调用**，只有 `confirm_inbox_action` 的 `service_delivery` 分支走同一条下载逻辑 |

**结论**：用户实际能点到的占位功能只有录音（存不转写）。其余 10 个命令的 UI 都没接线，所以"点按钮没反应"这类故障目前不会发生——但它们已经全部登记在 `src/types/commandMap.ts:470-485` 并在 `src/core/services/inbox.ts:109-124` 暴露为公开方法，随时可能被新 UI 静默接上。其中 6 个会**静默返回成功**，是必须先处理的雷。

---

## 数据链路断点

| # | 入口 | 断点位置 | 数据去哪了 | 严重度 |
| --- | --- | --- | --- | --- |
| 1 | watcher 自动导入文件 | `watcher.rs:185` emit `inbox:new_item` | 前端零监听，UI 不刷新 | 高 |
| 2 | IMAP 新邮件 | `email/mod.rs:353` 插入后无 emit | 前端零通知 | 高 |
| 3 | 邮件入库 | `email/mod.rs:334` 与 `:353` 两次 `conn.execute`，**无事务** | `email_records` 成功但 `inbox_items` 失败时，邮件在"邮件"模块可见、收件箱里没有，无任何补偿 | 中（SUSPECTED：需确认外层调用是否包了事务，我未追到调用方） |
| 4 | 法院送达下载 | `inbox.rs:2295-2300` | 文件落在 `…/inbox/`，不写 `case_files`、不改 `status`、不触发 OCR；无回执 | 高 |
| 5 | 收件项归卷 | `inbox.rs:88-100` 不写 `source_inbox_id` | 卷宗文件与收件项之间无溯源链 | 中 |
| 6 | AI 工具 `process_inbox_item` | `inbox.rs:352-364` 无 status 守卫 | 已归档项被改回 pending，随后 confirm 回执短路 → 永久卡死（S1-1） | 高 |
| 7 | 法条自动入库 | `inbox.rs:668-672` `let _ =` + `count=1` | 失败仍报"已导入 1 条" | 中 |
| 8 | 收件箱写操作的事件广播 | 全域无 `task:changed` / `inbox:changed` 事件 | `ARCHITECTURE.md` 的描述与实现不符；从统一捕获建的任务，若用户已在 `/tasks` 页则列表不刷新（`openCreated` 用 `router.push`，同路由不重新挂载） | 中 |
| 9 | 拖拽文件到应用 | `UnifiedCaptureDialog.vue:192-196` `onNativeDrop` 要求 `props.modelValue === true` | 快速捕获对话框未打开时，把文件拖进应用**完全无反应**（`App.vue:238` 派发了 `casy:file-drop`，但只有对话框打开时才接收） | 中 |
| 10 | 收件箱来源筛选 | `InboxView.vue:101-107` 提供 `wechat`/`email`/`note`/`file` | `inbox.rs:151-159` 的 `norm_source_type` 把 `wechat` 落到 `_ => "note"`，**永远不可能存出 `wechat`**；该筛选按钮恒为 0 条。`capture_clipboard` 产生的 `paste` 也不在筛选列表里，只能在"全部"看到 | 低 |

### 其他较小但确定的问题

- `InboxView.vue:394,443` 引用 `item.filePath`，但 `InboxItem`（`src/types/index.ts:248-263`）和 `InboxItemDto`（`inbox.rs:12-30`）**都没有这个字段** → 恒为 undefined，被 `item.sourcePath` 掩盖。
- `InboxView.vue:152,395` 引用 `item.caseName`，同样不在类型和 DTO 中 → 收件项列表里的案件名**永远不显示**。
- `InboxView.vue:353-361` `dismissItem`：`if (item.id)` 为假时会跳过 IPC 直接 `ElMessage.success('已忽略此项')`。实际路径上 id 恒存在，属于潜在隐患而非现行故障。
- `src/core/mockData.ts:312-326` 的 `confirm_inbox_action` mock **只实现了 `create_task`**，其余 action 返回 `undefined` → `tauriCallSafe` 返回 `{ok:true, data:undefined}`。浏览器预览下案件/知识/节假日/送达全部"成功"但什么都没做。这与 `ARCHITECTURE.md` 的告诫一致（不能用浏览器成功提示证明桌面持久化），仅记录。

---

## 修复建议（按收益排序）

1. **给 `process_inbox_item` 加 status 守卫，并修好回执短路**（S1-1）。两处小改：UPDATE 加 `AND status='pending'`，`confirm()` 的早返回分支补写 `status='filed'`。这一条单独就能消掉"收件项永远清不掉 + 卷宗出现重复文件"这一整类故障。
2. **接通收件箱的刷新事件**（S1-2）。`InboxView` 订阅 `inbox:new_item`；`email/mod.rs` 补发。约 10 行代码，直接消除"数据进来了但界面不认"的困惑。
3. **让 `service_delivery` 走完卷宗链路**（S1-3）。下载成功后若 `intent.caseId` 存在则调 `file_inbox_item(..., "received")`；不存在就返回明确错误，让前端提示"文书已下载但未归卷"，而不是"已完成处理"。
4. **把 6 个静默成功的占位命令改成显式报错**（S2-3）。`pause/resume/cancel_inbox_batch`、`retry_inbox_item/case` 全部返回 `Err("…功能开发中")`，`get_inbox_progress` 返回 `Err` 而非假 0/0/0。同时删掉 `commandMap.ts` 和 `services/inbox.ts` 里对应的 7 个声明，避免被误接线。成本 15 分钟，收益是消灭一整类未来的"假成功"。
5. **`list_inbox_items` 去掉硬编码 `LIMIT 100` 或加分页参数**（S2-1），并让 InboxView 的待处理计数来自后端聚合而非已加载数组长度。
6. **修 `extract_date_hint_at` 的优先级**（S2-2）：把 `parse_relative_date` 提到 `parse_absolute_date` 之前，并把绝对日期正则的 `[\-/\.月]` 收紧为只接受 `月`（或要求前面有"年"）。这一条会影响所有 create_task/create_event 的默认截止日期，收益很高但要补测试。
7. **补 `source_inbox_id`**（S2-4）：`register_inbox_case_file` 增加一个参数并写入 `inbox_item_id`，让卷宗文件可溯源。
8. **修法条计数的三处假成功**（S2-5）：用 `execute` 返回的 `changes()` 而不是 `.is_ok()` 计数；`let _ = conn.execute(...)` 改成 `?` 或至少把 count 置 0 并 push `*_failed`。
9. **收紧案件自动匹配的最小长度**（S2-8）：`quick_judge_text:1507-1509` 的 2 字阈值提高到 3 字以上，或要求全名匹配；同时给 `extract_parties_from_name` 第 3 步的 n-gram 兜底加长度/停用词上限，避免上千次 LIKE。
10. **修 `execute_auto_routes` 的注释与切片**（S2-6）：删掉"路由失败回滚"的错误注释，把 `inbox.rs:564` 的 `&content_text[..len.min(50)]` 换成 `truncate_text(content_text, 50)`（该函数已存在于 :2119）。

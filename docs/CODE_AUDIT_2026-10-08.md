# Casy 全面代码审计 · 2026-10-08

审计对象：接手 casy（本地优先律师案件工作台）前的完整代码审计。
基线：`cc7f440`（2026-10-04），工作区干净，schema **v43**，版本 `0.1.3-beta.1`。
方法：通读 `docs/` 全部现行文档与 `docs/bunny/` 全部材料（行动清单、总览、六份领域审计 02–07、UI/UX 方案、两份工作记录）；分模块通读 `src-tauri/src`（约 6.2 万行，含测试）、`tools/casy-doc-engine`（约 1.06 万行）、`src/` 前端（136 个 Vue + 124 个 TS，约 6.7 万行）；本机实测全部门禁；对 bunny 审计（2026-09-30 至 10-03）逐条复核并标注处置状态。

---

## 一、门禁实测（本机，2026-10-08）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| Rust 编译 | `cargo check --locked --all-targets`（src-tauri） | **exit 0**，4m55s，无警告阻断 |
| Rust 测试 | `cargo test --locked`（隔离 `CASY_TEST_DATA_DIR`） | **373 通过 / 0 失败 / 6 忽略** |
| 文档引擎 | `cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --locked --features models` | **22 通过 / 0 失败 / 5 忽略** |
| 前端类型 | `npm run typecheck`（vue-tsc --noEmit） | 无输出（干净） |
| 前端测试 | `npx vitest run --maxWorkers=2` | **372 通过 / 86 文件** |
| 脚本守护 | `node --test scripts/*.test.mjs` | **8 通过** |

对比 bunny 基线（194b314：Rust 368 / 前端 362）：净增约 5 项 Rust、10 项前端测试，全部真实通过。**门禁本身是可靠的，不要按"测试不稳定"处理**（bunny 01 记录过一次首次编译与 vitest 抢资源导致的假失败，本轮隔离复跑未复现）。

---

## 二、结论摘要

1. **工程质量高于同规模项目平均。** 迁移事务边界、连接池维护模式、AI 工具授权内核（默认拒绝 + L2/L3 确认 + 审计）、备份/恢复的工程严谨度、IPC 三方契约闸（`contract.commands.test.ts`）都是真实落地的，不是文档吹述。
2. **bunny 系列审计的高危缺陷大部分已修**（逐条核对见第四节）：收件箱状态守卫与事件接通、飞书 pull 覆盖本地编辑、MCP 写门禁绕过、任务推迟 void 陷阱、案件筛选面板失效、公式 EDATE、规则写入校验等均已关闭；OCR 裁图外置（`e9b80f1`/`be96f1b`）与迁移前快照是审计后新增的正确方向。
3. **仍有三个系统性问题**：
   - **外部服务是"半成品"而非"不可用"**：飞书重试计数恒 0、WebDAV 冲突原语零调用、SMTP/ICS 报文缺头缺折叠、IMAP 不自启——四条链路都"能编译、有界面、未跑过真实服务器"。
   - **文档管线的磁盘与内存仍无界**：`document-artifacts/` 只写不清、无断点续跑、16 MiB 上限语义未随资产外置修正。
   - **法定期限的日历覆盖与降级路径不统一**：内置日历漏 2025-10-08 且只到 2026；2027 年后的规则期限静默按"只跳周末"计算并以正常紧急度弹出。
4. **审计后新增代码与修复本身带入 10 处缺陷/残留**（manifest 非原子写、校订全量拷图、`finalizing:37` 裸字符串泄露到 UI、知识产物目录不回收、迁移快照无保留策略、completed 回退掩盖最新失败、重复邮件不推进水位等），见第五节 P2。

---

## 三、架构总览（现状实测）

### 3.1 后端（`src-tauri/src`，Rust / Tauri 2）

| 模块 | 职责 | 关键入口 |
| --- | --- | --- |
| `lib.rs` | setup：资料库锁 → 恢复中断恢复 → `open_db`/`init_db` → 后台 worker → 托盘 → 5 个全局热键 → inbox 目录监听 → MCP server → 飞书 watcher → 6 个定时调度（期限重算/早报/周报/决策复核/蒸馏/洞察）→ 提醒引擎 | `run()` |
| `db/mod.rs` | 连接复用（`ManagedConnection` drop 校验 autocommit + FK）、加密密钥、维护模式排他锁、**迁移前 `VACUUM INTO` 快照**（`init_db:433-437`） | `open_db` / `init_db` |
| `db/schema.rs` | v1–v43 逐版本事务迁移 + 条件补列段 + 种子规则 | `run_migrations` |
| `commands/`（51 文件） | IPC 面；`build_handler` 注册 347 个命令，前端 `commandMap.ts` 声明 344 条 `Cmd` | `commands::build_handler` |
| `document_pipeline.rs` | 引擎子进程调度、进度心跳、停滞保护（900s）、产物校验（源哈希/页 IR/来源映射/PDF 头） | `run_engine` / `run_revision` |
| `background_jobs.rs` | 持久化单飞 OCR worker（claim→run→persist→PageIndex），启动时中断标 failed | `start_background_worker` |
| `workspace_sync.rs` | 卷宗目录登记、变更协调、OCR/知识联动、10s tick | `start` |
| `deadline/` | 程序事件投影（procedure.rs）、规则引擎（engine.rs）、节假日日历（holidays.rs）、公式求值（formula/） | `DeadlineEngine` |
| `ai/`（17 文件） | 网关（proposal 审批 + pre_state_hash + 一次性 token）、多 profile、本地/远端 embedding、检索、PageIndex、洞察/蒸馏/报表 | `ai_chat` / `gateway` |
| `sync/` | WebDAV 客户端（含未被调用的 `put_if_match`）、飞书（鉴权/限流/表发现/双向同步/自动推送）、CalDAV | `sync_table_pull/push` |
| `email/` | IMAP IDLE 监听（UID 水位 + 白名单 + message_id 去重）、最小 SMTP + ICS | `start_email_monitor` |
| `mcp/` | 手写 HTTP/JSON-RPC，127.0.0.1:37877，Bearer 鉴权；写工具强制 pending 审批 | `server::run` |
| `docsy_engine/` | Typst 排版、DOCX 富导出、模板字段 | `export` |
| `tools/casy-doc-engine` | OCR 管线（PP-OCRv6 + Layout+）、表格/版面、可搜索 PDF、**资产外置 manifest v2** | `main.rs` |

### 3.2 前端（`src/`，Vue 3 + TS + Element Plus + Pinia）

- 唯一 `invoke` 在 `core/tauriBridge.ts:1`；全部调用受 `commandMap.ts` 泛型契约约束；长写命令由后端拥有完成语义（`invokeWithDeadline` 的 `backendOwnsCompletion` 名单）。
- 15 个服务（`core/services/`）+ 10 个业务插件（`core/plugins/`）+ `observeChanges` 域事件刷新层（80ms 合并 drain）。
- AI 工具内核 `core/plugin/context.ts`：未声明策略默认拒绝、写工具强制确认、`tool:executed` 落审计。
- 17 个业务模块（cases/tasks/calendar/files/knowledge/inbox/docs/…），21 个共享状态组件。

### 3.3 数据通路

`Vue → services/plugins → tauriBridge(CommandMap) → commands → SQLCipher/FTS5 + 文件系统`；OCR 走 `commands → document_pipeline → casy-doc-engine 子进程（stdin/stdout + progress.json 心跳 + 产物目录）`；外部服务经 `sync/email/ai` 出网。

---

## 四、bunny 审计处置核对（2026-09-30 → 2026-10-08）

口径：FIXED = 已关闭；PARTIAL = 主路径修了、残留仍在；PRESENT = 原样存在。证据行号均为现行代码。

### 4.1 收件箱（docs/bunny/03）——3 FIXED / 7 PRESENT

| # | 缺陷 | 状态 | 现行证据 |
| --- | --- | --- | --- |
| S1-1 | 已归档项被 AI 工具复活 + 回执短路 | **FIXED** | `commands/inbox.rs:274,355` 两处 `AND status='pending'` 守卫 + `ensure!(changed==1)`；`commands/inbox_actions.rs:25` 回执分支补写 `status='filed'` |
| S1-2 | 收件箱无实时刷新 | **FIXED** | `watcher.rs:186` 与 `email/mod.rs:379-382`（提交后）emit；`InboxView.vue:169` 订阅 `inbox:new_item` |
| S2-3 | 6 个占位命令静默成功 | **FIXED** | `inbox.rs:2402-2432` 全部改为显式 `Err`（含 `get_inbox_progress`） |
| S1-3 | 法院送达下载后不归卷 | **PRESENT** | `inbox.rs:2296-2301` 只下载并改 `source_path`，不写 `case_files`、不改 `status`、无回执 |
| S2-1 | `LIMIT 100` 硬编码 | **PRESENT** | `inbox.rs:229`；前端不传 status，待处理计数取已加载数组长度（`InboxView.vue:183,370`） |
| S2-2 | 绝对日期优先 + 小数点当日分隔 | **PRESENT** | `inbox.rs:2107-2112` 顺序未变；正则 `inbox.rs:2040,2057` 仍含 `\.` |
| S2-4 | 不写 `source_inbox_id` | **PRESENT** | `inbox.rs:88-100` 列清单仍无该列（列存在于 `db/schema.rs:388`） |
| S2-5 | 法条自动导入假计数 | **PRESENT** | `inbox.rs:669-674` `let _ =` + 无条件 `count=1`；`:650-662` 用 `.is_ok()` 计数，IGNORE 行也 +1 |
| S2-6 | 字节切片 panic 面 | **PRESENT** | `inbox.rs:565` `&content_text[..len.min(50)]`，`truncate_text`（`:2120`）未复用 |
| — | `item.filePath`/`caseName` 幽灵字段 | **PRESENT** | `InboxView.vue:399,400,448`；DTO 与 TS 类型均无这些字段 |

### 4.2 文档管线（docs/bunny/04）——2 FIXED / 3 PARTIAL / 3 PRESENT

| # | 缺陷 | 状态 | 现行证据 |
| --- | --- | --- | --- |
| S1 | finalizing 被 900s 停滞保护硬杀 | **PARTIAL** | 心跳已加：引擎 `tools/casy-doc-engine/src/main.rs:973-975` 每页写 `finalizing:<page>`，父进程 `document_pipeline.rs:360` 相位变化即刷新 → 主路径不再误杀。**残留**：无独立心跳字段；`main.rs:977-984`（外置 + 全量页 IR + source.map）整段零进度；单页 >15min 仍被杀；900s 常数未动（`document_pipeline.rs:370-372`） |
| S2 | base64 裁图进 SQLite / 16 MiB | **FIXED（新任务）** | 三条路径均已外置：卷宗 OCR `main.rs:526,978-981`、独立转换 `conversion.rs:118-122`、知识快照 `knowledge.rs:598-608`；读取走 `document_assets.rs:142-160`。**残留**：16 MiB 硬上限原样（`workspace_sync.rs:85`），`:87-91` 仍无上限载入全部页 `plain_text`+`regions_json`；存量未升级文档仍撞上限 |
| S3 | 不回退到上一个 completed job | **FIXED** | `workspace_sync.rs:83,67` 改 `ORDER BY (status='completed') DESC,rowid DESC`。**残留**：无 completed job 时仍是英文裸错误（`commands/mod.rs:59-62` 直接 `to_string()`） |
| S4 | `document-artifacts/` 只写不清 | **PRESENT（加重）** | `document_pipeline.rs:245-263` 只建不删；全仓 `remove_dir_all` 不覆盖产物目录；每次校订（`document_intelligence.rs:251-252`）、每次存储优化（`document_assets.rs:182`）各留一套 |
| S5 | 停滞保护吞错 + 文字路径无保护 | **PRESENT** | `document_pipeline.rs:351-352` 两处 `if let Ok` 仍静默；`:275-284` 文字路径仍无 timeout/取消检查 |
| M3 | DOCX 非原子 + 秒级命名 | **PRESENT** | `docsy_engine/export.rs:48` 裸 `fs::write`；`:187-188` 秒级时间戳 |
| M6/M7 | 重命名不挡 queued / 扫描无上限 / 全表载入 | **PRESENT** | `commands/files.rs:879,584-617,963-977` |

### 4.3 领域与数据库（docs/bunny/06）——3 FIXED / 2 PARTIAL / 6 PRESENT

| # | 缺陷 | 状态 | 现行证据 |
| --- | --- | --- | --- |
| C1 | 迁移无前备 + 单事务全有全无 | **PARTIAL** | 前备已加：`db/mod.rs:433-437` → `migration_snapshot()`（`:458-483`，`VACUUM INTO` + 0600 + `sync_all`）。**残留**：条件段仍是单事务（`schema.rs:3591-3889`）；快照无保留策略/无恢复入口（新缺陷 N-4） |
| C2 | 内置日历漏 2025-10-08、只到 2026 | **PRESENT** | `deadline/holidays.rs:56-62` 假日止于 10-07；`entries_for_year`（`:177-202`）2027+ 为空 |
| C3 | `holidays_json` 损坏三路径降级不一致 | **FIXED** | 三处均改为传播错误（`procedure.rs:109-114`、`formula/mod.rs:195-198`、`engine.rs:71-74`）。**注意**：这带来新风险——损坏时整个期限面硬失败（见 P1-26） |
| C4 | 规则引擎无"年份未覆盖"标记 | **PRESENT** | `engine.rs:119-149` 无 `entries_for_year` 检查；procedure 路径有（`procedure.rs:723-730`） |
| D1 | 待核对日期派发 R1/R2 | **PRESENT** | `commands/reminder.rs:255-269`：`needs_review` 只改标签不改级别 |
| D2 | 程序期限时段外提醒静默丢弃 | **PRESENT** | `reminder.rs:264` `continue`；`:267` `cal_ctx` 传 `None`，延迟派发不可达 |
| D3 | `EDATE(d,-1)` 返回自身 / 大月数 panic | **FIXED** | `formula/eval.rs:231-232` 范围校验 + `i32`；`add_months_clamp`（`:406-410`）改 `checked_*`；测试 `:491-517` |
| D4 | 10 条种子规则硬编码跳过 | **PRESENT** | `engine.rs:95`；`deadline_rules.rs:154-179` 仍允许编辑 + 审计 + 重算 |
| D5 | 规则写入不校验 offset/trigger/calc | **FIXED** | `deadline_rules.rs:137-149` 白名单 + 范围 + 枚举校验 |
| R1/R2 | 重复链停止 / 软删不留后继 | **PRESENT** | `task_lifecycle.rs:115,150-165`；`commands/tasks.rs:435-467` |
| F1/F2 | `delete_case` 孤儿 + 跨案父子链 | **PRESENT** | `db/cases.rs:508-531`：仅清同案 `parent_task_id`；`procedure_audit`/`procedure_item_states`/`procedure_reminder_receipts`/`reminder_log.case_id` 无清理 |

### 4.4 前端（docs/bunny/02）——9 FIXED / 2 PARTIAL / 2 PRESENT / 1 变化

| # | 缺陷 | 状态 | 现行证据 |
| --- | --- | --- | --- |
| 3.1-1 | 案件筛选面板整体失效（+5 个死筛选） | **FIXED** | `CaseListView.vue:776` 绑 `casesStore.setFilter`；`stores/cases.ts:120-124` assign+loadCases；三层（store 转发 `:139-143` / Rust `db/cases.rs:100-104,234-238` / `bindings.ts:59`）全部打通 |
| 3.1-3 | 任务推迟 void 陷阱 | **FIXED** | `TasksView.vue:577-597` 改 `tauriCallSafe` + `.ok` 判定 + 失败提示 + 关弹窗刷新 |
| 3.1-4 | CopilotSidebar `toggleExpand` 不存在 | **FIXED** | `CopilotSidebar.vue:51,106,160` 改 `emit('toggle-expand')` |
| 3.1-5 | `deadlineDate` 字段错配 | **FIXED** | `CalendarView.vue:659` 改 `dueDate` |
| 3.1-5b | `task.caseName` 恒「常规待办」+ 案件名搜索恒 0 | **PRESENT** | `CalendarView.vue:546,755,1270,1448,1559`；`TaskDto`（`commands/tasks.rs:18-60`）无 `case_name`，`loadTasks` 不做富化 |
| 3.1-6 | 首页承诺复选框弹回 | **FIXED** | `HomeView.vue:287,367` 改 `completed` 布尔/`===1`；`:292-300` 判 `result.ok` |
| 3.1-7 | reveal 假成功 | **FIXED** | `CaseListView.vue:582-587` 判 `result.ok` |
| 3.1-8/9/10 | 关系网络图标 / 看板审计日志 / saveGoal 静默 | **FIXED** | `CaseNetworkView.vue:172-174,134`；`KanbanView.vue:189-202`；`CaseDetailView.vue:348-352,477-484` |
| 备忘绕过 store | **FIXED**；case 事件订阅 | **PARTIAL** | `CaseListView.vue:479-483` 走 store；但 `:651-655` 仍只订阅 `inbox:confirmed` |
| 导出 CSV 死按钮 | **PRESENT** | `CaseFilterBar.vue:397` emit 无父级监听；`export_cases` 零调用 |
| 自定义透视重启即丢 / TodayResetDialog / CaseGroupPanel | **PRESENT** | `stores/tasks.ts:364` 零调用；`TasksView.vue:26,97` 不在 template；`CaseGroupPanel.vue` 零 import |
| 命令面变化 | **+6** | `commandMap.ts` 现有 **344** 条 `Cmd`（审计时 338）；`feishu_sync_pull/push` 已接通（`SyncStatusView.vue:229,250`），其余 28 条死命令仍在 |
| e2e 不进 CI | **PRESENT** | `vitest.config.ts:10` 只收 `tests/**/*.test.ts`；17 个 `.mjs` 零收集；`ci.yml` 不引用 |

### 4.5 外部服务（docs/bunny/05）——5 FIXED / 1 PARTIAL / 4 PRESENT

| # | 缺陷 | 状态 | 现行证据 |
| --- | --- | --- | --- |
| S-1 | 飞书 pull 必丢本地修改 | **FIXED** | `sync/feishu.rs:960-971` 双形态版本解析 + 缺失即 bail；`:999-1005` 本地修改冲突检测；事务双路径提交；回归测试 `:2305-2323` |
| S-3 | 导入吞 hearings/clients 失败 | **FIXED** | `import_feishu.rs:839-862` 改 `tx.execute(...)?`；全文件零 `let _ =` |
| S-5 | MCP 写门禁被绕过 | **FIXED** | `mcp/mod.rs:910-919` 先查 `WRITE_TOOLS` 再路由 pending；`tests/mcp_direct_write_gate_test.rs` 端到端（无行写入 + pending + 审计 + 双重批准不重复执行） |
| S-9 | 托盘菜单点了没反应 | **FIXED** | `tray.rs:37-56` 显示/聚焦/导航 + emit；`App.vue:236-254` 三个监听齐 |
| S-0 | 飞书 app_token/table_id 不持久 | **PARTIAL** | 持久化已修（`commands/sync.rs:263-275` 事务 + 拒绝空值；`FeishuSettings.vue:199` 调用点存在）。**残留**：`configured` 徽章仍只看 app_id/secret（`commands/sync.rs:228` → `SyncStatusView.vue:352-355` 对"有 app 无 table"显示已配置）；`autoPush.ts:55-63` 失败仍完全静默 |
| S-2 | 重试计数恒 0 | **PRESENT** | `sync/feishu.rs:1334,1337` `SELECT …,0 FROM sync_map`；`sync_map` 无 attempts 列（`db/schema.rs:612-624`）→ `attempts` 永远 0，失败记录每次同步重复 PUT 无退避 |
| S-4 | `put_if_match` 零调用 | **PRESENT** | `sync/webdav.rs:225-238` 全仓唯一出现；活路径 `sync/mod.rs:170-171` 无条件 put+MOVE；"保留本地"即无条件覆盖 |
| S-6 | SMTP 无 Date/Message-ID；ICS 无 TZID/折行/DTSTAMP 非法 | **PRESENT** | `email/smtp.rs:208-240`（两个头 grep 零命中）、`:71` 浮动 DTSTAMP、`:116-121` 无折行；`sync/caldav.rs:370-371,379-384` 同样问题且 `:416` **断言锁死错误形态** |
| S-7 | IMAP 不自启 + 启动不验证 | **PRESENT** | `lib.rs` setup 无 `start_email_monitor`（仅注册）；`email/mod.rs:443-477` 只查 running 标志，状态 API 无连接态 |

---

## 五、现存缺陷清单（按严重度，均为现行代码实测）

### P0 — 数据风险或核心功能不可用

1. **飞书失败记录无退避、无可见队列**（S-2）：`sync/feishu.rs:1334,1337,1247,1293`。一条永久失败的记录在每次同步/防抖窗口都被重新 PUT；`sync_status` 永远回写 `local_newer`。修需先给 `sync_map` 加 `attempts` 列（迁移），不是一行改。
2. **WebDAV 冲突保护原语未被接线**（S-4）：`sync/webdav.rs:225-238` vs `sync/mod.rs:170-171`。用户点"保留本地"= 无条件覆盖远程。
3. **SMTP/ICS 报文不合规**（S-6）：`email/smtp.rs:208-240,71,116-121`、`sync/caldav.rs:370-371,379-384`。真实服务器上大概率进垃圾箱或被拒；`caldav.rs:416` 的断言让修复必须同步改测试。
4. **`document-artifacts/` 无回收**（S4）：`document_pipeline.rs:245-263`、`background_jobs.rs:104`、`document_intelligence.rs:251-252`、`document_assets.rs:182`。100 份卷宗 × 2.5 次尝试 × 80 MB ≈ 20 GB 永久占用且无 UI 提示；磁盘写满后任务级联失败。
5. **法院送达文书不归卷**（S1-3）：`inbox.rs:2296-2301`。用户看到"已完成处理"，文书躺在 inbox 目录，卷宗/检索/回执全无。
6. **法定期限日历缺口三连**（C2/C4/D1/D2）：`holidays.rs:56-62`（漏 2025-10-08）、`engine.rs:119-149`（2027+ 无 `needs_review` 标记，静默按只跳周末计算）、`reminder.rs:255-269`（待核对日期以 R1 红色弹出）、`reminder.rs:264`（时段外 R1/R2 静默丢弃，关机即永久丢失）。
7. **迁移条件段单事务**（C1 残留）：`schema.rs:3591-3889`。已有前备快照兜底（`db/mod.rs:458-483`），但快照无保留上限（见 P2-N4），且单点失败仍会锁死启动直到人工干预。

### P1 — 功能缺陷/假成功/性能

8. 收件箱 `LIMIT 100` + 待处理计数虚假：`inbox.rs:229`、`InboxView.vue:183,370`。
9. 日期解析绝对优先 + 小数点分隔：「利率 3.5%，明天还本」→ 明年 3 月 5 日：`inbox.rs:2040,2057,2107-2112`。
10. `source_inbox_id` 不写，卷宗↔收件项溯源断裂：`inbox.rs:88-100`。
11. 法条自动导入假计数（失败也报"已导入 1 条"）：`inbox.rs:650-674`。
12. `execute_auto_routes` note 分支字节切片 panic：`inbox.rs:565`。
13. 停滞保护语义：`if let Ok` 吞错无日志 + 900s 硬编码 + `main.rs:977-984` 零进度段 + 文字路径无超时/取消：`document_pipeline.rs:351-352,370-372,275-284`。
14. DOCX 导出非原子 + 秒级命名覆盖：`docsy_engine/export.rs:48,187-188`。
15. `files.rs` 三件套：重命名不挡 `queued`（`:879`，排队任务因改名凭空失败）、扫描无上限（`:584-617`）、知识引用全表载入在写事务内（`:963-977`，阻塞全应用写）。
16. IMAP 不自动启动且状态不可观测：`lib.rs` setup、`email/mod.rs:443-477,757-772`。
17. 飞书"已配置"徽章不反映 table 配置 + autoPush 失败静默：`commands/sync.rs:228`、`SyncStatusView.vue:352-355`、`autoPush.ts:55-63`。
18. 前端遗留 P0 级：导出 CSV 死按钮（`CaseFilterBar.vue:397`）、`task.caseName`（`CalendarView.vue:546,755,1270,1448,1559`）、case 事件不订阅（`CaseListView.vue:651-655`）、自定义透视重启即丢（`stores/tasks.ts:364`）、`TodayResetDialog`/`CaseGroupPanel` 不可达、`loadTodayTasks` 死 IPC（`CalendarView.vue:832-837`）、侧栏搜索无防抖（`CaseListView.vue:794`）、⌘K 任务无深链（`GlobalSearch.vue:102`）。
19. e2e 17 脚本零 CI 覆盖：`vitest.config.ts:10`、`ci.yml:34-37`。
20. 重复任务链"完成→撤销→再完成"停止 + 软删源任务不留后继：`task_lifecycle.rs:115,150-165`、`commands/tasks.rs:435-467`。
21. `delete_case` 四表孤儿 + 跨案父子链可致删除永久失败：`db/cases.rs:508-531`。
22. `ADD(date,n)` 溢出 panic（EDATE 已修、这里漏了）：`formula/eval.rs:314,317`。
23. 提醒引擎单点失败丢整轮：`reminder.rs:240,254`（`check_procedure_deadline_rules` 在规则循环内 `?` 上抛）。
24. `engine.rs:95` 跳过的 10 个种子 id 恰为全部 statutory 规则 → 有 `stay_date` 的案件规则期限为零且无标记：`engine.rs:88,95`、`schema.rs:3925-4108`。
25. OCR 批量入口（`ocr_all_pending` 等 3 条）与看板/关系网络无 UI 入口：URL-only / 后端零调用。
26. **新风险**：`holidays_json` 损坏从"静默降级"变为"期限面整体硬失败"：`engine.rs:71-74` → `commands/mod.rs:76-84`、`lib.rs:331-336` 每日重算同样中断，无用户可操作的修复路径。

### P2 — 审计后新增代码带入的缺陷（2026-10-04 基线）

- **N1** `ProcessingCenter` 显示裸 `finalizing:37`：引擎心跳复用 phase 字段（`main.rs:974`），父进程原样落库（`document_pipeline.rs:363-365`），`ProcessingCenter.vue:42` 无前缀匹配（`FileConversionDialog.vue:114` 已兼容）。
- **N2** 资产 manifest 非原子写：`tools/casy-doc-engine/src/assets.rs:88` 裸 `fs::write`（同文件其他写路径都做了 `create_new`+`sync_all`）；截断的 manifest 会使 `assets::load` 全部失败（`document_pipeline.rs:547`、`document_assets.rs:99,156`、`knowledge.rs:603`），存量文档也无法再被存储升级修复。
- **N3** 每次区域校订复制全量裁图：`document_intelligence.rs:251-259` 在 `<job id>/` 下 `copy_to` 整套 PNG，60 页卷宗每次校订一份全量拷贝，且无回收（叠加 P0-4）。
- **N4** 迁移快照无保留策略：`db/mod.rs:458-483` 每次版本升级在 `backups/` 留一份完整库副本，无上限、无清理、无恢复入口。
- **N5** `workspace_sync` 回退掩盖最新失败：`workspace_sync.rs:83,67` 的 `error` 取自已完成 job，用户看不到"最近一次重试/校订已失败"（bunny 原文建议未实现）。
- **N6** 重复邮件不推进 `last_sync_uid`：`email/mod.rs:307-319` 命中 message_id 去重即返回，水位不前移 → 同一封邮件每个 IDLE 周期重复拉取解析（白名单路径 `:287` 已正确前移）。
- **N7** `process_inbox_item` 可重复触发自动路由：`inbox.rs:352-364` 处理后会写回 `status='pending'`（语义即"分析后仍待确认"），AI 工具重复调用会重复建开庭任务/知识条目。
- **N8** `dismiss_inbox_item` 无 status 守卫：`inbox.rs:1135-1152` 可把已归卷（filed）项改回 ignored。
- **N9** `ProposalDiffCard.vue:153-157`：`reject_ai_proposal` 是 `null` 返回命令，`if (ok !== null)` 恒 false，"已拒绝"永不显示。
- **N10** 前端同类新缺陷：`HomeView.vue:285` 读不存在的 `t.category`；`CaseFilesView.vue:211-219` 案件头失败静默；`InboxView.vue:192-199` 失败静默 + 案件下拉 50 条截断。

---

## 六、文档与代码不一致（需在文档更新中处置）

| # | 文档 | 文档表述 | 代码实测 |
| --- | --- | --- | --- |
| 1 | `ARCHITECTURE.md:42`、`DATA_AND_SECURITY.md:66`、`README.md:13`、`Casy-STATUS.md:3` | schema **42** | `db/schema.rs:5` = **43**（v43 `draft_versions` + 快照触发器，`schema.rs:4758-4774`） |
| 2 | `ARCHITECTURE.md:37` | 121 个 Vue、CommandMap 330 个 `Cmd`（2026-09-22） | 136 个 Vue、344 个 `Cmd`、347 个已注册命令（`build_handler`） |
| 3 | `ARCHITECTURE.md:66` | "甘特保存广播 task:changed 使任务、首页、案件与通知消费者刷新" | `task:changed` 是前端 ctx 事件（`services/calendar.ts:67`、`services/tasks.ts:58`）确实存在；但收件箱确认只发 `inbox:confirmed`（`services/inbox.ts:72`），TasksView/CalendarView 不消费 → 统一捕获建的任务在 `/tasks` 页不刷新。bunny 03 断点#8"`task:changed` 在后端不存在"的表述不准确，结论（该路径不刷新）成立 |
| 4 | `DOCUMENT_PIPELINE.md:68` | "后端保留 15 分钟无页数或阶段变化的停滞保护；不是整个文件只能处理 15 分钟" | 基本成立（finalizing 心跳已加），但 `main.rs:977-984` 外置+页 IR 段仍零进度、单页 >15min 仍误杀；文档未反映心跳机制 |
| 5 | `DOCUMENT_PIPELINE.md:32` | "`source.md` 内部裁图仍可包含 base64" | 新任务已全部外置（`main.rs:526,978-981`）；存量未升级文档仍含 base64；文档未反映外置与升级/回退入口（`optimize_document_storage`） |
| 6 | `Casy-STATUS.md:28`、`RELEASE_0.1.3.md` | 测试数字为 09-23 现场 | 现行实测见本文件第一节（应作为新基线记录） |
| 7 | bunny 06 称 `year_range()` 无人调用 | — | `commands/settings.rs:116` 有调用（ bunny 该条不准确，不影响结论） |
| 8 | `CODE_REVIEW_2026-09-22.md` R-06 | "其他长写完整结果恢复仍需补齐" | 仍成立（无 resume 路径，`processing.rs:18-24` 仅标 failed） |

---

## 七、修复优先级建议

**P0（1–2 天，堵数据风险）**
1. 飞书 `sync_map` 加 `attempts` 列 + 指数退避 + 失败队列可见（迁移 + `sync/feishu.rs:1247,1293,1334,1337`）。
2. `put_if_match` 接线：`resolve_keep_local/keep_remote` 与手动推送改带 `If-Match`（`sync/mod.rs:170-171`）。
3. SMTP 补 `Date:`/`Message-ID:`；ICS 补 `VTIMEZONE`/`TZID`/75 字节折行/`DTSTAMP` 加 `Z`；同步修 `caldav.rs:416` 断言（`email/smtp.rs:208-240`、`sync/caldav.rs:346-416`）。
4. 产物保留策略：失败/取消即清（无其他 completed job 引用时）+ 启动扫描 + 设置项展示占用（`background_jobs.rs:78`、`document_intelligence.rs:171`）。
5. `service_delivery` 归卷：`intent.caseId` 缺失即明确报错，否则调 `file_inbox_item(..., "received")`（`inbox.rs:2296-2301`）。
6. 期限三连：补 `2025-10-08`；`engine.rs` 对未覆盖年份强制 `deadline_source="unconfirmed_calendar"` + `[待核对]` 前缀；`reminder.rs` 对待核对项跳过 R1/R2 并接入 `defer_reminder_job`（`reminder.rs:255-269`）。
7. 迁移快照加保留上限（如只留最近 3 份）（`db/mod.rs:458-483`）。

**P1（3–7 天）**
8. 收件箱：`LIMIT 100` 改分页/参数；待处理计数走后端聚合；日期解析相对优先 + 收紧分隔符；补 `source_inbox_id`；法条计数用 `changes()`；`:565` 换 `truncate_text`（`inbox.rs`）。
9. `document_pipeline`：900s 改"心跳静默超时"语义；`:351-352` 换 `match` + `tracing::warn!`；文字路径包 timeout + 取消检查（`:275-284`）。
10. `export.rs` 原子写 + `persist_noclobber`（`:48,187-188`）。
11. `files.rs`：`:879` 含 `queued`；`:584-617` 加上限；`:963-977` 改游标逐条 UPDATE 或移出写事务。
12. IMAP 启动自启 + 状态 API 报告连接态（`lib.rs`、`email/mod.rs:443-477`）。
13. 前端 P0 清单（第 18 项）与 N9/N10；`Cmd<P,void>` 类型陷阱的制度性修复（bunny 02 P2-17）。
14. e2e 以 `CASY_E2E=1` 门控进 CI，或撤下文档中"端到端"措辞。
15. 任务链/孤儿/`ADD` 溢出/reminder tick 隔离/种子规则数据驱动化（第 20–24 项）。

**P2（结构性，按 bunny 10 方案推进）**
16. 组件层（8–10 个原语）→ 断点收敛到 3 个 → 四态落地 → 再视觉升级；顺序不可颠倒（先接通死 UI）。
17. 断点续跑与完整任务恢复（R-02/R-06）仍为开放式架构项。

---

## 八、验收边界（本轮未验证，不得推断为通过）

- 未联调任何真实外部服务（飞书 / WebDAV / CalDAV / IMAP / SMTP / 远程 AI）；现有覆盖均为本地 TCP fixture 或纯函数。
- 未做 100/500 页扫描 PDF 压力、磁盘满、断电恢复、SIGKILL 残留清单。
- 未做 Windows 原生安装/运行验收；未触发远端 CI、Clippy 全量、依赖 audit、公证。
- 测试结构盲区仍在：e2e 为零；字段名错配类（`caseName` 类）与 `void` 陷阱类无任何测试能发现；L2/L3 确认弹窗与 `tauriCallSafe` 错误分支零覆盖（bunny 02 §7.4 结论仍成立）。
- 本审计未读取或核验用户私有案件数据；所有结论来自源码与本机隔离测试。

---

*审计执行：2026-10-08。证据行号对应 `cc7f440`；后续提交请以本文档为基准重新核对。*

---

## 2026-10-08 修复处置记录（同日执行）

范围：本审计第七节 P0 全部 7 项、P1 第 8–15 项与第 18–24 项、P2 新缺陷 N1–N10；bunny 10 UI/UX 升级方案与 R-02/R-06 架构项不在本轮（见下）。基线：`cc7f440` → 本轮提交。

**门禁实测（修复后）**：`cargo check --locked --all-targets` exit 0；Rust **401 通过 / 0 失败 / 6 忽略**；引擎 **23 通过 / 5 忽略**；前端 **377 通过 / 87 文件**，类型检查干净；脚本 **8 通过**；`export_bindings` 无漂移。

| 审计项 | 处置 | 落点 |
| --- | --- | --- |
| P0-1 飞书重试退避 | schema v44 幂等补列 `sync_map.attempts/last_attempt_at`；退避窗口 1→60 分钟；5 次后 `push_failed` 并进入 24 小时重试队列；成功重置计数 | `db/schema.rs`（v44 + 条件补列）、`sync/feishu.rs` |
| P0-2 WebDAV 条件上传 | 新增比对时 ETag 基线（`webdav_observed_etag`）；手动推送与保留本地/远端走 `put_if_match`，412 返回中文冲突错误；HEAD 失败降级并记日志 | `sync/webdav.rs`、`sync/mod.rs`、`commands/sync.rs` |
| P0-3 SMTP/ICS 合规 | 补 `Date:`/`Message-ID:`；DTSTAMP/DTSTART/DTEND 统一 UTC（Z）；75 字节折行覆盖属性名前缀、不切断多字节字符；修正断言错误形态的测试 | `email/smtp.rs`、`sync/caldav.rs`、`tests/calendar_delivery_test.rs` |
| P0-4 产物回收 | 失败/取消即清（无其他 completed job 引用同 file_id+sha 时）；启动扫描删除孤儿与可回收目录；completed/queued/running 一律保留 | `background_jobs.rs`、`document_intelligence.rs` |
| P0-5 送达归卷 | `service_delivery` 缺少 caseId 明确报错，否则下载后走 `file_inbox_item(..., "received")` 归卷 | `commands/inbox.rs` |
| P0-6 期限日历 | 补 2025-10-08；`covers_year`；规则引擎对未覆盖年份输出 `unconfirmed_calendar` + `[待核对]`；待核对项不再派 R1/R2（降级通知中心）；程序期限时段外走 `defer_reminder_job`；单规则失败不丢整轮 | `deadline/{holidays,engine,procedure}.rs`、`commands/reminder.rs` |
| P0-7 快照保留 | 迁移前快照仅保留最新 3 份 | `db/mod.rs` |
| P1-8 收件箱 | `list_inbox_items` 加分页参数；新增 `count_inbox_items` 后端聚合（徽标真实）；日期解析相对优先、小数点不再是分隔符、数量单位不误判；`source_inbox_id` 溯源；法条计数用 `changes()`；字节切片改 `truncate_text`；自动路由仅首次处理执行；dismiss 不覆盖 filed | `commands/inbox.rs`、`commandMap.ts`、`services/inbox.ts`、`InboxView.vue` |
| P1-9 停滞保护 | 进度读取失败改 `tracing::warn!`；文字路径 600s 超时 + 取消检查；引擎 finalizing 尾部逐页心跳 | `document_pipeline.rs`、`tools/casy-doc-engine/src/main.rs` |
| P1-10/11 | DOCX 原子写 + 序号不覆盖；重命名挡 queued；扫描 10 万上限；知识引用改先取 id 再逐条 UPDATE | `docsy_engine/export.rs`、`commands/files.rs` |
| P1-12/13 | IMAP 启动自启 + 逐账号连接探针与状态；飞书 configured 徽章反映 table；autoPush 失败可见 | `lib.rs`、`email/mod.rs`、`commands/sync.rs`、`src/core/autoPush.ts` |
| P1-14/18 前端 | 导出 CSV 接线；`task.caseName` 富化；case 事件订阅；自定义透视启动加载；删死 IPC 与死 UI 声明；侧栏搜索防抖；⌘K 任务深链 | `src/**`（含 5 个新测试） |
| P1-15 e2e | `npm run test:e2e` + CI `CASY_E2E=true` 门控步骤 | `package.json`、`.github/workflows/ci.yml`、`docs/DEVELOPMENT.md` |
| P1-20–24 | 重复链可再生（后继已删时）；软删源任务连带软删后继；`delete_case` 清理四张孤儿表 + 跨案父子链；`ADD` 溢出防护；种子规则跳过改为数据驱动（settings 名单 + 无 update 审计） | `task_lifecycle.rs`、`tasks.rs`、`db/cases.rs`、`formula/eval.rs`、`deadline/engine.rs` |
| P2 N1–N10 | ProcessingCenter finalizing 前缀；manifest 原子写；校订拷图跳过已存在；知识产物目录随删除清理；回退带最新 job 错误；重复邮件推进水位；ProposalDiffCard void 陷阱；HomeView/CaseFilesView/InboxView 新缺陷 | 对应模块 |

**未纳入本轮（明确遗留）**：bunny 10 的组件层/断点/四态/视觉升级（多周工程，需先完成上述接通）；R-02 有界内存与 R-06 断点续跑（架构项）；真实外部服务联调、100/500 页压力、Windows 原生与公证验收（验收边界不变）。

---

## 2026-10-08 UI/UX 升级与 R-02/R-06 处置记录

**UI/UX（bunny 10 第 1–4 步，提交 `05bc426`）**：组件层落成（`src/shared/ui/` 9 原语 + `ui-utils.css` 全局工具类，重复签名 120→56 组）；断点收敛为 900/1100/1360 三档（38 条 <800 死规则删除）；四态落地（主工作面全部失败路径接 DegradedBanner/el-alert + 重试，内联空态替换为 EmptyState 并区分“空/无结果”，三处首屏骨架屏）；期限视觉通道（`deadline-channel.css` + `DeadlineChip`，R1–R4 + △待核对覆盖级别）、危险动作分级（`.btn-danger`）、列表键盘流（`useListNavigation`，j/k/Enter/Esc）、密度 token。门禁：typecheck 干净、vitest 377/87。

**R-02 有界内存（本轮）**：
- 引擎结果不再携带页面集合：`ProcessResult.pages` → `page_count`，页面只落盘（`source.document.json`），stdout 只回传路径与摘要。
- 父进程 `stream_disk_pages` 以 SeqAccess 逐页流式读取；`validate_result` 的逐页校验、资产校验、Markdown 哈希、来源映射比对全部改为流式（`SourceMapBuilder` 逐页 append）；`background_jobs::persist_success` 逐页流式落库；`conversion.rs` 页数取摘要。同数据 3–4 份驻留消除为“内存中只有一页 + 来源映射累计文本”。
- `workspace_sync::get_workspace_document` 的 `regions_json` 增加 32 MiB 总预算，超预算页只保留纯文本（续篇检测降级并记日志）。
- 残留（未纳入本轮）：引擎识别阶段仍累积 `Vec<Page>` 至 finalize；可搜索 PDF 的 lopdf 整份载入；来源映射 `text` 全量驻留。三者需引擎逐页识别落盘 + PDF 流式写才能彻底消除，列入后续架构项。

**R-06 长任务恢复与结果查询（本轮）**：
- schema v45 新增 `document_job_results`（job_id 主键，file_id+source_sha256 索引）：完整产物清单（页 IR/Markdown/可搜索 PDF/来源映射路径）+ 页数 + 引擎 + markdown 哈希，与任务完成同事务落库。
- 新命令 `get_document_job_result(job_id)`：窗口断开、重启后均可查询持久化结果；commandMap/service 契约已更新，bindings 重新生成。
- 幂等重试：`retry_document_job` 对失败/已取消任务，若同一文件同一哈希已有完成结果则直接复用（返回 `reused:true`），不重跑；处理中心与案件文件页按复用给出不同提示。显式重跑已完成任务仍会重新识别。
- 崩溃恢复：启动时（产物回收扫描之前）对 `INTERRUPTED` 任务尝试用已落盘产物直接完成——校验源文件哈希、流式读完页 IR 后重建结果并落库，不重跑 OCR；每次启动最多 20 个。
- 磁盘满：`persist_failure` 既有错误码透传保持不变；结果清单与任务完成同事务，不会出现“任务完成但无结果记录”的半状态。

---

## 2026-10-08 R-02 引擎侧收尾与 R-06 断点续算

**引擎侧有界内存（R-02 残留消除）**：识别阶段不再累积 `Vec<Page>`——新增流式页 IR 写入器 `PageIrWriter`（创建/续写/收束，逐页 flush + sync），`recognize` 识别一页即落盘一页并返回页数；`add_search_layer` 与引擎侧来源映射/Markdown 生成改为从页 IR 流式读取（`stream_disk_pages`/`count_disk_pages`，SeqAccess 实现），`native_text` 判定也流式统计。校订（revise）路径同样先逐页落盘再流式 finalize。lopdf 文档整份驻留仍是已知残留（可搜索 PDF 重建的行内性质），来源映射的累计文本驻留随之消除。

**断点续算（R-06 残留消除）**：`ProcessRequest` 新增 `resume_from`；worker claim 时若同一任务产物目录已有部分页 IR（`count_disk_pages`），从第 N+1 页继续识别（跳过已落盘页，不重跑 OCR）；`retry_job` 把失败任务未完成的部分产物目录移交给新任务 id 作为续算起点；引擎侧 `PageIrWriter::open_append` 去掉收尾 `]` 后续写，页数不一致或 JSON 截断直接报错。崩溃恢复（产物写全即完成）与断点续算（产物写了一半就续算）由此形成完整分层。

新增守护测试：引擎侧流式页 IR 往返（create/push/finish/count/stream/侧车半截续写/无标记未收束报错——首轮即抓到缺分隔符逗号的真实 bug）、父进程 count 与 stream 一致性。

启动崩溃恢复路径此前零覆盖（每次启动都会改写任务状态），已补故障注入集成测试 `src-tauri/tests/job_recovery_test.rs`：产物写全的中断任务被恢复为 completed（页 IR 写入 document_pages、结果清单落库、卷宗状态翻转），无产物的中断任务保持 failed 交由重试/续算。

**本机真实 OCR 实测（2026-10-08，release 引擎 + 完整模型）**：100 页峰值 RSS 1311 MB / 220.1s，500 页 1136 MB / 800.8s——页数 5 倍而峰值不增（模型与 lopdf 为固定项），磁盘产物线性增长（3→14 MB）。断点续算端到端：把 100 页产物截断为“第 40 页后崩溃”状态后以 `resumeFrom=40` 重跑，退出码 0、最终恰好 100 页且序号连续、finalize 产物齐全、引擎确认“沿用已落盘的 40 页”。期间发现并修复真实 bug：崩溃留下的半截页 IR（无收尾 `]`）会被按字节截断误删一个完整页——改为侧车标记（页数+字节偏移，先落页后写标记，崩溃至多落后一页）精确定位续写点。数据表见 [DOCUMENT_PIPELINE](DOCUMENT_PIPELINE.md#有界内存与断点续算实测2026-10-08本机真实-ocr)。

---

## 2026-10-09 snow-apps OCR 研究落地

参照 `mg-chao/snow-apps` 的 `rapid-ocr-rs`（13k 行 Rust PaddleOCR 兼容实现）与 `snow-ocr-process`（共享内存 OCR 进程）完成 6 项落地/实证：

| 项 | 结论 |
| --- | --- |
| EXIF 方向 | **已修复**：`read_raster` 出口统一应用（PIL exif_transpose 对齐的 8 映射）；此前 `image::open` 不应用方向，拍照件会横竖颠倒识别。2 个单测 |
| 词级框 | **已启用**：`return_word_box(true)` + 页 IR `regions[].wordBoxes` + 父进程 `DocumentRegion`/bindings 同步。实测 10 页×3 轮：warm 2432.6 vs 2437.6 ms/页（无差异）、峰值 +140 MB。1 个往返单测 |
| OCR 基准 | **已固化**：引擎 `bench` 子命令（cold/warm 每页耗时 + getrusage 峰值 RSS，pid 隔离目录）。100 页：cold 2761 / warm 1998 ms/页、峰值 1068 MB |
| cls 180° | **接线但默认关**：实证 PP-OCRv2 mobile cls 在 oar_ocr 0.9.2 下预测接近随机（正常件退化为乱码、颠倒件修不好，非标签翻转）——仅 `CASY_TEXT_LINE_ORIENTATION_MODEL` 显式启用，待规格匹配的模型 |
| 竖排 crop 旋转 | **实证不需要**：PP-OCRv6 medium 直接识别竖排日文（真实模型回归通过），snow 需要该启发式因其模型较旧 |
| 宽幅垂直 padding | **实证不需要**：2400×320 极端宽幅顶/底贴边行均正确识别，oar_ocr 内部已处理 |

新增依赖：`kamadak-exif`、`libc`（均为小体积纯 Rust；lockfile 增量 +33 行，无无关升级）。真实模型回归（中文扫描/韩文/多语言）在每项改动后重跑通过。门禁：Rust 403/0/6、引擎 27/5、前端 377/87、clippy 0 警告。

---

## 2026-10-09 方向分类模型落地（接上文“cls 待模型”）

**根因**：oar_ocr 0.9.2 的文本行方向适配器与 PaddleOCR 官方预处理规格不匹配（关键是 **BGR vs RGB** 通道序，另有归一化/尺寸细节），导致 PP-OCRv2 mobile cls 预测接近随机——正常件退化为乱码、颠倒件修不好。

**实现**：`tools/casy-doc-engine/src/orientation.rs` 用 ort 按官方规格自研（resize 高 48/宽自适应上限 192、BGR、`/255→(x-0.5)/0.5`、CHW、右补零；单行阈值 0.9、页面多数票决），模型目录自动发现 + env 覆盖，加载失败自动退回置信度启发式。

**倒置页的坐标/行序修正**（上一轮遗留的真 bug）：翻转重识别后若把坐标映射回原图系，正文行序会颠倒（版面排序按原图位置）。现改为坐标保持正置系 + 页面记录 `orientation_degrees=180`（schema v46，`document_pages` 幂等补列）+ 页面预览渲染同步旋转，高亮与所见一致。页 IR/`DocumentPage`/`DocumentPageView`/bindings 同步扩展。

**实证**：倒置中文密集页 7 行全部正确、行序正确、首行在正置系顶部（y=197）、`orientation=180`；正向页与 15° 倾斜页不触发、无旋转角；3 个真实模型回归通过。门禁：Rust 403/0/6、引擎 28/5、前端 377/87、clippy 0。

# 02 · 前端架构与数据链审计

审计日期：2026-09-30。范围：`src/core/**`、`src/types/**`、`src/stores/**`、`src/router/index.js`、`src/modules/**`、`src/shared/**`、`tests/**`。
方法：全量读取 + 静态扫描（未使用声明检测、模板绑定解析、未检查调用检测、命令覆盖率统计）+ 逐条跨文件 grep 验证。
结论口径：**CONFIRMED** = 我在源码中直接读到并交叉验证了字段/类型/调用方；**SUSPECTED** = 逻辑上成立但需要特定时序或数据才能显现。

---

## 一、概述

前端分层是**真实存在且基本一致的**，不是纸面架构。`ARCHITECTURE.md` 里那句自我批评「并非所有页面都经过 Context 服务」已经**过期**——我扫遍全仓，`invoke()` 的唯一直接调用点在 `src/core/tauriBridge.ts:1`，其余 358 处调用全部走 `tauriCallSafe`/`tauriCall`/Context 服务，且全部受 `commandMap` 静态约束。10 个插件、14 个服务、`observeChanges` 事件刷新层、请求令牌竞态防护都真实落地了。

所以问题**不在架构层，而在具体链路上**。这一轮找到的缺陷高度集中在四类：

1. **UI 改了状态但没人去取数**（筛选器只 `Object.assign` 到 store，不触发重新查询）→ 整个面板看起来能用，实际完全不生效。
2. **字段名对不上**（前端读一个后端 DTO 里根本不存在的字段）→ 该 UI 永远走一个固定分支，看起来"能跑"但数据是死的。
3. **成功路径不可达**（`void` 返回的命令用 `!== null` 判定成功）→ 写进数据库了，但 UI 不刷新、不提示、弹窗不关。
4. **组件调用的函数/组件不存在**（模板绑定了 script 里没有的标识符）→ 点击直接抛异常，功能完全不可用。

这四类都是"点了没反应 / 显示的不对"，恰好对应"想做生产实验但 bug 不断"的体感。**其中第 1 类（案件筛选面板整体失效）是本轮最严重的单点缺陷**——见 3.1 第 0 条。

**关键统计**

| 项 | 数值 |
| --- | --- |
| `commandMap` 契约条目 | 338 |
| 全部有 Rust handler（已确认） | 338 |
| **前端从未调用的命令** | **30（8.9%）** |
| 浏览器 mock 覆盖的命令 | 61 / 338（**277 条在浏览器模式静默失败**） |
| 全仓未使用的 ref/computed/function | **0**（这项很干净） |
| `tests/e2e/*.mjs` 脚本 | 17 个，**0 个被 vitest 收集，0 个进 CI** |
| 被 vitest 收集的测试 | 355（全部 unit + jsdom 组件） |

---

## 二、架构实况

### 2.1 分层是真的

- **唯一写入口**：`src/core/tauriBridge.ts:1` 是全仓唯一的 `import { invoke }`。`grep -rn "invoke(" src` 除该文件外零命中。所有组件/服务通过 `tauriCallSafe`（返回 `{ok,data,error}`，不抛）或 `tauriCall`（失败弹 toast 返回 null）调用。
- **契约强制**：`tauriCallSafe<K extends keyof CommandMap>` 的泛型约束让命令名和参数类型在编译期受约束。`tests/contract.commands.test.ts` 再做三方子集校验（前端 ⊆ CommandMap ⊆ Rust handler）。这比多数 Tauri 项目强。
- **服务层**：`src/core/services/index.ts:63-77` 注册 14 个服务，`provide` 带 `inject` 依赖校验，安装失败按快照回滚。
- **权限内核**：`src/core/plugin/context.ts:300-380` 的 `executeTool` 对未声明策略的工具**默认拒绝**（origin='ai' 时），写工具强制确认，且 `tool:executed` 事件落 `audit_events`。这是全仓最关键的安全面，且有测试守着。

### 2.2 刷新层是真实存在的（这一点比文档说的好）

`src/core/observeChanges.ts` 把 `{domain}:{changed,created,updated,deleted,completed,confirmed,imported}` 七种动作 × N 个 domain 批量订阅，80ms 合并 + 串行化 drain。已接入：

| 组件 | 订阅 domain |
| --- | --- |
| `HomeView.vue:441` | task, case, calendar, inbox |
| `TasksView.vue:278` | task, case, inbox |
| `CalendarView.vue:767` | task, calendar, case, inbox, holiday, plan |
| `DocWorkshopView.vue:495` | doc, case |
| `KnowledgeNotebookView.vue:485` | knowledge, case, inbox |
| `NotificationBell.vue:45` | task, calendar, holiday |
| `RelatedWork.vue:31` | task, case, calendar, knowledge, doc, file |
| `useEditorTasks.ts:199` | task |

**服务层发出的 domain 名与订阅方完全对得上**（我核对过 `mutation('task'|'file'|'knowledge'|'doc'|'calendar'|'project'|'plan')` 与各 `observeChanges` 的参数，无拼写错配）。全仓无 `<KeepAlive>`，路由级组件切换即重挂载。

> 结论：**"编辑任务后首页/案件/日历不刷新"这类问题在主链路上不存在。** 真正的刷新缺口是别的（见第五节）。

### 2.3 竞态防护也是真的

`stores/tasks.ts:182`、`stores/cases.ts`、`CaseFilesView.vue:225`（`loadRevision`）、`TasksView.vue:281`（`tasksRequest`）、`ReasoningSearchPanel.vue:41`（`revision`）、`GlobalSearch.vue:71`（`sequence`）都有单调递增令牌。`observeChanges` 自身串行化。**这也是为什么本报告的竞态条目很少。**

### 2.4 架构层面的真实缺口

| 缺口 | 说明 |
| --- | --- |
| **浏览器模式静默失败** | `tauriBridge.ts:62-66`：无 mock 的命令只 `console.warn`，返回 `{ok:false, error:'browser-mode: no mock'}`。**277/338 条命令属于此类。** 文档反复用"浏览器 Mock 验证"作为证据，但浏览器里 82% 的命令是哑的。 |
| **30 条死命令** | 见 4.1。白板/事实节点 9 条（被 `*_document` 系列取代）、OCR 3 条、`feishu_sync_*` 2 条等。 |
| **提案无列表接口** | `ai_proposals` 表有写入，但前端**从不调用任何 list 接口**（`grep list_ai_proposals` 零命中），提案只存在于 `tool-caller.ts:151` 的进程内 `Map`。重启即丢（后端 TTL 5 分钟，影响有限），但审计视图看不到待批提案。 |
| **commandMap 的 `void` 陷阱** | `Cmd<P, void>` 的调用方无法用返回值区分成功/失败——`tauriCall` 成功返回 `null`、失败也返回 `null`。这直接导致了第三节的头号缺陷。 |

---

## 三、断链清单

### 3.1 高危（写进去了 / 点不动 / 显示错的）

| 位置 | 现象 | 触发 | 后果 |
| --- | --- | --- | --- |
| `CaseFilterBar.vue:103-217` + `CaseListView.vue:775` | **整个筛选面板是惰性的**。13 个 `updateX()` 处理器（`updateTrack`/`updateStatus`/`updateRoute`/`updateCivilStatus`/`updateInvalidationStatus`/`updateAdminStatus`/`updateSortBy`/`updateClient`/日期区间/期限快捷/开庭区间/办案人）**只 emit `update:filter`**；父组件是 `(v) => Object.assign(casesStore.filter, v)`——纯对象变更，**不调 `loadCases()`**。只有 3 处会 emit `search`：搜索框（`:100`，有 300ms 防抖）、`loadFilter`(`:257`)、`clearFilters`(`:297`) | 打开筛选抽屉 → 选轨道=「民事侵权」（就在常驻首行，`:321`），或排序方式=「案件名称」，或设置立案起止 | **CONFIRMED**。左侧案件列表与「N 件」总数**逐字节不变**，不重新查询。只有搜索框和"加载已存筛选"会生效。已核实 `stores/cases.ts:120` 的 `loadCases()` 是 `filter` 的唯一消费者，全仓**没有任何 `watch` 监听 filter**；`CaseListView` 里 5 处 `loadCases()` 调用分别来自 `inbox:confirmed`、路由 query、选中案件、`CaseAttributes @saved`——**没有一处由筛选触发** |
| `stores/cases.ts:125-141` + Rust `db/cases.rs:92-109` | `deadlineFrom/deadlineTo/hearingFrom/hearingTo/operator` 被 UI 写入 store，但 `loadCases()` **不转发**这 5 个 key；后端 `CaseFilter` 结构体也**没有这些列**（只有 `get_case_unified_view` 读它们，而案件模块从不调该命令）。`src/types/bindings.ts:59` 同样缺失 | 更多筛选 → 期限快捷=「已逾期」/ 期限起止 / 开庭起止 / 办案人 | **CONFIRMED**。控件接受输入，**什么都不会变**。即使修好上一条，这 5 个筛选仍然是死的。需要同时改 store 转发、后端 `CaseFilter`、TS 类型三层 |
| `CaseFilterBar.vue:397` | 「导出 CSV」按钮 `emit('export')`，而 `CaseListView.vue:771-779` 只绑了 `@update:filter` / `@update:groupBy` / `@search` / `@create` | 更多筛选 → 导出 CSV | **CONFIRMED**。**点击字面意义上什么都不发生**——无文件对话框、无提示、无下载。`CasesService.exportCases`（`services/cases.ts:105`）与 `export_cases` 命令全仓无任何视图调用，是彻底的死出口 |
| `CaseListView.vue:78,773,776` | `groupBy` 可选（不分组/按客户/按轨道/按路由/按法院）、会被 `useViewMemory` 持久化，但**从不参与渲染**——列表是扁平 `v-for="item in casesStore.cases"`（`:807`）。真正实现分组的 `CaseGroupPanel.vue` **被零个文件 import** | 更多筛选 → 分组方式 =「按客户」 | **CONFIRMED**。**完全相同的扁平列表**。一个 242 行的组件从未被挂载 |
| `TasksView.vue:582` `:592` | `defer_task`/`clear_task_defer` 声明为 `Cmd<…, void>`，Rust 返 `Result<(),String>`→序列化为 `null`；代码用 `if (ok !== null)` 判成功 | 任务 ⋯ 菜单 →「推迟到…」→ 选日期 → 确定 | **CONFIRMED**。成功分支**可证明不可达**。数据库写入成功，但无成功提示、**弹窗不关**、`deferTargetTask` 不清空、`loadTasks()` 不执行 → 任务仍留在「今日专注」，再次点击又弹出预填的旧日期。「结束推迟」同样失效 |
| `CopilotSidebar.vue:51,106,160` | 模板 3 处 `@click="toggleExpand(item.id)"`，script **无此函数**；只有 `toggleSection(key)` 和一个从未触发的 `emit('toggle-expand')` 声明 | 写作页 AI 智伴侧栏，点任意知识卡片标题 | **CONFIRMED**。`WritingView.vue:458` 已正确绑定 `@toggle-expand="copilotToggleExpand"`，`useCopilot.js:259` 实现完整——**唯独子组件从不 emit**。点击抛 `TypeError`，卡片永不展开，AI 引用溯源功能完全不可用 |
| `CalendarView.vue:546,755,1270,1448,1559` | 5 处读 `task.caseName`。Rust `TaskDto`（`commands/tasks.rs:19-58`）有 `case_id` **无 `case_name`**；`TasksService.list` 只补 `parentId`；`Task` 接口也没有 `caseName` | 时间线视图 / 待排池 / 年热力图 | **CONFIRMED**。时间线每张任务卡恒显示「常规待办」（:546）；**按案件名搜索恒 0 结果**（:755 那个 OR 分支永久 undefined）；待排池的案件标签永不渲染（:1448） |
| `CalendarView.vue:659` | `deadlineWarnings.value.some(w => w.deadlineDate === dateStr)`。Rust `DeadlineResult`（`deadline/engine.rs:11-22`）字段是 `due_date` → camelCase **`dueDate`** | 预测视图 | **CONFIRMED**。`w.deadlineDate` 恒 undefined，条件恒 false。**法定期限引擎的结果完全不参与「预测」视图的 risk 判定**——这是法律工具的核心视图。且 `loadDeadlineWarnings()` 每次 `loadData()` 都发一次完整 IPC，结果除了这一处（失效的）再无消费者 |
| `HomeView.vue:287` | `completed: t.status === 'completed' \|\| t.status === 'done'`。`Task` 无 `status` 字段，完成度是 `completed: 0\|1` | 首页「今日承诺」勾选框 | **CONFIRMED**。`c.completed` 恒 `false`。`:checked="c.completed"` 绑在永远不变的值上——点下去 Vue 立刻 patch 回 `false`，**复选框弹回未选**；`is-completed` 样式永不生效；`:569` 的"未完成"徽标筛选是空操作 |
| `HomeView.vue:292-297` | `try { await tasksStore.toggleTask(item.id) } finally {…}` 丢弃 `{ok,error}`；`TasksService.toggle` 用 `tauriCallSafe`（不弹 toast） | 首页勾选承诺时数据库被锁 | **CONFIRMED**。写入失败**零反馈**。叠加上一条，用户只看到复选框自己弹回去 |
| `CaseListView.vue:582-586` | `revealFile()` 丢弃 `files.reveal()` 返回值，无条件 `ElMessage.success('已在访达/资源管理器中定位')`。紧邻上方的 `openFile()` 却正确判了 `result.ok` | 案件右键菜单 →「在访达/资源管理器中显示」，文件已被移走/删除 | **CONFIRMED**。**假成功提示**。本应用的模型明确允许"DB 登记与磁盘文件分离"（有「已移除登记」列表、`apply_case_file_renames`），所以这不是边缘情况 |
| `CalendarView.vue:1594-1595` vs `:348,356` | 日程编辑弹窗有**两个**日期输入（开始日期 / 截止日期），但 `calendar_events` 只有 `event_date` 一列；保存时 `eventDate: item.dueDate \|\| item.startDate`，`dueDate` 优先 | 双击任一日程 → 只改「开始日期」→ 保存修改 | **CONFIRMED**。**静默丢弃编辑 + 弹绿色「已保存日程修改」**。必须同时清空「截止日期」才开始日期才生效 |
| `CaseNetworkView.vue:172` | `<el-icon-connection />` **不是真实的 Element Plus 组件**（全仓仅此一处；正确写法是 `<el-icon><Connection /></el-icon>`） | 打开 `/cases/network` 案件关系网络 | **CONFIRMED**。Vue 报 "Failed to resolve component" 并渲染为空。**每对关联案件之间的箭头全部不可见** |
| `CaseListView.vue:479-483` | `submitMemo` 用 `casyContext.cases.update(...)` 直写、**绕过 `casesStore.updateCase`**，store 里的 `cases[i].notes` 保持陈旧；但本地 `caseMemos` 被乐观更新了。而 `selectCase()`（`:349-356`）→ `parseMemosFromNotes(item?.notes)` 读的是**陈旧的 store 副本** | 「记录与备忘」tab → 写备忘 → 「存入本案备忘库」（**成功提示会出现**）→ 点另一个案件 → 点回来 | **CONFIRMED**。**刚成功保存的备忘消失了**，无任何报错。该路径上没有 `loadCases()` |
| `CaseDetailView.vue:320-324` | 「关联知识 (N)」的 `searchTerms` = `[caseName, caseType, clientName].filter(Boolean).join(' ')`——**一个空格拼接的字符串**。后端把整串包成**一个 FTS5 引用短语**（`safe_fts_phrase`，`commands/knowledge.rs:813-820`），LIKE 回退也是同一个 `%XX公司 YY公司%`。且 `Case` 表**没有 `caseType` 列**（`db/cases.rs` 里两个字符串都不存在），所以实际查询是 `caseName + " " + clientName` | 打开任何有客户名的案件，看「关联知识」旁边的数字 | **CONFIRMED**。**几乎恒为 0**。需改成每个词一次 `search()`（或走 `global_search`） |
| `CaseDetailView.vue:349-359` | `saveGoal` 是 `if (result.ok) {…}` **无 else** | 数据库只读/被锁时编辑「案件目标」 | **CONFIRMED**。**完全静默**——无错误提示，编辑框还开着、输入还在，与"点了没反应"无法区分 |
| `CaseDetailView.vue:479-489` | `toggleTaskComplete` 同样的 `if (result.ok)` 无 else。注意它写的是「任务待办」表的复选框（`:1006-1009`）和「下一步行动」的完成按钮（`:607`） | 点任务复选框 | **CONFIRMED**。**无 toast、无错误**，看起来点击无效。同文件 `:505` 的 `unlockNextTask` 判了——不一致是局部的 |
| `CaseImportDialog.vue:1078-1086` | 导入报告头部**无条件**是绿色 ✓ +「案件数据导入完成」+「所有变更已通过单事务安全写入」，不看 `importReport.failedCount` | 导入一个每行都校验失败的 sheet | **CONFIRMED**。绿色「导入完成」标题压在红色「失败行数 N」上方。**公允地说**：部分失败本身是**有**暴露的（`:1102-1105` 红色统计块 + `:1128-1135` 逐行异常明细 + `emit('imported')` 正确刷新列表）——这只是标题不降级，不是静默部分失败 |
| `CaseImportDialog.vue:1054-1073` | 第 3 步标题写「清洗后数据预览」，提示写「日期、金额与承办人已**按照底层算法完成格式归一化**」，但 `previewRows` 就是 `excel_inspect_sheet`/`feishu_inspect_bitable` 返回的原始值、之后再没被碰过；`executeImport`（`:476-486`）只用 `columns` 建映射，从不用 `previewRows`。Rust 侧确认无归一化（`import_excel.rs:1387+` 直接用 `cell_to_string`） | 到第 3 步，相信预览来确认「2024/3/5」会导入成 `2024-03-05」 | **CONFIRMED**。预览显示的是**未归一化的源串**，且**没有"导入后"视图**可对照 |

### 3.2 中危（功能不可达 / 数据不落 / 静默失败）

| 位置 | 现象 | 触发 | 后果 |
| --- | --- | --- | --- |
| `CaseNetworkView.vue:133` | `{{ group.icon }}` 读 `relationTypeMap`（`:16-21`）里**不存在的属性**——该 map 每种类型只定义了 `{label, color}` | 打开 `/cases/network` | **CONFIRMED**。四个关系统计卡的 `<span class="stat-icon">` **全部渲染为空** |
| `CaseListView.vue:650-654` | 唯一的事件监听是 `inbox:confirmed`；`case:created/updated/deleted/imported`（`services/cases.ts:62,74,83,201/252/274/300` 全部发出）**一个都不订阅**。全 app 唯一广泛消费者是 `DashboardView.vue:79` | 让 AI 助手建/改/删案件（走 `cases-plugin` 工具），或从整库飞书快照导入，而 `/cases` 恰好是当前路由 | **CONFIRMED**。侧栏列表保持旧集合，**直到你离开 `/cases` 再进来**。`autoPush.notifyDataChange()` 只是飞书推送触发器，**不做 store 失效**，没有兜底 |
| `KanbanView.vue:193-201` | 拖拽审计日志**把新值记成了旧值**：`:193` 先 `caseItem[statusField] = newStatus`，`:200` 再读 `caseItem[statusField]` 拼日志 | 在案件看板拖动案件到新列 | **CONFIRMED**。办案日志写成「civilStatus 从『in_trial』变更为『in_trial』」——**旧状态不可恢复**。修法：`:193` 前先存到局部变量 |
| `CaseListView.vue:403` | 「本案全景时间轴」只调 `casyContext.calendar.events(year, month)`（2 个参数），`monthCount` 为 `undefined`，Rust 走 `month_count.unwrap_or(1)`（`commands/calendar.rs:67`） | 在「记录与备忘」登记下个月的开庭/日程，然后打开本案时间轴 | **CONFIRMED**。该面板自称「按时间顺序**全量**沉淀本案的客观事件」，但**下个月的事件不在里面**，且永远不会出现（`loadCaseEvents` 每次都查同一个单月） |
| `AddRelationDialog.vue:42-47` | 案件下拉 `casyContext.cases.list({})` 不传 `page/perPage`，Rust `filter.per_page.unwrap_or(50)`（`db/cases.rs:240`）→ **硬截断 50 条**；且只在父组件 `onMounted` 拉一次，而该弹窗**常驻挂载**在 `CaseDetailView.vue:1112`（只靠 v-model 切显隐） | 建好第 51 个案件 → 打开任意案件详情 → 添加关联 | **CONFIRMED**。**第 51 个案件不在下拉里**，此后新建的任何案件也都不在 |
| `CaseNetworkView.vue:25` | 同样的 50 条截断，且 `getCaseById`（`:69`）只在第 1 页切片里搜 | 案件数 > 50 时打开 `/cases/network` | **CONFIRMED**。指向第 50 条之后案件的关系渲染成「未知案件」（`:168,181`），并被 `:88` 的 `.filter(r => r.relatedCase)` **整条丢弃** → 「关联案件 (N)」计数**少报，无任何提示** |
| `KanbanView.vue:222-231` | 看板只在 `onMounted` 加载，自己拖拽后才 reload，**不订阅任何 case 事件** | 在 `CaseDetailView` 的「案件属性」里改了状态，然后回到看板 | **CONFIRMED**。看板保持陈旧直到离开再进入。**另外：`/cases/kanban` 与 `/cases/network` 在整个 UI 里没有任何入口**——`grep -rn "case-kanban\|案件看板" src/` 只找到路由定义和视图自身。这两个视图是 URL-only |
| `CaseDetailView.vue:458` `:468` | `deleteHearing` 是完整实现的 handler 但**无 UI 入口**（操作列 `:956-961` 只有「编辑」「收转文与修订记录」）；`toggleHearingStatus` 是死函数——状态标签（`:947-954`）有 `cursor: pointer` 样式，但 handler 绑的是 `openEditHearing` | 想删一条录错的开庭记录 | **CONFIRMED**。**全应用无处可删**，`delete_case_hearing` 从 UI 不可达；那个"可点"的状态标签点了只是打开编辑框 |
| `CaseListView.vue:793` | 侧栏搜索框 `@input` 直接改 `casesStore.filter.search` 并调 `onSearch()`，**无防抖**（`CaseFilterBar` 里等价的搜索有 300ms 防抖，相距两个组件） | 在案件侧栏连续打字 | **CONFIRMED（性能，非正确性）**。每次按键一次全量 `list_cases` IPC。不是陈旧覆盖竞态——`stores/cases.ts:121` 的 `listRequest` 正确丢弃被取代的响应 |
| `CaseDetailView.vue:320-324` 见 3.1 | — | — | — |
| `TasksView.vue:26,97` | `TodayResetDialog` 已 import、`showTodayReset` 已声明，**两者都不在 template 里**。全仓无其他消费者 | 想用「整理今天」 | **CONFIRMED**。272 行组件**完全没有入口**，功能不存在。（该组件是 `isFocus` 的唯一写入者之一） || `stores/tasks.ts:364` | `loadCustomPerspectives()` 定义后**零调用方** | 重启应用 | **CONFIRMED**。`_persistPerspectives` 写进 localStorage 但**没人读回来**，`customPerspectives` 每次启动都是 `[]` → `TasksView.vue:679` 的 `v-if="customPerspectives.length"` 恒 false →「我的清单」每次重启全丢 |
| `TasksView.vue:207` | `getCustomTasks` 走 `tasksStore.getTasksByCustomPerspective()`，读 `state.tasks`；但 TasksView 用**自己的本地 ref**（`:47`），从不调 `tasksStore.loadTasks()` | 新建自定义清单后点它 | **CONFIRMED**。即使同会话内也恒「暂无任务」。被上一条掩盖 |
| `TasksView.vue:1070` | `<AreasDialog v-model="showAreasDialog" />` 无 `@changed`，而子组件在 `:75,109,120` 三处 `emit('changed')`，自己的注释还写着「由父级刷新领域缓存」 | 管理领域里把「客户开发」改名 → 关闭 | **CONFIRMED**。所有任务的领域标签显示旧名，直到某次无关的任务/案件写入碰巧触发 `loadData` |
| `TaskRow.vue:62,76,127,131` | `focusable` prop 门控 ★ 按钮、`toggle-focus` emit 是其 handler，但**两个调用点都没传** | 想把任务设为今日重点 | **CONFIRMED**。★ 按钮永不出现，`isFocus` 无写入路径 |
| `TasksView.vue:122,240` | `savedFilters = computed(() => filtersStore.filters)` 从不在 template 出现，但 `onMounted` 真的发了 `list_saved_filters` | 在任务页保存筛选方案 | **CONFIRMED**。**发了 IPC 拿回来扔掉**。案件页的 `CaseFilterBar.vue:94,383` 渲染了同一个 computed——任务页漏了 |
| `TasksView.vue:66` | `selectedContextFilter` 被 `useViewMemory` 持久化、被过滤链消费（`:211`），但**没有任何控件写它** | 想按 @办公室 / @电话沟通 过滤 | **CONFIRMED**。上下文过滤不可达；值只能是 `'all'` 或上次会话的残留 |
| `TasksView.vue:979-1002` | 抽屉表单 `taskForm.ts:86` 读 `areaId`、`:110` 写回，但**没有领域选择控件** | 建好领域后想给任务分派 | **CONFIRMED**。任务模块里**无法给任务指定领域**，`TaskRow.vue:159` 的领域标签只能靠其他途径种出来的数据 |
| `CalendarView.vue:832-837` | `loadTodayTasks()` 调 `casyContext.tasks.list({startBucket:'today'})` 写 `todayTasks.value`，而 `todayTasks` 全仓**只有声明和赋值两处引用** | 任何一次任务保存/勾选（`:343`、`:916` 都会调它） | **CONFIRMED**。**每次任务写操作都多发一次全表任务查询，结果丢弃**，且该函数无任何错误处理 |
| `CalendarView.vue:839-848` | `loadCases` 失败时 `if (!result.ok) return`——静默保留旧值（首次即空）。无 `caseError`，`:975` 的错误横幅只覆盖 holiday/task/plan | `list_cases` 失败 | **CONFIRMED**。时间线的案件筛选侧栏、编辑弹窗的「关联案件」下拉、`TaskGantt :cases` **全空且无错误、无重试**。同文件的 `loadHolidays`/`loadTaskPlans` 都有 error ref——**同一文件内不一致** |
| `CalendarView.vue:803-816` | `listEvents()` 的 `.ok` **从不检查**，只有 `results` 进 `failed` 判定。而 `independent.data` 是事件对象上 `taskId`/`location`/`notes` 的**唯一来源** | `list_calendar_events` 失败 | **CONFIRMED（失败路径）**。已排期任务重新出现在「未安排」池（`:735`）、同一任务在 TimeGrid 里渲染两次（`:945`）、事件弹窗的「打开关联任务」按钮消失（`:1584`）。**全程无任何报错** |
| `CalendarView.vue:655 vs :712`, `:661 vs :717` | 预测视图头部统计与下方 14 行列表用**两套口径**：工时 `\|\| 0` vs `\|\| 60`；休息日判 `restIntervals.length===0` vs `isPlanningWorkday()`；风险日只数 `court\|hearing\|deadline*` vs 还含 `appeal` 和 deadlineWarnings | 切到预测视图，把三张统计卡和下面 14 行对账 | **CONFIRMED**。**三个数字与它们所概括的列表对不上**。半天自休的天会出现在"仅看专注空闲窗口"里却不计入"黄金专注窗口 N 天" |
| `CaseFilterBar.vue:220` | `remoteClientSearch` 挂在 `el-select :remote-method`（`:343`）上，**每次按键发一次全量案件 FTS，无防抖、无请求令牌** | 在案件列表的客户筛选框里连续打字 | **CONFIRMED**。「张」的比「张三」的慢返回 → 候选列表被**旧结果覆盖**。搜索框看起来"时灵时不灵"。对比：`PersonsView.vue:54-55` 有 300ms 防抖 + `loadRevision`，`ProjectsView.vue:115` 有令牌——**只有这里漏了**。且 `!result.ok` 时不提示、不清空 |
| `EvidenceLinkPicker.vue:277` | `searchEntities()` 无令牌，且中途读 `targetType.value` 决定写哪个列表 | 检索进行中切换目标类型 / 双击检索 | **SUSPECTED**。旧类型的结果写进新类型的列表 |
| `GlobalSearch.vue:100` | 任务结果的 `route: '/tasks'` **硬编码，不带 id**。而 `TasksView.vue:254,262` 本身就支持 `?edit=<id>` 深链 | ⌘K 搜到一条任务 → 回车/点击 | **CONFIRMED**。**跳到任务列表页，具体任务既不高亮也不定位**，用户得自己再找一遍。案件（`/cases/:id`）、知识（`?select=`）、案卷（`?select=`）**都做了深链——唯独任务没有** |
| `GlobalSearch.vue:~228` | `choose()` 里的 `if (!target.route)` 分支提示「「项目」独立视图在 A1-1 阶段二提供」——但每个 ResultItem 的 `route` 都非空（项目是 `/projects`） | 无 | **CONFIRMED**。死分支 + 过期文案 |
| `OverdueMorningBrief.vue:96-122` | 组件在 `App.vue:445` 常驻挂载，`onMounted` 里 `if (route.path !== '/') return`；`watch(route.path)` 只负责**关闭**弹窗，不重新加载 | 应用以 `#/tasks` 等非 `/` 路由启动（书签/恢复 hash） | **SUSPECTED**。整个会话早报不再加载，导航回 `/` 也不补 |
| `HomeView.vue:78-102` | `today`/`currentHour`/`currentDay` 是模块级 const；`dateDisplay`/`fullDateDisplay` 是**零响应式依赖的 computed**，Vue 只算一次。无定时器 | 应用跨零点不关 | **CONFIRMED**。页头日期、四个分组的过滤基准全部停在挂载那一刻。（`TasksView.vue:189,238` 有定时器，`src/shared/useLocalDay.ts` 现成）另：`dateDisplay` 全仓无引用，是死变量 |
| `AIAuditView`/`AICompanionView`/`DecisionsView` 共 14 个 loader | 单次 `await`，无令牌/防抖 | 反复点刷新 | **SUSPECTED**（低）。多数是手动触发，危害小 |

### 3.3 低危 / 死代码（已验证，不建议优先修）

| 位置 | 现象 |
| --- | --- |
| `CaseListView.vue:191-225` | `proceedingsList` computed 定义后**零引用**。为它存在的 4 轨 `STANDARD_PROCEEDING_NODES` 映射（`:138-165`，约 60 行）也随之成为死代码——「程序期限与统筹」tab（`:1309`）渲染的是 `ProcedureBoard` |
| `CaseGroupPanel.vue` | 242 行组件，**被零个文件 import**。分组功能因此完全不存在（见 3.1） |
| `CaseDetailView.vue:202` | `sequentialCompletionRate` computed 从不读取——卡片头部（`:684`）手写了同样的算式 |
| `CaseDetailView.vue:63-66,373-374,389` | `hearingForm.actualStatus` 是死字段：被 `openAddHearing`/`openEditHearing` 写入，但弹窗（`:1103-1105`）只暴露 `lifecycleStatus`，`saveHearing` 在 `:416/440` 又用 `lifecycleStatus` 覆盖它 |
| `CaseDetailView.vue:831` | `v-if="relatedCases.length > 0 \|\| true"`——恒真条件，左操作数是死代码 |
| `CalendarView.vue:409-415` | `weekHours` computed 扫描全部事件与任务算显示小时区间，**零引用**（`TimeGrid.vue:17` 硬编码 `v-for="h in 24"`） |
| `CalendarView.vue:486-495` | `hasWaitingOnDay()` / `hasPlanEventOnDay()` 定义后从不调用（`:611` 内联重实现了一遍） |
| `CalendarView.vue:20-34` | 18 个导入图标中 **9 个从未使用** |
| `CalendarView.vue:1745,1758,3290` | `.natural-input-box` 三处 CSS 规则，**模板无任何元素带这个 class**（约 30 行孤儿样式） |
| `CalendarView.vue:125,290,305` | `editingItem.completed` 被写但无模板绑定、也不随保存发送；`openEditDetail` 总是设非空 `id`，`:321`/`:354` 的 `!item.id` 新建分支**永不可达** |
| `stores/tasks.ts:326,639` | `tasksStore.activePerspective` 全应用**只写不读**；`getTasksByPerspective()`、`taskStats()` 无任何视图调用。视角真值在 TasksView 本地 ref，store 那份是死的 |
| `TasksView.vue:656` | `getCustomPerspectiveCount` 定义后从不调用——「我的清单」不显示条目数，而内置清单显示（`:667,672`） |
| `stores/calendar.ts` | `useCalendarStore` 只被 `HomeView.vue:36,49` 引用；`CalendarView` 自己加载事件/节假日。store 的 `prevMonth`/`nextMonth`/`goToday`/`eventsByDate`/`isWorkday` 全是死代码，**且与视图的加载路径重复** |
| `PerspectiveManager.vue:130` | `reactive({...defaultFormData})` 浅拷贝引用 → `formData.filters` **就是** `defaultFormData.filters`。取消后重开，创建表单被上次的筛选污染。需 `structuredClone` 或工厂函数 |
| `AreasDialog.vue:100` | `update_area` 的 icon 是直接赋值（非 COALESCE），`saveEdit` 只发 `{name,description}` → **每次改名都把 icon 抹成 NULL**。当前不可见（没有任何代码写 icon，列恒 NULL），但**一旦有人开始写 icon 就会立刻丢**。另：`removeArea` 无二次确认 |
| `HomeView.vue:365` | `totalCases = casesStore.cases?.length` 而 store 按 `perPage:50` 分页（真总数是 `casesStore.total`）；且它传给 `metrics.totalCases` 后**从未被渲染**（`useBriefingModel.ts:132-137` 只渲染期限/排期/承诺/已完成四项） |
| `CaseAttributes.vue:25` | 循环把不属于当前 `caseRoute` 的状态字段置 null。若历史数据存在"程序与当前程序不符"的行，**改任何其他属性都会静默清空它** |
| `src/core/autoPush.ts:56-62` | `notifyDataChange()` 的 `try/catch` 是**死代码**——`tauriCallSafe` 内部已捕获，永不 reject。且 `!result.ok` 也不处理。飞书自动推送失败**完全无声**（连 console 都没有） |
| `stores/tasks.ts:reorderToday` | 循环里逐个 `await this.updateTask()`，而**每个 `updateTask` 内部又 `await this.loadTasks()`** → 重排 N 个任务发 **2N 次**全表查询，中途无事务、无失败处理 |

---

## 四、命令与契约层

### 4.1 30 条前端从未调用的命令

```
update_case_status            list_field_groups            get_case_unified_view
get_all_case_type_metrics     detect_relations              remove_relation
get_folder_template           feishu_sync_pull              feishu_sync_push
sync_export_persons_to_vcard  sync_export_whiteboards_to_blob
get_command_route_info        list_knowledge_blocks         knowledge_stats
get_area                      create_notification           apply_smart_rules
list_pending_ocr_files        ocr_case_file                 ocr_all_pending
get_whiteboard_scene          save_whiteboard_scene         list_fact_nodes
create_fact_node              update_fact_node              delete_fact_node
list_whiteboard_edges         create_whiteboard_edge        delete_whiteboard_edge
list_document_jobs
```

**不是 30 个坏掉的 UI**。白板 9 条（`get/save_whiteboard_scene`、`*_fact_node`、`*_whiteboard_edge`）已被 `RichCanvas.vue:64,86` 用的 `get_whiteboard_document`/`save_whiteboard_document` 取代——是**遗留后端面**。`update_case_status` 被 `update_case` + `civilStatus`/`invalidationStatus`/`adminStatus` 三字段取代。

真正值得注意的：
- **`ocr_case_file` / `ocr_all_pending` / `list_pending_ocr_files` 三条零调用**。前端走的是 `queue_document_processing`（`CaseFilesView.vue:145`），`ProcessingCenter.vue:110` 还专门提示「请在文件详情点击"开始识别并生成可搜索 PDF"」。**没有"一键识别全部待识别"入口**——后端能力在，UI 没有。
- `feishu_sync_push` / `feishu_sync_pull` 零调用：飞书同步只有 `autoPush.trigger_feishu_push`（单向推送），**拉取方向前端无入口**。

### 4.2 服务层绕过

**没有发现绕过。** 逐项核实：
- 组件层的 `tauriCallSafe` 直调共 40 处，命令全部在 `commandMap` 内，无拼写错误（`tests/contract.commands.test.ts` 也静态守着）。
- 没有 `invoke('...')` 硬编码字符串绕过 bridge（该测试 `:151` 明确禁用）。
- `as any` / `@ts-ignore` 共 31 处，集中在 types 与测试，不在调用点。

**唯一真正的"绕过"是语义上的**：`TasksView.vue:578,591` 用 `tauriCall` 直调 `defer_task`/`clear_task_defer`，**绕过了 `TasksService`**，因而：
1. 丢失了 `mutation()` 的 `task:changed` 事件 → 这两次写对事件总线不可见；
2. 丢失了 `.ok` 语义 → 直接催生了 3.1 的头号缺陷。
这是全仓**唯一**一个对 `void` 命令做 `!== null` 判定的地方；同为 `void` 的 `snooze_task` 走 `TasksService.snooze` → `mutation()` → 判 `result.ok`，是对的。

### 4.3 浏览器模式的静默失败面

`tauriBridge.ts:62-66`：277/338 条命令在浏览器模式返回 `{ok:false, error:'browser-mode: no mock'}`，**只 `console.warn`，无用户可见提示**。

这条对用户的实验工作**直接相关**：文档里 `UX_CONSISTENCY_AUDIT` / `VALIDATION_2026-09-28` 反复用"浏览器 Mock 验证"作为验收证据，但浏览器里 **82% 的命令是哑的**——包括 `create_case`、`update_case`、`search_cases`、`add_relation`、`import_files_to_case`、`webdav_*`、`create_backup` 全部。**浏览器里"点得动"完全不能推断桌面端可用。**

---

## 五、错误处理缺失

### 5.1 空 catch 全集（仅 4 处，且都基本正当）

| 位置 | 内容 | 判定 |
| --- | --- | --- |
| `main.js:26` | `.catch(() => {})`（挂载容错） | 正当 |
| `TasksView.vue:404` | `try { JSON.parse(e.dataTransfer.getData(...)) } catch {}` | **正当**。只吞畸形拖拽载荷，`onDropToQuadrant:406` 有 `!task?.id` 兜底 |
| `TasksView.vue:640` | 删除透视 confirm 的 `.catch(() => {})` | 基本正当（吞 ElMessageBox cancel）。但 `ElMessage.success('透视已删除')` 在**持久化确认之前**就弹了——localStorage 写满会假成功 |
| `KnowledgeNotebookView.vue:116` | `void persistence.clearRecovery().catch(() => {})` | 正当（fire-and-forget） |
| `CalendarView.vue:239` | `catch {}` | 见下 |

**空 catch 不是本项目的问题。** 真正的问题是**"catch 之后什么都不告诉用户"**（5.2）。

### 5.2 无用户可见反馈的失败（比空 catch 严重）

| 位置 | 失败时用户看到什么 |
| --- | --- |
| `HomeView.vue:292` | 勾选承诺写失败 → **零反馈**，复选框弹回（因为 3.1 的 `status` bug） |
| `CaseDetailView.vue:349` `saveGoal` | 改「案件目标」失败 → **完全静默**，编辑框还开着，与"点击无效"无法区分 |
| `CaseDetailView.vue:479` `toggleTaskComplete` | 点任务复选框失败 → **无 toast 无错误**，看起来点击无效 |
| `CaseListView.vue:584` | reveal 失败 → **绿色成功提示** |
| `CalendarView.vue:839` | `list_cases` 失败 → 侧栏和下拉全空，无错误无重试 |
| `CalendarView.vue:850` | `get_deadline_warnings` 失败 → 无 else 分支，完全不可见 |
| `CalendarView.vue:812` | `listEvents` 失败 → 三处视图数据静默错误，无报错 |
| `CalendarView.vue:832` | `loadTodayTasks` 失败 → 静默 no-op |
| `CaseFilterBar.vue:227` | 客户搜索失败 → 保留旧候选，无提示，用户以为"没搜到" |
| `CaseListView.vue:775` | 筛选失败 → **根本没有请求发出**，谈不上失败 |
| `autoPush.ts:58` | 飞书推送失败 → 连 console 都没有 |
| `AIChatPanel.vue:88` / `AICompanionView.vue:41,45` | `offPlugins` / `stopTools` 是**空函数体**（unsubscriber 收集变量，从未被调用）→ 组件重挂载时旧的 `plugins:ready` 监听器泄漏 |

### 5.3 危险方向：比缺提示更糟

1. `CaseListView.vue:585` — 假成功（reveal）。
2. `CalendarView.vue:1594` — 假成功（丢弃开始日期）+ 编辑丢失。
3. `CaseImportDialog.vue:1078` — 100% 全失败时头部仍显示绿色 ✓「导入完成」。
4. 通知/提醒域已按 UX 审计移除了"已读/忽略"假成功（做得好），但上面四处是同型问题，未被那次审计覆盖。

---

## 六、状态刷新缺口

**主链路是好的**（见 2.2）。真实缺口：

| 缺口 | 影响 |
| --- | --- |
| **`CaseListView.vue:775` 筛选变更不触发重取** | 见 3.1 首条。这不是"刷新缺口"而是"从未建立刷新"——比缺口更严重 |
| `CaseListView.vue:650-654` 不订阅 `case:created/updated/deleted/imported` | AI 改案件 / 飞书整库导入后，`/cases` 列表陈旧直到重进路由。全 app 唯一广泛消费者是 `DashboardView.vue:79` |
| `CaseListView.vue:479-483` 备忘绕过 store | 写入成功但 store 陈旧，切走再切回备忘消失（3.1） |
| `KanbanView.vue:222-231` 无 case 事件订阅 | 他处改状态后看板陈旧。叠加「无 UI 入口」，实际使用面很小 |
| `TasksView.vue:1070` AreasDialog 无 `@changed` | 领域改名后所有任务标签过期，直到无关写入碰巧触发刷新 |
| `defer_task`/`clear_task_defer` 绕过 service → 不发 `task:changed` | 这两次写对整个事件总线不可见（3.1 头号缺陷的副作用） |
| `HomeView.vue:78-102` 无时钟 | 跨零点后所有"今日"分组按昨天的日期算 |
| `OverdueMorningBrief.vue:96` | 非 `/` 路由启动则整个会话不加载 |
| `stores/tasks.ts:364` localStorage 只写不读 | 自定义透视重启即丢（**这是"持久化缺失"不是"刷新缺失"**，但用户观感一样） |
| `AIChatPanel.vue:88` / `AICompanionView.vue:41,45` 空 unsubscriber | 监听器泄漏累积 |
| `CalendarView.vue:928,935` 切视图触发**两次**并发相同 `loadEvents` | 有令牌不会写坏数据，但每次切视图多发 2 轮 IPC，输的那份被丢弃 |
| `CalendarView.vue:822,839,850` 六个 loader 中三个无令牌无卸载守卫 | 与 `observeChanges` drain（写后 ~80ms）撞车时，慢的旧 `list_tasks` 可覆盖新 `tasks.value`（**SUSPECTED**，需慢 IPC 显现） |

**跨模块回答**：
- 编辑**任务** → HomeView / CaseDetailView / CalendarView / 通知铃 **都会刷新**（`observeChanges` 覆盖 + 无 KeepAlive）。
- 编辑**案件** → CaseDetailView 自订阅 `case:updated`（`:528`）；但**案件列表不订阅**（见上表第 2 行），看板也不订阅。
- 案件**筛选**变更 → **无人重取**（3.1 首条）。这是本轮唯一一个跨模块刷新链**根本没有建立**的地方。

---

## 七、测试可信度评估

### 7.1 数量核对

`vitest.config.ts:10` → `include: ['tests/**/*.test.ts']`。

- **355 项全部被收集，79 个文件**（`tests/unit/**` 73 文件 325 项 + `tests/*.test.ts` 根级 6 文件 30 项）。
- **`tests/e2e/` 有 17 个 `.mjs` 脚本，一个都没被收集**（扩展名不匹配），`package.json` 无脚本引用，`.github/workflows/ci.yml:35` 只跑 `npm run test:unit`。且 `playwright` 不在 `package.json` 也不在 `node_modules`，需要预编译的 `*_local_bridge` 二进制和 `CASY_OCR_QA_SOURCE` 等环境变量才能跑。
- **CI 里的端到端覆盖 = 0。**

### 7.2 断言风格分类

| 类别 | 文件 | 测试 | 占比 |
| --- | --- | --- | --- |
| (a) 纯函数 / 真实行为单测 | 44 | 248 | **69.9%** |
| (b) 组件 mount + **真实行为断言** | 35 | 107 | 30.1% |
| (c) 快照测试 | **0** | 0 | 0% |
| (d) 纯 mock 回声 / 同义反复 | **0** | 0 | 0% |

**这与"355 大部分是浅层快照/冒烟"的猜测相反，我核查后不认同该猜测。** 零快照、零同义反复测试。最浅的几个文件也断言真实可观测状态：
- `slashCommandMenu.test.ts:25-42` 断言真实 ProseMirror doc 变更 + 触发字符被删 + 卸载时全局监听被移除。
- `settingsCredentials.test.ts:12-16` 断言 `keychainStatus` **在 mount 时不被调用**、点击后**恰好调用一次**（安全意图测试）。
- `workspacePlugin.test.ts:9-18` 用 12,600 字草稿断言分窗**完整且不重叠**（`first.content + second.content === content`）；`:20-25` 断言 `get_settings` **剥离 secrets**。
- `aiToolSurface.test.ts:38-49` 实例化**每一个真实插件**，遍历全部 40+ 工具，断言每个都声明 `policy.write` 且在 `always_reject` 下被拦。

### 7.3 355 项真正买到了什么

- ✅ 纯逻辑的真实行为覆盖：markdown 往返、日期/日历运算、节假日解析、案件归一化、Tiptap schema。
- ✅ **AI 工具授权策略**（全app 最高后果的安全面）覆盖扎实。
- ✅ **异步竞态纪律高于平均**：14 个文件用 deferred promise 强制乱序响应（`caseNavigation.test.ts:10-25` 晚到的 'a' 不覆盖 'b'；`processingCenter.test.ts:31-40` 轮询重叠 + 陈旧响应拒绝 + 卸载清定时器；`settingsSaveScope.test.ts:16-25` 在途保存不覆盖新输入）。
- ✅ **静态三方命令契约闸**（`contract.commands.test.ts`）——多数 Tauri 项目完全没有。
- ✅ 诚实断言**失败**路径的测试不少（`captureRetry.test.ts:42-48`、`personalAvailability.test.ts:38-40`）。

### 7.4 355 项**没有**买到什么

1. **数字暗示了不存在的 e2e 覆盖。** 应读作「**325 项 unit + jsdom 组件测试，0 项 e2e**」。
2. **`src/stores/tasks.ts`——最大的 store——零测试。** 它第 40 行和第 182 行**明确写着单调 load token 注释**（正是竞态那一类），而 `cases.ts` 里等价的守卫**是有测试的**。并发纪律在最大状态机上是缺席的。
3. **L2/L3 确认弹窗从未执行。** `context.ts:477-516` 的 `requestConfirm`（L3 要键入「确认」、正则 `/^确认$/`）**没有任何测试调用**——`aiToolSurface`/`aiWorkspace` 能过是因为 `always_reject` 在 `:338` 短路、`always_approve` 在 `:341` 走了 `skipConfirm`。把 L3 的 `inputPattern` 写反、或让 `catch { return false }` 返回 `true`，测试**全绿放行**。（子代理实测注入探针确认该路径可达且功能正常——只是没被测。）
4. **`tauriCallSafe`/`tauriCall` 的错误分支从未执行**（`:72-76`、`:103-111`），`setGlobalErrorNotify`（`:39`）、`openPath`（`:117`）零测试。
5. **参数形状漂移**只靠 `vue-tsc` 挡，测试只校验命令**名**是字符串。而 3.1 的头号缺陷（`Cmd<…, void>` + `!== null`）**正是类型系统放过去的**——`void | null` 让 `!==` 比较合法，typecheck 通过。
6. **路由、`App.vue`、`main.ts` 引导零覆盖**（`grep src/router tests/` 零命中）。

### 7.5 净结论

测试是**诚实的**——零同义反复、零快照、最浅的也断言真实行为，比典型 Vue/Tauri 套件好。
但「355 通过」**高估了信心**，主因是第 1 条（数字暗示了不存在的 e2e），次因是第 2、3 条（两个高后果、竞态/确认敏感的模块无测试）。
**用户应据此校准预期**：这份套件能保证"纯逻辑和授权策略没坏"，**完全不能保证"任何一条 UI→IPC→DB 的链在生产里是通的"**——本报告 3.1 节的 **19 条高危缺陷，没有一条会被现有测试发现**。因为它们的共同特征恰好是测试套件的三盲区：(a) 需要真实后端字段才能暴露的字段名错配（`caseName`/`deadlineDate`），(b) 需要真实交互才会发生的"UI 改了状态但没人取数"（筛选面板），(c) 需要真实 `void` 返回值才会暴露的类型陷阱（`defer_task`）。

---

## 八、修复建议（按投入产出排序）

### P0 — 一行到三行改动，直接消灭用户可见故障

| # | 改动 | 消灭 |
| --- | --- | --- |
| 1 | **`CaseListView.vue:775`**：`@update:filter` 处理器改成 `v => { Object.assign(casesStore.filter, v); casesStore.loadCases() }`（或加 `watch(() => ({...casesStore.filter}), casesStore.loadCases)`）。更稳的做法是让 `CaseFilterBar` 在 filter 变化后 300ms 防抖再 emit `search` | **本轮最严重的单点缺陷**：13 个筛选器全部失效 |
| 2 | `CaseFilterBar.vue:397`：「导出 CSV」接上父组件 `@export` → `CasesService.exportCases()` | 死按钮（`export_cases` 至今零调用） |
| 3 | `TasksView.vue:578-596`：把 `defer_task`/`clear_task_defer` 收进 `TasksService`（用 `tauriCallSafe`），判 `.ok`。顺带恢复 `task:changed` 事件 | 推迟功能完全不可用 |
| 4 | `CopilotSidebar.vue`：把 3 处 `toggleExpand(item.id)` 改成 `emit('toggle-expand', item.id)`（emit 已在 `defineEmits` 里声明，父组件监听已就位） | AI 引用卡片永不展开 |
| 5 | `CalendarView.vue:659`：`w.deadlineDate` → `w.dueDate` | 预测视图丢失全部法定期限信号 |
| 6 | `HomeView.vue:287`：`t.status === 'completed' \|\| t.status === 'done'` → `t.completed === 1` | 首页承诺复选框弹回 |
| 7 | `CalendarView.vue:546,755,1270,1448,1559`：`task.caseName` → 经 `cases` 列表解析 `caseId → caseName`（`loadCases` 已在跑，只是结果没用上） | 时间线案件名恒「常规待办」+ **案件名搜索恒 0 结果** |
| 8 | `GlobalSearch.vue:100`：`route: '/tasks'` → `` `/tasks?edit=${t.id}` ``（`TasksView.vue:254,262` 已支持该深链） | ⌘K 任务结果跳到列表页而非目标任务 |
| 9 | `CaseListView.vue:584`：判 `result.ok`，失败 `ElMessage.error`（照抄上方 `openFile`） | 假成功提示 |
| 10 | `CaseNetworkView.vue:172`：`<el-icon-connection />` → `<el-icon><Connection /></el-icon>` | 关系网络箭头全部不可见 |
| 11 | `CaseNetworkView.vue:133`：`relationTypeMap` 每项补 `icon` | 四个统计卡图标全空 |
| 12 | `KanbanView.vue:193`：把 `caseItem[statusField]` 存进局部变量再赋值，`:200` 用局部变量 | 审计日志丢失旧状态 |
| 13 | `CalendarView.vue:812`：把 `independent.ok` 纳入 `failed` 判定 | 三处视图静默数据错误 |
| 14 | `CaseDetailView.vue:349`/`:479`：给 `saveGoal`、`toggleTaskComplete` 补 `else ElMessage.error(result.error)` | 两处静默写失败 |

### P1 — 小改动，消灭"看起来能用其实没接线"的整块功能

| # | 改动 | 消灭 |
| --- | --- | --- |
| 9 | `stores/tasks.ts`：在 `TasksView.onMounted`（或 store 的 `setup`）调 `loadCustomPerspectives()` | 自定义透视重启即丢 + 列表恒空 |
| 10 | `TasksView.vue:1070`：`<AreasDialog v-model="showAreasDialog" @changed="loadAreas" />` | 领域改名后标签不更新 |
| 11 | `CalendarView.vue:1594-1595` + `348,356`：删掉日程编辑弹窗里多余的那个日期输入（只有一个日期列），或让 `startDate` 真正参与保存 | 静默丢弃编辑 + 假成功 |
| 12 | `CaseFilterBar.vue:220`：加 300ms 防抖 + 请求令牌（照抄 `PersonsView.vue:54-55`） | 客户筛选框搜索结果随机错乱 |
| 13 | `CalendarView.vue:832-855`：`loadCases` / `loadDeadlineWarnings` 补 `error` ref，接进 `:975` 的错误横幅（与同文件 `loadHolidays`/`loadTaskPlans` 对齐） | 三个 loader 静默失败 |
| 14 | `HomeView.vue:292`：判 `result.ok` 并 `ElMessage.error`（照抄 `TasksView` 的 `completeTaskOptimistic`） | 首页写失败零反馈 |
| 15 | `CalendarView.vue:832-837`：删掉 `loadTodayTasks`（结果无人读），同时从 `:343`/`:916` 移除调用 | 每次任务写多发一次全表查询 |
| 16 | `CalendarView.vue:655/712`、`661/717`：让头部统计与行列表共用同一套判定函数 | 三个数字与列表对不上 |
| 17 | `CaseListView.vue:793` 侧栏搜索加 300ms 防抖 | 每次按键一次全表 IPC |

### P2 — 需要设计决定

| # | 事项 | 说明 |
| --- | --- | --- |
| 17 | **`Cmd<P, void>` 的类型陷阱** | 这是 P0#3 的根因，会再次发生。建议二选一：(a) 让这些命令返回 `boolean`/`{ok:true}`；(b) 在 `tauriCall` 上给 `void` 结果加一个 `tauriCallVoid(command, args): Promise<{ok,error?}>` 专用签名，让 `!== null` 在类型上就非法。**这是本次审计里唯一一处"架构级"修复。** |
| 18 | **筛选变更 → 重新查询的通用约定** | 建议在 `stores/cases.ts` 里加一个 `setFilter(patch)` action 统一 `Object.assign` + `loadCases()`，所有 UI 走它。否则这个 bug 会在下一个筛选面板重演 |
| 19 | **`deadlineFrom/hearingFrom/hearingTo/operator` 5 个筛选** | 需要同时改三处：`stores/cases.ts:125-141` 转发、Rust `CaseFilter`（`db/cases.rs:92-109`）加列并实现谓词、`src/types/bindings.ts:59` 加字段。或先从 UI 移除这 5 个控件，别留无效输入 |
| 20 | `CaseListView.vue:650-654` 补 `observeChanges(casyContext, ['case','inbox'], loadCases)`；`KanbanView.vue` 同理 | AI/飞书改案件后列表与看板陈旧 |
| 21 | `CaseListView.vue:479` 备忘改走 `casesStore.updateCase` | 备忘切走再切回消失 |
| 22 | `CaseListView.vue:403`：`calendar.events(year, month, 3)` 传 `monthCount`，或按事件日期范围查询 | "全量"时间轴只覆盖当月 |
| 23 | `AddRelationDialog.vue:42` / `CaseNetworkView.vue:25` 的 50 条截断 | 案件 > 50 时下拉缺项、关系图丢边且计数少报（后者更危险，**静默丢数据**） |
| 24 | `CaseDetailView.vue:320-324`：`searchTerms` 改为逐词 `search()` 或走 `global_search` | 关联知识恒为 0 |
| 25 | `CaseDetailView.vue:458`/`:468`：给开庭记录加删除入口、状态标签接 `toggleHearingStatus`（或删掉 `cursor: pointer` 样式） | 功能不可达 / 误导性 affordance |
| 26 | `groupBy` + `CaseGroupPanel.vue`：接上组件或**删掉 242 行** | 分组功能不存在但 UI 提供了选项 |
| 27 | `TasksView.vue:26,97` 的 `TodayResetDialog` 与 `:95` 的 `showCreateDialog` | 决定是接线还是删除。272 行不可达代码会持续误导后续开发（含 README/设计文档的"已完成"叙述） |
| 28 | `TaskRow.vue` 的 `focusable`/`toggle-focus`、`TasksView.vue:66` 的 `selectedContextFilter`、`:979-1002` 抽屉缺 `areaId` 控件 | 都是"props/emits/state 齐了，UI 控件没做"。要么补控件，要么删声明 |
| 29 | `PerspectiveManager.vue:130` 改 `structuredClone(defaultFormData)` | 表单污染 |
| 30 | `CaseImportDialog.vue:1078` 头部按 `failedCount` 降级为警告色；`:1054` 预览标题改为"原始数据预览"，或真的接上归一化 | 假成功标题 + 误导性预览承诺 |
| 31 | 30 条死命令 | 建议逐一标注 deprecated 或从 `commandMap` 移除，减少"后端有能力=功能存在"的误读 |
| 32 | OCR 批量入口 + 看板/关系网络的 UI 入口 | `ocr_all_pending` 后端已有；`/cases/kanban`、`/cases/network` 目前是 URL-only |

### P3 — 补测试（针对本报告暴露的盲区，**不是**泛化提覆盖率）

| # | 补什么 | 为什么 |
| --- | --- | --- |
| 23 | `src/stores/tasks.ts` 的 load-token 竞态测试 | 照抄 `caseNavigation.test.ts:10-25`（`cases.ts` 已有，tasks.ts 没有）。这是最大 store |
| 24 | `context.ts:477-516` 的 L2/L3 `requestConfirm` 测试（stub `ElMessageBox`） | 安全控制从未被执行；写反了测试全绿 |
| 25 | `tauriCallSafe`/`tauriCall` 错误分支 + `setGlobalErrorNotify` | 桥接层错误处理零覆盖 |
| 26 | **DTO 字段名一致性测试**：对 `CalendarEvent`/`Task`/`DeadlineResult` 等投影，断言前端读取的每个字段名存在于 Rust DTO | 本次 3.1 的**两条高危（`caseName`、`deadlineDate`）都是这个类别**，且现有 355 项一条都发现不了。`contract.commands.test.ts` 已有命令名三方校验的框架，加一层字段名校验成本很低、收益极高 |
| 27 | 把 `tests/e2e/*.mjs` 至少接进 CI（哪怕需要 `CASY_E2E=1` 门控），或从文档里撤下"端到端"措辞 | 消除"355 暗示含 e2e"的误导 |

---

## 附：本次**没有**发现的问题（避免重复审计）

- **无 raw `invoke` 绕过**：`invoke` 的唯一 import 在 `tauriBridge.ts:1`。
- **无命令名拼写错误**：358 处 bridge 调用全在 `commandMap` 内，且有 `contract.commands.test.ts` 静态守着。
- **无 domain 名错配**：`mutation('task'|'file'|'knowledge'|'doc'|'calendar'|'project'|'plan')` 与 8 处 `observeChanges` 参数完全一致。
- **无未使用的 ref/computed/function**：全仓 121 个 `.vue` 扫描，仅 3 个误报。死代码集中在**函数级**（`proceedingsList`/`CaseGroupPanel`/`weekHours` 等），不是声明级。
- **任务链路的刷新正常**：任务改动会刷新 Home / CaseDetail / Calendar / 通知铃（有 `observeChanges` 覆盖 + 无 KeepAlive）。**案件链路才缺订阅**。
- **案件模块的每个 `catch {}` 都正当**（`CaseListView:455/615/635`、`CaseFilesPanel:103/168`、`ProcedureBoard:109` 全是 `ElMessageBox.confirm` 的取消），没有一个吞真错误。
- **`stores/cases.ts` 的 `listRequest`/`caseRequest` 令牌正确**丢弃被取代的响应，`finally` 有守卫。
- **`CasesService` 的"成功后 emit"纪律是对的**（`services/cases.ts:62,74,83` 都在 `result.ok` 之后）——缺的是消费侧。
- **`CaseImportDialog` 正确暴露了部分导入失败**（红色统计块 + 逐行异常明细 + `emit('imported')` 刷新列表）。这是本轮检查的最高风险区域，实现基本正确，只有 3.1 的标题/预览两处表述问题。
- **路由顺序 `/cases/:id` 在 `/cases/kanban` 之前没问题**——vue-router 4 静态段优先于参数段。
- **日历模块的 `delete_calendar_event` 假成功不可达**：`openEditDetail:271-277` 在弹窗前把所有非 `event` 类型分流走，删除按钮只对真实 `calendar_events` 行显示。
- **`tauriCallSafe` 永不 reject**，所以各处 `try/finally` 不会泄漏 unhandled rejection。
- **文档保存冲突链路完整**：文书工坊 / 写作页 / 知识笔记三处都接了 `SaveConflictDialog` + `EDIT_CONFLICT` 识别 + 冲突副本另存 + 暂停自动提交。
- **IPC 超时策略合理**：`convert_file_to_markdown` 与备份/导入/导出/飞书同步交给后端管完成，不做前端提前超时（避免"报失败但后台还在写"）。
- **i18n key 齐全**：日历用的 15 个 key 在 `zh-CN.json` 与 `en-US.json` 中都存在。
- **`startBucket` 取值合法**（`inbox`/`anytime`/`today` 满足 `schema.rs:2154` 的 CHECK 约束）；`monthCount`（3/4/12）满足后端 `1..=12` 守卫。

# Casy 全栈代码审查（全量重审 + 未提交改动）

> **日期**: 2026-09-04　**审查范围**: `src-tauri/src`（Rust 后端）+ `src`（Vue3/TS 前端）全量，以及工作区未提交改动（56 文件 / +2987 −3697）
> **基线**: `vue-tsc --noEmit` **0 错误** · `cargo test` **163 passed / 0 failed** · `npm audit` **未取得结果**（registry 503，耗时 7 分钟后放弃，未重试）
> **结论**: **2 项在野 P0** + **5 项潜伏 P0** · **8 项 P1** · **8 项 P2**；另**纠正上一轮报告 3 处结论**
> **方法**: 人工静态审查 + 并行深度核查。文中每条结论均回到源码逐行核实，附 `文件:行号`。凡本轮未能验证的，均已显式标注。

---

## 零、本轮与上一轮的关系（先看这张表，避免重复投入）

上一轮（2026-09-03）报告的 3 项 P0，**本轮逐一复验**：

| 上一轮 | 本轮复验 | 证据 |
|---|---|---|
| **P0-1** `update_case_status` 的 `track` SQL 注入 | ✅ **已修复** | `commands/cases.rs:794-803` 函数入口已加白名单，非法值 `return Err`，位于 SQL 执行之前 |
| **P0-2** `reveal_path` / `open_file_with_default` 路径穿越 | ⚠️ **只修了一半** | `reveal_path` ✅ 已修（`commands/files.rs:449-507`，canonicalize + 反查案件 + `starts_with`）；**`open_file_with_default` ❌ 未修**（`files.rs:509-534`，本次 diff 完全未触碰该函数） |
| **P0-3** 三处 `v-html` XSS | ✅ **三处全部已修复** | `HomeView.vue:710` / `KnowledgeSidebar.vue:42` / `DocumentGenView.vue:95` 均已接入 `sanitizeInlineHtml` / `sanitizePreviewHtml`（`shared/markdown/mdBridge.ts:246-262`，`ALLOWED_ATTR: []` 剥离全部属性，`<b>` 在白名单内故 FTS5 高亮保留——取舍正确） |

**但是**：上一轮的 XSS 清点**不完整**。它只找到 6 处 `v-html`，漏掉了 `src/modules/docs/extensions/` 目录下的 **2 处渲染点**，其中 1 处是完全绕过 Vue 的原生 `innerHTML`。**这两处是本轮新发现的在野 P0**（详见第一节）。

同时，本轮批量改动**新引入**了 3 个接受路径参数、且无目录归属校验的写入命令（`export_edited_docx`、`export_knowledge_markdown`、`copy_file_with_progress`），加上原有的 `export_docx`。

---

## 一、P0 —— 在野（无需前置条件，正常操作即可触发）

### P0-1｜`BlockReference.ts:83` 知识库正文裸 `v-html`

**位置**：`src/modules/docs/extensions/BlockReference.ts:83`

```ts
<div class="block-ref-body" v-html="blockData.block?.content || blockData.item?.content || ''" />
```

**数据来源**（同文件 `:43-51`，已逐行确认）：`casyContext.knowledge.getWithBlocks(knowledgeId)` → `result.data.blocks[].content` / `result.data.item.content`。

**为什么是在野 P0**：这正是威胁模型里定义的**跨信任边界数据**——知识库正文来自导入的 Markdown、OCR 卷宗、飞书同步、AI 生成。律师在文书编辑器里插入一个块引用，对方当事人提交材料中的 `<img src=x onerror=...>` 即原样执行。

与已修复的 `KnowledgeSidebar:42`（FTS5 snippet，同一数据源）相比，**此处完全无净化**。上一轮修了 sidebar，没扫到这个目录。

**影响**：webview 取得脚本执行权 → 可调用任意 Tauri 命令 → 读全库 / 操作文件系统。

**修复**：改为 `v-html="mdToSafeHtml(...)"`（content 为 Markdown）或 `sanitizePreviewHtml(...)`（若为 HTML）。注意该组件用**字符串模板**（`template:` 选项式），需在 `setup` 里返回净化后的 computed，不能直接在模板中 import 函数。

---

### P0-2｜`WikiLinkSuggestion.ts:162` 原生 `innerHTML` 拼接知识标题

**位置**：`src/modules/docs/extensions/WikiLinkSuggestion.ts:162-165`

```ts
div.innerHTML = `
  <span style="font-weight: 500;">${item.title}</span>
  ${item.category ? `<span style="...">${item.category}</span>` : ''}
`
```

**为什么是在野 P0**：`item.title` / `item.category` 是知识条目字段（用户输入 / 导入 / 同步），**未转义**。这是**绕过 Vue 的原生 DOM 操作**——模板层任何净化都覆盖不到，静态扫 `v-html` 也扫不出来。这正是上一轮遗漏的机理。

**强对照组（证明是遗漏，不是设计选择）**：同项目 4 个平行的建议浮层**全部正确使用 `textContent`**：

| 文件 | 行号 | 写法 |
|---|---|---|
| `composables/partyNameSuggestion.js` | :74 | `div.textContent = ...` ✅ |
| `composables/legalProvisionSuggestion.js` | :90 | `div.textContent = ...` ✅ |
| `composables/caseFieldSuggestion.js` | :77, :81 | `div.textContent` / `span.textContent` ✅ |
| `composables/knowledgeReferenceSuggestion.js` | :73 | `div.textContent = ...` ✅ |
| **`extensions/WikiLinkSuggestion.ts`** | **:162** | **`div.innerHTML = \`...${item.title}...\`` ❌** |

这 4 个文件本批都被改过（改的是补 `PluginKey`，与安全无关），而 `WikiLinkSuggestion.ts` 因不在 `composables/` 目录下被整轮清理漏掉。

**修复**：改用 `createElement` + `textContent`，与 4 个兄弟文件对齐；样式移入 CSS 类。

---

## 二、P0 —— 潜伏（需先取得 webview 执行权，但这是把 XSS 升级为完全失守的放大器）

严重性说明：以下 5 项在**正常 UI 流程下传的都是后端生成或用户自选的路径**，不会自发触发。但它们是任意文件处置/写入原语，一旦 P0-1/P0-2 的 XSS 成立，即构成完整攻击链。且**修复成本极低**（`reveal_path` 已经提供了可直接复用的样板）。

### P0-3｜`open_file_with_default` 无路径校验（上一轮 P0-2 的漏修项）

**位置**：`src-tauri/src/commands/files.rs:509-534`

```rust
pub async fn open_file_with_default(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    { std::process::Command::new("open").arg(&path).spawn()... }
```

原始 `path` 直接透传给 `open` / `cmd /C start` / `xdg-open`。macOS 的 `open` 会**直接执行** `.command` 等可执行文件，构成任意文件处置 / 代码执行原语。

**已在 `commands/mod.rs:362` 注册为 Tauri 命令**。当前 3 个前端调用点（`DocumentGenView.vue:506`、`KnowledgeNotebookView.vue:159`、`CaseListView.vue:618`）在正常流程下传的都是后端路径，故为潜伏。

**修复**：与已修好的 `reveal_path`（`files.rs:449-507`）用同一套校验——`canonicalize` + 反查 `case_files` + `starts_with(root)`。这是同文件内现成的代码，成本约 20 行。

---

### P0-4｜`export_docx` / `export_edited_docx` 的 `output_path` 零校验 → 任意文件写入

**位置**：`commands/docs.rs:76-105`（`export_docx`，既有）+ `commands/docs.rs:107-123`（`export_edited_docx`，**本批新增**）；根因在两处路径解析：

```rust
// docsy_engine/export.rs:31-34
let output = match output_path {
    Some(p) => PathBuf::from(p),      // ← 零校验
    None => generate_output_path(template)?,
};
if let Some(parent) = output.parent() { fs::create_dir_all(parent)?; }   // :37-39

// docsy_engine/rich_export.rs:477-480
fn resolve_output_path(title: &str, output_path: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = output_path { return Ok(PathBuf::from(path)); }   // ← 零校验
    let output_dir = crate::runtime_paths::export_root();                 // :481 才有默认目录
}
```

前端侧 `outputPath` 是显式入参（`types/commandMap.ts:306` 声明 `outputPath?: string | null`；`useDocsyBridge.js:80` `exportDocx(templateId, caseId, outputPath = null)`）。

**为什么是潜伏**：`DocumentGenView` 的两个调用点（`:478`、`:483`）**均未传 `outputPath`**，走后端默认目录；`KnowledgeNotebookView.vue:178` 传的是 `chooseExportPath()` 保存对话框返回的用户自选路径。故正常流程下不会自发触发。

**本批新增的 `export_edited_docx` 复制了同一缺陷**——说明这不是一次性疏忽，而是导出链路缺少统一的路径守卫。

**修复**：`output_path` 为 `Some` 时，`canonicalize` 后校验 `starts_with(export_root())`，否则忽略该参数回退默认目录。建议在 `docsy_engine` 层做一次，两个命令同时受益。

---

### P0-5｜`export_knowledge_markdown`（本批新增）仅校验绝对路径与扩展名

**位置**：`src-tauri/src/commands/knowledge.rs:616-656`

```rust
if !path.is_absolute() { return Err(...); }                    // :622 弱校验 1
if extension != "md" && extension != "markdown" { ... }        // :630 弱校验 2
std::fs::write(path, markdown.as_bytes())?;                    // :647 任意绝对 .md 路径
```

只校验"绝对路径"和".md 扩展名"，**未做目录归属校验**。写入内容来自 `knowledge_items.content`（可被 AI 生成 / 飞书同步 / 导入文档污染，即跨信任边界数据）。

**正面对照**：本批新增的 `KnowledgeExportDto`（`knowledge.rs:609-613`）**正确加了** `#[serde(rename_all = "camelCase")]`——说明新代码在命名契约上是对的，但路径安全上漏了。

**修复**：限定 `output_path` 必须 `canonicalize` 后落在导出目录内。

---

### P0-6｜`copy_file_with_progress` 的 `target_category` 裸拼路径

**位置**：`src-tauri/src/commands/inbox.rs:2125-2130`（已注册，`commands/mod.rs:305`）

```rust
let cases_root = crate::runtime_paths::documents_root()
    .join("cases").join(&folder_name).join(&target_category);   // ← target_category 原始 String
std::fs::create_dir_all(&cases_root)?;                          // 穿越目录会被一并创建
std::fs::copy(source, &target)?;                                // :2157
```

`target_category` 是原始命令入参，**未走本文件 `:2084` 现成的白名单 `category_to_folder()`**。`Path::join` 不归一化 `..`，OS 在写时解析。

**重要限定（影响严重性判定）**：本命令**前端零调用点**（已 grep 确认 `src/` 下无 `copy_file_with_progress` 引用），故为潜伏，且当前无自发触发路径。

**活路径是安全的（已核实，避免误判）**：真正的收件箱落卷走 `confirm_inbox_action`(:2225) → `file_inbox_item`(:917) → `crate::files::file_to_case`(`files/mod.rs:378`) → `route_file_to_subdir`（`:186-208`，category 经 match 映射为**固定子目录名集合**中的一项）+ `smart_rename`（`:240-262`，category 映射为固定中文标签，默认"其他"）。**两处均为白名单，活路径无穿越风险。**

**修复**：`target_category` 经 `category_to_folder()` 映射后再 `join`；或直接删除这个无调用方的死命令。

---

## 三、P1 —— 正确性缺陷（会导致错误行为或数据问题）

### P1-1｜`docs.rs` 缺 `rename_all` → 文书「未填充字段」告警**永不显示**、导出后「打开文件」**必然失败**

这是本轮**价值最高的发现**——一条正在生效的功能性 Bug，而非理论风险。

**Rust 侧**（`commands/docs.rs:14-27`）：

```rust
#[derive(Debug, Serialize, Deserialize, specta::Type)]   // ← 无 rename_all
pub struct RenderResponse { pub html: String, pub text: String,
    pub used_fields: HashMap<String,String>, pub missing_fields: Vec<String> }

#[derive(Debug, Serialize, Deserialize, specta::Type)]   // ← 无 rename_all
pub struct ExportResponse { pub output_path: String, pub file_size: u64, pub exported_at: String }
```

Tauri v2 **只对入参做 camelCase→snake_case 自动转换，返回值序列化完全由 serde 决定**。故下行 JSON 是 **snake_case**。

**前端侧**按 camelCase 断言（`core/services/docs.ts:46-59`）：`usedFields` / `missingFields` / `outputPath` / `fileSize` / `exportedAt`。

**可观测后果**（`DocumentGenView.vue`）：

| 行号 | 代码 | 实际运行 |
|---|---|---|
| :49 | `已填充 {{ Object.keys(renderResult.usedFields \|\| {}).length }} 个` | `usedFields` 为 `undefined` → **永远显示"已填充 0 个"** |
| :150 | `v-if="renderResult.missingFields?.length > 0"` | → **「有 N 个字段未填充」告警永不出现** |
| :499 | `ElMessage.success(\`DOCX 已导出: ${result.data.outputPath}\`)` | → **"DOCX 已导出: undefined"** |
| :506 | `casyContext.files.open(result.data.outputPath)` | → **以 `undefined` 调用，必然失败** |

**为什么是 P1 而非 P2**：`:150` 的缺失字段告警是律师生成法律文书时**唯一的占位符未填充防线**。它静默失效意味着可能提交带未替换占位符的正式文书。

**决定性对照（证明是 `docs.rs` 的遗漏）**：
1. 全仓 20+ 个模块都用了 `rename_all = "camelCase"`，`docs.rs` **全文件无一处**；
2. 本批**新增**的 `knowledge.rs:609-613` 的 `KnowledgeExportDto` 就**写对了**；
3. 于是同一"导出"功能的两个命令线上命名相反，直接证据是本批新写的防御性代码（`KnowledgeNotebookView.vue:180`）：
   ```js
   const exportedPath = result.data?.outputPath || result.data?.output_path || outputPath
   ```
   开发者在运行时撞到过这个不一致，选择就地兜底而非修契约。

**附带核实**：`src/types/bindings.ts`（Specta 生成）中**不存在** `ExportResponse` / `RenderResponse`（已 grep 确认无 `used_fields` / `output_path`），故这两个结构体**未纳入 Specta 类型生成**——这也是前端只能手写断言、无从发现矛盾的原因。

**修复**：给两个结构体补 `#[serde(rename_all = "camelCase")]`，或纳入 Specta 生成后前端改为 import 生成类型；删除 `docs.ts` 的手写重复类型与 `:180` 的兜底。

---

### P1-2｜时区：上一轮只修了 1 个入口函数，全仓 **19 处**同类写法未扫

上一轮修复的 `daysUntil`（`modules/tasks/utils/taskDisplay.ts:9-20`，改本地午夜解析）**经复验正确** ✅。

但**同一文件 30 行下方**的 `formatDate` 仍是旧写法（`taskDisplay.ts:50-61`）：

```ts
const date = new Date(dateStr)                            // UTC 解析
const iso = (d: Date) => d.toISOString().split('T')[0]    // :55 本地时间转 UTC 取日期
if (dateStr === iso(today)) return '今天'
```

**Bug 机理**（东八区，全部目标用户）：北京时间 `09-04 03:00` → `toISOString()` = `'2026-09-03T19:00:00.000Z'` → 取日期得 **`'2026-09-03'`**。即**每日 00:00–07:59 的 8 小时窗口内，"今天"整体偏早一天**。

全仓 **19 处**该写法（已逐一 grep 核实）。按危害排序，真正影响期限判定的：

| 位置 | 后果 |
|---|---|
| `stores/tasks.ts:317` `isTaskOverdue` | 凌晨窗口内，**昨天到期的任务不被判为逾期** |
| `modules/cases/views/CaseDetailView.vue:156` | 案件逾期计数偏低 |
| `modules/clients/views/ClientView.vue:33` | 客户视图漏算逾期 |
| `shared/components/OverdueMorningBrief.vue:29,74,80` | **早报**：`shouldShow()` 用它判断"今日是否已展示"，凌晨取到昨天日期 → **早报在它最该出现的清晨时段被跳过** |
| `modules/cases/components/CaseFilterBar.vue:158,170` | "逾期"快捷筛选上界偏早一天 → **今日刚逾期的案件筛不出来** |
| `taskDisplay.ts:55` | 到期日显示"明天"而实际是今天 |
| 其余：`stores/tasks.ts:95,109,126,430,438,443`、`KanbanView.vue:212`、`CaseListView.vue:513`、`AICompanionView.vue:103` | 同类偏移 |

**另两处混用本地/UTC 基准导致 off-by-one**：`stores/tasks.ts:323-329` `isTaskDueSoon`（`new Date(task.dueDate)` UTC 减 `new Date()` 本地再 `Math.ceil`，差值含小时分量，"今天到期"随时刻在 0/1 间跳变）；`taskDisplay.ts:64-70` 与 `stores/tasks.ts:334-339` 两份同名 `getWaitingDays`（重复实现，且带同一缺陷）。

**影响**：律师错过法定期限 = 丧失权利。这是本项目最高价值的关注面。

**修复**：把 `daysUntil` 里已验证正确的本地午夜解析抽成 `parseLocalDate(s)` + `todayLocalISO()`，替换全部 19 处；补一个把时区设为**负偏移**（如 `America/New_York`）的单测——正偏移时区测不出反方向错误。

---

### P1-3｜案件列表在第 50 条静默截断，同时 UI 显示真实总数

**位置**：`stores/cases.ts:39`（`perPage: 50`）、`:126`、`modules/cases/views/CaseListView.vue:847`（`:total="casesStore.total"`）

三条证据链均已 grep 核实：
1. `casesStore.page` **全仓只被重置、从不被递增**（`CaseListView.vue:762` 是唯一赋值点，值为 `1`）
2. **全仓零分页、零虚拟滚动**（`grep -rln "el-pagination|virtual-scroll|useVirtualList|RecycleScroller"` → 无命中）
3. 但 `:847` 把 `total`（DB 真实计数）传给筛选栏展示

**影响**：案件数超过 50 时，界面显示"共 120 件"却只渲染前 50 件，**其余 70 件在应用内无任何路径可达，且无任何截断提示**。对案件管理工具是数据可达性缺陷，不是性能问题。上一轮把这条归为 P2「渲染压力」，**低估了**——真正的问题不是慢，是**看不见**。

旁证：各调用点自行拍不同上限（`DocumentGenView.vue:336` 写 `perPage: 500`），说明契约缺口已被感知但未系统解决。

---

### P1-4｜本批改动引入回归：`resolveCaseName` 在案件超出首页时返回空串

**位置**：`modules/home/HomeView.vue:313-325`（本批新增）

```js
const caseNameMap = computed(() => {           // 数据源受 perPage: 50 限制（见 P1-3）
  const map = new Map()
  for (const c of casesStore.cases || []) { if (c.id && c.caseName) map.set(c.id, c.caseName) }
  return map
})
function resolveCaseName(task) {
  if (!task?.caseId) return ''
  return caseNameMap.value.get(task.caseId) || ''     // ← 未命中即空串
}
```

**改动前后**（`git diff` 核实）：

```diff
-    caseName: t.caseName || t.caseId || '重点在办案件',
+    caseName: resolveCaseName(t),
```

原意是好的（避免在卡片上暴露原始 UUID），但任务关联的案件若不在前 50 条内，`resolveCaseName` 返回 `''`，配合同时新增的 `v-if="c.caseName"`（`:625`）→ **案件名整行消失**。改动前至少有 `'重点在办案件'` 兜底。

其余三处（`:399`、`:429`）保留了 `|| '重点案件'` 兜底，影响较轻——**同一次改动内兜底策略不一致**，进一步说明是疏漏。

**修复**：`resolveCaseName` 保留降级链（`caseNameMap → task.caseName → '重点在办案件'`）；根本解在 P1-3。

---

### P1-5｜早报失败时**编造**随机逾期数据

**位置**：`shared/components/OverdueMorningBrief.vue:54-66`

```js
} else {
  brief.value = {                                     // 回退"占位数据"
    overdueDeadlines: Math.floor(Math.random() * 5) + 1,
    overdueTasks: Math.floor(Math.random() * 3),
    dueTodayTasks: Math.floor(Math.random() * 4) + 1,
    todayHearings: Math.floor(Math.random() * 2),
  }
}
} catch (e) { console.warn(...); brief.value = { overdueDeadlines: 2, overdueTasks: 1, dueTodayTasks: 3, todayHearings: 1 } }
```

命令失败时向律师展示 `Math.random()` 生成的**虚构逾期数量**，UI 上与真实数据无任何视觉区分，仅 `console.warn` 一行。

**影响**：在期限管理场景下，**编造的数据比报错更危险**——律师看到"3 项逾期"会去找这 3 项，或反之因显示"0 项"而漏看真实逾期。结合 P1-2，早报本身在清晨时段还可能被跳过，两个缺陷叠加。

**修复**：失败即展示错误态/空态（项目已有 `EmptyState.vue`、`StateFeedback.vue`），绝不合成业务数字。

---

### P1-6｜撤销删除任务：id 漂移 + 完成状态被清零

**位置**：`core/taskActions.ts:119-136` + `src-tauri/src/commands/tasks.rs:198,234`

前端 undo 闭包把**运行时完整任务对象**传给 `create`：

```ts
const create = await casyContext.tasks.create(task as unknown as Record<string, unknown>)
```

后端 `create_task` 则：

```rust
let id = db::new_id();                                    // :198 忽略入参 id，另生成
// INSERT ... VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ...)   // :234 completed 硬编码 0
```

两个已核实的事实性后果：
1. **id 漂移**：恢复出的任务在 DB 里是**新 id**，但前端 `hooks.restore?.()` 放回列表的是**带旧 id 的对象** → 后续对该行的一切操作（完成/延期/改期）都打到不存在的 id。若有其他表（提醒、案件日志、顺序依赖 `sequence_order`）引用原 id，引用全部悬空。
2. **`completed` 被清零**：INSERT 中 `completed` 是**字面量 `0`**，不读取入参 → 撤销一个**已完成**任务的删除，恢复出来是**未完成**。

**对上一轮的纠正**：上一轮称"撤销时仅用 `TaskLike` 的 6 个字段重建，`notes`/`priority` 全部丢失"。**经核实不准确**——运行时传的是完整对象（含 notes/priority/context 等），`TaskLike` 只是结构性类型声明（7 个字段），不构成运行时裁剪。真正的问题是上面两条。

**修复**：`create_task` 支持传入 `id`（存在则按指定 id 插入）与 `completed`；或新增专用 `restore_task(snapshot)` 命令走完整快照还原。

---

### P1-7｜类型契约真实覆盖率 **16.8%**，上一轮的 60% 基于错误前提

**复验数字**：

| 指标 | 上一轮 | 本轮实测 |
|---|---|---|
| `CommandMap` 键数 | 158 | **159**（本批 +2：`export_knowledge_markdown`、`export_edited_docx`） |
| 调用点总数 | 282 | **280** |
| **真正获得编译期检查** | 168 (59.6%) | **47 (16.8%)** |

**上一轮 168 为何不成立**——关键在重载解析（`core/tauriBridge.ts:24-31`）：

```ts
export async function tauriCallSafe<K extends keyof CommandMap & string>(
  command: K, args: CommandMap[K]['params']): Promise<TauriResult<CommandMap[K]['result']>>   // 重载1：有契约
export async function tauriCallSafe<R = unknown>(
  command: string, args?: Record<string, unknown>): Promise<TauriResult<R>>                    // 重载2：无契约
```

**只要显式写了泛型**（如 `tauriCallSafe<DocsyExportResult>('export_docx', ...)`），泛型实参不满足重载 1 的约束，重载 1 落选，**必然命中重载 2** —— 参数零校验，返回值纯断言。

四分类实测：A 无泛型且已入表 **47**（16.8%，✅ 双向检查）｜B 无泛型未入表 16（❌）｜C **显式泛型 172（61.4%，❌ 纯断言）**｜D `tauriCall<T>` 45（❌，`tauriBridge.ts:61` 无 CommandMap 重载）。

C 类中有 **82 处命令其实已在 `CommandMap` 里**，却因显式泛型把现成契约丢掉，其中 **48 处泛型与 `CommandMap` 声明直接矛盾**（如 `services/cases.ts:11` 的 `<CaseListResponse>` vs 生成的 `CaseListResult`——手写类型**少 `page`/`perPage` 两字段**，正是 P1-3「无人做分页」的类型层帮凶）。

**核心写命令入表情况**（逐一核实）：`create_task`/`update_task`/`toggle_task`/`delete_task`/`create_draft`/`update_draft`/`delete_case` **全部 MISSING**；`create_case`/`update_case`/`update_case_status` **IN**。同域内不一致——cases 域的写命令已入表，tasks 域一个都没有。上一轮"最核心的写操作恰恰没有类型保护"**仍成立且未改善**。

**修复**：① 让"显式泛型"无处可写（如把动态命令拆到独立 `tauriCallUnsafe`）；② 删除与 `bindings.ts` 重复的手写类型，统一 import 生成产物；③ 优先把 task/draft 写命令入表。

---

### P1-8｜提醒等级靠正则刮取中文展示文案，无视后端已提供的结构化字段

**位置**：`shared/components/ReminderBanner.vue:16-31`

```js
if (line.startsWith('案件:')) result.caseName = line.replace('案件:', '').trim()
if (line.startsWith('剩余:')) { const m = line.match(/-?\d+/); if (m) result.daysLeft = parseInt(m[0]) }
```

`classifyLevel`（`:33-39`）据此推 R2/R3/R4 并决定是否弹逾期横幅。但后端**已经提供**结构化等级（`types/bindings.ts:15`：`DeadlineWarning = { daysLeft, level, levelLabel, levelColor, message }`）。

**影响**：任何对提醒文案的措辞调整或 i18n 化（项目已装 vue-i18n、已有 `src/locales/`）都会让 `startsWith('案件:')` 全部失配 → `daysLeft` 变 `null` → **逾期横幅静默不再弹出**。

**对照**：同项目 `ReminderView.vue:40-43` **做对了**——`if (entry.level && ['R1','R2','R3','R4'].includes(entry.level)) return entry.level`，后端字段优先、仅缺失时回退解析。`ReminderBanner` 无此优先级，是严格更差的实现。

**修复**：对齐 `ReminderView` 的"后端字段优先"；两份近乎相同的解析函数合并为一处，仅作兼容旧数据的回退。

---

## 四、P2 —— 一致性与可维护性

| # | 问题 | 位置 | 说明 |
|---|---|---|---|
| P2-1 | 净化器在模板中按渲染调用 | `HomeView.vue:710`、`KnowledgeSidebar.vue:42`、`DocumentGenView.vue:95`、`BriefingModal.vue:364` | 安全性正确，但写在模板里是**函数调用**，每次重渲染都重跑 DOMPurify；`KnowledgeSidebar:42` 还在搜索结果 `v-for` 内（每项 × 每次渲染）。对比 `KnowledgeNotebookView.vue:196` 的 `computed(() => mdToSafeHtml(...))` 才是正确范式 |
| P2-2 | `computed` 内建 DOM 且未净化 | `DocumentGenView.vue:301-307` | `const div = document.createElement('div'); div.innerHTML = editedHtml.value`。已核实当前**不可利用**（Rust 侧 `docsy_engine/renderer.rs:151,154` 对所有输出路径调了 `escape_html`，`renderResult.html` 是转义过的），属纵深防御缺口。建议改 `DOMParser` + 净化 |
| P2-3 | 竞态守卫的 `loading` 未在 stale 分支复位 | `useDocsyBridge.js`（本批新增） | `renderRequestId` 机制本身正确，但 stale 早退分支未复位 `loading`。实测各交错顺序下均由后继请求兜底，**当前无可见 Bug**，属脆弱实现。`DocumentGenView.vue:409-411` 的 `previewing` 用裸布尔，两次预览重叠时先返回者会提前关掉 spinner |
| P2-4 | `catch {}` 吞掉真实失败 | `KnowledgeNotebookView.vue:153-161`（本批新增）等 **12 处** | `try` 同时包住 `ElMessageBox.confirm`（取消即 throw，属预期）和 `casyContext.files.open(path)`（:159）。用户点"打开文件"若失败无任何反馈。全仓 12 处空 catch（`TasksView:462,591`、`CalendarView:158,217,253,398`、`ReminderView:141`、`CaseListView:503,669,681`、`KnowledgeNotebookView:160,352`）。建议区分"用户取消"与"真实错误" |
| P2-5 | 重复实现 | `taskDisplay.ts:64-70` 与 `stores/tasks.ts:334-339` 两份 `getWaitingDays`；`ReminderBanner.vue:16` 与 `ReminderView.vue:17` 两份消息解析器 | 两份 `getWaitingDays` 都带 P1-2 的时区缺陷，修复时易漏改 |
| P2-6 | 死代码 | `shared/components/SkeletonCard.vue` | 复验确认全仓 0 引用 |
| P2-7 | 硬编码色板绕过主题令牌 | `TasksView.vue:105-116`、`taskDisplay.ts:38-43` | `TasksView` 的 `perspectives` 中 `all` 用 `var(--c-primary)`，**其余 10 项全是裸 hex** —— 同一数组内自相矛盾，是令牌化重构的漏网点 |
| P2-8 | 巨型组件（复验） | `CalendarView.vue`(script 878 行)、`CaseListView.vue`(792)、`TasksView.vue`(699)、`CaseImportDialog.vue`(613)、`WhiteboardView.vue`(508)、`CaseDetailView.vue`(487)、`HomeView.vue`(486，含 25 个 computed) | 与上一轮一致、未改善。`HomeView` 的 25 个 computed 集中了全部首页派生逻辑，P1-4 的回归即出在其中 |

**后端 P2 补充**：
- `watcher.rs:24-58`：`mem::forget(watcher)` 常驻 + 无重入保护（重复调用会起多个监听线程）+ 每事件 `open_db()` 重做 KDF。
- `commands/reminder.rs:1050-1058`：osascript 字符串拼接，仅转义 `"` 与换行，未处理反斜杠。
- `db/schema.rs` 迁移**非事务化**：本批新增的 `DECISIONS_REBUILD_V23B` 设计合理（先 `INSERT...SELECT` 复制再 `DROP`，复制失败不丢原表；用 `sql.contains("'recommendation'")` 做幂等判定 ✅），但 `run_migrations` 整体未包事务，其他 12 步重建型迁移若在 DROP 后、CREATE 前崩溃会留下缺表状态。
- `db/mod.rs:77-90` 新增的 `with_conn` 用 `std::sync::Mutex` 串行化，**不可重入**，闭包内再调 `with_conn` 会死锁；且存量命令仍走每命令重开的 `open_db()`，**不受该锁保护**，并发写仍可能 `SQLITE_BUSY`（上一轮 P1-5 部分仍成立）。
- `formula/parser.rs:376,388,397` 等处用 `panic!` 而非 `Result` 处理异常 AST。
- `credentials/mod.rs:256-268` / `db/mod.rs:124-133`：keychain 不可用时密钥明文落盘（0600）。密钥本身为 `getrandom` 强随机、强度可接受，属需知悉的取舍。

---

## 五、对上一轮报告的纠正（避免按错误结论排查）

| 上一轮结论 | 本轮复验 |
|---|---|
| 「`ProposalDiffCard.vue:181` 的 `tickTimer` 无 `onUnmounted` 清理」 | ❌ **误判**。清理存在于 `ProposalDiffCard.vue:187-188`：`onBeforeUnmount(() => { if (tickTimer) clearInterval(tickTimer) })`。上一轮只 grep 了 `onUnmounted`，漏掉 `onBeforeUnmount`。**无泄漏** |
| 「282 次调用中 168 次命中 CommandMap（59.6%）」 | ❌ **前提有误**。显式泛型会使重载 1 落选，真实命中 **47（16.8%）**，见 P1-7 |
| 「撤销删除仅用 `TaskLike` 的 6 个字段重建，notes/priority 全丢」 | ❌ **不准确**。运行时传的是完整对象，字段未丢。真实问题是 **id 漂移 + `completed` 被清零**，见 P1-6 |
| 「P2：案件主列表裸 `v-for`，数据量增长后有渲染压力」 | ⚠️ **低估**。真问题是 50 条静默截断 + 无分页 + 显示真实总数 → 数据不可达，应升 P1-3 |
| 「Tauri 事件监听 3 处均正确 unlisten」 | ⚠️ 不完整。`safeListen` 实际 **5 处**；`App.vue:175` 的 `tauri://drag-drop` **未接收 unlisten 句柄**（与同函数 :170 写法不一致）。因 `App.vue` 为根组件不卸载，**实际无泄漏**，属不一致 |

**上一轮以下结论经复验依然成立**：315 处 `unwrap` / 22 处 `expect` 在命令路径上无可 panic 点；`parseWhen` 未进渲染循环；`deep: true` 仅 2 处（`stores/aiSettings.ts:40`、`KnowledgeNotebookView.vue:331`，后者带 `hydrating` 门闩与 `editRevision` 版本号的防抖自动保存，设计得当）；`mockData.ts` 为浏览器预览桥接必需，非死代码；AI 网关 `origin` 信任模型（荣誉制，现状无法绕过）。

---

## 六、本批未提交改动的质量评估

**改得好的部分**（明确记录，避免后续误改回去）：
- `cases.rs:794-803` 的 `track` 白名单——注释明确回指审查编号，位置正确（SQL 之前）。
- `files.rs:449-507` 的 `reveal_path` 校验——canonicalize + 反查案件 + `starts_with` 三层，且注释说明了"现有 4 处调用点均传已登记案件文件，零误伤"。**这套代码正是 P0-3/P0-4/P0-5/P0-6 可直接复用的样板。**
- `BriefingModal.vue` 重写（−1586/+915）——**无安全回归**。新版 `renderMarkdown` 保留了先 `escapeHtml`（`:255`，转义 `& < >`）再只拼 `<strong>` 的自转义机制，所有插值落在文本位置而非属性位置。✅
- `knowledge.rs:609-613` 新结构体的 `rename_all = "camelCase"` ✅（与 `docs.rs` 的缺失形成对比）。
- `db/mod.rs` 隔离 profile 不读写正式 Keychain、密钥只存测试目录 ✅——测试隔离改进，避免测试污染生产密钥。
- `db/mod.rs` 的 `key_file_path()` / `db_path()` 统一走 `runtime_paths::data_root()` ✅——消除了原先 `dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))` 在 `data_dir()` 不可用时**把数据库和密钥文件写到当前工作目录**的隐患。
- `schema.rs` 的 `DECISIONS_REBUILD_V23B` 幂等判定与复制顺序设计合理 ✅。
- 新增 `test_export_knowledge_markdown_uses_saved_content_and_validates_extension`（`knowledge.rs:1618+`）✅ 有测试意识。

**新引入的问题**：
- `export_edited_docx`（`docs.rs:107-123`）——复制了 `export_docx` 的路径校验缺失（P0-4）。
- `export_knowledge_markdown`（`knowledge.rs:616-656`）——仅校验绝对路径与扩展名（P0-5）。
- `HomeView.vue:313-325` —— `resolveCaseName` 丢兜底（P1-4）。
- `KnowledgeNotebookView.vue:153-161` —— `catch {}` 吞掉打开失败（P2-4）；`:180` 的 `outputPath || output_path` 兜底掩盖了契约不一致（P1-1 的现场证据）。
- `useDocsyBridge.js` —— stale 分支未复位 `loading`（P2-3，当前无可见 Bug）。

---

## 七、建议修复顺序

1. **P0-1 / P0-2**（两处在野 XSS，各 5–10 行）——净化基础设施已就绪，成本最低、收益最高。`WikiLinkSuggestion` 直接照 4 个兄弟文件改 `textContent`。
2. **P0-3**（`open_file_with_default`，约 20 行）——上一轮 P0-2 的漏修项，复用同文件 `reveal_path` 现成代码。
3. **P0-4 / P0-5**（导出路径守卫，建议在 `docsy_engine` 层做一次，两个/三个命令同时受益）——其中 P0-5 是本批新代码，趁热修。
4. **P1-1**（`docs.rs` 补两个 `rename_all`）——改动最小，直接修掉一个正在生效的文书告警失效 + 导出打开失败。
5. **P1-2**（时区 19 处）——先抽 `parseLocalDate` / `todayLocalISO` 两个工具再批量替换，补负偏移时区单测。**期限计算是本项目最高价值面。**
6. **P1-5**（删除编造的早报数据）——单文件、低风险。
7. **P1-3 + P1-4**（分页缺口与其引发的首页回归）——一起做；`resolveCaseName` 先补降级链止血。
8. **P1-6**（撤销删除 id 漂移 + completed 清零）——直接关乎律师场景的数据完整性。
9. **P0-6 / P1-7 / P1-8 / P2** 按迭代节奏收敛。

---

## 八、复验方式与遗留项

```bash
npm run typecheck              # 本轮基线：0 错误
cd src-tauri && cargo test     # 本轮基线：163 passed / 0 failed
```

**必须说明**：P1-1（serde 命名）、P1-6（id 漂移）、P1-7（纯断言）、P2 各项**全部无法被 `vue-tsc` 捕获**——它们不在类型系统视野内。**编译通过 ≠ 没有这些问题。**

**本轮未能验证的项**：
- `npm audit` —— registry 返回 503，耗时 7 分钟后放弃，未取得结果。前端依赖的已知漏洞状态**未知**，建议网络恢复后单独跑一次。
- `cargo audit` —— 未执行。

**修复后建议补充的回归测试**：
- `export_docx` / `export_edited_docx` / `export_knowledge_markdown` 传入导出目录外的绝对路径应被拒绝；
- `open_file_with_default` 传入案件卷宗外的路径应被拒绝；
- `update_case_status` 传入非法 `track` 应被拒绝（P0-1 已修，需补测试防回退）；
- 一条**元测试**：校验「`CommandMap` 键集合 ⊇ 全部 `tauriCallSafe` 字面量命令」，从机制上阻止 P1-7 继续劣化；
- 时区：在 `TZ=America/New_York` 下跑日期相关单测。

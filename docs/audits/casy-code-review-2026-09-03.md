# Casy 全栈代码审查

> **日期**: 2026-09-03　**范围**: `src-tauri/src`（89 个 .rs / 44,388 行）+ `src`（97 .vue + 68 .ts / 58,959 行）
> **基线**: `vue-tsc --noEmit` 通过（0 错误）· `cargo test` 基线 119+7 ✓
> **结论**: **3 项 P0**（其中 1 条 SQL 注入可致全库读取，3 条 P0 可串成完整攻击链）· **5 项 P1** · **9 项 P2**
> **方法**: 人工静态审查 + 三路并行深度核查；文中每条结论均经源码逐行核实，附 `文件:行号` 与代码片段

---

## 零、威胁模型（先明确，否则无法判断严重性）

Casy 是**本地优先**应用，攻击者不是远程黑客，而是**流经系统的数据本身**。

专利律师的日常输入面：对方当事人提交的无效宣告请求书、证据 PDF、往来邮件、OCR 识别件、AI 摘要。这些内容会经过以下通路进入 UI：

```
外部文档 → OCR/解析 → 知识库正文 → FTS5 snippet() → v-html 渲染
外部文档 → AI 摘要 → dynamicRecommendations → v-html 渲染
```

一旦 webview 获得脚本执行权，即可调用 `update_case_status`（P0-1）dump 全库，或调用 `open_file_with_default`（P0-2）打开任意文件。**本报告的 3 条 P0 不是三个孤立缺陷，而是一条完整链条的三个环节。**

因此，本次审查中「数据来源是否跨越信任边界」是判定严重性的首要标准，而非传统的"本地应用无需防御"假设。

---

## 一、P0 — 必须修复

### P0-1｜SQL 注入：`update_case_status` 的 `track` 参数直接拼接进 SQL

**位置**：`src-tauri/src/commands/cases.rs:791-811`

```rust
#[tauri::command]
pub async fn update_case_status(
    case_id: String,
    track: String,          // ← 用户可控入参，无任何校验
    new_status: String,
    note: Option<String>,
) -> Result<db::cases::Case, String> {
    // 1. 获取当前状态
    let old_status: Option<String> = conn.query_row(
        &format!("SELECT {} FROM cases WHERE id = ?1", track),   // ← 裸拼
        ...
    // 2. 更新状态
    conn.execute(
        &format!("UPDATE cases SET {} = ?1, updated_at = ?2 WHERE id = ?3", track),  // ← 裸拼
```

**为什么危险**：同函数内的 `track.as_str()` match（:837）只用于生成历史记录展示标签（`_ => "其他"` 兜底不拒绝），**且位于 SQL 执行之后**，不构成任何校验。`new_status` / `note` / `case_id` 都已参数化，唯独 `track` 漏网。

可构造 `track = "case_status='x', case_no=(SELECT group_concat(sql) FROM sqlite_master) WHERE 1=1 --"` 之类载荷，经 UPDATE 侧信道 dump 全部表结构；反之亦可写入任意列。

**对照组（说明这是遗漏而非设计选择）**：全仓另外 4 处动态 SQL 拼接**均已正确防御**，只有此处漏掉：

| 位置 | 防御方式 | 状态 |
|---|---|---|
| `ai/gateway.rs:77-84` | 白名单 match，非法即 `bail!` | ✅ 安全 |
| `commands/ai_routes.rs:421` | `table` 来自 `proposal_entity_table` 白名单（:501） | ✅ 安全 |
| `commands/tasks.rs:939` | `sets.join(", ")` 的列名由宏硬编码字面量生成，值全参数化 | ✅ 安全 |
| `formula/mod.rs:272` | `col` 来自代码内 `formula_*` 字面量元组，非用户输入 | ✅ 安全 |
| **`commands/cases.rs:802/809`** | **无** | ❌ **漏洞** |

**修复**（约 5 行）：在函数入口加白名单枚举，非法值直接 `return Err`：

```rust
match track.as_str() {
    "civil_status" | "invalidation_status" | "admin_status" => {}
    _ => return Err(format!("invalid track: {}", track)),
}
```

---

### P0-2｜任意路径文件操作：`reveal_path` / `open_file_with_default` 无范围校验

**位置**：`src-tauri/src/commands/files.rs:448-502`

```rust
#[tauri::command]
pub async fn reveal_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").args(["-R", &path]).spawn()...
}

#[tauri::command]
pub async fn open_file_with_default(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&path).spawn()...
}
```

**为什么危险**：`path` 直接透传给 `open` / `explorer` / `xdg-open`，**未做** `canonicalize` + `starts_with(root)` 校验。无命令注入（单参数传递），但构成**任意文件处置原语**——`open_file_with_default` 会以系统默认应用打开任意路径。

**对照组（同样是遗漏，不是设计选择）**：同一文件内已有现成的校验函数却未被复用：

- `canonical_file_in_case`（`files.rs:172`）与 `canonical_dir_in_case`（`files.rs:163`）均含 `starts_with(root)` 校验

**修复**：两个函数入口先调用 `canonical_file_in_case`，越界即拒绝。

---

### P0-3｜XSS：`v-html` 三处未净化，其中两处的数据源跨越信任边界

全仓 `v-html` 共 6 处，**3 处安全、3 处不安全**：

| 位置 | 内容来源 | 净化 | 判定 |
|---|---|---|---|
| `modules/home/HomeView.vue:708` `v-html="rec.text"` | `dynamicRecommendations`（:702）AI 生成 | 无 | ❌ **危险** |
| `modules/knowledge/components/KnowledgeSidebar.vue:42` `v-html="item.snippet"` | `globalSearch` 的 FTS5 `snippet()` | 无 | ❌ **危险** |
| `modules/docs/views/DocumentGenView.vue:95` `v-html="displayHtml"` | `renderResult.value?.html`（Rust 模板渲染）+ `editedHtml`（:294，用户可编辑） | 无 | ❌ **危险** |
| `modules/ai/views/AICompanionView.vue:701` | `renderMarkdown`(:382) **先 `esc()` 转义 `&<>` 再生成自有标签** | 自转义 | ✅ 安全 |
| `shared/components/BriefingModal.vue:364` | `renderMarkdown`(:253) 同上机制 | 自转义 | ✅ 安全 |
| `modules/knowledge/views/KnowledgeNotebookView.vue:339` | `mdToSafeHtml`（:117 → `mdBridge.ts:234` DOMPurify） | DOMPurify | ✅ 安全 |

**关键证据 —— FTS5 `snippet()` 不转义原文**：`src-tauri/src/commands/search.rs:25,32`

```sql
snippet(knowledge_fts, -1, '<b>', '</b>', '...', 64) as snippet
```

SQLite 的 `snippet()` 仅插入高亮标签，**对原文中的 HTML 原样输出**。知识库正文（来自导入的 Markdown、AI 生成、飞书同步）若含 `<img src=x onerror=...>`，将被原样带出并交由 `v-html` 执行。

**修复**：三处统一接入 `mdToSafeHtml`（`mdBridge.ts:234`，已配 DOMPurify 白名单）。`KnowledgeSidebar` 因需保留 `<b>` 高亮，应在 Rust 侧生成 snippet 前先转义正文，或前端用 DOMPurify 并显式放行 `b` 标签。

---

## 二、P1 — 正确性与健壮性

### P1-1｜AI 授权网关是"调用方自证"，非服务端强制

**位置**：`src-tauri/src/ai/gateway.rs:379`

```rust
if origin == Some("ai") { /* 校验 proposal token */ }
// origin != "ai" 时直接 Ok(()) 放行
```

`origin` 由调用方提供的 JSON 决定，**非密码学绑定**。当前接受 `origin` 的 7 个命令（cases 的 create/update/delete、tasks 的 4 处）均正确守卫，故**现状无法绕过**；但安全边界依赖调用方诚实标注，属荣誉制。

项目宣称的「AI 无特权通道 / 未授权默认拒绝」在服务端**并不真正成立**。建议：由服务端基于会话能力判定来源，或将所有 AI 写入口收口到单一受控分发器。

### P1-2｜撤销删除任务会静默丢失字段

**位置**：`src/core/taskActions.ts:119-136`（`deleteTaskOptimistic`）、`:93-100`（undo 闭包）

撤销时仅用 `TaskLike` 的 6 个字段重建任务，`notes` / `priority` / `context` / 描述等**全部丢失**；且 undo 闭包捕获的 `task` 引用在 `loadTasks` 重建数组后成为游离对象，撤销结果不反映到 UI。

律师误删任务后 ⌘Z 恢复出残缺任务 = 隐性数据丢失。修复：删除前存完整快照，undo 用完整对象写回。

### P1-3｜类型安全仅覆盖约 60% 的调用

**位置**：`src/core/tauriBridge.ts:28-31`（通用重载）、`src/types/commandMap.ts`（158 键）

实测统计：**282 次调用中 168 次命中 CommandMap，114 次（40.4%）走零类型检查的通用重载**；未入表命令 **111 个**（文档自认"动态余量 ~86"，实际更多）。

更关键：`tauriCall<X>` 有 **37 处是纯断言**（`tauriBridge.ts:61`，该变体只有通用重载，泛型仅把 `unknown` 强转）。`create_task` / `update_task` / `toggle_task` / `delete_task` / `create_draft` / `update_draft` **均不在 CommandMap 内**——即最核心的写操作恰恰没有类型保护，Rust 字段变更时前端静默失效。

### P1-4｜错误无结构化映射，失败可被静默吞掉

**位置**：`src/core/tauriBridge.ts:50-53`

Rust 错误仅以 `err.message` 字符串透传 `ElMessage`，前端**无任何 error_code 映射**（全仓仅 `DocumentJobDto.errorCode` 是 OCR 业务字段，非错误码体系）。173 处 `tauriCallSafe` 依赖 `ok` 标志，漏判即静默失败。

反向问题：Rust 侧绝大多数命令签名 `-> Result<T, String>`，`?` 透传 `anyhow::Error`，其 `Display` 含完整错误链（SQL 语句、`db_path` 绝对路径）。`error_code.rs` 仅在 projects/inbox/tasks/settings/relations 部分命令使用，`cases.rs` 主命令几乎未用（仅 :1135）。既不可本地化，又构成信息泄露。

建议：Rust 端统一 `{code, message}` 返回，前端建错误码→提示/重试映射。

### P1-5｜数据库连接每命令重开，长事务易触发 SQLITE_BUSY

**位置**：`src-tauri/src/db/mod.rs:207`（`open_db_encrypted` 每次新建连接并重做 `PRAGMA key` KDF）、`:227`（`busy_timeout=5000`）

`import_excel.rs` / `inbox.rs` 等重命令在单连接长事务内处理大量数据，并发命令在 5s 内争锁会抛 `SQLITE_BUSY` 直接透传前端。建议引入连接池/单例 `Arc<Mutex<Connection>>`，重命令分批提交。

---

## 三、P2 — 一致性与可维护性

| # | 问题 | 位置 | 说明 |
|---|---|---|---|
| P2-1 | 巨型组件 | `CalendarView.vue`(script 878 行)、`CaseListView.vue`(792)、`TasksView.vue`(699)、`CaseImportDialog.vue`(613)、`CaseDetailView.vue`(487)、`HomeView.vue`(484) | 单文件 ref 17–26 个、computed 8–25 个、函数 22–48 个，数据获取/筛选排序/弹窗编排/格式化混写 |
| P2-2 | 空态手搓 5 处 | `CalendarView.vue:998/1259/1374/1454/1550` | 同文件 5 处结构相同的空态，且**整文件未 `import EmptyState`** |
| P2-3 | 已有抽象未复用 | `StateFeedback.vue` 仅 2 处引用（`TasksView:23`、`CaseListView:57`） | 其余 4 个巨型视图各自手搓 loading/empty |
| P2-4 | 文档数字失准 | `README.md` / `Casy-STATUS.md` 称"38 个注册工具" | 实测 9 个插件共 **76** 次 `registerTool(`（cases 12 / inbox 10 / knowledge 10 / tasks 10 / reminder 8 / sync 8 / calendar 6 / files 6 / settings 6）。插件数 9 与文档一致 |
| P2-5 | 快捷键绕过统一注册 | `WritingView.vue:345`、`ProposalDiffCard.vue:184`、`SlashCommandMenu.vue:300` | 直接 `window.addEventListener('keydown')`，绕过 `shared/keyboard.ts:80,89` 的冲突检测（三处均有 `removeEventListener`，非泄漏） |
| P2-6 | 平行重复色板 | `TasksView.vue:107-114` 与 `taskDisplay.ts:38-43` | 两份各自硬编码的十六进制色板，绕过 `shared/theme.ts` 令牌（项目刚完成"令牌化 514 处"重构） |
| P2-7 | 硬编码中文未走 i18n | `CalendarView.vue` 含中文行 168 但 `t()` 仅 47；`CaseImportDialog.vue` `t()` 仅 14 | 项目已装 vue-i18n 且有 `src/locales/` |
| P2-8 | 死代码 | `src/shared/components/SkeletonCard.vue` | 全仓零引用 |
| P2-9 | 凭据明文回退 | `credentials/mod.rs:263`（AI API Key）、`db/mod.rs:124-133`（SQLCipher 主密钥，0600 明文 32 字节 hex） | keychain 不可用时落明文文件。密钥本身为 `getrandom` 强随机、非弱 KDF，强度可接受，但明文落盘属需知悉的取舍，建议 UI 显式提示 |

**另附两项次要项**：`ProposalDiffCard.vue:181` 的 `tickTimer` 无 `onUnmounted` 清理；`CaseListView.vue:876` 案件主列表为裸 `v-for`，全仓无虚拟滚动/`el-pagination`，数据量增长后存在渲染压力（同项目 `CaseImportDialog.vue:1041` 已有 `el-table` + `max-height` 先例可循）。

---

## 四、已验证安全（澄清，避免团队重复排查）

审查过程中以下项被重点怀疑，经核实**均安全**，特此记录以免后续重复投入：

| 项 | 核实结论 |
|---|---|
| 315 处 `.unwrap()` / 22 处 `.expect()` | 命令执行路径上**未发现可 panic 点**：多数为 `.unwrap_or_default`（dashboard.rs:145、settings.rs:23、filters.rs:88、reminder.rs:210）；`commands/` 内无 `.lock().unwrap()`；`cases.rs:1297+` 的 `open_in_memory().unwrap()` 位于测试夹具，非生产路径 |
| `panic!` 15 处 / `unsafe` 0 处 / `todo!` 0 处 | 无生产路径风险 |
| `src/shared/nlp/parseWhen.ts` 性能 | 仅在 `TasksView.vue:416`、`UnifiedCaptureDialog.vue:146` 的事件处理器中调用，**未进入渲染循环** |
| `watch` 深监听 | 全仓 `deep: true` 仅 2 处（`stores/aiSettings.ts:40`、`KnowledgeNotebookView.vue:252`），均不在巨型组件内，无连锁重载 |
| `src/core/mockData.ts` | 被 `tauriBridge.ts` / `tauriEvents.ts` / `App.vue` / `CaseImportDialog.vue` 引用，为浏览器预览运行时探测与回退，**非死代码** |
| Tauri 事件监听清理 | 经 `core/tauriEvents.ts` 的 `safeListen` 统一封装，组件侧 3 处均正确 `unlisten` |

---

## 五、建议修复顺序

1. **P0-1**（`cases.rs` 加 `track` 白名单，约 5 行）—— 风险最高、成本最低，应最先落地
2. **P0-2**（`files.rs` 复用现成的 `canonical_file_in_case`）—— 同样低成本，已有函数可直接调用
3. **P0-3**（三处 `v-html` 接入 `mdToSafeHtml`）—— 基础设施现成，逐个替换即可
4. **P1-2**（撤销删除存完整快照）—— 直接关乎律师场景的数据安全，优先级高于同级的架构类问题
5. **P1-3 / P1-4**（补 CommandMap 核心写命令 + 错误码映射）—— 建议在 B1 DomainCommand 定型时一并收口
6. **P1-1**（AI 网关 origin 信任模型）—— 需设计决策，建议随 B1 一并处理
7. P2 各项按迭代节奏逐步收敛

---

## 六、复验方式

```bash
npm run typecheck   # 本次审查前基线：通过（0 错误）
cd src-tauri && cargo test
```

修复 P0 后建议补充回归测试：`update_case_status` 传入非法 `track` 应被拒绝；`reveal_path` / `open_file_with_default` 传入案卷根目录外路径应被拒绝。

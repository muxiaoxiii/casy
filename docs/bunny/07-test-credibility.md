# 07 · 测试可信度与构建/发布完整性审计

审计日期：2026-09-30
审计基线：`194b314`（审计中途工作区被提交为该 commit；`d5a4e9f` 是最后一次有 CI 记录的 commit）
执行环境：macOS 26 / Apple Silicon / rustc 1.96.0 / node 24
本机实跑：`cargo test --locked`、`npm run test:unit`、`npm run build`、`node --test scripts/*.test.mjs`、`cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --features models`

> 结论一句话：**这套测试证明的是「已写的 739 个断言没坏」，不是「产品是对的」。** 它对 61k 行后端只覆盖了 31% 的命令函数，对 46k 行前端只触达 26% 的文件；CI 从未在这份代码上运行过；17 个 e2e 脚本在物理上无法执行；最新的数据完整性修复从未在 Windows 上验证过。

---

## 0. 复现命令（全部实测通过，可直接重跑）

```bash
# Rust 全量（本机实测 358 passed / 0 failed / 5 ignored）
cd src-tauri && CASY_TEST_DATA_DIR="$(mktemp -d /tmp/casy-audit.XXXXXX)" cargo test --locked

# 文档引擎（19 passed / 5 ignored）
cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --locked --features models

# 前端（79 文件 / 355 passed / 0 skipped / 0 todo）
npm run test:unit -- --maxWorkers=2

# 脚本（7 passed）
node --test scripts/*.test.mjs

# 构建 + 类型
npm run build

# bindings 漂移（实测无漂移）
cd src-tauri && cargo test --lib export_bindings && cd .. && git diff --exit-code -- src/types/bindings.ts

# CI 史实
gh api "repos/muxiaoxiii/casy/actions/runs?per_page=60" \
  --jq '.workflow_runs[] | "\(.created_at) \(.name) \(.head_branch) \(.conclusion)"'
```

---

## 1. 测试画像表

### 1.1 总量（实测）

| 套件 | 文件数 | 用例总数 | 通过 | 忽略 | 跳过 | 本机耗时 | 门禁位置 |
|---|---:|---:|---:|---:|---:|---:|---|
| Rust 库单测（`src-tauri/src/**/#[cfg(test)]`） | 58 个源文件 | 296 | 295 | 1 | 0 | ~57s | `ci.yml` rust job |
| Rust 集成测试（`src-tauri/tests/*.rs`） | 29 | 67 | 63 | 4 | 0 | ~25s | `ci.yml` rust job |
| 文档引擎（`tools/casy-doc-engine`） | 3 源文件 | 24 | 19 | 5 | 0 | 1.3s | `ci.yml` rust job |
| 脚本（`scripts/*.test.mjs`） | 4 | 7 | 7 | 0 | 0 | 0.05s | `ci.yml` frontend job |
| 前端 vitest | 79 | 355 | 355 | 0 | **0** | 15.2s | `ci.yml` frontend job |
| 前端 e2e（`tests/e2e/*.mjs`） | 17 | — | **0** | — | **17** | 无法执行 | **无** |
| **合计（真正跑过的）** | **171** | **749** | **739** | **10** | **17** | | |
| **合计（含从未运行的 e2e）** | **188** | **749+** | **739** | **10** | | | |

**文档与实测的差异（说明文档数字不可作为证据）**

| 项 | `Casy-STATUS.md` / `RELEASE_0.1.3.md` 声称 | 实测 | 差 |
|---|---:|---:|---:|
| Rust 通过 | 345 | **358** | +13（文档少报） |
| Rust 忽略 | 4 | **5** | +1（文档少报） |
| 脚本通过 | 5 | **7** | +2（文档少报） |
| schema 版本 | 42 | `CURRENT_SCHEMA_VERSION = 43`（`src-tauri/src/db/schema.rs:5`） | 文档落后 1 版 |
| 前端 | 355 | 355 | 一致 |

### 1.2 断言类型分类

| 类型 | 定义 | Rust | 前端 | 评估 |
|---|---|---:|---:|---|
| **A. 真实行为断言** | 走生产函数 + 校验具体值/状态/回滚/文件字节 | 约 120（33%） | 约 200（56%） | 唯一有价值的部分 |
| **B. 命令/接线断言** | 只证明「函数名被注册」「bridge 用某个名字被调用」 | 约 30（8%） | 约 40（11%） | 只防拼写漂移，不防逻辑错 |
| **C. 快照 / 渲染存在性** | `toMatchSnapshot` / `wrapper.exists()` / 只渲染不断言 | **0** | **15** | 全仓零快照 |
| **D. 自指 / 测试替身** | 测 mock 数据层、grep 源码字符串、只证明「不 panic」 | 约 3 | 约 40 | 见 §1.4 |

**关键分布事实**

- 前端：**1074 个 `expect()` / 355 个用例 = 3.02 个断言/用例**。这个密度是健康的，不是「一堆空壳」。
- 前端：**0 个 `toMatchSnapshot`、0 个 `.skip`、0 个 `.todo`、0 个 `.only`**（实测 `grep -rhoE "(it|test|describe)\.(skip|todo|only)"` → 无输出）。前端没有为了变绿而被关掉的测试。
- 后端：3 个用例**零断言**（见 §1.4），2 个只写 `assert!(x.is_ok())`（`src-tauri/tests/ai_gateway_test.rs:73`、`:227`）。
- 后端**完全没有快照测试**——没有一条「输出结构变了会红」的护栏。

### 1.3 最高价值的测试（正面结论）

`src-tauri/tests/read_failure_regression_test.rs`（110 行，1 个用例）是全仓质量最高的一个测试，也是唯一一个「按缺陷类别」而不是「按函数」组织的测试：

```rust
// src-tauri/tests/read_failure_regression_test.rs:18-21
INSERT INTO drafts(id,title,content) VALUES('d',X'FF','text');   // 注入非法 UTF-8
INSERT INTO notifications(id,type,title) VALUES('n','system',X'FF');
...
// :30-39 逐一断言 9 个读取命令必须返回 Err（而不是静默少一行）
assert!(drafts::list_drafts().await.is_err());
// :41-50 注入第二条关系写入失败，断言第一条必须回滚
assert_eq!(count, 0, "failed detection must not leave the first relation committed");
```

它用真实 SQLite、真实生产命令、真实事务，验证「故障时不静默丢数据」。**问题不是它质量差，而是它只覆盖 9 个命令——同样的缺陷类别在 `ai/` 下还有 20+ 处活的（见 §3.2）。**

### 1.4 几乎什么都没断言的测试（清单）

| 位置 | 断言数 | 问题 |
|---|---:|---|
| `src-tauri/tests/schema_v20_test.rs:139-148` `v20_migration_is_idempotent` | **0** | 断言被删。函数体只剩 `run_migrations(&conn,0).unwrap()` 加一句注释「版本号…保持一致即可（**上文已断言**）」，`let _version` 把查询结果丢弃。用例名声称验证幂等，实际只验证「跑第二遍不报错」。（CONFIRMED 被弱化；但注释诚实——`schema.rs:4267/4297/4341/4491` 确实另有 4 处断言 `user_version == CURRENT_SCHEMA_VERSION`） |
| `src-tauri/src/db/schema.rs:4369` `test_v23_fresh_schema_accepts_notebook_categories` | 0 个 `assert!` | 靠 `unwrap_or_else(\|e\| panic!(...))` 承担断言语义。逻辑上等价，但迁移失败会以 panic 而非 assert 呈现，可读性差 |
| `src-tauri/src/export_bindings.rs:6` `export_ts_bindings` | 0 | 生成器，按设计如此；其"断言"是 CI 里的 `git diff --exit-code`（实测有效） |
| `src-tauri/tests/ai_gateway_test.rs:73` | 1 | `assert!(res_user.is_ok())` — 不检查返回内容 |
| `src-tauri/tests/ai_gateway_test.rs:227` | 1 | `assert!(valid.is_ok())` — 同上 |
| `tests/unit/captureMock.test.ts`（2 用例） | 6 | 测的是 `src/core/mockData.ts`（**浏览器预览用的假数据层**），不是生产链路。RELEASE_0.1.3.md 用它作为 UI 证据 |
| `scripts/read-integrity.test.mjs:5-11` | 2 | **grep 源码字符串**当门禁，见 §1.5 |

另有 96 个 Rust 用例只有 ≤1 个 `assert`（`formula/eval.rs` 的表驱动测试属正常，已排除），完整清单可用 §0 的脚本复现。

### 1.5 「用 grep 当测试」的伪门禁（严重）

`scripts/read-integrity.test.mjs` 声称守护「SQL 行解码错误不得被静默丢弃」——这是审计报告里确认的 4 个真实数据完整性缺陷之一。但它只做两件事：

```js
// scripts/read-integrity.test.mjs:6-10
for (const file of readdirSync('src-tauri/src/commands').filter(n => n.endsWith('.rs'))) {
  assert.doesNotMatch(source, /filter_map\(\|r\| r\.ok\(\)\)/, file)
  assert.doesNotMatch(source, /(?:rows|case_iter)\.flatten\(\)/, file)
}
```

两个致命缺陷：

1. **只扫 `src-tauri/src/commands/` 一个目录**。`ai/`、`sync/`、`deadline/`、`email/`、`mcp/`、`db/`、`files/`、`parse/`、`docsy_engine/` 全部不扫。
2. **正则太窄**。`.ok().flatten()`、`filter_map(Result::ok)`、`.filter_map(|row| row.ok())` 变体、变量名非 `rows`/`case_iter` 的 `flatten()` 全部漏过。

**实测反例：`ai/` 下同一缺陷类别目前有 20+ 处活的。**

```
src-tauri/src/ai/recursive_check.rs:236,260,283,305,326   .filter_map(|r| r.ok())
src-tauri/src/ai/insights.rs:49,83,113,147                 .filter_map(|r| r.ok())
src-tauri/src/ai/reports.rs:491,697,727,744,799            .filter_map(|r| r.ok())
src-tauri/src/ai/mod.rs:99                                 .filter_map(|r| r.ok())
src-tauri/src/ai/distillation.rs:179                      .filter_map(|r| r.ok())
src-tauri/src/db/cases.rs:614                              for row in rows.flatten()
src-tauri/src/ai/profiles.rs:73,209                        .ok().flatten()
```

也就是说：**这个「门禁」在它宣称保护的目录里已经生效，但在同一个 bug 类别的其它目录里完全失守，而那些文件正好是最没人测的**（`ai/reports.rs` 1062 行、`ai/recursive_check.rs` 435 行、0 个命令被测试触及）。这是本审计认为最容易被 Codex 拿来当"已修好"证据的一块。

---

## 2. 后端模块覆盖表

**度量方法**：静态提取全部 `#[tauri::command] pub fn NAME`（369 个，与 `commands/mod.rs` `generate_handler![]` 的 370 条注册项一致），再检查 `NAME` 是否出现在 `src-tauri/tests/*.rs` 或任一源文件的 `#[cfg(test)]` 模块中。**这是上界**——名字出现不等于被有效调用。

**总体：369 个命令中 113 个（31%）出现在测试代码里，256 个（69%）从未被任何测试提及。**

| 模块 | 行数 | 命令数 | 被测 | 占比 | 评估 |
|---|---:|---:|---:|---:|---|
| `commands/portable_backup.rs` | 933 | 2 | 2 | 100% | ✅ 有真实加密归档 fixture 验证 |
| `commands/tasks.rs` | 2115 | 14 | 8 | 57% | ⚠️ 核心任务链有覆盖，模板/延后/期限别名无 |
| `commands/knowledge.rs` | 1897 | 19 | 5 | 26% | 🔴 导入/导出/版本恢复全无 |
| `commands/import_excel.rs` | 2128 | 4 | 3 | 75% | ✅ 但私有工作簿用例被 `#[ignore]` |
| `commands/files.rs` | 1157 | 15 | 13 | 87% | ✅ 路径逃逸防护测得扎实 |
| `commands/drafts.rs` | 312 | 7 | 6 | 86% | ✅ |
| `commands/deadline_rules.rs` | 284 | 5 | 4 | 80% | ✅ |
| `commands/calendar_events.rs` | 258 | 5 | 4 | 80% | ✅ |
| `commands/dashboard.rs` | 216 | 5 | 4 | 80% | ✅ |
| `commands/relations.rs` | 269 | 4 | 3 | 75% | ✅ 含事务回滚 |
| `commands/task_plans.rs` | 238 | 2 | 2 | 100% | ✅ 独立计划全链路 |
| `db/knowledge_index.rs` | 407 | 4 | 4 | 100% | ⚠️ 见 §3.1（明文 SQLite） |
| `commands/conversion.rs` | 306 | 1 | 1 | 100% | 🔴 唯一用例 `#[ignore]`，需真实引擎 |
| `commands/inbox.rs` | **2739** | 25 | 5 | **20%** | 🔴 最大命令文件，80% 无测试 |
| `commands/reminder.rs` | **2150** | 10 | 2 | **20%** | 🔴 提醒引擎（含休息日扫描）几乎裸奔 |
| `commands/cases.rs` | 1485 | 21 | 8 | 38% | ⚠️ 搜索/统计/导出/字段组无测试 |
| `commands/sync.rs` | 1263 | 32 | 3 | **9%** | 🔴 32 个命令测 3 个 |
| `commands/mod.rs` | 1017 | 30 | 2 | **7%** | 🔴 注册表本身几乎无契约测试 |
| `commands/whiteboard.rs` | 295 | 12 | 1 | 8% | 🔴 |
| `commands/import_feishu.rs` | 1282 | 4 | 0 | **0%** | 🔴 |
| `commands/feishu_snapshot.rs` | 985 | 6 | 0 | **0%** | 🔴 私有快照用例被 `#[ignore]` |
| `commands/ai_routes.rs` | 554 | 10 | 2 | 20% | 🔴 审批/策略无测试 |
| `commands/docs.rs` | 468 | 6 | 1 | 17% | 🔴 DOCX/PDF 导出无测试 |
| `commands/smart_rules.rs` | 437 | 10 | 3 | 30% | ⚠️ 批量失败上报已被新测试覆盖 |
| `commands/caldav.rs` | 410 | 4 | 1 | 25% | 🔴 |
| `commands/whiteboard_document.rs` | 409 | 4 | 0 | **0%** | 🔴 |
| `commands/backup.rs` | 294 | 4 | 1 | 25% | 🔴 恢复路径 |
| `commands/areas.rs` | 271 | 6 | 1 | 17% | 🔴 |
| `commands/persons.rs` | 253 | 7 | 3 | 43% | ⚠️ 新完整性测试覆盖了空角色/NULL 重复 |
| `commands/projects.rs` | 221 | 4 | 0 | **0%** | 🔴 |
| `commands/filters.rs` | 219 | 3 | 0 | **0%** | 🔴 |
| `commands/search.rs` | 210 | 3 | 1 | 33% | ⚠️ |
| `commands/decisions.rs` | 197 | 3 | 0 | **0%** | 🔴 |
| `commands/whiteboard_scene.rs` | 182 | 3 | 0 | **0%** | 🔴 |
| `commands/linking.rs` | 167 | 4 | 1 | 25% | 🔴 backlinks |
| `commands/timeline.rs` | 167 | 3 | 0 | **0%** | 🔴 |
| `commands/notifications.rs` | 128 | 6 | 1 | 17% | 🔴 |
| `commands/linking.rs`/`search.rs`/`filters.rs` | | | | | |
| `ai/mod.rs` | **1279** | 6 | 0 | **0%** | 🔴 配置/连接测试/用量全无 |
| `ai/reports.rs` | **1062** | — | 0 | — | 🔴 无 `#[cfg(test)]`，20+ 处 `.ok()` 静默丢行 |
| `ai/recursive_check.rs` | 435 | 1 | 0 | 0% | 🔴 5 处静默丢行 |
| `ai/learning.rs` | 418 | — | 0 | — | 🔴 无测试模块 |
| `ai/gateway.rs` | 415 | — | 0 | — | ⚠️ 有 `ai_gateway_test` 测 `update_task` 副作用 |
| `ai/profiles.rs` | 323 | 3 | 0 | 0% | 🔴 但有 `ai_profiles_test` 5 例 |
| `ai/embeddings.rs` | 309 | 1 | 0 | 0% | 🔴 |
| `email/mod.rs` + `email/smtp.rs` | **1365** | 7 | 0 | **0%** | 🔴 **整个邮件模块零测试** |
| `mcp/server.rs` | 430 | 1 | 0 | 0% | ⚠️ 有 3 个协议级单测（token/JSON/202） |
| `sync/feishu.rs` | **2287** | — | 0 | — | 🔴 **后端第二大文件，零测试** |
| `sync/caldav.rs` | 431 | — | 3 inline | — | ⚠️ |
| `sync/webdav.rs` | 345 | — | 1 inline | — | ⚠️ 有 `webdav_full_backup_test` |
| `deadline/recalc.rs` | 88 | 1 | 0 | 0% | ⚠️ 有 1 个 inline 测试 |
| `db/cases.rs` | 724 | — | 0 | — | 🔴 `rows.flatten()` @614 |
| `db/vector_index.rs` | 551 | — | 0 | — | 🔴 Zvec 封装层无直接测试 |
| `db/search.rs` | 234 | 2 | 0 | **0%** | 🔴 混合检索无测试 |
| `files/mod.rs` | 468 | — | 0 | — | 🔴 |
| `parse/mod.rs` | 354 | — | 0 | — | ⚠️ `text_document`/`doc2docx` 有 inline 测试 |

**没有 `#[cfg(test)]` 模块的后端源文件：62 / 120 个，24,700 行（后端总量的 40%）。** 复现：

```bash
python3 - <<'PY'
import os
n=t=l=0
for r,d,fs in os.walk('src-tauri/src'):
    for f in fs:
        if f.endswith('.rs'):
            p=os.path.join(r,f); s=open(p,encoding='utf8',errors='ignore').read()
            t+=1; l+=s.count('\n')+1
            if '#[cfg(test)]' not in s and '#[test]' not in s: n+=1
print(n,'/',t,'files without tests, of',l,'LOC')
PY
```

---

## 3. 前端模块覆盖表

**度量方法**：对每个 `src/**` 生产文件，检查其 basename 是否出现在任一 `tests/**/*.test.ts` 中。≥80 LOC 的文件共 170 个。

**总体：88 / 170 个文件（35,574 行 = 56%）被零个测试引用。**

| 模块 | 行数 | 文件数 | 零引用文件 | 零引用行数 | 评估 |
|---|---:|---:|---:|---:|---|
| `src/modules/cases` | 10683 | 23 | 18 | 8175 | 🔴 `CaseListView.vue` **3038 行**、`CaseImportDialog.vue` 1913、`KanbanView.vue` 467 全部无测试 |
| `src/modules/settings` | 5992 | 19 | 16 | 5127 | 🔴 `FeishuSettings.vue` 808、`BriefingStyleSettings.vue` 600、`FolderTemplateSettings.vue` 530、`DeadlineRulesSettings.vue` 488 全无 |
| `src/modules/ai` | 3769 | 6 | **6** | **3769** | 🔴 **整个 AI 界面零测试**：AICompanionView 1181、AIChatPanel 917、DecisionsView 571、ProposalDiffCard 426。实测 `grep` 确认无任何测试 import 该目录任何 `.vue` |
| `src/modules/tasks` | 3702 | 9 | 5 | 3078 | 🔴 `TasksView.vue` **1843 行**、TaskRow 513、AreasDialog 275、TodayResetDialog 272 无测试 |
| `src/modules/docs` | 5954 | 14 | 7 | 2676 | 🔴 `WritingView.vue` 924、CopilotSidebar 533、EvidenceLinkPicker 486 无测试 |
| `src/modules/knowledge` | 3921 | 14 | 9 | 2727 | 🔴 `KnowledgeView.vue` **1090**、KnowledgeGraphView 396、BacklinksPanel 382 无测试 |
| `src/modules/calendar` | 4121 | 13 | 2 | 158 | ✅ 相对最好（`CalendarView.vue` 3320 被 `calendarTimeline.test.ts` mount） |
| `src/modules/home` | 1471 | 1 | 1 | 1471 | 🔴 `HomeView.vue` 唯一"引用"是 `mdBridge.test.ts:215` 的一句注释 |
| `src/modules/persons` | 1589 | 5 | 4 | 1518 | 🔴 PersonsView 452、CasePersonsPanel 479、PersonDetailDrawer 396 |
| `src/modules/files` | 1144 | 3 | 2 | 1029 | 🔴 `CaseFilesView.vue` 918 |
| `src/modules/inbox` | 677 | 3 | 3 | 677 | 🔴 InboxView 506 全无 |
| `src/modules/sync` | 571 | 1 | 1 | 571 | 🔴 `SyncStatusView.vue` 570 |
| `src/modules/reminder` | 481 | 1 | 1 | 481 | 🔴 `ReminderView.vue` 480 |
| `src/modules/notifications` | 470 | 1 | 1 | 470 | 🔴 `NotificationBell.vue` 469 |
| `src/modules/projects` | 293 | 1 | 1 | 293 | 🔴 |
| `src/modules/whiteboard` | 825 | 7 | 1 | 91 | ✅ |
| `src/modules/dashboard` | 180 | 1 | 0 | 0 | ✅ 被 `visualData.test.ts` shallowMount |
| `src/modules/clients` | 127 | 1 | 0 | 0 | ✅ 被 2 个测试 mount |
| `src/shared` | 9692 | 60 | 24 | — | ⚠️ `DocumentEditor.vue` **1467**、`EditorToolbar.vue` 393、`ReminderBanner.vue` 235、`OverdueMorningBrief.vue` 337 无测试 |
| `src/core` | 6145 | 44 | 13 | — | ⚠️ 8 个 plugin（`initializer.ts` 95、files/sync/reminder/inbox/tasks/cases/settings-plugin）全部无测试；`src/core/ai/proposals.ts` 126 无测试 |
| `src/types` | 2191 | 8 | 5 | — | ⚠️ `ipc.ts` 318 无测试 |

### 3.1 前端测试跑在什么上

- `vitest.config.ts:8` → `environment: 'node'`（默认），52 个文件用 `// @vitest-environment jsdom` 文档注释切到 jsdom，3 个显式 node。**没有 Playwright、没有浏览器、没有真实 CSS 布局**（DEVELOPMENT.md 自己也承认「jsdom 不验证 CSS 排版」）。
- **79 个测试文件里 42 个用 `vi.mock`**。所有跨进程边界都被 mock 掉：**前端套件里没有任何一个测试触发过真实的 Tauri IPC**。
- 100 / 355 个用例 `mount()` 真实组件；其余 255 个是纯函数/组合式函数测试。
- 42 个文件测的是 `src/core/services/*`（`tauriCallSafe` 薄封装）。这类测试断言的是"我用正确的命令名调了 bridge"——防拼写漂移有用，**对后端行为零信息量**。好在没有一个文件是 100% 只做这种断言（实测逐文件统计）。
- **全仓零快照**。`expect(wrapper.html()).toContain` 类弱断言只有 15 处。

### 3.2 唯一真正跨边界的静态门禁

`tests/contract.commands.test.ts`（10 用例）是全套里唯一检查前后端契约的东西：

```
frontend ⊆ CommandMap ⊆ Rust(generate_handler![])
```

它用正则从前端源码抽 `tauriCallSafe/tauriCall/invoke` 的字符串字面量，与 `src/types/commandMap.ts` 的键、`commands/mod.rs` 的注册表做双向子集校验。**这是有真实价值的**（能抓住"前端调了后端没有的命令"）。局限：

- 纯词法，**不校验参数名/参数结构**。命令存在但签名变了，它照样绿。
- 只能证明命令**注册**了，不能证明它**能工作**。

---

## 4. 被跳过 / 忽略 / 无法运行的测试清单

### 4.1 Rust `#[ignore]`（主仓 5 个）

| 位置 | 理由 | 覆盖了什么被跳过的东西 |
|---|---|---|
| `src-tauri/src/commands/conversion.rs:250` | 需打包文档引擎 + `CASY_CONVERSION_TEST_PDF` | **真实文档转换**。这是最该跑也最没跑过的一类 |
| `src-tauri/tests/local_retrieval_test.rs:14` | 需 E5-base 模型 + `CASY_DOC_ENGINE` | **本地向量语义检索**。整个 `ai/retrieval.rs` 的实际效果无任何验证 |
| `src-tauri/tests/excel_import_test.rs:218` | 需私有工作簿 `CASY_TEST_EXCEL_PATH` | 真实客户 Excel 台账导入 |
| `src-tauri/tests/feishu_snapshot_test.rs:176` | 需私有快照 | 真实飞书多维表格导入 |
| `src-tauri/tests/zvec_index_test.rs:157` | HNSW 召回/磁盘基准 | **向量召回率**。索引"能建"被测了，"查得准"没有 |

### 4.2 文档引擎 `#[ignore]`（5 个）

`tools/casy-doc-engine/src/main.rs:1318 / 1437 / 1534` — 真实中文扫描 PDF、韩文识别、多语种+伪造文本，全部需本地已验证模型资产。
`tools/casy-doc-engine/src/table.rs:680 / 711` — 私有报告表格还原。

**结论：真实 OCR / 真实模型 / 真实向量召回这一整类，本机与 CI 都是 0 次执行。** 文档里所有关于 OCR 效果的表述都来自人工一次性验收，不是可重复的门禁。

### 4.3 前端 e2e —— 17 个脚本，物理上无法执行（最严重的"看起来有测试"）

```
tests/e2e/  ai-local.mjs  capture-local.mjs  conversion-local.mjs  dashboard-local.mjs
            delivery-local.mjs  document-local.mjs  document-source-local.mjs
            editor-typesetting-local.mjs  file-lifecycle-local.mjs  intake-local.mjs
            knowledge-index-local.mjs  layout-local.mjs  notebook-editing-local.mjs
            processing-native.mjs  tasks-local.mjs  text-document-local.mjs  workspace-local.mjs
```

**为什么跑不了（三重阻断，全部 CONFIRMED）：**

1. **依赖未安装**。每个文件首行 `const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')`。实测：
   ```
   $ node -e "import('playwright')"   →  PLAYWRIGHT MISSING: ERR_MODULE_NOT_FOUND
   $ ls node_modules/playwright       →  No such file or directory
   ```
   `package.json` 的 dependencies / devDependencies 里**没有 playwright**。
2. **无人调用**。实测 `grep -rn "e2e" package.json .github/workflows/` → **零命中**。`vitest.config.ts:7` 的 `include: ['tests/**/*.test.ts']` 也不匹配 `.mjs`。
3. **需私有环境变量**，缺则立即 `assert()` 崩溃：`CASY_QA_DIR`（`intake-local.mjs:6` 甚至要求目录里已有 `snapshot.json`）、`CASY_OCR_QA_SOURCE`（`document-local.mjs:9`）、`CASY_CONVERSION_TEST_PDF`、`CASY_QA_URL`、`CASY_DOC_ENGINE`。
4. 还需要预构建的 `src-tauri/target/debug/examples/*_local_bridge`（16 个 example 二进制），这些**不是测试目标**，`cargo test` 不会构建它们。

**这 17 个脚本本身写得不错**（有故障注入、有 console error 断言、有 UI 交互断言），但它们是**文档，不是门禁**。DEVELOPMENT.md 说得很诚实：「它们没有被普通 `npm run test:unit` 自动运行」——问题是没有任何其他东西运行它们。

### 4.4 条件早退 / 隐式跳过

- `vitest` 层：**0 个**。前端没有条件跳过。
- Rust 层：0 个 `return;` 早退。
- `db/mod.rs:292` / `:394` —— 测试模式下**绕过 SQLCipher 加密**，见 §5.1。这不是"跳过测试"，是"静默换掉了被测系统的一层"，更隐蔽。

---

## 5. 测试数据隔离（结论：这一项是过关的，但属于侥幸过关）

### 5.1 已验证的事实

- `src-tauri/src/runtime_paths.rs:25-32` `isolated_data_root()`：`--profile-dir <绝对路径>` 优先于 `CASY_TEST_DATA_DIR`；两者都只在 `cfg!(debug_assertions)` 下生效（`:16-23`）。
- 集成测试中 **15 个文件自己 `std::env::set_var("CASY_TEST_DATA_DIR", tempfile)`**；其余 14 个文件全部走 `db::enable_test_mode()`（`db/mod.rs:388`）或 `Connection::open_in_memory()`。
- `db/mod.rs:392-402` `db_path()` 在 test mode 下返回一个 `OnceLock` 共享的 `std::env::temp_dir()/casy_test_<uuid>.db`，**永不指向 `data_root()`**。

### 5.2 决定性实验：假 HOME 隔离验证

把 `HOME` 指向空临时目录、**显式取消** `CASY_TEST_DATA_DIR` 与 `TEST_ENV`，逐个跑所有相关测试二进制，然后检查假 HOME 下是否产生任何文件：

```bash
FAKEHOME=$(mktemp -d /tmp/casy-fakehome.XXXXXX)
for t in backup_restore_test feishu_import_test feishu_snapshot_test schema_v20_test \
         schema_v21_document_test task_patch_test zvec_index_test ai_gateway_test \
         dashboard_data_test excel_import_test ai_profiles_test text_documents_test \
         knowledge_index_test local_retrieval_test; do
  BIN=$(ls -t src-tauri/target/debug/deps/${t}-[0-9a-f]* | grep -v '\.d$' | head -1)
  env -u CASY_TEST_DATA_DIR -u TEST_ENV HOME="$FAKEHOME" "$BIN" --test-threads=1
done
find "$FAKEHOME" -type f | wc -l
```

**实测结果：`0`。所有 14 个相关测试二进制全部 `test result: ok`，且未在假 HOME 下创建任何文件。**

**CONFIRMED：当前没有任何测试能碰到真实用户数据库 `~/Library/Application Support/Casy/casy.db`。** 这一项可以放心。

### 5.3 但有三个真实隐患

1. **`npm test` 不设隔离环境变量。** `package.json:8`：
   ```json
   "test": "npm run typecheck && npm run test:unit && cd src-tauri && cargo test"
   ```
   没有 `CASY_TEST_DATA_DIR`。今天安全**纯属** §5.2 的结论成立。一旦有人新增一个「用 `open_db()` 但忘了 `enable_test_mode()`」的测试，就会直接写进用户真实库 + 真实钥匙串（`db/mod.rs:196-198` 会去读 macOS Keychain）。这正是 `VALIDATION_2026-09-28_UX.md` 记录过的、修了 3 个用例的那类问题——**修了 3 个，没修成机制**。
2. **`enable_test_mode()` 是进程级全局。** `IS_TEST_MODE: AtomicBool` + `TEST_DB_PATH: OnceLock`（`db/mod.rs:384-390`）→ 同一测试二进制内所有测试**共享同一个临时数据库**。实测有 2 个文件多测试且无互斥锁：`ai_profiles_test.rs`（5 例）、`knowledge_index_test.rs`（2 例）。它们之间会看到对方的行。
3. **`std::env::set_var` 在多线程测试进程里不是线程安全的。** Rust 2021 起 `set_var` 在多线程下是 UB/未定义。目前 15 个调用点都恰好在单测试文件或 `OnceLock` 里（实测无 `>1 测试 + set_var` 的组合），所以现在没炸。这是靠巧合维持的，不是靠设计。

### 5.4 测试跑的是明文 SQLite，不是 SQLCipher（重要盲区）

`db/mod.rs:292-296`：

```rust
if IS_TEST_MODE.load(Ordering::SeqCst) || std::env::var("TEST_ENV").is_ok() {
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    return Ok(conn);          // ← 没有 PRAGMA key
}
```

`db/mod.rs:401` 生产路径是 `data_root().join("casy.db")` + SQLCipher 密钥。

**后果：43 个 schema 版本的全部迁移、所有集成测试，都是在明文 SQLite 上跑的。** 生产库是 SQLCipher 加密的。`RELEASE_0.1.3.md` 写「节假日 SQLCipher 集成测试验证…」——实际上那条路径在测试模式下不开加密。

全仓只有 **1 处** 测试真正用过 `PRAGMA key`（`src-tauri/tests/backup_restore_test.rs:71`），而且是手工构造的加密文件，不走 `open_db_encrypted()`。**加密打开、密钥不匹配、明文→密文迁移（`migrate_to_encrypted`）、`VACUUM INTO` 加密备份** 这些只在生产路径上，测试覆盖约等于零。

---

## 6. CI 门禁实况表

### 6.1 `ci.yml` 定义 vs 实际是否跑过

**决定性事实：`.github/workflows/ci.yml` 的 `push` 触发条件是 `branches: [main]`。当前全部工作在 `codex/casy-0.1.3-production-validation` 分支上，且仓库里没有任何 PR。**

```
$ gh api "repos/muxiaoxiii/casy/actions/runs?per_page=60" \
    --jq '.workflow_runs[] | "\(.created_at) \(.name) \(.head_branch) \(.conclusion)"'
2026-09-28T06:14:56Z  Windows loader diagnosis  codex/...  success
2026-09-28T06:14:56Z  Beta desktop packages    codex/...  failure     ← 至今最新
2026-09-28T05:59:48Z  Beta desktop packages    codex/...  cancelled
2026-09-28T05:23:38Z  Beta desktop packages    codex/...  failure
2026-09-28T05:00:29Z  Beta desktop packages    codex/...  cancelled
...
2026-08-26T11:05:02Z  CI                      main       success    ← CI 最后一次运行，5 周前
2026-08-26T10:46:50Z  CI                      main       failure
...
```

| job | 定义内容 | 是否阻断合并 | 是否真跑过 | 证据 |
|---|---|---|---|---|
| `frontend` | `typecheck` → `test:unit` → `node --test scripts/*.test.mjs` → `build`，上传 `dist` | 是（`rust`/`tauri-build` 都 `needs: [frontend]`） | ❌ **从未在当前代码上跑过** | 最后一次 2026-08-26 on `main`（比 HEAD 落后 25 个 commit、5 周时间） |
| `rust` (ubuntu) | `cargo test --locked` + doc-engine + **clippy -D warnings** + **cargo audit** + **bindings 漂移** | 是 | ❌ 同上 | 无任何 run 记录 |
| `rust` (macos) | 同上（无 clippy/audit） | 是 | ❌ 同上 | — |
| `rust` (windows-2025) | 额外加 renderer/OpenSSL 准备、`--tests --no-run`、`diagnose-windows-loader.py` | 是 | ❌ 同上（windows 是 round-2 才加回 `ci.yml` 的） | `read-integrity.test.mjs:13` 甚至断言 `os: [ubuntu-latest, macos-latest, windows-2025]` 必须存在 |
| `tauri-build` | 仅 `refs/tags/v*` 且非 `-beta.`；`needs: [frontend, rust]`；macOS only；跑 `npm run release:validation`；上传**草稿** Release | 是（tag 路径唯一出口） | ❌ **从未触发过**（无 v* tag） | — |

**结论：`ci.yml` 里所有的门禁——typecheck、前端 355、脚本 7、clippy 零警告、cargo audit、bindings 漂移、Windows 测试、三平台矩阵——在这份代码上一次都没有执行过。**

### 6.2 真正跑过的 `beta.yml`（分支触发）——目前是红的

`beta.yml` 的 `push.branches: [codex/casy-0.1.3-production-validation]`，所以它确实在跑。最近一次（2026-09-28T06:14:56Z，commit `d5a4e9f`）：

```
$ gh api repos/muxiaoxiii/casy/actions/runs/36385509259/jobs --jq '.jobs[] | "\(.name) \(.conclusion)"'
frontend                            success
package (windows-2025, Windows-x64) failure
package (macos-26, macOS-arm64)     success
release                             skipped
```

**Windows 上有 2 个 Rust 测试真实失败**（`test result: FAILED. 287 passed; 2 failed; 1 ignored`）：

```
---- commands::portable_backup::tests::relocation_respects_path_boundaries stdout ----
thread '...' panicked at src\commands\portable_backup.rs:743:9:
assertion `left == right` failed
  left: "/new/case\\file.pdf"        ← 生产代码产出了混合分隔符路径
 right: "/new/case/file.pdf"

---- commands::portable_backup::tests::markdown_attachments_move_without_rewriting_other_text stdout ----
panicked at src\commands\portable_backup.rs:762:9:
assertion failed: moved.contains("[original](/new/case/evidence.pdf)")
```

同一 commit 在 macOS 上这两项 **ok**。

**这是"全绿"最具体的反例：同一份代码、同一个测试，macOS 绿、Windows 红，而且这个红已经持续了至少 3 次 CI 运行（3 failure + 4 cancelled）。**

**`194b314`（审计中途的提交）改了 `portable_backup.rs` +171 行并引入了 `portable_path()` 做分隔符归一化，看起来正是针对这个缺陷的修复。但：**

- 该 commit **从未被任何 CI 运行过**（`ci.yml` 不在分支上触发，`beta.yml` 还没有新 run）。
- 因此「Windows 上的 Markdown 附件重定位已修好」是**未验证的推断**，不是事实。
- 而且新测试的期望值改成了 `Path::new("/new/case").join("file.pdf").to_string_lossy()`——在 Windows 上就是 `\new\case\file.pdf`。这意味着测试现在**接受反斜杠形式**。原缺陷是"混合分隔符"，新断言确实能挡住混合分隔符（因为纯 PathBuf 渲染不含混合），所以这个改动方向是对的——但**没人在 Windows 上跑过它**。

### 6.3 Clippy / audit / 漂移检查是真门禁吗？

| 检查 | 定义 | 真门禁？ |
|---|---|---|
| `cargo clippy --all-targets -- -D warnings` | `ci.yml:126-129`，仅 Linux | ✅ 真门禁，无 `continue-on-error`。**但从未执行过** |
| `cargo install cargo-audit --locked \|\| true` | `ci.yml:128` | ⚠️ `\|\| true` 只吞掉**安装失败**；`cargo audit` 仍会跑并在未安装时报错退出 → 事实上仍是硬门禁，但**安装失败时门禁变成"audit 不存在"的假绿风险**。且 `--locked` 装的是最新版 cargo-audit，不是锁定版本 |
| `git diff --exit-code -- ../src/types/bindings.ts` | `ci.yml:131-136` | ✅ **真门禁，本机实测有效**（跑 `cargo test --lib export_bindings` 后 `git diff` 为空，退出 0）。但依赖上一步真的重写了文件——若测试被名字过滤掉（`cargo test` 不加 `--lib` 时有多个二进制输出 `0 tests`），`git diff` 会平凡通过。当前实测能命中，风险是潜在的 |
| 沙箱端口限制 | `VALIDATION_2026-09-28_UX.md` 记录 MCP/WebDAV 本机模拟服务测试曾被沙箱阻止 | ✅ 正确处理（不记为业务通过）。CI runner 无此限制 |

### 6.4 没有任何 job 跑 e2e / 真实 OCR

- `ci.yml`、`beta.yml` 均无 `tests/e2e` 引用（实测 `grep -rn "e2e" package.json .github/workflows/` → 零命中）。
- 无 `playwright` 依赖 → 物理上不可能跑。
- 真实 OCR / E5 / Zvec 召回基准**全部在 `#[ignore]` 里**，任何 job 都不 `--ignored`。
- `beta.yml` 的 `release:validation` 会跑包内真实 OCR（`scripts/smoke-bundle.mjs`）——这是唯一真正触及真实引擎的门禁，**只在 macOS-26 上，且只在 beta/tag 路径**。最近一次 macOS 是 **success**（`194b314` 之前）。

---

## 7. 测试套件脆弱性 / 自我欺骗

| # | 发现 | 级别 |
|---|---|---|
| 1 | **`v20_migration_is_idempotent` 断言被删除**（`src-tauri/tests/schema_v20_test.rs:139-148`），只留 `let _version` + 注释。用例名声称验证幂等，实际只验证"不 panic"。注释诚实（他处有 4 处版本断言），但**用例名与实际断言不符** | CONFIRMED 弱化 |
| 2 | **`read-integrity.test.mjs` 是 grep 伪门禁**，只扫 `commands/`，漏掉 `ai/`（20+ 处同类缺陷）、`db/`、`sync/`、`email/`、`mcp/` | CONFIRMED |
| 3 | `ai_gateway_test.rs:73,227` 只 `assert!(x.is_ok())` | CONFIRMED 弱断言 |
| 4 | 整个 61k 行后端**零快照测试**。任何数据结构/输出格式变化都不会变红 | CONFIRMED |
| 5 | 43 个 schema 版本，**没有「旧库迁移后 == 新装库」的等价性测试**。`test_full_migration_idempotent_reaches_target`（`schema.rs:4255`）只断言版本号到达 43；有 v11/v12/v23/v24 的单独幂等测试。**"迁移漂移"这一整类 bug 无护栏**——而 `db/schema.rs` 是全仓最大文件（4784 行） | CONFIRMED 缺失 |
| 6 | `RELEASE_0.1.3.md` 记录「Workspace lifecycle regression initially failed... Replaced that assertion with Markdown parsing, URL-to-path resolution, destination equality and actual file-content verification」——这是**加强**不是弱化，方向正确。`workspace_sync_test.rs:12-90` 现状确实是强断言（符号链接、`.tmp` 排除、快照内容校验）。**这一条是正面结论** | CONFIRMED 正面 |
| 7 | 两个测试文件（`ai_profiles_test` 5 例、`knowledge_index_test` 2 例）在共享 `OnceLock` 临时库上无互斥锁并发跑 | SUSPECTED 偶发 |
| 8 | `RELEASE_0.1.3.md` / `VALIDATION_2026-09-28_UX.md` 记录过多次「并行编译导致 vitest 5 秒超时 → 限制 worker 后通过」。这些**是真实超时不是断言失败**，处理正确，但意味着"355 全绿"依赖于 `--maxWorkers` 而 `package.json` 的 `test:unit` 没有设 | CONFIRMED |
| 9 | 文档数字（345/4、脚本 5、schema 42）与实测（358/5、脚本 7、schema 43）系统性偏离 | CONFIRMED |

---

## 8. 构建完整性

| 检查 | 是否门禁 | 状态 |
|---|---|---|
| `npm run typecheck`（`vue-tsc --noEmit`） | ✅ 是。`build` = `vue-tsc --noEmit && vite build`，`ci.yml` frontend job 单独再跑一次 | 实测通过。`tsconfig.json` `"strict": true` |
| `npm run build` | ✅ 是 | 实测通过（1.69s） |
| **Lint** | ❌ **完全没有** | 无 `.eslintrc*`、无 `eslint.config.*`、无 `.prettierrc*`、无 `biome.json`；`package.json` 无 `lint` script；CI 无 lint 步骤。`eslint` 不在依赖里 |
| **大型 chunk 警告** | ❌ 不门禁，且阈值被**调高**过 | `vite.config.js:19` `chunkSizeWarningLimit: 900`（Vite 默认 500）。文档说"仍有大型 chunk 提示"——实测**仍然触发**：<br>`chunk-EIO257PC-Cc9S-x58.js` **1,821.03 kB（gzip 744.22 kB）**<br>`elk-276RUBZZ-BDMgp8gP.js` 1,457.74 kB<br>`es-CQiPsXmv.js` 913.65 kB |
| bindings 漂移 | ✅ 真门禁（Linux job），本机实测有效 | 无漂移 |
| `npm run test` 的覆盖面 | ⚠️ 有缺口 | `typecheck` + `test:unit` + `cargo test`。**不含** `node --test scripts/*.test.mjs`、**不含** doc-engine 测试、**不设** `CASY_TEST_DATA_DIR`。而 `DEVELOPMENT.md` 的"验证分层"节把这两项列为必跑 |
| `scripts/verify-bundle.mjs` / `smoke-bundle.mjs` | ✅ 内容很硬（codesign `--deep --strict`、逐文件 SHA-256、manifest 组件完备性、`otool -L` 外部依赖白名单、真实 OCR 冒烟） | ❌ 但**本身零单测**（需要真实产物），只在 `release:validation` 里跑 → 只有 macOS tag/beta 路径覆盖 |
| `tools/casy-doc-engine` / `tools` 锁文件 | ✅ `Cargo.lock` 都在，CI 用 `--locked` | ✅ |

---

## 9. 严重问题（按严重度）

### 🔴 S1 — `ci.yml` 从未在这份代码上运行过

**证据**：`gh api .../actions/runs` 显示 `CI` workflow 最后一次运行是 2026-08-26 on `main`（落后 25 个 commit）；仓库零 PR；`ci.yml` 只在 `main` 和 `v*` tag 上触发。
**影响**：typecheck、355 前端、7 脚本、clippy 零警告、cargo audit、bindings 漂移、Windows 测试、三平台矩阵——**全部是纸面门禁**。
**"全绿"因此不成立**：所有绿灯都来自某个人在某一台 macOS 上手动跑的命令。

### 🔴 S2 — 分支 CI 长期红色，且失败点正是数据路径

**证据**：`beta.yml` 最近 3 次 `package (windows-2025)` = failure/failure/cancelled。2026-09-28 那次 2 个 `portable_backup` 测试失败，`left: "/new/case\\file.pdf"` vs `right: "/new/case/file.pdf"`——**生产代码在 Windows 上生成了混合分隔符路径**，会被写进备份 Markdown。
**影响**：`194b314` 的修复**从未在 Windows 上验证**。备份/恢复是数据不丢的最后一道线。

### 🔴 S3 — 69% 的后端命令（256/369）从未被任何测试提及

**证据**：§2 表格 + 复现脚本。`email/`（1365 行）、`sync/feishu.rs`（2287 行）、`ai/reports.rs`（1062 行）、`commands/inbox.rs`（2739 行，20% 覆盖）、`commands/sync.rs`（1263 行，9% 覆盖）、`db/search.rs`、`db/vector_index.rs` 是 0 覆盖重灾区。
**这直接回答"哪些功能可能坏了没人知道"**：邮件同步、飞书同步、AI 报告与递归检查、案件检索、向量检索封装、全局搜索、白板场景、案件时间线、项目/筛选器/决策审批。

### 🔴 S4 — `ai/` 下 20+ 处活的「静默丢行」缺陷，而守护它的门禁扫不到那里

**证据**：§1.5 的 grep 输出。审计报告自己把这一类列为"confirmed data-integrity defect"，`read_failure_regression_test.rs` 为此写了 9 个命令的回归，但 `ai/insights.rs`、`ai/reports.rs`、`ai/recursive_check.rs`、`ai/mod.rs`、`ai/distillation.rs`、`db/cases.rs:614` 同样的写法还在。
**影响**：AI 洞察/报告在数据库行解码失败时**返回残缺列表并显示为成功**。用户看到的是"AI 说没有"，而不是"读取出错"。这正是用户描述的"功能坏了但没人知道"的典型形态。

### 🔴 S5 — 56% 的前端文件（35,574 行）零测试引用，整个 AI 界面无测试

**证据**：§3 表格。`src/modules/ai` 全部 6 个文件 3,769 行，实测无任何测试 import。`AICompanionView.vue` 1,181 行、`AIChatPanel.vue` 917 行。`TasksView.vue` 1,843 行、`CaseListView.vue` 3,038 行、`DocumentEditor.vue` 1,467 行、`HomeView.vue` 1,471 行同样无测试。
**影响**：所有前端测试都在 `vi.mock` 之后跑，**没有一个测试跨过 IPC 边界**。前端绿灯 = "本地状态机和纯函数没坏"，不是"界面能用"。

### 🟠 S6 — 17 个 e2e 脚本物理上无法执行，但外观上像有 e2e

**证据**：§4.3。`playwright` 不在依赖里且不在 `node_modules`；`package.json` 和两个 workflow 都无 `e2e` 引用；`vitest.config.ts` 的 `include` 不匹配 `.mjs`；3 个脚本 `assert()` 私有环境变量。
**影响**：`DEVELOPMENT.md` 说"17 个本地脚本"——这份清单会被读成测试覆盖，实际是 0 覆盖。真实 OCR / 真实引擎 / 真实文件选择的验证全部依赖人工一次性验收，不可重复。

### 🟠 S7 — 43 版 schema 迁移没有「迁移后 == 新装」等价性测试

**证据**：`db/schema.rs` 4,784 行，`CURRENT_SCHEMA_VERSION = 43`；只有 v11/v12/v23/v24 的单独幂等测试和 4 处版本号断言；无 `sqlite_master` 全量比对。
**影响**：一个用户从 v20 升到 v43 的路径，与一个全新安装的 v43，可能结构不同。这类 bug 只在真实用户升级时暴露，而测试永远发现不了。**这是"生产实验时冒 bug"的最可能来源之一。**

### 🟠 S8 — 测试跑在明文 SQLite 上，SQLCipher 路径几乎无覆盖

**证据**：`db/mod.rs:292-296` test mode 直接 `Connection::open` 无 `PRAGMA key`；全仓仅 `backup_restore_test.rs:71` 一处用过 `PRAGMA key`。
**影响**：迁移、`VACUUM INTO` 备份、密钥不匹配、明文→密文迁移（`migrate_to_encrypted`，含 `ATTACH ... KEY` + `sqlcipher_export`）这些只在生产路径。`RELEASE_0.1.3.md` 称"节假日 SQLCipher 集成测试"是不准确的。

### 🟡 S9 — 零 lint

无 eslint/prettier/biome/oxlint，无 lint script，无 CI lint。`skipLibCheck: true`。约 46k 行前端 + 61k 行后端只有编译器做静态检查。

### 🟡 S10 — `npm test` 不设隔离环境变量；`enable_test_mode` 是进程级全局

见 §5.3。今天安全靠巧合，不靠机制。

---

## 10. 「你不该相信什么」清单

以下每一条都是"某个先前的绿色结论**没有**建立的事实"。

1. **"345 个 Rust 测试全过" ≠ 后端是对的。** 369 个命令里 256 个（69%）从未被任何测试提及。邮件、飞书同步、AI 报告、案件检索、向量检索封装、时间线、项目、筛选器、决策审批——一个都没测。

2. **"355 个前端测试全过" ≠ 界面能用。** 56% 的前端文件（35,574 行）零测试引用；所有测试都在 `vi.mock` 之后，**没有一个触发过真实 IPC**；整个 AI 界面（3,769 行）零测试。

3. **"CI 是绿的" ≠ CI 跑过。** `ci.yml` 最后一次运行是 **2026-08-26 on `main`**，落后 25 个 commit。clippy 零警告、cargo audit、bindings 漂移、**Windows 测试**——在这份代码上**一次都没跑过**。

4. **"分支 CI 在跑" ≠ 它是绿的。** `beta.yml` 最近 3 次 Windows 打包 = failure/failure/cancelled。`portable_backup` 2 个测试在 Windows 上真实失败，暴露了**生产代码生成混合分隔符路径**。**"全绿"只是 macOS 的全绿。**

5. **"17 个 e2e 测试" ≠ 有 e2e 覆盖。** playwright 没装、无人调用、需私有环境变量。**它们一次都没运行过，也不能运行。**

6. **"OCR / 向量检索 / 真实模型验证过" ≠ 可重复验证。** 真实 OCR、真实 E5 语义检索、HNSW 召回基准、私有 Excel、飞书快照——共 10 个 `#[ignore]`，**任何 CI job 都不带 `--ignored`**。相关结论全部来自人工一次性验收。

7. **"隔离测试资料库" ≠ 靠机制保证。** 实测确实安全（假 HOME 验证 0 文件产生），但 `npm test` 不设 `CASY_TEST_DATA_DIR`，安全性依赖"每个测试作者都记得调 `enable_test_mode()`"这个约定。历史上已经因此坏过 3 次。

8. **"数据完整性缺陷已修复" ≠ 缺陷类别已消除。** 审计确认的"静默丢行"缺陷在 `commands/` 里被门禁守住，但 `ai/`、`db/` 里**同一写法还有 20+ 处活的**。`read-integrity.test.mjs` 只扫一个目录。

9. **"schema 已到 42/43 且迁移有测试" ≠ 升级路径正确。** 没有任何测试比较"旧库迁移后"与"新装库"的 schema。43 个版本、4,784 行迁移代码，只有 4 个版本有单独幂等测试。

10. **"类型检查 + 构建通过" ≠ 有代码质量门禁。** 仓库**没有任何 lint 工具**。`chunkSizeWarningLimit` 被从默认 500 调到 900，而实际仍有 1,821 kB（gzip 744 kB）的 chunk。

11. **"验证分层"文档里的命令清单 ≠ 实际会跑的东西。** `DEVELOPMENT.md` 要求跑脚本测试和 doc-engine 测试，但 `package.json` 的 `npm test` 不含这两项。

12. **"文档里的数字" ≠ 现实。** 文档：Rust 345/4 忽略、脚本 5、schema 42。实测：**358/5 忽略、脚本 7、schema 43**。文档系统性地落后于代码，用它当验收依据会误导。

13. **`v20_migration_is_idempotent` 通过 ≠ 迁移幂等被验证。** 它的断言被删了，只剩"跑第二遍不报错"。

14. **"Windows 在测试矩阵里" ≠ Windows 被验证过。** `windows-2025` 是 round-2 才加回 `ci.yml` 的，`read-integrity.test.mjs:13` 甚至专门断言它必须在。但**从未执行过**，而 beta 路径上的 Windows 是红的。

---

## 11. 修复建议（按性价比排序）

### P0 —— 一小时内可做，立刻改变"绿灯"的信息量

**1. 打开 `ci.yml` 的分支触发。** 把 `push.branches: [main]` 改成 `[main, codex/casy-0.1.3-production-validation]`，或（更好）为该分支开一个 PR。
> **为什么排第一**：这是唯一一个把"所有门禁"从纸面变成现实的改动。没有它，下面 2-9 项做出来也还是只有本机 macOS 一个证据来源。成本：改一行 YAML。

**2. 把 `CASY_TEST_DATA_DIR` 写进 `npm test`。**
```json
"test": "npm run typecheck && npm run test:unit && node --test scripts/*.test.mjs && cd src-tauri && CASY_TEST_DATA_DIR=$(mktemp -d) cargo test --locked"
```
> **为什么排第二**：目前 `npm test` 连脚本测试和 doc-engine 测试都不跑，与 `DEVELOPMENT.md` 的"验证分层"不一致。改完之后 `npm test` 才是真正的"一次跑全"。同时把隔离从约定变成机制。

**3. 迁移等价性测试（最高性价比的单条测试）。** 在 `src-tauri/src/db/schema.rs` 加一个测试：
```rust
#[test]
fn migrated_database_matches_a_fresh_install() {
    let fresh = Connection::open_in_memory().unwrap();
    fresh.execute_batch(SCHEMA_SQL).unwrap();
    let old = Connection::open_in_memory().unwrap();
    old.execute_batch(SCHEMA_SQL).unwrap();   // 或 v1 骨架
    old.pragma_update(None, "user_version", 1).unwrap();
    run_migrations(&old, 1).unwrap();
    // 对两库 dump sqlite_master(sql) + PRAGMA table_info 全部表 + 索引名 + trigger 名，比对
}
```
> **为什么排第三**：约 60 行代码，直接覆盖 S7 这一整类"用户升级才暴露"的 bug，而 `db/schema.rs` 是全仓最大文件（4,784 行）且用户正在做生产实验。这是**最便宜的单条测试能捕获最多真实 bug** 的地方。同一测试可对 v20、v30、v40 三个起点各跑一次。

**4. 把 `read-integrity.test.mjs` 的扫描范围从 `commands/` 扩到全部后端目录，并加宽正则。**
```js
const DIRS = ['commands','ai','sync','deadline','email','mcp','db','files','parse','docsy_engine','credentials','workspace_sync.rs'];
// 正则补上：/\.ok\(\)\.flatten\(\)/, /filter_map\(\s*Result::ok/, /filter_map\(\|\s*\w+\s*\|\s*\w+\.ok\(\)\s*\)/
```
> **为什么排第四**：改 10 行，就能让当前 20+ 处活的静默丢行（S4）立刻暴露。这不是"写新测试"，是"让已存在的门禁真的守住它声称守住的范围"。

### P1 —— 一两天，覆盖最高风险的数据链

**5. 修 `portable_backup` 的 Windows 门禁并验证。** 在 Windows runner 上跑一次 `cargo test --lib commands::portable_backup`，确认 `194b314` 的 `portable_path()` 归一化真的修好了混合分隔符。顺手把 `relocation_respects_path_boundaries` 的期望值改成 `portable_path(&...)` 包裹，让它显式表达"必须是单一分隔符"的不变式。
> 直接消除 S2。这是备份/恢复链，丢数据的代价最高。

**6. 复制 `read_failure_regression_test.rs` 的模板到 `ai/`。** 加一个 `#[tokio::test]`，对 `ai::insights::list_*`、`ai::reports::*`、`ai::recursive_check::*`、`db::cases::*` 注入 `X'FF'` 脏行，断言返回 `Err`。
> 约 80 行，覆盖 S4 的全部 20+ 处。可以先只做 `ai/reports.rs`（1,062 行，0 测试，5 处丢行）——这是单个文件中风险最高的。

**7. 补 `email/mod.rs` + `email/smtp.rs`（1,365 行，0 覆盖）的最小测试。** 至少覆盖：IMAP 凭据解析、SMTP 发送失败时的错误传播、`send_ics_invitation_cmd` 的 ICS 内容正确性。参照 `ai_profiles_test.rs` 的 `TcpListener::bind("127.0.0.1:0")` + `INIT.call_once` 本地服务器模式（该模式已在 3 个测试里跑通，可直接复用）。
> `email/` 是**唯一一个 1,000+ 行、7 个命令、零测试的整块子系统**。

**8. 让 `npm test` / CI 至少跑通一个真实 e2e。** 二选一：
- (a) 加 `playwright` 到 devDependencies，写一个**只覆盖一条最关键链路**的 e2e（例如"新建案件 → 上传文件 → 重启 → 数据仍在"），需要真后端桥；
- (b) 若不做 (a)，**把 `tests/e2e/` 移出仓库或重命名为 `qa/`**，并在 `DEVELOPMENT.md` 里明确写"这些不是测试，是手工 QA 脚本"。

> (b) 成本 5 分钟，消除"看起来有 e2e"的误导。(a) 成本约半天，但能真正堵住最大的洞。**建议先做 (b)，把它记为欠债，再排 (a)。**

### P2 —— 一周内

**9. 加 ESLint + `eslint-plugin-vue` + `no-floating-promises`，并接入 `ci.yml` frontend job。** 从 `recommended` 起步，先只开 `no-floating-promises` 和 `vue/essential`。约 1 小时，立刻覆盖 S9。
> 前端 46k 行目前只有编译器在管。异步 promise 吞掉错误是这类应用最常见的静默失败。

**10. 给 `ai/retrieval.rs`、`ai/gateway.rs` 加「真实模型」冒烟测试，并让 CI 至少跑一次 `--ignored` 的白名单。**
```bash
cargo test --locked -- --ignored local_retrieval zvec_recall
```
配合缓存模型资产（~1.1 GB，可以放 CI cache）。
> 目前"向量检索有效"完全是人工断言。HNSW 召回率尤其重要——它是"知识库搜索"这个核心功能的质量底座。

**11. 给前端输出/数据结构加 3-5 个快照测试。** 针对 `mdBridge`（已有 31 个用例）、`docsy_engine` 的 `export_datatype` 输出、`bindings.ts` 的关键 DTO。
> 零快照意味着任何数据结构变化都不会变红。3-5 个快照比再加 30 个 `toBe(true)` 有用得多。

**12. 修 `read_failure_regression` 类测试的进程级共享 DB 问题。** 给 `ai_profiles_test.rs`（5 例）和 `knowledge_index_test.rs`（2 例）加 `static MUTEX: tokio::sync::Mutex<()>`，与 `excel_import_test.rs` 已有的 `TEST_MUTEX` 模式保持一致。
> 消除 S7 隐患。成本 5 行 × 2 文件。

### P3 —— 需要产品决策

**13. 让 SQLCipher 至少有一条端到端测试。** 在 `db/mod.rs` 加一个不受 `IS_TEST_MODE` 影响的测试入口（或用 `TEST_ENV` 之外的显式 flag），走完 `open_db_encrypted()` 全流程：建库 → 写入 → 关闭 → 重开（带 key）→ 校验 → 用错 key 重开 → 断言失败且库未被修改。
> 消除 S8。这是唯一能验证"加密库升级不出事"的方式。

**14. 处理 1,821 kB 的 chunk。** `chunk-EIO257PC-Cc9S-x58.js`（gzip 744 kB）值得查一下是什么——如果是 mermaid/cytoscape 之类的懒加载依赖没拆开，这是启动性能问题。同时把 `chunkSizeWarningLimit` 改回 500，让警告重新有意义。
> 需要 `vite build --mode analyze` 或 rollup visualizer 确认构成。

---

## 12. 一句话总结

**739 个测试在跑，749 个用例，0 个失败——但它们覆盖的是 61k 行后端里的 31% 和 46k 行前端里的 44%。剩下的部分不是"测过没问题"，是"没人看过"。** 同时，纸面上的所有门禁（clippy、audit、bindings 漂移、Windows 矩阵）在这份代码上一次都没跑过，唯一跑过的分支 CI 现在是红的，而它的失败点恰好在备份路径重定位——数据不丢的最后一道线上。**"测试全过"这句话在这份代码上的信息量，约等于"我写的 739 个断言没坏"。**

# specta derive 扫描报告（D-3 接入清单 · 只读盘点，未接 CI）

> **日期**: 2026-08-24　**扫描基线**: a3d2b68 + 工作区（K-3 深化后）
> **范围**: src-tauri/src 全部 `#[tauri::command]`（204 个）的参数/返回类型链路
> **性质**: 静态正则扫描 + 人工抽查。已知误差：同名类型定义以最后扫到者为准（个别需人工核对）；宏生成签名不在覆盖内。

---

## 一、结论摘要

| 口径 | 数量 | 说明 |
|---|---|---|
| 命令总数 | **204** | 与 v1.5 时的 202 基本一致（新增 `reminder_recompute_now`、`record_ai_tool_audit`） |
| 纯标量 / 已就绪 | 65 | 参数返回全是基本类型，specta 无需 DTO 工作 |
| **含 JSON 动态参数** | **92** | `data: serde_json::Value` 型命令——**specta 无法为其生成有意义类型**，其类型化依赖 B1 DomainCommand 收口（先有强类型才有导出），是 D-3 与 B1 的耦合点 |
| 其余（命名 DTO 参与但未就绪） | 47 | 覆盖下表 34 个类型即可全部接入 |
| 已具 `specta::Type` | 0 | 尚未引入依赖，符合预期 |

**核心数字：补齐 34 个类型的 derive，即可让约 112 个非动态命令进入类型导出范围；剩余 92 个动态参数命令随 B1 收口自然解锁。**

## 二、待补清单（34 类型）

图例：现状 S=仅 Serialize、D=仅 Deserialize、SD=齐备。「方向」= 该类型出现在命令的哪个位置。

### 返回型（需 `Serialize`(部分缺) + `specta::Type`）

| 类型 | 现状 | 命中命令数 | 定义位置 |
|---|---|---|---|
| SyncResult | SD | 5 | sync/mod.rs |
| Draft | SD | 4 | commands/drafts.rs |
| Case | SD | 3 | db/cases.rs |
| CaseFile | SD | 2 | commands/files.rs |
| CaseRelation | SD | 2 | commands/relations.rs |
| FeishuSyncReport | SD | 2 | sync/feishu.rs |
| ReminderLogEntry | SD | 2 | commands/reminder.rs |
| AiChatResult | **S** | 1 | ai/mod.rs（本轮 K-3 新增） |
| AiConfig | SD | 1 | ai/mod.rs |
| CalendarSyncReport | **S** | 1 | commands/caldav.rs |
| CaseListResult | **S** | 1 | db/cases.rs |
| CaseStats | **S** | 1 | commands/cases.rs |
| CommandRoute | SD | 1 | commands/ai_routes.rs |
| DashboardStats | **S** | 1 | commands/cases.rs |
| DeadlineResult | **S** | 1 | deadline/engine.rs |
| DeadlineWarning | SD | 1 | commands/reminder.rs |
| ExportResponse | SD | 1 | commands/docs.rs |
| ImportReport | **S** | 1 | commands/import_feishu.rs |
| McpPendingWrite | SD | 1 | mcp/mod.rs |
| QuickJudgeResult | **S** | 1 | commands/inbox.rs |
| RecordDiff | **S** | 1 | commands/sync.rs |
| RelatedCase | **S** | 1 | commands/relations.rs |
| ReminderRule | SD | 1 | commands/reminder.rs |
| RenderResponse | SD | 1 | commands/docs.rs |
| SchemaDiff | **S** | 1 | commands/sync.rs |
| SearchResult | SD | 1 | db/search.rs |
| SyncStatus | SD | 1 | sync/mod.rs |
| TaskTemplate | SD | 1 | commands/tasks.rs |
| TemplateListResponse | SD | 1 | commands/docs.rs |
| TimelineEvent | **S** | 1 | commands/timeline.rs |

### 参数型（需 `Deserialize`(个别缺) + `specta::Type`）

| 类型 | 现状 | 命中命令数 | 定义位置 |
|---|---|---|---|
| CaseFilter | D | 2 | db/cases.rs |
| ChatMessage | SD | 1 | ai/mod.rs |
| ImapAccountConfig | SD | 1 | email/mod.rs |

> 注：返回型中标注「S」的 12 个类型缺 Serialize 的原因多为历史上的只写不读；统一补上即可，无行为影响。

## 三、三域先行（B1: tasks/calendar/inbox）明细

| 域 | 可立即覆盖 | 依赖 B1 收口 |
|---|---|---|
| calendar | CalendarEvent[SD]（get_calendar_events） | —— |
| tasks | TaskTemplate[SD]（模板 3 命令）；TaskFilter 待查（list_tasks 参数为 Value+自定义 Filter 混合，见 §四注） | create/update/apply_template 等 6 命令走 JSON 动态 |
| inbox | 几乎为零 | **25 命令中 24 个走 JSON 动态**（收件箱处理链路全是 DomainCommand 化对象） |

**结论：inbox 域的类型导出与 B1 是同一件事**；calendar/tasks 先行可出成果，inbox 随收口解锁。

## 四、JSON 动态参数说明（92 命令）

这批命令的参数形态是 `data: serde_json::Value`（或部分字段动态），serde 层无结构可导出。它们正是 B1 计划中的 DomainCommand 改造对象——改造完成后自动获得强类型，届时按本文清单流程追加 derive 即可。**不建议为 Value 手写 TS any 占位**（违背 D-3 目的）。

## 五、接入步骤（供执行时照抄，本报告不含实现）

1. Cargo.toml 加 dev-dependencies 或 dependencies：
   `specta = "2"`、`specta-typescript = "0.7"`（版本以 cargo search 为准）
2. 目标 struct 加 `#[derive(specta::Type)]`（与现有 serde derive 并列，无需改 serde 配置）
3. 建 export 桩（推荐 tests 形式，`cargo test` 即产出）：

   ```rust
   #[test]
   fn export_bindings() {
       specta_typescript::Typescript::default()
           .export_to::<TaskFilter, _>("../src/types/bindings/")
           .unwrap();
   }
   ```
4. CI 校验（后续接 R-3）：`cargo test && git diff --exit-code src/types/bindings`
5. 前端消费顺序：bindings → tauriBridge CommandMap 泛型映射 → 三域 service 方法签名替换（views 零改动）

## 六、与排期的关系

- 本报告对应 §八第 3 周「specta 落地启动」的准备产物
- 实际 derive 补齐建议随 B1 各域收口逐域进行（每域 ≤10 分钟机械工作）
- CI 校验挂在 R-3 流水线上，避免双轨维护

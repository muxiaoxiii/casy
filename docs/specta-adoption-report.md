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

## 五、接入实现（✅ 已落地，2026-08-24）

实际落地与原计划有两处演进：

1. **版本对**：registry 当前稳定组合为 `specta = "1.0.5"`（features = ["export"]），
   未引入 specta-typescript / specta-serde（0.0.12 线绑定 specta v1 / v2-rc 各异，碎片化）。
   specta v1 自带 TS 导出器（`specta::export::ts_with_cfg`），derive 宏经 ctor 自动注册全局类型表。
   未来迁 v2 时 derive 同名（`specta::Type`），迁移成本≈改依赖版本。
2. **导出桩**：src-tauri/src/export_bindings.rs（#[cfg(test)]），`cargo test export_bindings`
   生成 `src/types/bindings.ts`；口径 i64 → number（Casy 的 i64 均为计数/时间戳，安全整数范围）。
   CI 校验（git diff --exit-code src/types/bindings.ts）随 R-3 接入。

### 落地结果

| 项 | 数值 |
|---|---|
| 带 `specta::Type` 的类型 | **39**（清单 34 + 传递闭包 5：RecentActivity/QuickRecommendation/DocsyTemplate/FieldDiffItem/RecordDiffItem + TemplateField） |
| 补 Serialize 的返回型 | 12 |
| 生成的 bindings | src/types/bindings.ts（39 个 export type，camelCase，含文档注释） |
| 缓办 | CalendarEvent ×2（commands/calendar.rs 为 M-CAL-1 主线占用文件且存在同名双定义，待解冻后统一） |

#[derive(specta::Type)]`（与现有 serde derive 并列，无需改 serde 配置）
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

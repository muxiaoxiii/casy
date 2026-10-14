# Casy 0.2.0 升级计划 · 卷宗智归（Intake Intelligence）

核对日期：2026-10-13。适用于 `codex/casy-0.1.3-production-validation`（当前 `0.1.4-beta.1`）。
本文件是**大型版本升级的规划**，不是实施记录；实施进度另记。

> **协作内容一律不做**（2026-10-13 确认）：单机单人定位。原方案里唯一带协作语义的是"待认领池"，已改为单用户的"待定归属"（§3.4）；OA 的参与人转化、协同日程推送等只作学习记录，不进入实施。

---

## 0. 结论先行

**一句话**：把"取文 → 解析 → 建议 → 冲突确认 → 建档 → 留痕"做成**一条统一协议**，让任何来源的文件在进入 Casy 后都自动走完这条路，而不是每个入口各写一遍、且都在最后一步断掉。

- 当前最痛的断裂：法院送达文书下载后**零解析**——不 OCR、不提开庭日期、不建 `hearings`、不建期限，只搬了个文件（`src-tauri/src/commands/inbox.rs:1517` `download_service_delivery_via_api` 落盘后只 UPDATE `inbox_items`）。
- 但 PDF 里的开庭日期**本来抽得出来**：`parse::parse_summons` / `parse_hearing_notice` 已有 `hearing_date` 正则（`src-tauri/src/parse/mod.rs`），只是它们只在**捕获期的粘贴文本**上跑，归卷后的 PDF 走 OCR 管线后不再做任何语义抽取。两条解析路径分裂，是本次升级要缝合的核心伤口。
- Casy 独有的差异化优势：OCR 产物带 `source.map.json`（文本片段 → 页码/区域/版面坐标，`tools/casy-doc-engine/src/source_map.rs`），所以每个自动提取的字段都能**回溯到原件上的确切位置**。参照的律智荟 OA 完全没有解析能力，更谈不上溯源。
- 分 4 个 Phase，每个可独立验收、独立发布；Phase 1 直接解决用户点名的痛点。

---

## 1. 现状诊断：链路在哪断的

以下每条都经代码勘察确认（括号内为位置）。

| # | 断裂点 | 现状 | 后果 |
|---|---|---|---|
| D1 | 送达文书下载后零解析 | `download_service_delivery_via_api` 只落盘 + UPDATE `inbox_items`，不 queue OCR、不读正文 | 用户原话："下载之后，就没了" |
| D2 | OCR 管线无语义层 | `document_pipeline.rs` 到 markdown/page IR 为止，无开庭/期限/案号抽取 | 传票里的开庭日期永远不自动进日程 |
| D3 | 归卷不触发 OCR | `file_inbox_item → register_inbox_case_file`（`inbox.rs:70`）只 INSERT `case_files`；OCR 入队只在 `add_case_file` 且 `category=="other"` 时发生（`commands/files.rs:78`） | 送达文书归卷后仍是"未解析的死文件" |
| D4 | 开庭只能手动建 | `hearings` 行只来自 `create_case_hearing`、飞书/Excel 导入、demo；`auto_create_hearing_tasks`（`inbox.rs:892`）只建"庭审准备任务"，不建 hearings | 开庭提醒依赖用户手抄短信 |
| D5 | `create_deadline` 降级 | inbox 的 deadline action 实际走 `create_task_in_transaction` 建任务（`inbox_actions.rs:46`），不写 `case_deadlines` | 法定期限与手动期限入口分裂，期限引擎算出的结果无处落 |
| D6 | 日程创建无冲突检测 | `create_task_in_transaction`（`tasks.rs:238`）、`create_event_in_transaction`（`calendar_events.rs:97`）只做格式校验；`procedure.rs:1157` 明确"同日开庭只是排期风险" | 同案两个开庭日期、任务撞期都不报警 |
| D7 | 无重复材料检测 | 无 sha256/文书编号判重路径（`case_files.source_sha256` 已存在但只用于变更检测） | 同一传票被送达两次会归卷出两份 |
| D8 | 批量能力是占位 | `start_inbox_batch`（`inbox.rs:2775`）、`capture_screenshot`、语音转写均返回"开发中" | 大口袋只能一件件处理 |

---

## 2. 参照系：律智荟 OA 手册的得与失

已完整抓取该帮助文档（188/188 页，MkDocs 静态站）。**关键发现：这份手册恰恰证明"用户点名的那些交互"连成熟 OA 也没有**，所以它们是 Casy 的真实差异化机会，而不是"别人有我们没有"的补课。

### 2.1 它没有、而我们能差异化（重点）

| 缺失能力 | 手册证据 | Casy 机会 |
|---|---|---|
| 收件箱 / 邮件短信取文 | 137 页全文检索命中 0；文件只能手工上传、云盘同步、模板反向生成 | 大口袋是 Casy 的立身之本，应做到自动取文 + 自动归集 |
| 文书解析与字段提取 | `OCR`/`解析`/`智能识别` 命中 0；上传后不提取任何字段 | OCR + 语义抽取 + **溯源到原件坐标** |
| 法定期限管理 | `举证`/`上诉`/`期限`/`时效` 命中 0，期限被"日程+任务截止"弱替代 | `deadline/engine.rs` 已有 15 条法定规则，应显式建模并暴露 |
| **同案两个开庭日期的提示** | 无；开庭信息是自由录入列表 | **用户点名的核心交互，直接做** |
| **改期确认 / 开庭变更留痕** | `改期` 命中 0 | 改期必须留痕 + 证据指针 |
| 期限冲突提示 | 无 | 期限与开庭撞期检测 |
| 重复材料检测 | `重复材料` 命中 0 | sha256 + 法院文书编号双重判重 |

### 2.2 它有、值得我们学（5 条）

1. **待认领池**（收款认领，`/web/financial-affairs/collection-management/`）：无主事项先入池，由人认领并挂到正确实体，认领后才进审核。→ **借鉴其"无主事项有明确归宿、且归属未定前不进入下游"的机制**，但去掉多主体认领，改为单用户的"待定归属"（§3.4）。
2. **流程晋升节点做一次重复数据确认**（案件转正时校验客户是否重复，`/web/case/case-filing-process/positive-cases/`）：用一个人工确认点兜住主数据质量，比全程自动去重更可控。
3. **可回退的审批分支**（5 种审批结果，含"退回给冲突检索人"）：让结论能被精准修正而不是全盘重来。→ 对应我们的"确认后可改判"。
4. **不可逆操作前置告知**（"确认保存后不可再替换模板"）：把不可逆性说在前面。
5. **机器分词结果可人工编辑后重搜**（冲突检索）：把 AI 召回变成可调参数而不是黑盒。→ 对应我们的抽取结果应可人工修正，且修正要回流为学习信号。

### 2.3 它有、我们不能学（4 条）

- **整套审批流/角色权限/多租户**：单机单人场景只会变成永远 pending 的负担。改为"单人自审 + 事后留痕"。
- **第三方云盘作为文档主存储**：与本地优先冲突。本地文件系统是唯一真相源。
- **依赖第三方服务的企业信息匹配/电子签章/合同库**：必须离线可用，集成只能做可选插件并明确降级。
- **统计报表体系 / 工作台插件区 / 多语言**：过度配置化。桌面应用应给一套打磨好的默认。

---

## 3. 目标架构：一条流水线 + 一个协议

```
任意来源（sms/file/email/paste/note/imap）
   │
   ▼
① 取文（已有：add_inbox_item / 送达 API 下载）
   │
   ▼
② 解析层 Document Facts（新）── OCR（已有）+ 语义抽取（新）+ 溯源（已有 source.map.json）
   │
   ▼
③ 建议引擎 Proposal（新）── 生成"待确认建议"：归卷/建开庭/建期限/建任务/判重
   │
   ▼
④ 冲突确认协议 Conflict Protocol（新，核心）── 与既有实体一致性检查 → 用户确认
   │
   ▼
⑤ 建档（部分已有）── hearings / case_deadlines / calendar_events / tasks / case_files
   │
   ▼
⑥ 留痕（新）── schedule_change_log + intake_feedback，全部带证据指针
```

### 3.1 文书语义层（Document Facts）

统一抽取器，**双输入**：捕获期文本（现有 `parse::` 正则）与 OCR 产物（markdown + `source.map.json`）。

```rust
struct DocumentFacts {
    doc_type: DocType,            // summons / hearing_notice / judgment / ...
    court: Option<String>,
    case_no: Option<String>,
    parties: Vec<Party>,
    hearings: Vec<HearingFact>,   // 日期 + 时段 + 地点 + 法庭 + 届次
    deadlines: Vec<DeadlineFact>, // 举证期限 / 上诉期 / 答辩期 / 答辩状
    confidence: f32,
}
struct Fact<T> { value: T, provenance: Provenance, confidence: f32 }
struct Provenance { page: u32, region: Option<usize>, bbox: Option<[f32;4]>, markdown_span: Option<(usize,usize)> }
```

要点：
- **每个字段都带出处**。这是 Casy 对一切参照产品的代差优势，也是"自动建档敢让用户信"的前提。
- 规则抽取优先（可解释、离线、零依赖），AI 抽取作为可选增强（已有 `ai/extract_info.md` prompt 含 `hearing_date/venue`，接进来即可）。
- 抽取结果**可人工修正**，修正写 `intake_feedback` 回流（现有 `reject_inbox_recommendation` 已有此机制，扩展为字段级）。

### 3.2 建议引擎（Proposal）

解析成功不直接执行，而是生成**待确认建议**（对应 OA 的"待认领/待审核"）：

```rust
enum ProposalKind {
    LinkCase { case_id, confidence },      // 归卷到哪个案件
    CreateHearing { hearing: HearingFact },
    CreateDeadline { deadline: DeadlineFact },
    CreateTask { ... },
    Dedupe { existing_case_file_id },      // 判定为重复材料
    HoldForTriage,                         // 案件不明 → 待定归属（暂不建档，等用户指定）
}
struct Proposal { kind, payload, evidence: Vec<Fact>, confidence, status }
```

铁律：**自动建档只发生在置信度超过阈值且无冲突时**；其余一律进待确认。阈值可调，默认保守（宁可不自动，不可建错——日程建错的代价高于漏建）。

### 3.3 冲突确认协议（Conflict Protocol）——本次升级的核心

任意自动建档动作在**写入前**与既有实体做一致性检查，命中即生成 `ConflictCase` 并请求用户确认：

| 编号 | 冲突类型 | 触发条件 | 用户选项 |
|---|---|---|---|
| C1 | 同案开庭日期不一致 | 新传票的开庭日期 ≠ 该案任一既有 `hearings.hearing_date` | ①两个开庭都建 ②**改期**（原开庭标记 rescheduled，留痕：旧日期/新日期/依据文书） ③都不是开庭（类型误判） |
| C2 | 同日多实体 | 开庭/期限截止/任务 due 落在同一天 | ①都建并标记同日风险 ②调整某一个 ③忽略 |
| C3 | 重复材料 | sha256 相同，或法院文书编号（`c_wsbh`，送达 API 已返回）相同 | ①跳过并关联到已有文件 ②仍建副本 |
| C4 | 目标案件不确定 | 案号匹不到或多候选 | ①选候选案件 ②**留待指定**（暂不建档，进"待定归属"） ③新建案件 |
| C5 | 实体类型存疑 | `doc_type` 置信度低 | ①按建议类型 ②改为其他类型 ③仅存档不解析 |

协议铁律：
- **任何选择都留痕**（`schedule_change_log` + `intake_feedback`），并带证据指针（`case_file_id` + fact provenance）。
- 改期是**状态迁移**不是删除：旧开庭保留、标记 rescheduled、新开庭建立，两者用 `supersedes_id` 关联。现有 `update_hearing`（`cases.rs:1084`）改期强制填 `changeReason` 的护栏思路，扩展到自动路径。
- 确认 UI 复用统一捕获对话框的"复核"阶段模式（`UnifiedCaptureDialog.vue` 已有 review/done 阶段状态机）。

### 3.4 待定归属（Unassigned，单用户语义）

解析成功但案件不明（C4 选项②）的文书进入"待定归属"状态，**不建档、不丢、不自动猜**。条目带：抽取出的案号/当事人/法院/开庭日期（作为指定提示）、OCR 状态、来源。用户指定时：
- 若案号能唯一匹配 → 直接建议归卷
- 若不能 → 列出候选案件 + "新建案件"
- 指定后照走 ③→⑤

> 参照 OA 的"待认领池"，但**去掉多主体认领语义**：单用户场景没有"谁认领"的问题，只有"归属待定"。这不是协作功能。
> 该队列同时是批量处理的工作台（对应 D8）。

### 3.5 日程与期限模型统一

厘清四类时间实体的职责，消灭 D5 的分裂：

| 实体 | 语义 | 来源 |
|---|---|---|
| `case_deadlines` | 法定期限/命令期限，有 `deadline_source` 与 `legal_basis` | ①`deadline/engine.rs` 法定规则自动算 ②法院命令文书提取 ③手动 |
| `hearings` | 开庭，有届次与改期链 | ①传票/开庭通知提取 ②手动 ③导入 |
| `calendar_events` | 一般日程（会议/拜访/预约） | 手动/导入/任务投影 |
| `tasks` | 待办，可带 dueDate | 手动/建议/自动 |

动作：`create_deadline` 改为真正写 `case_deadlines`（保留 `court_order_ref` 指向依据文书）；新增统一"日程总览"视图，把四类实体按时间轴合并展示（`get_calendar_events` 已在做 hearings + calendar_events 合并，扩展到 deadlines 与 tasks）。

### 3.6 数据模型变更

**新表**

```sql
-- 文书语义抽取结果（一 case_file/inbox_item 一行，可重跑覆盖）
document_facts(
  id, ref_type, ref_id,           -- ref_type: case_file | inbox_item
  doc_type, court, case_no, parties_json,
  hearings_json, deadlines_json,  -- 每项含 value + provenance + confidence
  overall_confidence, extractor_version, created_at
)

-- 待确认建议（幂等回执同 inbox_action_results 模式）
intake_proposals(
  id, ref_type, ref_id, kind, payload_json,
  evidence_json, confidence,
  conflict_json,                   -- 命中的冲突（C1..C5）
  status,                          -- pending | confirmed | rejected | superseded | expired
  resolution_json,                 -- 用户选了什么、改判了什么
  created_at, resolved_at
)

-- 日程/期限变更留痕（开庭改期、期限调整、任务改期）
schedule_change_log(
  id, entity_type, entity_id, change_kind,
  old_value_json, new_value_json, reason,
  evidence_ref_type, evidence_ref_id,   -- 依据哪份文书/哪个 fact
  proposal_id, created_at
)

-- 外部编号（法院文书编号 c_wsbh 等），服务 C3 判重
document_external_ids(ref_type, ref_id, id_kind, id_value)
```

**现有表加列**

- `hearings`：`status`（scheduled/rescheduled/cancelled/held）、`source`（manual/document/import）、`source_case_file_id`、`supersedes_id`、`confidence`、`updated_at`。`actual_status` 保留作实际出庭结果。
- `case_deadlines`：`source_case_file_id`（指向依据文书）、`proposal_id`。
- `inbox_items`：`claim_status`（unclaimed/claimed/dismissed）+ `facts_id`。
- `case_files`：`service_doc_no`（法院文书编号，判重键）。

---

## 4. 分期实施计划

每个 Phase 独立可验收、独立可发布；门禁见 §5。

### Phase 1 · 送达文书闭环（直接解决用户点名痛点）

**目标**：粘入法院送达短信 → 下载 → 自动 OCR → 抽取出开庭日期 → 生成待确认建议 → 冲突确认 → 建档留痕。

交付物：
1. 下载完成后自动 `queue_document_processing`（修 D1/D3；复用 `document_intelligence` 既有 durable single-flight worker，不造新队列）
2. `parse::` 抽取器升级为双输入（文本 + OCR markdown），输出 `DocumentFacts` 带 provenance（修 D2）
3. 传票 → 开庭日期 → `intake_proposals(kind=CreateHearing)`
4. **C1 冲突确认协议落地**（同案不同开庭日期 → 三选一，改期留痕）
5. 确认后写 `hearings` + `calendar_events` + `schedule_change_log`（修 D4/D6）
6. 前端：复核对话框展示"建议建开庭 2026-10-26 14:00 · 依据：传票第 1 页 · 置信度 0.93"，含三个选项

验收口径：
- 用真实短信（青岛中院传票）端到端：下载 → OCR → 出现开庭建议 → 确认 → `hearings` 行 + 日历事件 + 案卷关联 + 留痕
- 再喂一份**改期后**的传票 → 触发 C1 → 选"改期" → 旧日期保留并标记 rescheduled、新日期建立、`schedule_change_log` 有完整链路
- 回归：`cargo test --locked` 全绿 + 既有 `delivery_lifecycle_test` / `bunny_deadline_test` / `file_lifecycle_test` 不破

预估：中（主体是接线，抽取器与协议需新写）

### Phase 2 · 统一收件箱流水线 + 待定归属

**目标**：所有来源走同一条 ②→③→④ 流水线；大口袋变成批量工作台。

交付物：
1. `file`/`email`/`imap`/`paste` 来源统一进入解析层（不再只有 SMS）
2. 待定归属队列（§3.4）+ 指定案件交互（C4）
3. `start_inbox_batch` 落地为批量队列（复用 durable worker，断点续算）（修 D8）
4. C3 重复材料检测（sha256 + `c_wsbh`）+ C5 类型存疑确认
5. 置信度反馈闭环：字段级修正回流 `intake_feedback`，用于阈值自调

验收口径：一次导入 20 份混合文书 → 全部 OCR → 建议按案件分组 → 冲突/判重/待定归属分别进入对应队列 → 逐个确认后全部正确建档；中途杀进程不丢进度。

预估：中大

### Phase 3 · 日程与期限模型统一

**目标**：消灭 D5，让期限成为一等公民，冲突检测覆盖全部日程实体。

交付物：
1. `create_deadline` 真正写 `case_deadlines`（含 `court_order_ref` 指向依据文书）
2. 文书提取的期限（举证期限/上诉期/答辩期）→ `CreateDeadline` 建议
3. C2 冲突检测扩展到：同日多开庭、期限截止与开庭撞期、任务 due 与开庭撞期
4. 日程总览视图：`case_deadlines` + `hearings` + `calendar_events` + `tasks` 四类合并时间轴，按案件/法庭/阶段筛选（借鉴 OA 的"开庭信息按案件/法庭/阶段筛选"）
5. 期限引擎结果可直接"转为正式期限"（当前 `evaluate_case` 算完无处落）

验收口径：传票含举证期限 → 建议建期限 → 确认 → `case_deadlines` 行 → 期限引擎的预估审限与手动期限在同一视图不重复不冲突。

预估：中

### Phase 4 · 体验、留痕与稳健

**目标**：把协议变成用户每天用的一个地方，而不是散落的弹窗。

交付物：
1. **确认中心**：一个页面集中处理所有 `pending` 建议与冲突（对应 OA 的"工作提醒/待办"）
2. `schedule_change_log` 统一查询（"这个开庭日期为什么变了"一键回溯到依据文书）
3. 占位能力决策：语音转写/截图捕获——要么落地（本地 Whisper 类方案需评估体积），要么明确标注"规划中"并从菜单隐藏，不留假入口
4. 开庭变更同步外部日历（CalDAV 已有 `sync_reminders_to_calendar`，扩展到 hearings 改期链）
5. 空态/降级：OCR 模型缺失、抽取置信度整体偏低时的明确提示

预估：中

---

## 5. 测试与门禁策略

**新增测试（每 Phase 必须）**
- `document_facts` 抽取：用真实 OCR 产物 fixture（`tests/fixtures/`）断言开庭日期/案号/当事人的抽取值与 provenance 页码正确；含反例（非传票文书不应抽离开庭）
- 冲突协议：C1 三种选择的完整状态迁移（两个开庭 / 改期链 / 类型误判），断言 `schedule_change_log` 链路
- 判重：同 sha256、同 `c_wsbh` 两种重复路径
- 待定归属：指定、改判、过期
- 批量：20 份混合文书 + 中途中断恢复
- 联网路径：延续现有 `#[ignore]` 真机测试模式（如 `service_delivery_live_api`），不打网络

**既有门禁保持**
`cargo test --locked`（当前 408 passed）、`cargo clippy --all-targets` 零警告、引擎 `--features models` 28 passed、`npm run typecheck`、vitest 377、`node --test scripts/*.test.mjs`、`cargo audit`。

**新增门禁建议**
- 抽取器准确率快照测试：固定 fixture 集，准确率下降即失败（防回归）
- 冲突协议状态机穷举测试：所有 C1..C5 × 所有选项的组合都必须落到确定的终态

---

## 6. 风险与取舍

| 风险 | 对策 |
|---|---|
| 开庭日期抽错 → 自动建错日程（最严重） | 三重保险：①每个 fact 带 provenance，UI 必须展示依据位置 ②默认阈值保守，低于阈值只进待确认 ③确认前不写任何日程实体 |
| OCR 未完成就抽正文 → 空结果 | 解析严格依赖 OCR 完成事件，未完成只入队不抽取 |
| 改期链复杂化 | 改期只做一层（`supersedes_id` 单向链），不做多代重排；多代情况人工处理 |
| 待确认堆积成新的"大口袋" | 确认中心按紧急度排序（开庭临近优先）；超期未确认自动过期并留痕 |
| 批量导入把 OCR 队列打满 | 复用既有 durable single-flight + 优先级：手动 > 待定归属 > 批量 |
| 计划过大导致烂尾 | 4 个 Phase 各自独立可发布；Phase 1 单独就有用户价值 |

---

## 7. 明确不做（Out of Scope）

- **一切协作能力**：多人协作、角色权限、审批流、待认领（多主体）、参与人转化、协同推送——单机单人定位，一律不做
- 服务端同步、多租户、云盘主存储
- 第三方企业信息匹配、电子签章、在线合同库（可后续做可选插件）
- 统计报表体系、工作台插件化、多语言
- 语音转写的重量级方案（先标注规划中；若要落地需单独评估模型体积，遵循现有 `models/` 档位治理方式）

---

## 附：本计划的输入来源

- 代码勘察：`src-tauri/src/commands/inbox.rs`、`commands/files.rs`、`deadline/engine.rs`、`document_pipeline.rs`、`document_intelligence.rs`、`db/schema.rs`、`tools/casy-doc-engine/src/source_map.rs` 等
- 参照产品手册：律智荟（AiLinkedLaw）帮助中心 `https://help.ailinkedlaw.com/web/`，188 页全量抓取
- 法院送达链路实测：zxfw「一张网」API 真机联调（2026-10-13，青岛中院传票）

# 模块 01 · 案件管理

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `architecture.md` §5 / `02-status-machine.md` / `06-calendar-deadline.md` / `09-files.md`

---

## 一、职责边界

### 1.1 做什么

- 案件主数据（`cases`）的创建、读取、更新、删除、搜索。
- 客户主数据（`clients`）的维护与案件归属。
- 案件时间线（`case_logs`）与开庭记录（`hearings`）。
- 案件关系网络（`case_relations`，同一专利/同一当事人/上诉关系/交叉引用）。
- 案件相关官方人员（`officials` + `case_officials`）。
- 三轨状态机的写入入口（`update_case_status`）与聚合状态（`case_status`）推导（详见 02）。
- 案件维度聚合查询（`case_stats` / `get_dashboard_stats` / `get_case_unified_view`）。

### 1.2 不做什么

- **不负责**期限计算与提醒（见 06、12）。
- **不负责**案件文件夹的物理管理（见 09，`create_case` 只触发文件夹创建并回写 `folder_path`）。
- **不负责**知识库沉淀（见 07，收件箱归档时的知识写入在 04）。
- **不负责**状态机的完整推导规则（见 02，本模块只提供手动切换入口）。

---

## 二、数据模型

### 2.1 `cases` 主表（核心字段分组）

| 分组 | 字段 | 说明 |
|---|---|---|
| 身份 | `id` / `track` / `case_name` / `case_no` / `internal_no` / `cause_action` | `track` 枚举 `patent_invalidation / admin_litigation / civil_tort / other` |
| 当事人 | `client_name` / `our_role` / `opponent_name` / `opponent_role` / `opponent_firm` / `opponent_agent` | 对手方信息留空默认 `''` |
| 审理 | `court` / `judge_panel` / `clerk` / `attorneys`(JSON) / `case_level` / `case_status` / `case_progress` / `case_result` | `case_level` 枚举 `一审/二审/再审/结案`；`case_status` 由触发器自动推导 |
| 专利 | `patent_name` / `patent_app_no` / `procedure_type` | `procedure_type` 枚举 `普通/简易` |
| 日期里程碑 | `filing_date` / `complaint_received_date` / `trial_date` / `trial2_date` / `trial3_date` / `verdict_type` / `verdict_date` / `stay_date` / `relief_deadline` | 三轨状态机的日期驱动输入 |
| 无效专属 | `petitioner_first_invalid` / `petitioner_supp_deadline` / `petitioner_submit_date` / `petitioner_received_date` / `petitioner_reply_deadline` / `patentee_received_date` / `patentee_statement_deadline` / `patentee_received_supp_date` / `patentee_supp_deadline` / `patentee_submit_supp_date` | 专利无效程序的请求人与专利权人双向期限 |
| 文件夹/文书 | `folder_path` / `last_doc_path` / `last_doc_at` | 由文件模块与文书模块回写 |
| 进度/备注 | `completed_text` / `notes` / `created_at` / `updated_at` | `updated_at` 由触发器自动更新 |

**设计约束**：
- `case_status` 只作为聚合筛选/统计字段，由触发器从 `case_result` 推导（`已完结/进行中/未知`），不得直接手改。
- `case_progress` 是自由文本进展字段，不是状态机来源（见 architecture.md §5.3）。
- 三轨状态字段（`case_route` / `civil_status` / `invalidation_status` / `admin_status`）见 02。

### 2.2 `clients` 客户主数据

- 稳定 `id` + `client_name`（+ 未来 `aliases` 别名归一）。
- 建案时 `INSERT OR IGNORE`，保证同名客户只落一行。
- **角色定位**：认知上是聚合维度（非第五大顶级对象），数据库里是基础主数据，供客户端聚合视图（名下案件数 / 进行中 / 等待中 / 快到期 / 历史结案）使用。

### 2.3 时间线 / 开庭 / 关系 / 官方人员

| 表 | 用途 | 关键约束 |
|---|---|---|
| `case_logs` | 案件动态（供时间线与仪表盘） | 由 `add_case_log` / `delete_case_log` 维护 |
| `hearings` | 开庭/口审记录 | 供日历硬性日程消费 |
| `case_relations` | 案件自引用多对多 | `relation_type` 枚举 `same_patent / same_party / appeal_of / cross_reference`，唯一约束 `(source,target,type)` |
| `officials` + `case_officials` | 官方人员及其与案件关联 | `officials.role` 枚举 `法官/法官助理/书记员/法院` |
| `case_track_history` | 三轨状态变更历史（v8） | 见 02，`source` 枚举 `manual/auto/ai` |

---

## 三、命令接口

| 命令 | 签名要点 | 说明 |
|---|---|---|
| `list_cases` | filters → 案件列表 | 支持按轨道/状态/客户/关键字筛选 |
| `get_case` | case_id → 单案全量 | 含三轨状态、当事人、专利、期限里程碑 |
| `create_case` | case 数据 | 写 `cases` + 客户 `INSERT OR IGNORE` + 触发文件夹创建 + 飞书自动推送通知 |
| `update_case` | case_id + 变更字段 | 允许改日期里程碑（触发 02 状态推导） |
| `delete_case` | case_id | 级联删除关联表（外键 ON DELETE CASCADE） |
| `search_cases` | keyword | 走 FTS5（`cases_fts`） |
| `case_stats` | — | 案件分布统计（按轨道/状态） |
| `get_dashboard_stats` | — | 首页仪表盘聚合（含期限预警，调用 06） |
| `export_cases` | — | 案件导出 |
| `list_field_groups` | — | 动态字段分组（多列表单） |
| `get_case_unified_view` | case_id | 案件详情页一站式数据（项目书结构） |
| `recalculate_case_formulas` / `recalculate_all_formulas` | case_id / 全部 | 公式引擎重算（调用 `formula/`） |
| `update_case_status` | case_id + track + to_status + note | 手动切换三轨状态 + 自动写 `case_track_history` |

时间线命令：`get_case_timeline` / `add_case_log` / `delete_case_log`  
关系命令：`add_relation` / `get_relations` / `remove_relation` / `detect_relations`

---

## 四、关键流程

### 4.1 案件创建

```text
前端提交 → create_case
  1. INSERT cases（含新三轨字段默认值）
  2. clients INSERT OR IGNORE（按 client_name）
  3. 自动创建案件文件夹 → 回写 folder_path
     （失败不回滚案件主记录，仅记日志）
  4. 飞书自动推送管理器通知（数据同步，非提醒）
```

**约束**：文件夹创建失败不能回滚案件主记录；补偿策略需明确"数据库成功、文件夹失败"的后续处理（architecture.md §7.1）。

### 4.2 状态切换

```text
前端选择轨道+目标状态 → update_case_status
  1. 校验轨道与目标状态合法性（02 的枚举）
  2. 更新 cases.<track>_status
  3. 写 case_track_history（from/to/changed_at/source='manual'/note）
  4. 若日期字段变化，由 02 的日期驱动规则补充推导
```

### 4.3 聚合状态推导（触发器，保留两套口径）

- 兼容口径：`track` + `case_status`（由 `case_result` 推导，现有触发器不变）。
- 新口径：`case_route` + `civil_status` + `invalidation_status` + `admin_status`（02 的状态机）。

> 两套口径并存期间，`case_status` 只作聚合筛选；轨道内细分状态一律以 02 枚举为准。

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 02 状态机 | `update_case_status`、`cases.*_status`、`case_track_history` | 状态合法性校验与推导规则在 02 定义，01 只提供入口 |
| 06 期限 | `get_dashboard_stats` 消费期限预警；`case_deadlines` 挂 `case_id` | 期限计算不在 01 内 |
| 09 文件 | `create_case` 触发建文件夹；`folder_path` 回写 | 物理文件操作归 09 |
| 07 知识 | 知识可引用案件（`knowledge_relations`） | 引用建立由 07 提供命令 |
| 04 收件箱 | 归档到案件（`filed_to` / `linked_case_id`） | 收件箱处理不直接改 `cases` |

---

## 六、演进方向（目标态）

1. **客户聚合视图**：以 `clients.client_id` 稳定聚合"名下案件/相关任务/相关文书/历史合作"（设计哲学 §1.3、§4.2）。
2. **案件项目化**：`cases.sequential=1` + `next_action_id` + 统计缓存（`overdue_task_count` / `remaining_task_count`），案件详情"下一步行动"卡（设计哲学 §5.3、§6）。
3. **动态字段系统（长期）**：`cases` 从固定 SQL 列演进为"字段定义 + 单元格"（设计哲学 §9），`list_field_groups` 已有雏形。
4. **案件类型差异化评估**：计算/探索/成长三类案件采用不同评估指标（设计哲学 §2.5）。

---

## 七、验收标准

1. 建案时客户自动落 `clients` 且不产生重复。
2. 文件夹创建失败不影响案件主记录。
3. `case_status` 只能由触发器推导，不可被命令直接写。
4. `update_case_status` 每次切换必写 `case_track_history`。
5. 案件详情页数据来自 `get_case_unified_view` 单命令（不逐表前端拼装）。

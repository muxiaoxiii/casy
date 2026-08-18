# 模块 13 · AI 智伴

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 目标态设计（大量能力为设计草案）  
> **关联**: `00-README.md` / `casy-design-philosophy.md` §11 / `architecture.md` §3.3（目标态边界）

---

## 一、职责边界

### 1.1 做什么（现状）

- AI 后端抽象：Ollama / OpenAI / DeepSeek 兼容接口，模型可切换。
- 文档分类（`auto_classify`）、信息提取、写作辅助（`ai/mod.rs`）。
- 收件箱 AI 分类/提取（供 04 调用）。
- AI 每日调用预算控制（`AiConfig.daily_limit`，默认 50）。

### 1.2 目标态（设计草案，未落库）

- 推荐决策引擎（今日任务/优先级/时间预估/当日排程）。
- AI 输出确认机制（L1/L2/L3 + effective_policy）。
- 递归确认（L3 场景，ContextPolicy 界定核对范围）。
- 决策记录（`decisions`）、行为事件（`task_events`）、AI 审计（`ai_runs` + `ai_context_items`）。
- 自动报表（每日/每周/每月/项目，对内结构对外可读）。
- 数据蒸馏与外置记忆（原则七）。
- 双向开放（MCP / Skill，见 16 或设计哲学 §11.11）。

> 本节所有"目标态"能力均为设计草案，不得写成现状（architecture.md §3.3）。

---

## 二、核心机制设计（目标态）

### 2.1 双路径铁律

| | 去 AI 路径（确定性） | 含 AI 路径（智能） |
|---|---|---|
| 性质 | 本地实现 + 本地同步 | AI 参与判断/生成 |
| 例子 | 期限计算、状态推导、逾期判断、CRUD | 推荐决策、报表叙事、知识提取、文案生成 |
| 可靠性 | 确定性，直接信任 | 必须过"完整 → 可查 → 可信 → 递归确认"四关 |

### 2.2 确认等级 = effective_policy

```text
effective_policy = max(
  system_minimum_policy,  -- 系统安全下限（外部写 = L3），硬编码，不可降低
  scenario_policy,        -- 场景风险（推荐 L1 / 提取 L2 / 外部写 L3）
  model_policy,           -- 模型质量（本地小模型 +1 级）
  user_policy             -- 用户设置（可提高，不能降低）
)
```

### 2.3 推荐闭环（六步）

```text
① 抽取信息（本地 SQL，不依赖 LLM）
② 构造上下文（ContextPolicy 界定，有界）
③ AI 生成推荐（模型可切换，带理由）
④ 用户确认（L1/L2）
⑤ 递归确认（L3 专属，ContextPolicy 核对范围）
⑥ 落地 + 反馈（写回 + 学习信号）
```

### 2.4 数据支撑（事件三分离）

| 表 | 专职 | 保留期 |
|---|---|---|
| `task_events` | 行为学习（预估校准/活跃时段） | 90 天清理 |
| `ai_runs` + `ai_context_items` | AI 审计（模型可见即记录） | 长期保留 |
| `audit_events` | 领域事件（append-only） | 长期 |
| `decisions` | 决策记录（含依据） | 长期 |
| `daily_stats` / `smart_summaries` | 对内统计 / 对外叙事 | 按策略 |

---

## 三、命令接口

**现状**：
- AI 配置通过 `settings` 命令维护（AI 后端/每日预算/智能推荐开关）。
- 收件箱 AI 命令见 04（`ai_analyze_inbox_item` / `process_inbox_item`）。

**目标态（设计草案）**：
- 推荐：`get_today_recommendations` / `estimate_task_time` / `suggest_schedule`
- 确认：`confirm_proposal` / `get_confirm_policy` / `set_confirm_policy`
- 报表：`generate_daily_brief` / `generate_weekly_summary` / `generate_monthly_report`
- 蒸馏：`run_distillation` / `list_candidate_memories` / `confirm_memory`
- 决策：`record_decision` / `review_decisions`

---

## 四、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 04 收件箱 | 分类/提取调用 | 只写 `ai_*` 字段 |
| 01/03 | 推荐落地（today_index / estimated_minutes） | 写回走确认流程 |
| 07 知识 | 隐性关联洞察沉淀 | AI 洞察经确认进知识库 |
| 12 提醒 | 时机智能 | 只建议触发时机，不直接改规则 |
| 08 文书 | Copilot 草稿生成 | L2 草稿确认 |

---

## 五、演进方向（分阶段）

| 阶段 | 范围 |
|---|---|
| **M0** | AI 分类/提取/写作现状保留；纯规则推荐兜底（due_date + flagged + 案件阶段） |
| **P1** | 领域层单写入口 + 最小 Confirmer（effective_policy）+ ai_runs 审计 + fallback contract |
| **P2** | 分级提醒（R1-R4）+ 规则推荐 |
| **P3** | AI 推荐决策引擎 + 确认机制 + 决策记录 + L2 蒸馏 |
| **P4** | 蒸馏生命周期自动化 + 跨模型成长 + 双向开放 |

---

## 六、验收标准

1. 确定性计算（期限/状态/逾期）永不走 LLM。
2. AI 产出必须带依据（source_ref）且可回溯。
3. 任何到达模型的内容都能从 `ai_runs` + `ai_context_items` 重建（模型可见即记录）。
4. 设置项只能提高确认等级，不能突破 `system_minimum_policy`。
5. AI 不可用时所有功能有规则版降级路径。

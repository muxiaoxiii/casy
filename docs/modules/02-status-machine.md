# 模块 02 · 三轨状态机

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 已实现基础（v8 落库）+ 设计中  
> **说明**: 替代原有 3 档 case_status（已完结/进行中/未知），支持诉讼轨+无效轨+行政诉讼轨的三轨并行状态机  
> **关联**: `00-README.md` / `01-cases.md`（写入入口 `update_case_status`）/ `architecture.md` §5.3

---

## 一、设计目标

1. **双轨并行**：一个案件可同时处于诉讼轨和无效轨（专利侵权诉讼+对方提无效是常见场景）
2. **三轨联动**：无效决定后可转入行政诉讼轨，行政诉讼和民事诉讼可并行
3. **日期驱动**：里程碑日期自动推导状态，减少手动标记
4. **审级历程**：记录案件从一审到再审的完整轨迹
5. **向后兼容**：保留 `case_status` 作为聚合字段，现有筛选/统计不受影响

---

## 二、轨道路由

### 2.1 轨道类型

| 轨道 | 字段 | 含义 |
|------|------|------|
| 民事诉讼 | `civil_status` | 从立案到执行的完整诉讼流程 |
| 专利无效 | `invalidation_status` | 在专利复审委员会的无效程序 |
| 行政诉讼 | `admin_status` | 对无效决定不服提起的行政诉讼 |

### 2.2 轨道路由字段

新增 `case_route` 字段，标识案件涉及哪些轨道：

```sql
case_route TEXT NOT NULL DEFAULT '民事诉讼'
  CHECK(case_route IN (
    '民事诉讼',          -- 仅诉讼轨
    '专利无效',          -- 仅无效轨
    '行政诉讼',          -- 仅行政诉讼轨（少见，理论上存在）
    '民事诉讼+专利无效', -- 诉讼+无效并行（最常见）
    '专利无效+行政诉讼', -- 无效→行政诉讼
    '三轨并行'           -- 诉讼+无效+行政诉讼同时进行
  ))
```

### 2.3 轨道激活规则

| case_route | civil_status | invalidation_status | admin_status |
|---|---|---|---|
| 民事诉讼 | ✅ 必填 | — | — |
| 专利无效 | — | ✅ 必填 | — |
| 行政诉讼 | — | — | ✅ 必填 |
| 民事诉讼+专利无效 | ✅ 必填 | ✅ 必填 | — |
| 专利无效+行政诉讼 | — | ✅ 必填 | ✅ 必填 |
| 三轨并行 | ✅ 必填 | ✅ 必填 | ✅ 必填 |

---

## 三、各轨状态定义

### 3.1 民事诉讼轨 (civil_status)

```
接案 → 立案受理 → 待开庭 → 审理中 → 已调解
                                      → 待判决 → 判决已出 → 上诉期 → 二审中 → 二审判决已出
                                                                                    → 再审
                                      → 执行中 → 已结案
         ↘ 中止（可在任意阶段挂起，等待无效结果）
```

| 状态 | 中文 | 推导条件 | 说明 |
|------|------|----------|------|
| `intake` | 接案 | 默认初始状态 | 案件接受，尚未立案 |
| `filed` | 立案受理 | `filing_date` 或 `complaint_received_date` 存在 | 法院已受理 |
| `pre_hearing` | 待开庭 | `trial_date` 存在且无 `verdict_date` | 等待开庭 |
| `in_trial` | 审理中 | 手动标记（开庭后、判决前的审理阶段） | 无法纯日期推导 |
| `settled` | 已调解 | `case_result = '已调解'` | 调解结案 |
| `awaiting_verdict` | 待判决 | 手动标记（最后一次开庭后） | 等待判决 |
| `verdict_issued` | 判决已出 | `verdict_date` 存在 | 判决书已送达 |
| `appeal_period` | 上诉期 | `verdict_date` 存在 + 未超过上诉期 | 15 天（判决）/ 10 天（裁定），由 `verdict_type` 区分 |
| `second_instance` | 二审中 | `trial2_date` 存在 | 进入二审 |
| `second_verdict` | 二审判决已出 | 手动标记（二审判决后） | 二审结果 |
| `retrial` | 再审 | `trial3_date` 存在 | 进入再审 |
| `enforcement` | 执行中 | 手动标记或 `relief_deadline` 存在 | 申请强制执行 |
| `suspended` | 中止 | `stay_date` 存在 | 审理中止（通常因无效程序） |
| `closed` | 已结案 | `case_result` IN ('结案','胜诉','败诉','对方撤案','撤诉') | 案件终结 |

**共 14 个状态**

### 3.2 专利无效轨 (invalidation_status)

```
待提无效 → 无效已受理 → 待口审 → 口审完成 → 待无效决定 → 决定已出
                                                          ↘ 转入行政诉讼轨
```

| 状态 | 中文 | 推导条件 | 说明 |
|------|------|----------|------|
| `preparing` | 待提无效 | 默认初始状态 | 准备无效请求书 |
| `filed` | 无效已受理 | `petitioner_submit_date` 存在 | 已向复审委提交 |
| `pre_oral` | 待口审 | 手动标记 | 等待口头审理 |
| `oral_done` | 口审完成 | 手动标记 | 口审结束 |
| `awaiting_decision` | 待无效决定 | 手动标记 | 等待复审委决定 |
| `decision_issued` | 决定已出 | 手动标记（新增字段 `invalidation_decision_date`） | 无效决定已出 |

**共 6 个状态**

### 3.3 行政诉讼轨 (admin_status)

```
行政诉讼立案 → 行政诉讼待开庭 → 行政诉讼审理中 → 行政诉讼待判决
  → 行政诉讼判决已出 → 行政诉讼二审中 → 行政诉讼已结案
```

| 状态 | 中文 | 推导条件 | 说明 |
|------|------|----------|------|
| `filed` | 行政诉讼立案 | 手动标记（新增字段 `admin_filing_date`） | 对无效决定不服，向北京知识产权法院起诉 |
| `pre_hearing` | 行政诉讼待开庭 | 手动标记 | 等待开庭 |
| `in_trial` | 行政诉讼审理中 | 手动标记 | 审理阶段 |
| `awaiting_verdict` | 行政诉讼待判决 | 手动标记 | 等待判决 |
| `verdict_issued` | 行政诉讼判决已出 | 手动标记（新增字段 `admin_verdict_date`） | 一审判决 |
| `second_instance` | 行政诉讼二审中 | 手动标记 | 上诉至最高人民法院 |
| `closed` | 行政诉讼已结案 | 手动标记 | 行政诉讼终结 |

**共 7 个状态**

---

## 四、聚合状态 (case_status)

`case_status` 保留为聚合字段，从三轨状态自动推导，用于筛选/统计/看板显示。

### 4.1 推导规则

```
case_status = CASE
  -- 已结案优先（所有轨道都结案才标结案）
  WHEN civil_status = 'closed'
   AND (invalidation_status IS NULL OR invalidation_status = 'decision_issued')
   AND (admin_status IS NULL OR admin_status = 'closed')
  THEN '已完结'

  -- 有任意轨道在进行中
  WHEN civil_status IS NOT NULL AND civil_status != 'closed'
    OR invalidation_status IS NOT NULL AND invalidation_status NOT IN ('decision_issued')
    OR admin_status IS NOT NULL AND admin_status != 'closed'
  THEN '进行中'

  ELSE '未知'
END
```

### 4.2 向后兼容

- 现有代码中 `case_status = '已完结'` 的筛选逻辑不变
- 现有代码中 `case_status = '进行中'` 的筛选逻辑不变
- 新增支持按 `civil_status`、`invalidation_status`、`admin_status` 分别筛选

---

## 五、审级历程

### 5.1 历程表设计

新增 `case_track_history` 表，记录每次状态变迁：

```sql
CREATE TABLE IF NOT EXISTS case_track_history (
  id          TEXT PRIMARY KEY,
  case_id     TEXT NOT NULL,
  track       TEXT NOT NULL CHECK(track IN ('民事诉讼','专利无效','行政诉讼')),
  from_status TEXT,
  to_status   TEXT NOT NULL,
  changed_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  source      TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual','auto','ai')),
  note        TEXT,
  FOREIGN KEY (case_id) REFERENCES cases(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_track_history_case ON case_track_history(case_id, track);
```

### 5.2 自动记录时机

| 触发点 | 记录内容 |
|--------|----------|
| 里程碑日期变更（如设置 `verdict_date`） | source='auto', track='民事诉讼', to_status=推导出的状态 |
| 手动切换状态（UI 操作） | source='manual' |
| AI 推荐状态（收件箱处理） | source='ai' |
| 轨道切换（如从无效轨转行政诉讼轨） | 在原轨道记录关闭，在新轨道记录激活 |

### 5.3 历程查询

```sql
-- 获取某案件的完整审级历程
SELECT track, from_status, to_status, changed_at, source, note
FROM case_track_history
WHERE case_id = ?1
ORDER BY changed_at ASC;
```

---

## 六、新增/修改字段

### 6.1 cases 表新增字段

```sql
-- 轨道路由
ALTER TABLE cases ADD COLUMN case_route TEXT NOT NULL DEFAULT '民事诉讼';

-- 三轨状态
ALTER TABLE cases ADD COLUMN civil_status TEXT DEFAULT 'intake';
ALTER TABLE cases ADD COLUMN invalidation_status TEXT;
ALTER TABLE cases ADD COLUMN admin_status TEXT;

-- 无效程序新增日期字段
ALTER TABLE cases ADD COLUMN invalidation_decision_date TEXT;  -- 无效决定日期
ALTER TABLE cases ADD COLUMN invalidation_decision_type TEXT;  -- 无效决定结果（全部无效/部分无效/维持有效）

-- 行政诉讼新增日期字段
ALTER TABLE cases ADD COLUMN admin_filing_date TEXT;    -- 行政诉讼立案日期
ALTER TABLE cases ADD COLUMN admin_verdict_date TEXT;   -- 行政诉讼判决日期
ALTER TABLE cases ADD COLUMN admin_trial2_date TEXT;    -- 行政诉讼二审日期
```

### 6.2 修改现有触发器

将现有的 `trg_cases_status_insert` 和 `trg_cases_status_update` 替换为新的推导逻辑：

```sql
-- 删除旧触发器
DROP TRIGGER IF EXISTS trg_cases_status_insert;
DROP TRIGGER IF EXISTS trg_cases_status_update;

-- 新触发器：civil_status 自动推导
CREATE TRIGGER IF NOT EXISTS trg_civil_status_insert
AFTER INSERT ON cases
FOR EACH ROW
WHEN NEW.civil_status IS NULL AND NEW.case_route LIKE '%民事诉讼%'
BEGIN
  UPDATE cases SET civil_status = 'intake' WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_civil_status_from_dates
AFTER UPDATE OF filing_date, complaint_received_date, trial_date, verdict_date, case_result, stay_date ON cases
FOR EACH ROW
WHEN NEW.case_route LIKE '%民事诉讼%'
BEGIN
  UPDATE cases SET civil_status = CASE
    -- 已结案优先
    WHEN NEW.case_result IN ('结案','胜诉','败诉','对方撤案','撤诉') THEN 'closed'
    WHEN NEW.case_result = '已调解' THEN 'settled'
    -- 中止
    WHEN NEW.stay_date IS NOT NULL AND NEW.stay_date != '' THEN 'suspended'
    -- 日期驱动
    WHEN NEW.verdict_date IS NOT NULL AND NEW.verdict_date != '' THEN 'verdict_issued'
    WHEN NEW.trial_date IS NOT NULL AND NEW.trial_date != '' THEN 'pre_hearing'
    WHEN NEW.filing_date IS NOT NULL AND NEW.filing_date != ''
      OR NEW.complaint_received_date IS NOT NULL AND NEW.complaint_received_date != ''
    THEN 'filed'
    -- 默认
    ELSE 'intake'
  END WHERE id = NEW.id;

  -- 同步更新聚合状态
  UPDATE cases SET case_status = CASE
    WHEN (NEW.case_result IN ('结案','胜诉','败诉','对方撤案','撤诉'))
      AND (NEW.invalidation_status IS NULL OR NEW.invalidation_status = 'decision_issued')
      AND (NEW.admin_status IS NULL OR NEW.admin_status = 'closed')
    THEN '已完结'
    WHEN NEW.case_result IS NOT NULL AND NEW.case_result != ''
      AND NEW.case_result NOT IN ('结案','胜诉','败诉','对方撤案','撤诉')
    THEN '进行中'
    ELSE '未知'
  END WHERE id = NEW.id;
END;
```

---

## 七、看板列映射

### 7.1 民事诉讼看板（5 列）

| 列名 | 包含状态 |
|------|----------|
| 待办 | `intake`, `filed` |
| 庭前 | `pre_hearing`, `in_trial`, `awaiting_verdict` |
| 特殊 | `settled`, `appeal_period`, `suspended` |
| 上诉/再审 | `second_instance`, `second_verdict`, `retrial` |
| 结案 | `enforcement`, `closed` |

### 7.2 无效程序看板（3 列）

| 列名 | 包含状态 |
|------|----------|
| 待办 | `preparing`, `filed` |
| 审理 | `pre_oral`, `oral_done`, `awaiting_decision` |
| 已决 | `decision_issued` |

### 7.3 行政诉讼看板（3 列）

| 列名 | 包含状态 |
|------|----------|
| 一审 | `filed`, `pre_hearing`, `in_trial`, `awaiting_verdict`, `verdict_issued` |
| 二审 | `second_instance` |
| 结案 | `closed` |

---

## 八、迁移策略

### 8.1 数据迁移

```sql
-- Step 1: 新增字段（所有 ALTER TABLE）
-- Step 2: 根据现有 case_result 回填 civil_status
UPDATE cases SET civil_status = CASE
  WHEN case_result IN ('结案','胜诉','败诉','对方撤案','撤诉') THEN 'closed'
  WHEN case_result = '已调解' THEN 'settled'
  WHEN case_result IS NOT NULL AND case_result != '' THEN 'pre_hearing' -- 近似
  ELSE 'intake'
END;

-- Step 3: 根据 track 字段设置 case_route
UPDATE cases SET case_route = CASE
  WHEN track = 'patent_invalidation' THEN '专利无效'
  WHEN track = 'admin_litigation' THEN '行政诉讼'
  ELSE '民事诉讼'
END;

-- Step 4: 重建触发器
```

### 8.2 前端迁移

1. `KanbanView.vue` — 列定义改为从 `case_route` 动态渲染
2. `CaseFilterBar.vue` — 筛选条件增加 `civil_status`、`invalidation_status`、`admin_status`
3. `CaseInfoPanel.vue` — 详情页显示当前轨道状态 + 历程时间轴
4. `CaseDetailView.vue` — 新增状态切换按钮（手动标记）

### 8.3 后端迁移

1. `db/schema.rs` — 新增字段 + 新触发器 + 历程表
2. `commands/cases.rs` — `create_case`、`update_case` 支持新字段
3. `commands/cases.rs` — 新增 `update_case_status` 命令（手动切换状态 + 自动记录历程）

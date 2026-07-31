# Casy 实现规格 — 核心模块（案件/任务/时间线/日历/期限/关系/迁移）

## 1. 案件管理

### 1.1 Rust CRUD

```rust
// src-tauri/src/db/cases.rs

/// 列表查询（分页+多条件筛选）
pub fn list_cases(conn: &Connection, filter: &CaseFilter) -> Result<CaseListResult> {
    let mut sql = String::from("SELECT * FROM cases WHERE 1=1");
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(track) = &filter.track {
        sql.push_str(" AND track = ?");
        params.push(Box::new(track.clone()));
    }
    if let Some(client) = &filter.client {
        sql.push_str(" AND client_name = ?");
        params.push(Box::new(client.clone()));
    }
    if let Some(court) = &filter.court {
        sql.push_str(" AND court = ?");
        params.push(Box::new(court.clone()));
    }
    if let Some(status) = &filter.status {
        sql.push_str(" AND case_status = ?");
        params.push(Box::new(status.clone()));
    }
    if let Some(search) = &filter.search {
        if !search.is_empty() {
            sql.push_str(" AND (case_name LIKE ? OR case_no LIKE ? OR client_name LIKE ? OR opponent_name LIKE ?)");
            let like = format!("%{}%", search);
            for _ in 0..4 { params.push(Box::new(like.clone())); }
        }
    }
    if let Some(date_from) = &filter.date_from {
        sql.push_str(" AND filing_date >= ?");
        params.push(Box::new(date_from.clone()));
    }
    if let Some(date_to) = &filter.date_to {
        sql.push_str(" AND filing_date <= ?");
        params.push(Box::new(date_to.clone()));
    }

    // 排序
    let order = match filter.sort_by.as_deref().unwrap_or("filing_date") {
        "filing_date" => "filing_date DESC NULLS LAST",
        "case_name" => "case_name ASC",
        "client_name" => "client_name ASC",
        "updated_at" => "updated_at DESC",
        _ => "filing_date DESC NULLS LAST",
    };
    sql.push_str(&format!(" ORDER BY {}", order));

    // 分页
    let page = filter.page.unwrap_or(1).max(1);
    let per_page = filter.per_page.unwrap_or(50).min(200);
    sql.push_str(&format!(" LIMIT {} OFFSET {}", per_page, (page - 1) * per_page));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let cases = stmt.query_map(param_refs.as_slice(), |row| row_to_case(row))?
        .collect::<Result<Vec<_>, _>>()?;

    // 总数（去掉 LIMIT）
    let count_sql = sql.split("ORDER BY").next().unwrap_or(&sql)
        .replace("SELECT *", "SELECT COUNT(*)");
    let total: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |r| r.get(0))?;

    Ok(CaseListResult { items: cases, total, page, per_page })
}

pub struct CaseFilter {
    pub track: Option<String>,
    pub client: Option<String>,
    pub court: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub sort_by: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub struct CaseListResult {
    pub items: Vec<Case>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// 全文搜索
pub fn search_cases(conn: &Connection, query: &str) -> Result<Vec<Case>> {
    let mut stmt = conn.prepare(
        "SELECT c.* FROM cases_fts f JOIN cases c ON c.rowid = f.rowid
         WHERE cases_fts MATCH ?1 ORDER BY rank LIMIT 50"
    )?;
    let cases = stmt.query_map([query], |row| row_to_case(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cases)
}

/// 按客户分组统计
pub fn case_counts_by_client(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT client_name, COUNT(*) FROM cases GROUP BY client_name ORDER BY COUNT(*) DESC"
    )?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 按轨道分组统计
pub fn case_counts_by_track(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT track, COUNT(*) FROM cases GROUP BY track ORDER BY COUNT(*) DESC"
    )?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 活跃案件（未完结）
pub fn active_cases(conn: &Connection) -> Result<Vec<Case>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cases WHERE case_status IS NULL OR case_status != '已完结' ORDER BY filing_date DESC"
    )?;
    let cases = stmt.query_map([], |row| row_to_case(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cases)
}

fn row_to_case(row: &rusqlite::Row) -> rusqlite::Result<Case> {
    Ok(Case {
        id: row.get("id")?,
        track: row.get("track")?,
        case_name: row.get("case_name")?,
        case_no: row.get("case_no")?,
        // ... 所有字段
        ..Default::default()
    })
}
```

### 1.2 Tauri 命令

```rust
// src-tauri/src/commands/cases.rs

#[tauri::command]
pub async fn list_cases(filter: CaseFilter) -> Result<CaseListResult, String> {
    run_blocking(move || {
        let conn = open_db()?;
        db::cases::list_cases(&conn, &filter)
    }).await
}

#[tauri::command]
pub async fn get_case(id: String) -> Result<Case, String> {
    run_blocking(move || {
        let conn = open_db()?;
        db::cases::get_case(&conn, &id)
    }).await
}

#[tauri::command]
pub async fn create_case(data: Case) -> Result<Case, String> {
    run_blocking(move || {
        let conn = open_db()?;
        let mut case = data;
        case.id = new_id();
        case.created_at = Some(now_local());
        case.updated_at = Some(now_local());
        // 自动创建客户（如果不存在）
        if !case.client_name.is_empty() {
            db::clients::ensure_client(&conn, &case.client_name)?;
        }
        // 创建案件文件夹
        let folder = files::ensure_case_folder(&case)?;
        case.folder_path = Some(folder.display().to_string());
        db::cases::insert_case(&conn, &case)?;
        Ok(case)
    }).await
}

#[tauri::command]
pub async fn update_case(id: String, data: serde_json::Value) -> Result<Case, String> {
    run_blocking(move || {
        let conn = open_db()?;
        db::cases::update_case(&conn, &id, &data)
    }).await
}

#[tauri::command]
pub async fn delete_case(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = open_db()?;
        db::cases::delete_case(&conn, &id)
    }).await
}
```

### 1.3 Vue 组件结构

```
src/modules/cases/
├── views/
│   ├── CaseListView.vue        # 列表页（分组+筛选+排序+分页）
│   ├── CaseDetailView.vue      # 详情页（三栏布局）
│   └── components/
│       ├── CaseFilterBar.vue   # 筛选条件栏
│       ├── CaseTable.vue       # 表格组件
│       ├── CaseGroupPanel.vue  # 分组面板
│       ├── CaseInfoPanel.vue   # 左栏：基本信息
│       ├── CaseTimelinePanel.vue # 中栏：时间线
│       └── CaseRelatedPanel.vue  # 右栏：关联
├── composables/
│   ├── useCases.js             # Pinia store
│   └── useCaseForm.js          # 表单逻辑
```

### 1.4 Pinia Store

```javascript
// src/modules/cases/composables/useCases.js
export const useCasesStore = defineStore('cases', {
  state: () => ({
    cases: [],
    currentCase: null,
    loading: false,
    filter: { track: null, client: null, court: null, status: null, search: '', sortBy: 'filing_date', page: 1, perPage: 50 },
    total: 0,
    stats: { byTrack: [], byClient: [], byStatus: [] },
  }),
  actions: {
    async loadCases() { /* tauriCallSafe('list_cases', { filter: this.filter }) */ },
    async loadCase(id) { /* tauriCallSafe('get_case', { id }) */ },
    async createCase(data) { /* tauriCallSafe('create_case', { data }) */ },
    async updateCase(id, data) { /* tauriCallSafe('update_case', { id, data }) */ },
    async deleteCase(id) { /* tauriCallSafe('delete_case', { id }) */ },
    async searchCases(query) { /* tauriCallSafe('search_cases', { query }) */ },
    async loadStats() { /* tauriCallSafe('case_stats', {}) */ },
  },
})
```

### 1.5 列表页交互

```
┌─────────────────────────────────────────────────────────────────┐
│  📋 案件管理                                    [搜索] [筛选]   │
│                                                                  │
│  筛选栏：[全部轨道 ▼] [全部客户 ▼] [全部法院 ▼] [全部状态 ▼]   │
│         排序：[立案日期 ▼]  共 59 件                            │
│                                                                  │
│  分组：按案由 / 按客户 / 按审理机关 / 按审级 / 不分组           │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ ▶ 专利无效 (24件)                                         │  │
│  │   隆基244号无效    (2024)京73行初1号  隆基绿能  国知局     │  │
│  │   隆基46号无效     —                 隆基绿能  国知局     │  │
│  │   钛金专利无效     —                 钛金公司  国知局     │  │
│  │   ...                                                     │  │
│  ├──────────────────────────────────────────────────────────┤  │
│  │ ▶ 专利侵权 (11件)                                        │  │
│  │   ...                                                     │  │
│  ├──────────────────────────────────────────────────────────┤  │
│  │ ▶ 专利行政 (9件)                                         │  │
│  │   ...                                                     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  行颜色：🔴 5天内有期限  🟡 15天内有期限  ⬜ 已完结              │
│  [上一页] 1 2 3 [下一页]                                        │
└─────────────────────────────────────────────────────────────────┘
```

### 1.6 边界情况

| 场景 | 处理 |
|------|------|
| 案号为空 | 显示"无案号"，排序放最后 |
| 同名案件 | 允许同名，用内部卷号区分 |
| 删除有关联的案件 | 级联删除日志/庭审/任务，提示用户确认 |
| 飞书导入时字段值超长 | 截断到 SQLite TEXT 限制（无硬限制，但建议 <10KB） |
| 空搜索 | 返回全部（不执行 FTS 查询） |
| 分页超出范围 | 返回最后一页 |

---

## 2. 案件详情

### 2.1 三栏布局

```
┌─ 基本信息 (320px) ──────┬─ 时间线 (flex) ──────────┬─ 关联 (280px) ──────┐
│                         │                          │                     │
│ 案件信息                │ [添加事件 +]             │ ⏰ 期限             │
│ 案号: [可编辑]          │                          │ 补充意见: 2025-01-15│
│ 内部卷号: [可编辑]      │ 📅 2024-06-15 立案      │ 预估审限: 2025-06-15│
│ 案由: [下拉选择]        │                          │                     │
│                         │ 📤 2024-08-20 交文      │ 🔗 关联案件         │
│ 当事人                  │    提交无效宣告请求书    │ 244号侵权 (同专利)  │
│ 客户: [可编辑]          │    [展开] [编辑] [删除]  │ 46号无效 (同客户)   │
│ 我方: [下拉]            │                          │                     │
│ 对方: [可编辑]          │ 📥 2024-09-10 收文      │ 👤 联系人           │
│ 对方代理: [可编辑]      │    收到专利权人陈述意见  │ 张法官 (审判长)     │
│                         │                          │ 王书记员            │
│ 审理                    │ 📅 2024-10-15 口审      │                     │
│ 审理机关: [下拉]        │    国知局                │ [生成文书]          │
│ 审级: [下拉]            │    合议组: 张、李、王    │ [打开文件夹]        │
│ 办案人: [多选]          │                          │ [查看案卷]          │
│                         │ 📥 2024-11-01 收文      │                     │
│ 专利                    │    收到无效决定书        │                     │
│ 专利名称: [可编辑]      │                          │                     │
│ 专利申请号: [可编辑]    │ ✅ 2024-12-01 已完结    │                     │
│                         │                          │                     │
│ 状态: [进行中 badge]    │                          │                     │
│ 结果: [下拉]            │                          │                     │
│                         │                          │                     │
│ 备注: [textarea]        │                          │                     │
└─────────────────────────┘──────────────────────────┘─────────────────────┘
```

### 2.2 自动保存

```javascript
// src/modules/cases/composables/useCaseForm.js
export function useCaseForm(caseId) {
  const store = useCasesStore()
  const form = reactive({})
  let saveTimer = null

  // 加载案件数据
  async function load() {
    const result = await store.loadCase(caseId)
    if (result.ok) Object.assign(form, result.data)
  }

  // 防抖自动保存（2秒）
  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(async () => {
      await store.updateCase(caseId, { ...form })
      ElMessage.success('已自动保存')
    }, 2000)
  }

  // 监听表单变化
  watch(form, scheduleSave, { deep: true })

  return { form, load }
}
```

### 2.3 计算字段显示

```javascript
// 案件状态（实时计算，不存库）
function computeCaseStatus(caseData) {
  const closedResults = ['结案', '胜诉', '败诉', '对方撤案', '撤诉']
  if (closedResults.includes(caseData.caseResult)) return '已完结'
  if (caseData.caseResult) return '进行中'
  return '未知'
}

// 状态 badge 颜色
function statusBadgeType(status) {
  return { '已完结': 'info', '进行中': 'success', '未知': 'warning' }[status] || 'info'
}
```

---

## 3. 时间线

### 3.1 事件合并

```rust
// src-tauri/src/db/timeline.rs

pub struct TimelineEvent {
    pub id: String,
    pub source_table: String,   // "case_logs" / "hearings" / "tasks"
    pub source_id: String,
    pub event_date: String,
    pub event_type: String,     // submitted/received/record/task/hearing
    pub title: String,
    pub detail: Option<String>,
    pub files: Option<String>,
    pub icon: String,
    pub color: String,
}

pub fn case_timeline(conn: &Connection, case_id: &str) -> Result<Vec<TimelineEvent>> {
    let mut events = Vec::new();

    // 办案日志
    let mut stmt = conn.prepare(
        "SELECT id, event_date, event_type, event_summary, content, files_json
         FROM case_logs WHERE case_id = ?1"
    )?;
    for row in stmt.query_map([case_id], |r| {
        Ok(TimelineEvent {
            id: r.get(0)?,
            source_table: "case_logs".to_string(),
            source_id: r.get(0)?,
            event_date: r.get(1)?,
            event_type: r.get(2)?,
            title: r.get(3)?,
            detail: r.get(4)?,
            files: r.get(5)?,
            icon: match_event_icon(&r.get::<_, String>(2)?),
            color: match_event_color(&r.get::<_, String>(2)?),
        })
    })? { events.push(row?); }

    // 庭审
    let mut stmt = conn.prepare(
        "SELECT id, hearing_date, hearing_name, venue, judges, actual_status
         FROM hearings WHERE case_id = ?1"
    )?;
    for row in stmt.query_map([case_id], |r| {
        Ok(TimelineEvent {
            id: r.get(0)?,
            source_table: "hearings".to_string(),
            source_id: r.get(0)?,
            event_date: r.get(1)?,
            event_type: "hearing".to_string(),
            title: r.get::<_, Option<String>>(2)?.unwrap_or("开庭".into()),
            detail: r.get::<_, Option<String>>(3)?,
            files: None,
            icon: "📅".to_string(),
            color: "#3b82f6".to_string(),
        })
    })? { events.push(row?); }

    // 任务（仅关联案件的）
    let mut stmt = conn.prepare(
        "SELECT id, created_date, task_name, description, deadline, completed
         FROM tasks WHERE case_id = ?1"
    )?;
    for row in stmt.query_map([case_id], |r| {
        let completed: i32 = r.get(5)?;
        Ok(TimelineEvent {
            id: r.get(0)?,
            source_table: "tasks".to_string(),
            source_id: r.get(0)?,
            event_date: r.get(1)?,
            event_type: "task".to_string(),
            title: r.get(2)?,
            detail: r.get(3)?,
            files: None,
            icon: if completed == 1 { "✅" } else { "📌" }.to_string(),
            color: "#8b5cf6".to_string(),
        })
    })? { events.push(row?); }

    // 按日期排序
    events.sort_by(|a, b| b.event_date.cmp(&a.event_date));
    Ok(events)
}

fn match_event_icon(event_type: &str) -> String {
    match event_type {
        "submitted" => "📤",
        "received" => "📥",
        "record" => "📝",
        "task" => "📌",
        _ => "📄",
    }.to_string()
}

fn match_event_color(event_type: &str) -> String {
    match event_type {
        "submitted" => "#22c55e",
        "received" => "#3b82f6",
        "record" => "#6b7280",
        "task" => "#8b5cf6",
        _ => "#9ca3af",
    }.to_string()
}
```

### 3.2 时间线 UI

```
┌─ 时间线 ─────────────────────────────────────────────────┐
│ [添加事件 +]  [筛选: 全部 ▼]  排序: 最新优先 ▼           │
│                                                           │
│ ── 2024年12月 ──                                         │
│ ✅ 2024-12-01  收到无效决定书                            │
│    📥 收文 — 无效决定：宣告专利权全部无效                 │
│    [展开详情] [编辑] [删除]                               │
│                                                           │
│ ── 2024年11月 ──                                         │
│ 📥 2024-11-01  收到无效决定书                            │
│    📥 收文                                               │
│                                                           │
│ ── 2024年10月 ──                                         │
│ 📅 2024-10-15  无效口审                                  │
│    📅 国知局 · 合议组: 张审查员、李审查员、王审查员       │
│    [展开详情] [编辑] [删除]                               │
│                                                           │
│ ── 2024年9月 ──                                          │
│ 📥 2024-09-10  收到专利权人陈述意见                      │
│    📥 收文                                               │
│                                                           │
│ ── 2024年8月 ──                                          │
│ 📤 2024-08-20  提交无效宣告请求书                        │
│    📤 交文                                               │
│    [展开详情] [编辑] [删除]                               │
│                                                           │
│ ── 2024年6月 ──                                          │
│ 📅 2024-06-15  立案                                     │
│    📝 记录                                               │
└───────────────────────────────────────────────────────────┘
```

---

## 4. 任务管理

### 4.1 四象限

```javascript
// src/modules/tasks/composables/useTasks.js
export const useTasksStore = defineStore('tasks', {
  state: () => ({
    tasks: [],
    loading: false,
    filter: { completed: false, caseId: null },
  }),
  getters: {
    quadrants: (state) => ({
      urgentImportant: state.tasks.filter(t => t.priority === 'urgent_important' && !t.completed),
      important: state.tasks.filter(t => t.priority === 'important' && !t.completed),
      urgent: state.tasks.filter(t => t.priority === 'urgent' && !t.completed),
      normal: state.tasks.filter(t => t.priority === 'normal' && !t.completed),
    }),
    overdue: (state) => state.tasks.filter(t => !t.completed && t.deadline && t.deadline < today()),
  },
  actions: {
    async loadTasks() { /* tauriCallSafe('list_tasks', { filter: this.filter }) */ },
    async createTask(data) { /* tauriCallSafe('create_task', { data }) */ },
    async toggleComplete(id) { /* tauriCallSafe('toggle_task', { id }) */ },
    async deleteTask(id) { /* tauriCallSafe('delete_task', { id }) */ },
  },
})
```

### 4.2 四象限 UI

```
┌─────────────────────────────────────────────────────────────────┐
│  📋 任务管理                                    [添加任务 +]     │
│                                                                  │
│  重要紧急 (3)              │  重要不紧急 (6)                     │
│  ┌──────────────────────┐  │  ┌──────────────────────┐          │
│  │ □ 提交补充意见       │  │  │ □ 整理证据清单       │          │
│  │   截止: 2025-01-15   │  │  │   截止: 2025-03-01   │          │
│  │   隆基244号          │  │  │   钛金专利           │          │
│  │ □ 准备口审材料       │  │  │ □ 研究类案           │          │
│  │   截止: 2025-01-10   │  │  │   无截止日           │          │
│  └──────────────────────┘  │  └──────────────────────┘          │
│  ──────────────────────────┼─────────────────────────────────── │
│  紧急不重要 (1)            │  不重要不紧急 (2)                   │
│  ┌──────────────────────┐  │  ┌──────────────────────┐          │
│  │ □ 打印案卷材料       │  │  │ □ 归档已结案案件     │          │
│  │   截止: 2025-01-08   │  │  │   无截止日           │          │
│  └──────────────────────┘  │  └──────────────────────┘          │
│                                                                  │
│  已完成 (4)  [展开查看]                                          │
└─────────────────────────────────────────────────────────────────┘
```

### 4.3 从庭审自动生成任务

```rust
/// 当创建庭审记录时，自动生成准备任务
pub fn generate_hearing_prep_tasks(conn: &Connection, hearing: &Hearing) -> Result<()> {
    let tasks = vec![
        ("准备口审/开庭材料", hearing.hearing_date.clone(), "urgent_important"),
        ("联系当事人确认出庭", subtract_days(&hearing.hearing_date, 7), "important"),
        ("整理证据清单", subtract_days(&hearing.hearing_date, 14), "important"),
    ];

    for (name, deadline, priority) in tasks {
        if deadline > today() {
            insert_task(conn, &Task {
                id: new_id(),
                case_id: Some(hearing.case_id.clone()),
                task_name: name.to_string(),
                deadline: Some(deadline),
                priority: Some(priority.to_string()),
                ..Default::default()
            })?;
        }
    }
    Ok(())
}
```

---

## 5. 日历

### 5.1 数据源

```rust
/// 日历事件（合并庭审+期限+任务）
pub struct CalendarEvent {
    pub id: String,
    pub date: NaiveDate,
    pub title: String,
    pub event_type: String,   // hearing / deadline / task
    pub color: String,
    pub case_id: String,
    pub case_name: String,
}

pub fn calendar_events(conn: &Connection, year: i32, month: u32) -> Result<Vec<CalendarEvent>> {
    let start = format!("{:04}-{:02}-01", year, month);
    let end = format!("{:04}-{:02}-31", year, month);
    let mut events = Vec::new();

    // 庭审
    let mut stmt = conn.prepare(
        "SELECT h.id, h.hearing_date, h.hearing_name, c.id, c.case_name
         FROM hearings h JOIN cases c ON c.id = h.case_id
         WHERE h.hearing_date BETWEEN ?1 AND ?2"
    )?;
    for row in stmt.query_map([&start, &end], |r| {
        Ok(CalendarEvent {
            id: r.get(0)?,
            date: NaiveDate::parse_from_str(&r.get::<_, String>(1)?, "%Y-%m-%d").unwrap_or_default(),
            title: r.get::<_, Option<String>>(2)?.unwrap_or("开庭".into()),
            event_type: "hearing".to_string(),
            color: "#3b82f6".to_string(),
            case_id: r.get(3)?,
            case_name: r.get(4)?,
        })
    })? { events.push(row?); }

    // 期限预警（调用 deadline engine）
    let warnings = deadline_engine::generate_warnings(conn, &calendar)?;
    for w in warnings {
        if w.due_date.year() == year && w.due_date.month() == month {
            events.push(CalendarEvent {
                id: format!("deadline-{}", w.case_id),
                date: w.due_date,
                title: w.rule_name,
                event_type: "deadline".to_string(),
                color: match w.urgency {
                    Urgency::Red => "#ef4444",
                    Urgency::Yellow => "#eab308",
                    Urgency::Green => "#22c55e",
                }.to_string(),
                case_id: w.case_id,
                case_name: w.case_name,
            });
        }
    }

    // 任务
    let mut stmt = conn.prepare(
        "SELECT id, deadline, task_name, case_id FROM tasks
         WHERE deadline BETWEEN ?1 AND ?2 AND completed = 0"
    )?;
    for row in stmt.query_map([&start, &end], |r| {
        Ok(CalendarEvent {
            id: r.get(0)?,
            date: NaiveDate::parse_from_str(&r.get::<_, String>(1)?, "%Y-%m-%d").unwrap_or_default(),
            title: r.get(2)?,
            event_type: "task".to_string(),
            color: "#8b5cf6".to_string(),
            case_id: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            case_name: String::new(),
        })
    })? { events.push(row?); }

    Ok(events)
}
```

### 5.2 日历 UI

```
┌─────────────────────────────────────────────────────────────────┐
│  📅 2026年8月                                    [< 今天 >]     │
│                                                                  │
│  日    一    二    三    四    五    六                          │
│                              1    2                              │
│   3    4    5    6    7    8    9                               │
│             🔵隆基口审                                          │
│  10   11   12   13   14   15   16                               │
│       🔴威灵开庭  🔵钛金口审                                    │
│  17   18   19   20   21   22   23                               │
│             🟡隆基二审  📌提交材料                               │
│  24   25   26   27   28   29   30                               │
│                                   ⏰补充意见到期                 │
│  31                                                              │
│                                                                  │
│  图例：🔵无效口审 🔴法院开庭 🟡二审 📌任务 ⏰期限              │
│                                                                  │
│  ┌─ 点击 8月5日 ──────────────────────────────────────────┐    │
│  │ 📅 隆基244号无效 · 口审 · 国知局 9:00                   │    │
│  │    [跳转到案件]                                         │    │
│  └────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 6. 期限引擎

### 6.1 节假日数据

```rust
// src-tauri/src/formula/holidays.rs

/// 内置 2025-2026 年中国法定节假日
/// 数据来源：国务院办公厅关于节假日安排的通知
/// 注意：每年需更新，可在设置页导入新年份的 JSON
pub const HOLIDAYS_2025: &[&str] = &[
    "2025-01-01", "2025-01-28", "2025-01-29", "2025-01-30", "2025-01-31",
    "2025-02-01", "2025-02-02", "2025-02-03", "2025-02-04",
    "2025-04-04", "2025-04-05", "2025-04-06",
    "2025-05-01", "2025-05-02", "2025-05-03", "2025-05-04", "2025-05-05",
    "2025-05-31", "2025-06-01", "2025-06-02",
    "2025-10-01", "2025-10-02", "2025-10-03", "2025-10-04", "2025-10-05", "2025-10-06", "2025-10-07",
];

pub const WORKDAYS_2025: &[&str] = &[
    "2025-01-26", "2025-02-08",  // 春节调休
    "2025-04-27",                 // 劳动节调休
    "2025-09-28", "2025-10-11",  // 国庆调休
];

// ⚠️ 以下为估算日期，正式使用前必须根据国务院办公厅通知核实
// 数据来源：国务院办公厅关于20XX年部分节假日安排的通知
// 支持通过 JSON 文件更新（设置页导入），无需改代码
pub const HOLIDAYS_2026: &[&str] = &[
    "2026-01-01", "2026-01-02", "2026-01-03",                        // 元旦
    "2026-02-15", "2026-02-16", "2026-02-17", "2026-02-18",          // 春节（需核实）
    "2026-02-19", "2026-02-20", "2026-02-21", "2026-02-22", "2026-02-23",
    "2026-04-04", "2026-04-05", "2026-04-06",                        // 清明
    "2026-05-01", "2026-05-02", "2026-05-03", "2026-05-04",          // 劳动节
    "2026-05-05",
    "2026-05-31", "2026-06-01", "2026-06-02",                        // 端午（需核实）
    "2026-10-01", "2026-10-02", "2026-10-03", "2026-10-04",          // 国庆+中秋（需核实）
    "2026-10-05", "2026-10-06", "2026-10-07", "2026-10-08",
];

pub const WORKDAYS_2026: &[&str] = &[
    "2026-02-14", "2026-02-28",  // 春节调休（需核实）
    "2026-04-26",                 // 劳动节调休（需核实）
    "2026-09-27", "2026-10-10",  // 国庆调休（需核实）
];

/// 从 JSON 文件加载（允许用户更新）
pub fn load_holiday_json(path: &Path) -> Result<HolidayCalendar> {
    let data = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&data)?;
    let holidays: HashSet<NaiveDate> = json["holidays"].as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()))
        .collect();
    let workdays: HashSet<NaiveDate> = json["workdays"].as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()))
        .collect();
    Ok(HolidayCalendar { holidays, workdays })
}
```

### 6.2 add_workdays 与 add_months（修正版）

实现专利法实施细则第5条的三项规则：
1. **起算日不计入**：期限从起算日的次日开始计算
2. **休假日顺延**：期限届满日为法定休假日的，顺延到休假日后的第一个工作日
3. **月末处理**：按月计算期限时，若目标月无对应日，以月末为届满日

```rust
// src-tauri/src/formula/holidays.rs — HolidayCalendar impl

impl HolidayCalendar {
    /// 判断某日是否为工作日（排除周末和法定假日，含调休上班日）
    pub fn is_workday(&self, date: NaiveDate) -> bool {
        if self.workdays.contains(&date) { return true; }  // 调休上班日
        let wd = date.weekday();
        if wd == Weekday::Sat || wd == Weekday::Sun { return false; }  // 周末
        !self.holidays.contains(&date)  // 法定假日
    }

    /// 如果 date 是非工作日，顺延到之后的第一个工作日；否则原样返回
    fn extend_to_workday(&self, date: NaiveDate) -> NaiveDate {
        if self.is_workday(date) { date }
        else {
            let mut d = date + Duration::days(1);
            while !self.is_workday(d) { d = d + Duration::days(1); }
            d
        }
    }

    /// 专利法实施细则算法：按日历月计算
    /// - 起算日不计入（从次日起算）
    /// - 日历月加法，月末钳制
    /// - 届满日为休假日则顺延
    pub fn add_months_patent(&self, start: NaiveDate, months: u32) -> NaiveDate {
        let from = start + Duration::days(1); // 专利法实施细则第5条：起算日不计入
        let due = add_months_clamp(from, months);
        self.extend_to_workday(due)
    }

    /// 专利法实施细则算法：按自然日计算
    /// - 起算日不计入
    /// - 直接加天数
    /// - 届满日为休假日则顺延
    pub fn add_days_patent(&self, start: NaiveDate, days: i64) -> NaiveDate {
        let from = start + Duration::days(1); // 起算日不计入
        let due = from + Duration::days(days - 1); // -1 因为 from 已经是第1天
        self.extend_to_workday(due)
    }

    /// 诉讼法算法：按日历月计算
    /// - 起算日不计入（通说）
    /// - 日历月加法，月末钳制
    /// - 届满日为休假日则顺延
    pub fn add_months_civil(&self, start: NaiveDate, months: u32) -> NaiveDate {
        let from = start + Duration::days(1);
        let due = add_months_clamp(from, months);
        self.extend_to_workday(due)
    }

    /// 诉讼法算法：按自然日计算
    /// - 起算日不计入（通说）
    /// - 直接加天数
    /// - 届满日为休假日则顺延
    pub fn add_days_civil(&self, start: NaiveDate, days: i64) -> NaiveDate {
        let from = start + Duration::days(1);
        let due = from + Duration::days(days - 1);
        self.extend_to_workday(due)
    }
}

/// 月份加法，含月末钳制
fn add_months_clamp(date: NaiveDate, months: u32) -> NaiveDate {
    let total = date.month() + months;
    let year = date.year() + ((total - 1) / 12) as i32;
    let month = ((total - 1) % 12) + 1;
    let max_day = days_in_month(year, month);
    NaiveDate::from_ymd_opt(year, month, date.day().min(max_day)).unwrap_or(date)
}
```

### 6.3 期限计算完整实现

```rust
// src-tauri/src/formula/engine.rs

pub struct DeadlineEngine {
    rules: Vec<DeadlineRule>,
    calendar: HolidayCalendar,
}

impl DeadlineEngine {
    pub fn new(conn: &Connection) -> Result<Self> {
        let rules = db::deadline_rules::load_all(conn)?;
        let calendar = HolidayCalendar::builtin();
        Ok(Self { rules, calendar })
    }

    /// 计算单个案件的所有期限（法定自动计算 + 手动录入）
    pub fn evaluate_case(&self, conn: &Connection, case: &Case, today: NaiveDate) -> Vec<DeadlineResult> {
        let mut results = Vec::new();

        // 1. 法定期限自动计算
        for rule in &self.rules {
            if rule.track != case.track { continue; }
            if !rule.auto_calculate { continue; }

            // 检查适用程序
            if let Some(proc_types) = &rule.procedure_types {
                if let Ok(types) = serde_json::from_str::<Vec<String>>(proc_types) {
                    if let Some(case_proc) = &case.procedure_type {
                        if !types.contains(case_proc) { continue; }
                    }
                }
            }

            // 获取触发日期
            let Some(trigger) = get_case_date_field(case, &rule.trigger_field) else { continue };

            // 根据 calc_method 选择算法
            let due = match rule.calc_method.as_str() {
                "patent" => {
                    // 专利法实施细则算法：起算日不计入 + 日历月/自然日 + 休假日顺延
                    match rule.offset_unit.as_str() {
                        "calendar_month" => self.calendar.add_months_patent(trigger, rule.offset_value as u32),
                        "day" => self.calendar.add_days_patent(trigger, rule.offset_value),
                        _ => continue,
                    }
                },
                "civil" | _ => {
                    // 诉讼法算法：起算日不计入 + 自然日/日历月 + 休假日顺延
                    match rule.offset_unit.as_str() {
                        "calendar_month" => self.calendar.add_months_civil(trigger, rule.offset_value as u32),
                        "day" => self.calendar.add_days_civil(trigger, rule.offset_value),
                        _ => continue,
                    }
                },
            };

            let days_left = (due - today).num_days();
            results.push(DeadlineResult {
                rule_id: Some(rule.id.clone()),
                rule_name: rule.rule_name.clone(),
                due_date: due,
                days_left,
                urgency: classify_urgency(days_left),
                deadline_source: "statutory".to_string(),
                legal_basis: Some(rule.legal_basis.clone()),
                case_id: case.id.clone(),
                case_name: case.case_name.clone(),
            });
        }

        // 2. 手动录入的期限（法院指定 / 自行确定）
        if let Ok(manual) = db::case_deadlines::by_case(conn, &case.id) {
            for dl in manual {
                if dl.completed { continue; }
                let due = NaiveDate::parse_from_str(&dl.due_date, "%Y-%m-%d").ok();
                if let Some(due) = due {
                    let days_left = (due - today).num_days();
                    results.push(DeadlineResult {
                        rule_id: dl.rule_id,
                        rule_name: dl.deadline_name,
                        due_date: due,
                        days_left,
                        urgency: classify_urgency(days_left),
                        deadline_source: dl.deadline_source,
                        legal_basis: dl.legal_basis,
                        case_id: case.id.clone(),
                        case_name: case.case_name.clone(),
                    });
                }
            }
        }

        results.sort_by_key(|r| r.due_date);
        results
    }

    /// 计算所有活跃案件的期限预警
    pub fn generate_all_warnings(&self, conn: &Connection) -> Result<Vec<DeadlineResult>> {
        let cases = db::cases::active_cases(conn)?;
        let today = Local::now().naive_local().date();
        let mut all = Vec::new();
        for case in cases {
            all.extend(self.evaluate_case(conn, &case, today));
        }
        all.sort_by_key(|r| r.days_left);
        Ok(all)
    }
}

fn classify_urgency(days_left: i64) -> Urgency {
    if days_left <= 3 { Urgency::Red }
    else if days_left <= 14 { Urgency::Yellow }
    else { Urgency::Green }
}
```

**两种算法的区别**：

| 规则 | 诉讼法算法 (civil) | 专利法算法 (patent) |
|------|-------------------|-------------------|
| 起算日 | 不计入（次日起算） | 不计入（次日起算） |
| 自然日计算 | 直接加天数 | 直接加天数 |
| 月计算 | 日历月，月末钳制 | 日历月，月末钳制 |
| 休假日顺延 | 届满日是休假日则顺延 | 届满日是休假日则顺延 |
| 工作日计算 | 不使用（诉讼法无此概念） | 不使用（实施细则用日历月） |

**两者的核心区别**在于：专利法实施细则的期限全部按日历月计算（1个月），而诉讼法的期限既有自然日（15天答辩期）也有日历月（3/6个月审限）。实际算法逻辑相同，区别在于适用的规则和触发条件。

**手动录入期限 UI**：
```
┌─ 添加期限 ──────────────────────────────────────────────┐
│ 期限名称：[____________]                                 │
│ 来源：○ 法院指定  ● 自行确定                             │
│ 届满日期：[2025-01-15]                                   │
│ 法律依据：[____________]（可选）                         │
│ 法院指定依据：[____________]（可选，如通知书编号）        │
│ 备注：[____________]                                     │
│                                                          │
│ [保存]  [取消]                                           │
└──────────────────────────────────────────────────────────┘
```

fn get_case_date_field(case: &Case, field: &str) -> Option<NaiveDate> {
    let s = match field {
        "filing_date" => case.filing_date.as_ref(),
        "complaint_received_date" => case.complaint_received_date.as_ref(),
        "trial_date" => case.trial_date.as_ref(),
        "verdict_date" => case.verdict_date.as_ref(),
        "petitioner_submit_date" => case.petitioner_submit_date.as_ref(),
        "petitioner_received_date" => case.petitioner_received_date.as_ref(),
        "patentee_received_date" => case.patentee_received_date.as_ref(),
        "patentee_received_supp_date" => case.patentee_received_supp_date.as_ref(),
        _ => None,
    }?;
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn check_condition(case: &Case, cond: &serde_json::Value) -> bool {
    // 案由条件
    if let Some(cause) = cond.get("cause_action").and_then(|v| v.as_str()) {
        if case.cause_action.as_deref() != Some(cause) { return false; }
    }
    // 审理程序条件（普通/简易）
    if let Some(procedure) = cond.get("procedure_type").and_then(|v| v.as_str()) {
        if case.procedure_type.as_deref() != Some(procedure) { return false; }
    }
    // 判决/裁定类型条件
    if let Some(vtype) = cond.get("verdict_type").and_then(|v| v.as_str()) {
        if case.verdict_type.as_deref() != Some(vtype) { return false; }
    }
    // 必填字段条件
    if let Some(required_field) = cond.get("required_field").and_then(|v| v.as_str()) {
        let val = match required_field {
            "petitioner_submit_date" => case.petitioner_submit_date.as_ref(),
            "petitioner_received_date" => case.petitioner_received_date.as_ref(),
            "patentee_received_date" => case.patentee_received_date.as_ref(),
            "patentee_received_supp_date" => case.patentee_received_supp_date.as_ref(),
            _ => None,
        };
        if val.is_none() { return false; }
    }
    true
}
```

---

## 7. 案件关系网络

### 7.1 数据操作

```rust
// src-tauri/src/db/relations.rs

pub fn add_relation(conn: &Connection, source_id: &str, target_id: &str, rel_type: &str, label: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO case_relations (id, source_case_id, target_case_id, relation_type, label)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![new_id(), source_id, target_id, rel_type, label],
    )?;
    // 双向插入
    conn.execute(
        "INSERT OR IGNORE INTO case_relations (id, source_case_id, target_case_id, relation_type, label)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![new_id(), target_id, source_id, rel_type, label],
    )?;
    Ok(())
}

pub fn case_relations(conn: &Connection, case_id: &str) -> Result<Vec<CaseRelation>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.relation_type, r.label,
                c.id, c.case_name, c.case_no, c.client_name, c.track, c.case_status
         FROM case_relations r JOIN cases c ON c.id = r.target_case_id
         WHERE r.source_case_id = ?1"
    )?;
    let relations = stmt.query_map([case_id], |r| {
        Ok(CaseRelation {
            id: r.get(0)?,
            relation_type: r.get(1)?,
            label: r.get(2)?,
            target_case: CaseSummary {
                id: r.get(3)?,
                case_name: r.get(4)?,
                case_no: r.get(5)?,
                client_name: r.get(6)?,
                track: r.get(7)?,
                case_status: r.get(8)?,
            },
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(relations)
}

pub fn remove_relation(conn: &Connection, relation_id: &str) -> Result<()> {
    conn.execute("DELETE FROM case_relations WHERE id = ?1 OR id IN (
        SELECT r2.id FROM case_relations r1 JOIN case_relations r2
        ON r1.source_case_id = r2.target_case_id AND r1.target_case_id = r2.source_case_id
        WHERE r1.id = ?1
    )", [relation_id])?;
    Ok(())
}
```

### 7.2 关系 UI

```
┌─ 关联案件 ──────────────────────────────────────────────┐
│                                                          │
│  同一专利 (244号)                                        │
│  ├ 244号无效   国知局   已完结  [跳转]                   │
│  └ 244号侵权   北京知产  进行中 [跳转]                   │
│                                                          │
│  同一客户 (隆基绿能)                                     │
│  ├ 46号无效   国知局   进行中 [跳转]                     │
│  ├ 7号分案    国知局   已完结  [跳转]                    │
│  └ 8号无效    国知局   进行中 [跳转]                     │
│                                                          │
│  上诉关系                                                │
│  └ 一审判决后行政上诉  最高法  [跳转]                    │
│                                                          │
│  [添加关联]  [自动检测]                                  │
└──────────────────────────────────────────────────────────┘
```

---

## 8. 飞书数据迁移

### 8.1 导入脚本

```rust
// src-tauri/src/migrate/feishu.rs

use serde_json::Value;

pub fn import_feishu_dump(conn: &Connection, json_path: &Path) -> Result<ImportReport> {
    let data = std::fs::read_to_string(json_path)?;
    let dump: Value = serde_json::from_str(&data)?;

    let mut report = ImportReport::default();

    // 导入案件
    if let Some(records) = dump.pointer("/tables/cases/records").and_then(|v| v.as_array()) {
        for record in records {
            match import_case(conn, record) {
                Ok(_) => report.cases += 1,
                Err(e) => report.errors.push(format!("案件导入失败: {}", e)),
            }
        }
    }

    // 导入日志
    if let Some(records) = dump.pointer("/tables/case_logs/records").and_then(|v| v.as_array()) {
        for record in records {
            match import_log(conn, record) {
                Ok(_) => report.logs += 1,
                Err(e) => report.errors.push(format!("日志导入失败: {}", e)),
            }
        }
    }

    // 导入庭审、任务、官方人员...
    // ...

    Ok(report)
}

fn import_case(conn: &Connection, record: &Value) -> Result<()> {
    let feishu_id = record["record_id"].as_str().unwrap_or_default();
    let fields = &record["fields"];

    let case = Case {
        id: feishu_id.to_string(), // 直接用飞书 record_id
        case_name: fields["案件信息"].as_str().unwrap_or("").to_string(),
        case_no: fields["案号"].as_str().map(|s| s.to_string()),
        cause_action: extract_single_select(&fields["案由"]),
        client_name: fields["客户名称"].as_str().unwrap_or("").to_string(),
        opponent_name: fields["对方名称"].as_str().unwrap_or("").to_string(),
        court: extract_single_select(&fields["审理机关"]),
        case_level: extract_single_select(&fields["审级"]),
        case_progress: extract_single_select(&fields["案件进展"]),
        case_result: extract_single_select(&fields["案件结果"]),
        patent_name: fields["专利名称"].as_str().map(|s| s.to_string()),
        patent_app_no: fields["专利申请号"].as_str().map(|s| s.to_string()),
        filing_date: extract_datetime(&fields["立案"]),
        trial_date: extract_datetime(&fields["开庭|口审"]),
        verdict_date: extract_datetime(&fields["收到判决/裁定/决定时间"]),
        notes: fields["备注"].as_str().map(|s| s.to_string()),
        // ... 更多字段
        ..Default::default()
    };

    // 幂等插入
    conn.execute(
        "INSERT OR REPLACE INTO cases (...) VALUES (...)",
        rusqlite::params![case.id, case.case_name, /* ... */],
    )?;

    Ok(())
}

fn extract_single_select(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(arr) => arr.first().and_then(|v| v.as_str()).map(|s| s.to_string()),
        Value::Object(obj) => obj.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
        _ => None,
    }
}

fn extract_datetime(value: &Value) -> Option<String> {
    match value {
        Value::Number(n) => {
            // 飞书时间戳是毫秒
            let ms = n.as_i64()?;
            let dt = chrono::NaiveDateTime::from_timestamp_millis(ms)?;
            Some(dt.format("%Y-%m-%d").to_string())
        }
        Value::String(s) => Some(s.to_string()),
        _ => None,
    }
}
```

### 8.2 迁移 UI

```
┌─────────────────────────────────────────────────────────────────┐
│  📥 导入飞书数据                                                │
│                                                                  │
│  选择文件：[选择 feishu-full-dump.json]                         │
│                                                                  │
│  ┌─ 数据预览 ─────────────────────────────────────────────────┐ │
│  │ 案件: 59 条   日志: 67 条   庭审: 55 条                    │ │
│  │ 任务: 19 条   联系人: 9 条                                 │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  导入模式：○ 首次导入（清空现有数据）                           │
│           ● 增量导入（跳过已存在的记录）                        │
│                                                                  │
│  [开始导入]                                                      │
│                                                                  │
│  ┌─ 导入结果 ─────────────────────────────────────────────────┐ │
│  │ ✅ 案件: 59 条成功                                         │ │
│  │ ✅ 日志: 67 条成功                                         │ │
│  │ ✅ 庭审: 55 条成功                                         │ │
│  │ ✅ 任务: 19 条成功                                         │ │
│  │ ⚠️ 联系人: 2 条跳过（已存在）                              │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘

---

## 9. 空状态设计

### 9.1 案件列表（0 个案件）
```
┌─────────────────────────────────────────────────────────────────┐
│  📋 案件管理                                                    │
│                                                                  │
│         📂                                                      │
│     还没有案件                                                   │
│                                                                  │
│     [创建第一个案件]  [从飞书导入]                                │
│                                                                  │
│     提示：你也可以拖入飞书导出的 JSON 文件直接导入                │
└─────────────────────────────────────────────────────────────────┘
```

### 9.2 收件箱（0 个待处理）
```
┌─────────────────────────────────────────────────────────────────┐
│  📥 收件箱                           0 待处理 | 0 已归档         │
│                                                                  │
│         📬                                                      │
│     收件箱是空的                                                 │
│                                                                  │
│     拖入文件、发送邮件到专用地址、或手动添加笔记                  │
│                                                                  │
│     [📎 添加文件]  [📝 新建笔记]  [📧 导入邮件]                 │
└─────────────────────────────────────────────────────────────────┘
```

### 9.3 日历（当月无事件）
```
┌─────────────────────────────────────────────────────────────────┐
│  📅 2026年8月                                    [< 今天 >]     │
│                                                                  │
│  日  一  二  三  四  五  六                                     │
│                        1   2                                     │
│   3   4   5   6   7   8   9                                     │
│  10  11  12  13  14  15  16                                     │
│  17  18  19  20  21  22  23                                     │
│  24  25  26  27  28  29  30                                     │
│  31                                                              │
│                                                                  │
│  本月没有待办事件                                                │
└─────────────────────────────────────────────────────────────────┘
```

### 9.4 案件详情（无日志/庭审/任务）
```
┌─ 时间线 ─────────────────────────────────────────────────┐
│                                                           │
│     📝                                                    │
│  还没有事件记录                                           │
│                                                           │
│  [添加第一条日志]  [添加庭审]  [添加任务]                  │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

---

## 10. 错误与加载状态

### 10.1 全局加载状态

```vue
<!-- 通用加载组件 -->
<template>
  <div v-if="loading" class="loading-state">
    <el-icon class="is-loading"><Loading /></el-icon>
    <span>{{ message || '加载中...' }}</span>
  </div>
  <div v-else-if="error" class="error-state">
    <el-icon><WarningFilled /></el-icon>
    <span>{{ error }}</span>
    <el-button size="small" @click="$emit('retry')">重试</el-button>
  </div>
  <slot v-else />
</template>
```

### 10.2 错误 Toast

```javascript
// tauriCallSafe 已自动处理错误，前端只需：
const result = await tauriCallSafe('some_command', { ... })
if (!result.ok) {
  ElMessage.error(result.error || '操作失败')
  return
}
```

### 10.3 删除确认（带撤销提示）

```javascript
async function deleteCase(id) {
  try {
    await ElMessageBox.confirm(
      '删除案件将同时删除关联的日志、庭审、任务和文件。此操作不可撤销。',
      '确认删除',
      { confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning' }
    )
    const result = await tauriCallSafe('delete_case', { id })
    if (result.ok) {
      ElMessage.success('案件已删除')
      await loadCases()
    }
  } catch {
    // 用户取消
  }
}
```

### 10.4 空结果处理

```javascript
// 列表空状态
<el-empty v-if="!cases.length && !loading" description="还没有案件">
  <el-button type="primary" @click="createCase">创建第一个案件</el-button>
  <el-button @click="importFromFeishu">从飞书导入</el-button>
</el-empty>

// 搜索无结果
<el-empty v-if="!cases.length && filter.search" description="没有找到匹配的案件">
  <el-button @click="filter.search = ''">清除搜索</el-button>
</el-empty>
```

---

## 11. 键盘快捷键

| 快捷键 | 操作 |
|--------|------|
| `Cmd/Ctrl + N` | 新建案件 |
| `Cmd/Ctrl + K` | 全局搜索 |
| `Cmd/Ctrl + S` | 保存当前编辑 |
| `Cmd/Ctrl + Z` | 撤销 |
| `Esc` | 关闭弹窗/取消编辑 |
| `→` / `←` | 日历翻月 |
| `Enter` | 确认弹窗 |

```javascript
// src/core/keyboard.js
export function registerGlobalShortcuts(router) {
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault()
      // 聚焦搜索框
      document.querySelector('.global-search')?.focus()
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'n') {
      e.preventDefault()
      router.push({ name: 'cases', query: { action: 'create' } })
    }
  })
}
```

---

## 12. 日历配置

```javascript
// 日历周起始日（中国用户默认周一）
const calendarWeekStart = ref(1)  // 0=周日, 1=周一

// 在日历组件中
const weekDays = calendarWeekStart.value === 1
  ? ['一', '二', '三', '四', '五', '六', '日']
  : ['日', '一', '二', '三', '四', '五', '六']
```

---

## 13. 响应式断点

```css
/* 桌面 */
@media (min-width: 1200px) {
  .case-detail { grid-template-columns: 320px 1fr 280px; }
}

/* 平板 (iPad) */
@media (min-width: 768px) and (max-width: 1199px) {
  .case-detail { grid-template-columns: 1fr; }
  .case-detail .related-panel { display: none; }  /* 隐藏右栏，用底部抽屉替代 */
}

/* 手机 */
@media (max-width: 767px) {
  .case-detail { grid-template-columns: 1fr; }
  .sidebar { display: none; }  /* 用底部导航替代侧边栏 */
}
```
```

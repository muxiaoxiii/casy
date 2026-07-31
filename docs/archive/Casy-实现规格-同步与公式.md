# Casy 实现规格 — 同步引擎 + 公式引擎

## 1. WebDAV 同步

### 1.1 依赖

```toml
reqwest = { version = "0.12", features = ["blocking"] }
```

### 1.2 WebDavClient

```rust
pub struct WebDavClient {
    base_url: String,        // "https://dav.jianguoyun.com/dav/"
    username: String,
    password: String,
    client: reqwest::blocking::Client,
}

impl WebDavClient {
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self>;

    /// PUT 上传文件，返回 ETag
    pub fn put(&self, remote_path: &str, data: &[u8]) -> Result<String>;

    /// GET 下载文件，返回 (data, etag)
    pub fn get(&self, remote_path: &str) -> Result<(Vec<u8>, String)>;

    /// HEAD 检查文件是否存在，返回 Option<etag>
    pub fn head(&self, remote_path: &str) -> Result<Option<String>>;

    /// MKCOL 创建目录
    pub fn mkcol(&self, remote_path: &str) -> Result<()>;

    /// DELETE 删除文件
    pub fn delete(&self, remote_path: &str) -> Result<()>;
}
```

### 1.3 实现要点

- 使用 `reqwest::blocking::Client` 发送 HTTP 请求
- PROPFIND/PUT/GET/HEAD/MKCOL/DELETE 方法
- Basic Auth: `reqwest::header::AUTHORIZATION` + base64 编码
- ETag 从响应头 `ETag` 获取
- 错误处理: 401=认证失败, 404=文件不存在, 409=冲突, 507=空间不足

### 1.4 SQLite 安全拷贝

```rust
/// 在 WAL 模式下安全拷贝数据库文件
pub fn safe_copy_db(conn: &Connection, dest: &Path) -> Result<()> {
    // 1. 检查点 WAL
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    // 2. 拷贝文件
    std::fs::copy(db_path, dest)?;
    Ok(())
}
```

### 1.5 SyncCoordinator

```rust
pub struct SyncCoordinator {
    webdav: WebDavClient,
    db_path: PathBuf,
    state: Arc<SyncState>,
    debounce_timer: Option<Instant>,
}

pub struct SyncState {
    pub last_etag: Mutex<Option<String>>,
    pub is_syncing: AtomicBool,
    pub last_sync_at: Mutex<Option<String>>,
    pub pending_pushes: AtomicU32,
}

impl SyncCoordinator {
    /// 启动时同步
    pub fn startup_sync(&mut self) -> Result<SyncResult>;

    /// 本地修改后调用（5秒防抖）
    pub fn schedule_push(&mut self);

    /// 手动触发
    pub fn manual_sync(&mut self) -> Result<SyncResult>;

    /// 检查远程版本
    fn check_remote(&self) -> Result<Option<String>>;

    /// PUSH 本地到远程
    fn push(&self) -> Result<SyncResult>;

    /// PULL 远程到本地
    fn pull(&self) -> Result<SyncResult>;
}

pub struct SyncResult {
    pub direction: String,  // "push" / "pull" / "none"
    pub success: bool,
    pub message: String,
}
```

### 1.6 同步协议

**核心原则**：不做自动推送，仅在手动同步和应用关闭时上传。

**设备版本号**：每个设备维护一个递增整数 `device_version`，每次本地写操作 +1。存入 SQLite `PRAGMA user_version`。

**ETag**：WebDAV 服务器返回的文件标识。下载时保存，上传时用 `If-Match` 检测冲突。

```
启动同步:
1. 读本地 device_version 和上次同步时的 remote_etag
2. HEAD 远程 → 获取 current_etag
3. 如果 current_etag == saved_etag → 远程未变
   a. 本地有修改 (device_version > last_sync_version) → PUSH
   b. 本地无修改 → 无需同步
4. 如果 current_etag != saved_etag → 远程已变
   a. 本地无修改 → PULL（下载替换本地）
   b. 本地有修改 → 冲突 → 弹窗让用户选择:
      - 保留本地（覆盖远程）
      - 保留远程（覆盖本地）
      - 另存为（本地数据导出为备份文件）

PUSH 流程:
1. VACUUM INTO → 生成临时文件 backup.db
2. PUT backup.db → 临时路径 /casy.db.uploading
3. MOVE /casy.db.uploading → /casy.db（原子操作）
4. 成功 → 保存新 ETag，更新 last_sync_version = device_version
5. 失败 → 删除临时文件，保留本地数据

PULL 流程:
1. GET /casy.db → 临时文件 download.db
2. 校验文件完整性（大小 + 可选 SHA-256）
3. 替换本地 casy.db
4. 重新打开数据库连接
5. 成功 → 保存新 ETag，更新 last_sync_version = device_version

**不做自动推送的理由**:
- 避免频繁 VACUUM INTO 的性能开销
- 避免编辑过程中频繁上传导致的数据覆盖风险
- 用户明确控制同步时机，减少冲突概率
```

### 1.7 案件附件同步

**v1 不同步附件**。案件文件夹存储在本地路径，数据库记录文件元数据。

**跨设备问题**：设备 A 创建的案件文件夹路径在设备 B 上不存在。解决方案：
- 文件路径使用相对于 `~/Documents/Casy/cases/` 的相对路径存储
- 每个设备的绝对路径由 `case_folder_base()` 动态计算
- 文件不存在时显示"文件缺失"提示，不报错

**v2 考虑**：可选的附件同步（WebDAV 上传案件文件夹）

---

## 2. 飞书同步

### 2.1 API 端点

```
Base: https://open.feishu.cn/open-apis/bitable/v1

获取 Token:  POST /auth/v3/tenant_access_token/internal
获取记录:    GET  /apps/{app_token}/tables/{table_id}/records?page_size=200
创建记录:    POST /apps/{app_token}/tables/{table_id}/records
更新记录:    PUT  /apps/{app_token}/tables/{table_id}/records/{record_id}
获取字段:    GET  /apps/{app_token}/tables/{table_id}/fields
```

### 2.2 限流与重试

飞书 API 限流按 API、应用、租户和套餐分别计算，不是统一的 100 次/秒。多维表格还建议单表同时只执行一次写操作。

```rust
pub struct RateLimiter {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
    /// 429 响应后的恢复时间（从 Retry-After 头读取）
    retry_after: Option<Instant>,
}

impl RateLimiter {
    pub fn new(max_per_second: f64) -> Self;

    /// 等待直到有可用 token
    pub async fn acquire(&mut self);

    /// 尝试获取，不等待
    pub fn try_acquire(&mut self) -> bool;

    /// 处理 429 响应
    pub fn handle_429(&mut self, retry_after_secs: u64) {
        self.retry_after = Some(Instant::now() + Duration::from_secs(retry_after_secs));
    }

    /// 检查是否在冷却期
    pub fn is_cooling(&self) -> bool {
        self.retry_after.map_or(false, |t| Instant::now() < t)
    }
}
```

**调用模式**：
```rust
async fn call_feishu_api(limiter: &mut RateLimiter, request: Request) -> Result<Response> {
    loop {
        if limiter.is_cooling() {
            let wait = limiter.retry_after.unwrap() - Instant::now();
            tokio::time::sleep(wait).await;
        }
        limiter.acquire().await;
        let response = client.execute(request).await?;
        if response.status() == 429 {
            let retry_after = response.headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            limiter.handle_429(retry_after);
            continue;
        }
        return Ok(response);
    }
}
```
```

### 2.3 字段映射

```rust
/// Casy 字段名 → 飞书字段名
const FIELD_MAP: &[(&str, &str)] = &[
    ("case_name", "案件信息"),
    ("case_no", "案号"),
    ("cause_action", "案由"),
    ("client_name", "客户名称"),
    ("opponent_name", "对方名称"),
    ("case_progress", "案件进展"),
    ("case_level", "审级"),
    ("case_result", "案件结果"),
    ("court", "审理机关"),
    ("filing_date", "立案"),
    ("trial_date", "开庭|口审"),
    ("verdict_date", "收到判决/裁定/决定时间"),
    // ... 完整映射
];

/// 飞书字段值类型转换
fn feishu_value_to_local(field_type: &str, value: &serde_json::Value) -> serde_json::Value;
fn local_value_to_feishu(field_type: &str, value: &serde_json::Value) -> serde_json::Value;
```

### 2.4 PULL 流程

```
1. GET /records?page_size=200&page_token=...
2. 对每条 record:
   a. 查 sync_map: feishu_record_id → local_id
   b. 不存在 → INSERT INTO cases + INSERT INTO sync_map
   c. 存在 → 比较 feishu_updated vs local_updated
      - 飞书更新 → UPDATE local
      - 本地更新 → 跳过
      - 冲突 → 标记
3. 翻页直到无更多记录
4. 返回 SyncReport
```

### 2.5 PUSH 流程

```
1. SELECT * FROM sync_map WHERE sync_status='local_newer'
2. 对每条:
   a. SELECT * FROM {local_table} WHERE id={local_id}
   b. 转换字段为飞书格式
   c. POST/PUT 飞书 API
   d. 成功 → UPDATE sync_map SET sync_status='synced'
   e. 失败 → attempts++, 超3次 → 'push_failed'
```

---

## 3. 公式引擎

### 3.1 中国法定节假日

```rust
pub struct HolidayCalendar {
    holidays: HashSet<NaiveDate>,   // 法定假日
    workdays: HashSet<NaiveDate>,   // 调休工作日
}

impl HolidayCalendar {
    /// 从 JSON 文件加载（每年更新）
    pub fn from_json(path: &Path) -> Result<Self>;

    /// 判断是否为工作日
    pub fn is_workday(&self, date: NaiveDate) -> bool {
        if date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun {
            self.workdays.contains(&date)
        } else {
            !self.holidays.contains(&date)
        }
    }

    /// N 个工作日后
    pub fn add_workdays(&self, start: NaiveDate, n: i64) -> NaiveDate {
        let mut current = start;
        let mut remaining = n;
        while remaining > 0 {
            current += Duration::days(1);
            if self.is_workday(current) {
                remaining -= 1;
            }
        }
        current
    }
}
```

### 3.2 期限计算

```rust
pub struct DeadlineEngine {
    rules: Vec<DeadlineRule>,
    calendar: HolidayCalendar,
}

pub struct DeadlineResult {
    pub rule_name: String,
    pub due_date: NaiveDate,
    pub days_left: i64,
    pub urgency: Urgency,
}

pub enum Urgency { Red, Yellow, Green }

impl DeadlineEngine {
    pub fn evaluate(&self, case: &Case, today: NaiveDate) -> Vec<DeadlineResult> {
        self.rules.iter()
            .filter(|r| r.track == case.track || r.track == "all")
            .filter_map(|rule| {
                let trigger = case.get_date_field(&rule.trigger_field)?;
                let due = match rule.offset_unit.as_str() {
                    "day" => trigger + Duration::days(rule.offset_days),
                    "workday" => self.calendar.add_workdays(trigger, rule.offset_days),
                    "month" => add_months(trigger, rule.offset_days as u32),
                    _ => return None,
                };
                let days_left = (due - today).num_days();
                Some(DeadlineResult {
                    rule_name: rule.rule_name.clone(),
                    due_date: due,
                    days_left,
                    urgency: if days_left <= 3 { Urgency::Red }
                             else if days_left <= 14 { Urgency::Yellow }
                             else { Urgency::Green },
                })
            })
            .collect()
    }
}

fn add_months(date: NaiveDate, months: u32) -> NaiveDate {
    let total_month = date.month() + months;
    let year = date.year() + ((total_month - 1) / 12) as i32;
    let month = ((total_month - 1) % 12) + 1;
    let day = date.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or(date)
}
```

### 3.3 案件状态计算

```rust
pub fn compute_case_status(case: &Case) -> Option<String> {
    if let Some(result) = &case.case_result {
        if ["结案", "胜诉", "败诉", "对方撤案", "撤诉"].contains(&result.as_str()) {
            return Some("已完结".to_string());
        }
    }
    if case.case_result.is_some() {
        Some("进行中".to_string())
    } else {
        Some("未知".to_string())
    }
}
```

### 3.4 节假日数据源

```
数据来源: 国务院办公厅关于节假日安排的通知
格式: JSON
{
  "year": 2026,
  "holidays": ["2026-01-01", "2026-01-29", ...],
  "workdays": ["2026-01-26", "2026-02-07", ...]
}

更新方式:
1. 内置当年数据
2. 设置页可手动导入新年份
3. 后续可从 API 自动获取
```

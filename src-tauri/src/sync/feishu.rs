//! 飞书双向同步模块
//!
//! - FeishuAuth: tenant_access_token 获取与自动刷新
//! - RateLimiter: 令牌桶限流 + 429 Retry-After
//! - sync_feishu_pull / sync_feishu_push: 双向同步逻辑

use anyhow::{Context, Result};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use crate::db::{new_id, now_local};

// ============================================================
// Keychain 存储
// ============================================================

const KEYCHAIN_SERVICE: &str = "com.casy.feishu";
const KEY_APP_ID: &str = "app_id";
const KEY_APP_SECRET: &str = "app_secret";

/// 保存飞书凭证到 OS keychain
pub fn save_feishu_credentials(app_id: &str, app_secret: &str) -> Result<()> {
    let entry_id =
        keyring::Entry::new(KEYCHAIN_SERVICE, KEY_APP_ID).context("创建 keychain entry 失败")?;
    entry_id
        .set_password(app_id)
        .context("保存 app_id 到 keychain 失败")?;

    let entry_secret = keyring::Entry::new(KEYCHAIN_SERVICE, KEY_APP_SECRET)
        .context("创建 keychain entry 失败")?;
    entry_secret
        .set_password(app_secret)
        .context("保存 app_secret 到 keychain 失败")?;

    Ok(())
}

/// 从 OS keychain 读取飞书凭证
pub fn load_feishu_credentials() -> Result<(String, String)> {
    let entry_id =
        keyring::Entry::new(KEYCHAIN_SERVICE, KEY_APP_ID).context("创建 keychain entry 失败")?;
    let app_id = entry_id
        .get_password()
        .context("读取 app_id 失败，请先配置飞书凭证")?;

    let entry_secret = keyring::Entry::new(KEYCHAIN_SERVICE, KEY_APP_SECRET)
        .context("创建 keychain entry 失败")?;
    let app_secret = entry_secret
        .get_password()
        .context("读取 app_secret 失败，请先配置飞书凭证")?;

    Ok((app_id, app_secret))
}

// ============================================================
// FeishuAuth — Token 管理
// ============================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FeishuTokenInfo {
    pub tenant_access_token: String,
    pub expire_at: u64, // Unix 时间戳（秒）
}

pub struct FeishuAuth {
    client: Client,
    token: Option<FeishuTokenInfo>,
}

impl FeishuAuth {
    pub fn new() -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("创建 HTTP client 失败");
        Self {
            client,
            token: None,
        }
    }

    /// 获取 tenant_access_token（自动刷新）
    pub async fn get_token(&mut self) -> Result<String> {
        // 检查缓存是否有效（提前 60 秒刷新）
        if let Some(ref info) = self.token {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            if now + 60 < info.expire_at {
                return Ok(info.tenant_access_token.clone());
            }
        }

        // 从 keychain 读取凭证
        let (app_id, app_secret) = load_feishu_credentials()?;

        // 请求新 token
        let resp = self
            .client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({
                "app_id": app_id,
                "app_secret": app_secret,
            }))
            .send()
            .await
            .context("请求 tenant_access_token 失败")?;

        let body: serde_json::Value = resp.json().await.context("解析 token 响应失败")?;

        if body["code"].as_i64().unwrap_or(-1) != 0 {
            let msg = body["msg"].as_str().unwrap_or("未知错误");
            anyhow::bail!("获取 token 失败: {}", msg);
        }

        let token_str = body["tenant_access_token"]
            .as_str()
            .context("响应中无 tenant_access_token")?
            .to_string();
        let expire = body["expire"].as_u64().unwrap_or(7200);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        self.token = Some(FeishuTokenInfo {
            tenant_access_token: token_str.clone(),
            expire_at: now + expire,
        });

        Ok(token_str)
    }
}

// ============================================================
// RateLimiter — 令牌桶 + 429 处理
// ============================================================

pub struct RateLimiter {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
    retry_after: Option<Instant>,
}

impl RateLimiter {
    pub fn new(max_per_second: f64) -> Self {
        Self {
            tokens: max_per_second,
            max_tokens: max_per_second,
            refill_rate: max_per_second,
            last_refill: Instant::now(),
            retry_after: None,
        }
    }

    /// 等待直到有可用 token
    pub async fn acquire(&mut self) {
        // 冷却期等待
        if let Some(retry_at) = self.retry_after {
            let now = Instant::now();
            if now < retry_at {
                tokio::time::sleep(retry_at - now).await;
            }
            self.retry_after = None;
        }

        loop {
            self.refill();
            if self.tokens >= 1.0 {
                self.tokens -= 1.0;
                return;
            }
            // 等待一个 token 填充的时间
            let wait = Duration::from_secs_f64(1.0 / self.refill_rate);
            tokio::time::sleep(wait).await;
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }

    /// 处理 429 响应
    pub fn handle_429(&mut self, retry_after_secs: u64) {
        self.retry_after = Some(Instant::now() + Duration::from_secs(retry_after_secs));
    }
}

// ============================================================
// 飞书 API 调用封装
// ============================================================

/// HTTP 方法
enum HttpMethod {
    Get,
    Post,
    Put,
}

/// 带限流和 429 重试的飞书 API 调用
async fn call_feishu_api(
    client: &Client,
    limiter: &mut RateLimiter,
    method: HttpMethod,
    url: &str,
    token: &str,
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value> {
    for _ in 0..3 {
        limiter.acquire().await;

        let response = match method {
            HttpMethod::Get => {
                client
                    .get(url)
                    .header("Authorization", format!("Bearer {}", token))
                    .send()
                    .await?
            }
            HttpMethod::Post => {
                let mut req = client
                    .post(url)
                    .header("Authorization", format!("Bearer {}", token));
                if let Some(b) = body {
                    req = req.json(b);
                }
                req.send().await?
            }
            HttpMethod::Put => {
                let mut req = client
                    .put(url)
                    .header("Authorization", format!("Bearer {}", token));
                if let Some(b) = body {
                    req = req.json(b);
                }
                req.send().await?
            }
        };

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5);
            limiter.handle_429(retry_after);
            continue;
        }

        let status = response.status();
        let resp_body: serde_json::Value = response.json().await?;

        if !status.is_success() {
            anyhow::bail!("飞书 API 错误 ({}): {}", status, resp_body);
        }

        return Ok(resp_body);
    }
    anyhow::bail!("飞书 API 调用超过最大重试次数")
}

// ============================================================
// 字段映射：Casy ↔ 飞书
// ============================================================

/// Casy DB 列名 → 飞书字段名
const FIELD_MAP: &[(&str, &str)] = &[
    ("case_name", "案件信息"),
    ("case_no", "案号"),
    ("internal_no", "内部卷号"),
    ("cause_action", "案由"),
    ("client_name", "客户名称"),
    ("our_role", "我方诉讼地位"),
    ("opponent_name", "对方名称"),
    ("opponent_role", "诉讼地位"),
    ("opponent_firm", "对方代理律所"),
    ("opponent_agent", "对方代理人"),
    ("court", "审理机关"),
    ("judge_panel", "合议庭"),
    ("clerk", "书记员"),
    ("attorneys", "代理人"),
    ("case_level", "审级"),
    ("case_progress", "案件进展"),
    ("case_result", "案件结果"),
    ("patent_name", "专利名称"),
    ("patent_app_no", "专利申请号"),
    ("procedure_type", "诉讼程序"),
    ("filing_date", "立案"),
    ("complaint_received_date", "收到起诉状时间"),
    ("trial_date", "开庭|口审"),
    ("trial2_date", "二次开庭|口审"),
    ("trial3_date", "三次开庭丨口审"),
    ("verdict_type", "收到判决/裁定/决定类型"),
    ("verdict_date", "收到判决/裁定/决定时间"),
    ("notes", "备注"),
];

/// 从飞书记录提取单选值
fn extract_single_select(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }
        serde_json::Value::Array(arr) => arr.first().and_then(|v| v.as_str()).map(|s| s.to_string()),
        serde_json::Value::Object(obj) => obj
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// 从飞书记录提取日期时间（毫秒时间戳 → YYYY-MM-DD）
fn extract_datetime(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Number(n) => {
            let ms = n.as_i64()?;
            if ms == 0 {
                return None;
            }
            let dt = chrono::DateTime::from_timestamp_millis(ms)?.naive_utc();
            Some(dt.format("%Y-%m-%d").to_string())
        }
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }
        _ => None,
    }
}

/// 从飞书多选字段提取 JSON 数组字符串
fn extract_multi_select_json(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if items.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string()))
            }
        }
        serde_json::Value::String(s) => {
            if s.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&[s]).unwrap_or_else(|_| "[]".to_string()))
            }
        }
        _ => None,
    }
}

/// 本地日期字符串 → 飞书毫秒时间戳
fn date_to_feishu_ms(value: &Option<String>) -> Option<serde_json::Value> {
    let s = value.as_ref()?;
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let dt = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    let ts = dt.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
    Some(serde_json::json!(ts))
}

/// 本地 JSON 数组字符串 → 飞书多选值
fn json_array_to_feishu_multi(value: &Option<String>) -> Option<serde_json::Value> {
    let s = value.as_ref()?;
    let arr: Vec<String> = serde_json::from_str(s).ok()?;
    if arr.is_empty() {
        None
    } else {
        Some(serde_json::json!(arr))
    }
}

// ============================================================
// Sync Report
// ============================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FeishuSyncReport {
    pub pulled: usize,
    pub pushed: usize,
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub synced_at: String,
}

// ============================================================
// PULL：从飞书拉取到本地
// ============================================================

pub async fn sync_feishu_pull_inner(
    app_token: &str,
    table_id: &str,
) -> Result<FeishuSyncReport> {
    let mut auth = FeishuAuth::new();
    let mut limiter = RateLimiter::new(5.0); // 保守限流
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()?;

    let mut report = FeishuSyncReport {
        pulled: 0,
        pushed: 0,
        created: 0,
        updated: 0,
        skipped: 0,
        errors: Vec::new(),
        synced_at: now_local(),
    };

    let conn = crate::db::open_db()?;
    let mut page_token: Option<String> = None;

    loop {
        let token = auth.get_token().await?;

        // 构建请求 URL
        let mut url = format!(
            "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/records?page_size=200",
            app_token, table_id
        );
        if let Some(ref pt) = page_token {
            url.push_str(&format!("&page_token={}", pt));
        }

        let body = call_feishu_api(&client, &mut limiter, HttpMethod::Get, &url, &token, None).await?;

        if body["code"].as_i64().unwrap_or(-1) != 0 {
            let msg = body["msg"].as_str().unwrap_or("未知错误");
            anyhow::bail!("获取记录失败: {}", msg);
        }

        let items = body["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        if items.is_empty() {
            break;
        }

        for item in &items {
            match pull_one_record(&conn, item) {
                Ok(action) => {
                    report.pulled += 1;
                    match action.as_str() {
                        "created" => report.created += 1,
                        "updated" => report.updated += 1,
                        _ => report.skipped += 1,
                    }
                }
                Err(e) => {
                    report.errors.push(format!("{}", e));
                }
            }
        }

        // 翻页
        let has_more = body["data"]["has_more"].as_bool().unwrap_or(false);
        if !has_more {
            break;
        }
        page_token = body["data"]["page_token"]
            .as_str()
            .map(|s| s.to_string());

        // 限流间隔
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // 更新同步状态到 settings
    update_sync_metadata(&conn, "feishu_last_pull_at", &report.synced_at)?;
    update_sync_metadata(
        &conn,
        "feishu_last_pull_count",
        &report.pulled.to_string(),
    )?;

    Ok(report)
}

/// 处理单条飞书记录的 pull
fn pull_one_record(conn: &Connection, item: &serde_json::Value) -> Result<String> {
    let record_id = item["record_id"]
        .as_str()
        .context("记录缺少 record_id")?;
    let fields = &item["fields"];

    // 查 sync_map
    let existing: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT local_id, sync_status, remote_updated FROM sync_map
             WHERE remote_id = ?1 AND remote_source = 'feishu'",
            params![record_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok();

    if let Some((local_id, _status, old_remote_updated)) = existing {
        // 已有映射 — 检查飞书端是否更新
        let new_remote_updated = item["last_modified_time"]
            .as_str()
            .or_else(|| item["created_time"].as_str())
            .unwrap_or("")
            .to_string();

        if old_remote_updated.as_deref() == Some(&new_remote_updated) && !new_remote_updated.is_empty() {
            return Ok("skipped".to_string());
        }

        // 飞书更新了 → 更新本地
        update_local_case(conn, &local_id, fields)?;

        conn.execute(
            "UPDATE sync_map SET remote_updated = ?1, sync_status = 'synced',
             last_synced_at = ?2 WHERE remote_id = ?3 AND remote_source = 'feishu'",
            params![new_remote_updated, now_local(), record_id],
        )?;

        Ok("updated".to_string())
    } else {
        // 新记录 → INSERT
        let local_id = insert_case_from_feishu(conn, fields)?;

        let remote_updated = item["last_modified_time"]
            .as_str()
            .or_else(|| item["created_time"].as_str())
            .unwrap_or("")
            .to_string();

        conn.execute(
            "INSERT INTO sync_map (id, local_table, local_id, remote_id, remote_source,
             remote_updated, sync_status, last_synced_at)
             VALUES (?1, 'cases', ?2, ?3, 'feishu', ?4, 'synced', ?5)",
            params![new_id(), local_id, record_id, remote_updated, now_local()],
        )?;

        Ok("created".to_string())
    }
}

/// 从飞书字段创建本地案件
fn insert_case_from_feishu(conn: &Connection, fields: &serde_json::Value) -> Result<String> {
    let case_name = fields["案件信息"].as_str().unwrap_or("").trim();
    if case_name.is_empty() {
        anyhow::bail!("案件名称为空");
    }

    let local_id = new_id();
    let track = match extract_single_select(&fields["案由"]).as_deref() {
        Some("专利无效") => "patent_invalidation",
        Some("专利侵权" | "技术秘密" | "著作权权属" | "专利权属" | "外观侵权" | "恶意诉讼不正当竞争") => {
            "civil_tort"
        }
        Some("专利行政" | "商标行政") => "admin_litigation",
        _ => "other",
    };

    conn.execute(
        "INSERT INTO cases (id, track, case_name, case_no, internal_no, cause_action,
         client_name, our_role, opponent_name, opponent_role, opponent_firm, opponent_agent,
         court, judge_panel, clerk, attorneys, case_level, case_progress, case_result,
         patent_name, patent_app_no, procedure_type,
         filing_date, complaint_received_date, trial_date, trial2_date, trial3_date,
         verdict_type, verdict_date, notes, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,
                 ?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32)",
        params![
            local_id,
            track,
            case_name,
            fields["案号"].as_str().unwrap_or(""),
            fields["内部卷号"].as_str().unwrap_or(""),
            extract_single_select(&fields["案由"]).unwrap_or_default(),
            fields["客户名称"].as_str().unwrap_or(""),
            fields["我方诉讼地位"].as_str().unwrap_or(""),
            fields["对方名称"].as_str().unwrap_or(""),
            extract_single_select(&fields["诉讼地位"]).unwrap_or_default(),
            fields["对方代理律所"].as_str().unwrap_or(""),
            fields["对方代理人"].as_str().unwrap_or(""),
            extract_single_select(&fields["审理机关"]).unwrap_or_default(),
            fields["合议庭"].as_str().unwrap_or(""),
            fields["书记员"].as_str().unwrap_or(""),
            extract_multi_select_json(&fields["代理人"]),
            extract_single_select(&fields["审级"]).unwrap_or_default(),
            extract_single_select(&fields["案件进展"]).unwrap_or_default(),
            extract_single_select(&fields["案件结果"]).unwrap_or_default(),
            fields["专利名称"].as_str().unwrap_or(""),
            fields["专利申请号"].as_str().unwrap_or(""),
            extract_single_select(&fields["诉讼程序"]).unwrap_or_default(),
            extract_datetime(&fields["立案"]),
            extract_datetime(&fields["收到起诉状时间"]),
            extract_datetime(&fields["开庭|口审"]),
            extract_datetime(&fields["二次开庭|口审"]),
            extract_datetime(&fields["三次开庭丨口审"]),
            extract_single_select(&fields["收到判决/裁定/决定类型"]),
            extract_datetime(&fields["收到判决/裁定/决定时间"]),
            fields["备注"].as_str().unwrap_or(""),
            now_local(),
            now_local(),
        ],
    )?;

    Ok(local_id)
}

/// 更新本地案件（从飞书数据）
fn update_local_case(conn: &Connection, local_id: &str, fields: &serde_json::Value) -> Result<()> {
    let mut sql = String::from("UPDATE cases SET updated_at = datetime('now','localtime')");
    let mut param_idx = 1;
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    // 文本字段
    let text_fields = [
        ("案件信息", "case_name"),
        ("案号", "case_no"),
        ("内部卷号", "internal_no"),
        ("客户名称", "client_name"),
        ("我方诉讼地位", "our_role"),
        ("对方名称", "opponent_name"),
        ("对方代理律所", "opponent_firm"),
        ("对方代理人", "opponent_agent"),
        ("合议庭", "judge_panel"),
        ("书记员", "clerk"),
        ("专利名称", "patent_name"),
        ("专利申请号", "patent_app_no"),
        ("备注", "notes"),
    ];

    for (feishu_key, db_col) in &text_fields {
        if let Some(val) = fields[*feishu_key].as_str() {
            sql.push_str(&format!(", {} = ?{}", db_col, param_idx));
            params_vec.push(Box::new(val.to_string()));
            param_idx += 1;
        }
    }

    // 单选字段
    let select_fields = [
        ("案由", "cause_action"),
        ("诉讼地位", "opponent_role"),
        ("审理机关", "court"),
        ("审级", "case_level"),
        ("案件进展", "case_progress"),
        ("案件结果", "case_result"),
        ("诉讼程序", "procedure_type"),
        ("收到判决/裁定/决定类型", "verdict_type"),
    ];

    for (feishu_key, db_col) in &select_fields {
        if let Some(val) = extract_single_select(&fields[*feishu_key]) {
            sql.push_str(&format!(", {} = ?{}", db_col, param_idx));
            params_vec.push(Box::new(val));
            param_idx += 1;
        }
    }

    // 多选字段
    if let Some(val) = extract_multi_select_json(&fields["代理人"]) {
        sql.push_str(&format!(", attorneys = ?{}", param_idx));
        params_vec.push(Box::new(val));
        param_idx += 1;
    }

    // 日期字段
    let date_fields = [
        ("立案", "filing_date"),
        ("收到起诉状时间", "complaint_received_date"),
        ("开庭|口审", "trial_date"),
        ("二次开庭|口审", "trial2_date"),
        ("三次开庭丨口审", "trial3_date"),
        ("收到判决/裁定/决定时间", "verdict_date"),
    ];

    for (feishu_key, db_col) in &date_fields {
        if let Some(val) = extract_datetime(&fields[*feishu_key]) {
            sql.push_str(&format!(", {} = ?{}", db_col, param_idx));
            params_vec.push(Box::new(val));
            param_idx += 1;
        }
    }

    sql.push_str(&format!(" WHERE id = ?{}", param_idx));
    params_vec.push(Box::new(local_id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;

    Ok(())
}

// ============================================================
// PUSH：本地推送到飞书
// ============================================================

pub async fn sync_feishu_push_inner(
    app_token: &str,
    table_id: &str,
) -> Result<FeishuSyncReport> {
    let mut auth = FeishuAuth::new();
    let mut limiter = RateLimiter::new(5.0);
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()?;

    let mut report = FeishuSyncReport {
        pulled: 0,
        pushed: 0,
        created: 0,
        updated: 0,
        skipped: 0,
        errors: Vec::new(),
        synced_at: now_local(),
    };

    let conn = crate::db::open_db()?;

    // 查询需要 push 的记录：local_newer 或没有映射的新记录
    let push_items = get_push_items(&conn)?;

    for item in push_items {
        let token = auth.get_token().await?;

        let fields_json = build_feishu_fields(&conn, &item.local_id)?;

        if let Some(ref remote_id) = item.remote_id {
            // 更新已有记录
            let url = format!(
                "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
                app_token, table_id, remote_id
            );
            let put_body = serde_json::json!({ "fields": fields_json });
            match call_feishu_api(&client, &mut limiter, HttpMethod::Put, &url, &token, Some(&put_body)).await {
                Ok(_) => {
                    conn.execute(
                        "UPDATE sync_map SET sync_status = 'synced', last_synced_at = ?1,
                         local_updated = ?2 WHERE id = ?3",
                        params![now_local(), now_local(), item.map_id],
                    )?;
                    report.pushed += 1;
                    report.updated += 1;
                }
                Err(e) => {
                    let attempts = item.attempts + 1;
                    let status = if attempts >= 3 {
                        "push_failed"
                    } else {
                        "local_newer"
                    };
                    conn.execute(
                        "UPDATE sync_map SET sync_status = ?1, last_synced_at = ?2 WHERE id = ?3",
                        params![status, now_local(), item.map_id],
                    )?;
                    report.errors.push(format!("推送失败 ({}): {}", item.local_id, e));
                }
            }
        } else {
            // 创建新记录
            let url = format!(
                "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/records",
                app_token, table_id
            );
            let post_body = serde_json::json!({ "fields": fields_json });
            match call_feishu_api(&client, &mut limiter, HttpMethod::Post, &url, &token, Some(&post_body)).await {
                Ok(body) => {
                    let new_remote_id = body["data"]["record"]["record_id"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    conn.execute(
                        "UPDATE sync_map SET remote_id = ?1, sync_status = 'synced',
                         last_synced_at = ?2, local_updated = ?3 WHERE id = ?4",
                        params![new_remote_id, now_local(), now_local(), item.map_id],
                    )?;
                    report.pushed += 1;
                    report.created += 1;
                }
                Err(e) => {
                    let attempts = item.attempts + 1;
                    let status = if attempts >= 3 {
                        "push_failed"
                    } else {
                        "local_newer"
                    };
                    conn.execute(
                        "UPDATE sync_map SET sync_status = ?1, last_synced_at = ?2 WHERE id = ?3",
                        params![status, now_local(), item.map_id],
                    )?;
                    report.errors.push(format!("创建失败 ({}): {}", item.local_id, e));
                }
            }
        }

        // 限流间隔
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // 更新同步状态
    update_sync_metadata(&conn, "feishu_last_push_at", &report.synced_at)?;
    update_sync_metadata(
        &conn,
        "feishu_last_push_count",
        &report.pushed.to_string(),
    )?;

    Ok(report)
}

/// Push 待处理项
struct PushItem {
    map_id: String,
    local_id: String,
    remote_id: Option<String>,
    attempts: i64,
}

/// 获取需要 push 的记录
fn get_push_items(conn: &Connection) -> Result<Vec<PushItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, local_id, remote_id, 0 FROM sync_map
         WHERE remote_source = 'feishu' AND sync_status = 'local_newer'
         UNION ALL
         SELECT sm.id, sm.local_id, sm.remote_id, 0
         FROM sync_map sm
         JOIN cases c ON c.id = sm.local_id
         WHERE sm.remote_source = 'feishu' AND sm.sync_status = 'synced'
           AND c.updated_at > COALESCE(sm.last_synced_at, '1970-01-01')",
    )?;

    let items = stmt
        .query_map([], |row| {
            Ok(PushItem {
                map_id: row.get(0)?,
                local_id: row.get(1)?,
                remote_id: row.get(2)?,
                attempts: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(items)
}

/// 从本地案件构建飞书字段 JSON
fn build_feishu_fields(conn: &Connection, local_id: &str) -> Result<serde_json::Value> {
    let mut stmt = conn.prepare("SELECT * FROM cases WHERE id = ?1")?;
    let case = stmt.query_row(params![local_id], |row| {
        Ok(serde_json::json!({
            "case_name": row.get::<_, Option<String>>("case_name")?,
            "case_no": row.get::<_, Option<String>>("case_no")?,
            "internal_no": row.get::<_, Option<String>>("internal_no")?,
            "cause_action": row.get::<_, Option<String>>("cause_action")?,
            "client_name": row.get::<_, Option<String>>("client_name")?,
            "our_role": row.get::<_, Option<String>>("our_role")?,
            "opponent_name": row.get::<_, Option<String>>("opponent_name")?,
            "opponent_role": row.get::<_, Option<String>>("opponent_role")?,
            "opponent_firm": row.get::<_, Option<String>>("opponent_firm")?,
            "opponent_agent": row.get::<_, Option<String>>("opponent_agent")?,
            "court": row.get::<_, Option<String>>("court")?,
            "judge_panel": row.get::<_, Option<String>>("judge_panel")?,
            "clerk": row.get::<_, Option<String>>("clerk")?,
            "attorneys": row.get::<_, Option<String>>("attorneys")?,
            "case_level": row.get::<_, Option<String>>("case_level")?,
            "case_progress": row.get::<_, Option<String>>("case_progress")?,
            "case_result": row.get::<_, Option<String>>("case_result")?,
            "patent_name": row.get::<_, Option<String>>("patent_name")?,
            "patent_app_no": row.get::<_, Option<String>>("patent_app_no")?,
            "procedure_type": row.get::<_, Option<String>>("procedure_type")?,
            "filing_date": row.get::<_, Option<String>>("filing_date")?,
            "complaint_received_date": row.get::<_, Option<String>>("complaint_received_date")?,
            "trial_date": row.get::<_, Option<String>>("trial_date")?,
            "trial2_date": row.get::<_, Option<String>>("trial2_date")?,
            "trial3_date": row.get::<_, Option<String>>("trial3_date")?,
            "verdict_type": row.get::<_, Option<String>>("verdict_type")?,
            "verdict_date": row.get::<_, Option<String>>("verdict_date")?,
            "notes": row.get::<_, Option<String>>("notes")?,
        }))
    })?;

    // 转换为飞书字段格式
    let mut fields = serde_json::Map::new();

    for &(db_col, feishu_key) in FIELD_MAP {
        if let Some(val) = case.get(db_col) {
            match val {
                serde_json::Value::String(s) if !s.is_empty() => {
                    // 日期字段需要转为毫秒时间戳
                    let date_fields = [
                        "filing_date",
                        "complaint_received_date",
                        "trial_date",
                        "trial2_date",
                        "trial3_date",
                        "verdict_date",
                    ];
                    if date_fields.contains(&db_col) {
                        if let Some(ms) = date_to_feishu_ms(&Some(s.clone())) {
                            fields.insert(feishu_key.to_string(), ms);
                        }
                    }
                    // 多选字段
                    else if db_col == "attorneys" {
                        if let Some(arr) = json_array_to_feishu_multi(&Some(s.clone())) {
                            fields.insert(feishu_key.to_string(), arr);
                        }
                    }
                    // 单选/文本字段
                    else {
                        fields.insert(feishu_key.to_string(), serde_json::json!(s));
                    }
                }
                _ => {}
            }
        }
    }

    Ok(serde_json::Value::Object(fields))
}

// ============================================================
// 同步元数据辅助
// =================================================/// 读取同步元数据
pub fn get_sync_metadata(conn: &Connection, key: &str) -> Result<Option<String>> {
    let result = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .ok();
    Ok(result)
}

/// 更新同步元数据
pub fn update_sync_metadata(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

/// 测试飞书连通性
pub async fn test_feishu_connection_inner() -> Result<String> {
    let mut auth = FeishuAuth::new();
    let token = auth.get_token().await?;
    if token.starts_with("t-") || token.len() > 20 {
        Ok("飞书连接成功".to_string())
    } else {
        anyhow::bail!("获取的 token 格式异常")
    }
}

/// 检查是否已配置飞书凭证
pub fn is_feishu_configured() -> bool {
    load_feishu_credentials().is_ok()
}

// ============================================================
// 自动推送管理器（watch 通道 + 5 秒防抖）
// ============================================================

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::watch;

/// 自动推送管理器
///
/// 使用 `tokio::sync::watch` 通道通知数据变更，
/// 后台 task 接收信号后执行 5 秒防抖再推送。
pub struct AutoPushManager {
    /// 变更通知发送端（每次 notify_change 递增计数器）
    tx: watch::Sender<u64>,
    /// 是否启用自动推送
    enabled: Arc<AtomicBool>,
    /// 变更计数器（由 tx 驱动）
    change_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl AutoPushManager {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(0u64);
        Self {
            tx,
            enabled: Arc::new(AtomicBool::new(false)),
            change_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// 启用/禁用自动推送
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        log::info!("飞书自动推送: {}", if enabled { "已启用" } else { "已禁用" });
    }

    /// 通知有数据变更（通过 watch 通道触发 5 秒防抖推送）
    pub fn notify_change(&self) {
        if !self.enabled.load(Ordering::SeqCst) {
            return;
        }
        if !is_feishu_configured() {
            return;
        }

        let new_count = self.change_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.tx.send(new_count);
        log::debug!("飞书自动推送: 检测到变更 #{}", new_count);
    }

    /// 获取当前状态
    pub fn get_status_sync(&self) -> serde_json::Value {
        let enabled = self.enabled.load(Ordering::SeqCst);
        serde_json::json!({
            "enabled": enabled,
            "configured": is_feishu_configured(),
        })
    }
}

/// 全局自动推送管理器
static AUTO_PUSH_MANAGER: std::sync::OnceLock<Arc<AutoPushManager>> = std::sync::OnceLock::new();

/// 获取全局自动推送管理器
pub fn get_auto_push_manager() -> &'static Arc<AutoPushManager> {
    AUTO_PUSH_MANAGER.get_or_init(|| Arc::new(AutoPushManager::new()))
}

/// 启动飞书自动推送后台监听 task
///
/// 在 `lib.rs` 的 `setup` 中调用，启动一个永久运行的后台 task：
/// 1. 通过 `watch::Receiver` 监听数据变更信号
/// 2. 收到信号后等待 5 秒（防抖）
/// 3. 5 秒内再次收到信号则重新计时
/// 4. 防抖结束后执行 push
pub fn start_auto_push_watcher() {
    let manager = get_auto_push_manager();
    let mut rx = manager.tx.subscribe();
    let enabled = manager.enabled.clone();

    tokio::spawn(async move {
        log::info!("飞书自动推送 watcher 已启动");

        loop {
            // 等待变更信号
            if rx.changed().await.is_err() {
                log::warn!("飞书自动推送 watcher: 发送端已关闭，退出");
                break;
            }

            if !enabled.load(Ordering::SeqCst) {
                continue;
            }

            // 5 秒防抖：持续等待直到 5 秒内无新变更
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;

                // 检查防抖期间是否有新变更
                if rx.has_changed().unwrap_or(false) {
                    // 有新变更，重新计时
                    log::debug!("飞书自动推送: 防抖期间收到新变更，重新计时");
                    // 消费掉当前值，继续等待
                    let _ = *rx.borrow_and_update();
                    continue;
                }

                // 5 秒内无新变更，可以执行推送
                break;
            }

            // 再次检查启用状态
            if !enabled.load(Ordering::SeqCst) {
                continue;
            }

            if !is_feishu_configured() {
                log::debug!("飞书自动推送: 凭证未配置，跳过");
                continue;
            }

            // 执行推送
            log::info!("飞书自动推送: 防抖结束，开始执行推送");
            match execute_auto_push().await {
                Ok(report) => {
                    log::info!(
                        "飞书自动推送完成: 推送 {} 条, 新建 {} 条, 更新 {} 条",
                        report.pushed,
                        report.created,
                        report.updated
                    );
                }
                Err(e) => {
                    log::warn!("飞书自动推送失败: {}", e);
                }
            }
        }
    });
}

/// 执行自动推送
async fn execute_auto_push() -> Result<FeishuSyncReport> {
    // 从 settings 读取飞书表格配置
    let conn = crate::db::open_db()?;
    let app_token = get_sync_metadata(&conn, "feishu_app_token")?
        .unwrap_or_default();
    let table_id = get_sync_metadata(&conn, "feishu_table_id")?
        .unwrap_or_default();

    if app_token.is_empty() || table_id.is_empty() {
        anyhow::bail!("飞书表格未配置");
    }

    sync_feishu_push_inner(&app_token, &table_id).await
}

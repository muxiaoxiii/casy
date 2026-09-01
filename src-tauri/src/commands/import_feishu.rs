use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::DateTime;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::import_excel::{
    clean_array_to_json, clean_date_str, extract_dates_list, infer_track_and_route,
    match_field_confidence, CaseImportConfig, CaseImportReport, ColumnMappingRecommendation,
    SubtableImportConfig, SubtableImportReport,
};
use super::run_blocking;
use crate::db;
use crate::sync::feishu::{load_feishu_credentials, FeishuAuth, RateLimiter};

// ============================================================
// 飞书配置状态与多维表格元数据
// ============================================================

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConfigStatus {
    pub configured: bool,
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FeishuTableMeta {
    pub table_id: String,
    pub name: String,
    pub revision: Option<i64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FeishuBitableInspectResult {
    pub app_token: String,
    pub table_id: String,
    pub table_name: String,
    pub tables: Vec<FeishuTableMeta>,
    pub total_records: usize,
    pub columns: Vec<ColumnMappingRecommendation>,
    pub preview_rows: Vec<HashMap<String, serde_json::Value>>,
}

// ============================================================
// 飞书 URL 与 Token 提取算法
// ============================================================

/// 从用户粘贴的飞书链接或原始 Token 中提取 app_token 与 table_id
pub fn extract_feishu_tokens(input: &str) -> (Option<String>, Option<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (None, None);
    }

    // 1. 尝试从 URL 提取：https://*.feishu.cn/base/{app_token}?table={table_id}
    let base_re = Regex::new(r"/(?:base|apps)/([a-zA-Z0-9_-]{15,40})").unwrap();
    let app_token = if let Some(caps) = base_re.captures(trimmed) {
        Some(caps[1].to_string())
    } else if trimmed.starts_with("bascn")
        || (trimmed.len() >= 15 && trimmed.len() <= 40 && !trimmed.contains('/'))
    {
        Some(trimmed.to_string())
    } else {
        None
    };

    // 2. 尝试提取 table_id：?table=tblXXXXXXXX 或 &table=tblXXXXXXXX
    let table_re = Regex::new(r"[?&]table=([a-zA-Z0-9_-]{10,40})").unwrap();
    let table_id = if let Some(caps) = table_re.captures(trimmed) {
        Some(caps[1].to_string())
    } else if trimmed.starts_with("tbl") && !trimmed.contains('/') {
        Some(trimmed.to_string())
    } else {
        None
    };

    (app_token, table_id)
}

// ============================================================
// 飞书字段值转为通用字符串 (Bitable Value Converter)
// ============================================================

pub fn convert_feishu_val_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                // 如果数值大于 1_000_000_000_000 (约等于 2001 年以后的毫秒级时间戳)
                if i > 1_000_000_000_000 && i < 2_500_000_000_000 {
                    if let Some(dt) = DateTime::from_timestamp_millis(i) {
                        return dt.format("%Y-%m-%d").to_string();
                    }
                }
                i.to_string()
            } else if let Some(f) = num.as_f64() {
                format!("{:.2}", f)
            } else {
                num.to_string()
            }
        }
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    parts.push(name.trim().to_string());
                } else if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(text.trim().to_string());
                } else if let Some(s) = item.as_str() {
                    parts.push(s.trim().to_string());
                } else {
                    let sub = convert_feishu_val_to_string(item);
                    if !sub.is_empty() {
                        parts.push(sub);
                    }
                }
            }
            parts.join("、")
        }
        serde_json::Value::Object(obj) => {
            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                text.trim().to_string()
            } else if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                name.trim().to_string()
            } else if let Some(link) = obj.get("link").and_then(|l| l.as_str()) {
                link.trim().to_string()
            } else {
                String::new()
            }
        }
    }
}

// ============================================================
// 飞书 OpenAPI 交互辅助
// ============================================================

async fn get_feishu_client_and_token() -> Result<(Client, String)> {
    let mut auth = FeishuAuth::new();
    let token = auth
        .get_token()
        .await
        .map_err(|e| anyhow!("FEISHU_AUTH_FAILED: {}", e))?;

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(45))
        .build()
        .context("创建 HTTP 客户端失败")?;

    Ok((client, token))
}

// ============================================================
// Tauri IPC 命令
// ============================================================

/// 检查飞书自建应用配置状态
#[tauri::command]
pub async fn feishu_check_config() -> Result<FeishuConfigStatus, String> {
    run_blocking(move || match load_feishu_credentials() {
        Ok((app_id, _)) => Ok(FeishuConfigStatus {
            configured: true,
            app_id: Some(app_id),
        }),
        Err(_) => Ok(FeishuConfigStatus {
            configured: false,
            app_id: None,
        }),
    })
    .await
}

/// 探测飞书多维表格结构、字段列表与样本数据
#[tauri::command]
pub async fn feishu_inspect_bitable(
    url_or_token: String,
    table_id_override: Option<String>,
) -> Result<FeishuBitableInspectResult, String> {
    let (app_token_opt, table_id_opt) = extract_feishu_tokens(&url_or_token);

    let app_token = app_token_opt.ok_or_else(|| {
        "无法从输入的链接或文本中解析出多维表格 app_token (如 bascn...)".to_string()
    })?;

    let target_table_id = table_id_override.or(table_id_opt);

    // 1. 获取客户端与 token
    let (client, token) = get_feishu_client_and_token()
        .await
        .map_err(|e| e.to_string())?;

    // 2. 获取该 Base 下的所有工作表 (tables)
    let tables_url = format!(
        "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables",
        app_token
    );
    let tables_resp = client
        .get(&tables_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("请求飞书工作表列表网络失败: {}", e))?;

    let tables_body: serde_json::Value = tables_resp
        .json()
        .await
        .map_err(|e| format!("解析飞书响应失败: {}", e))?;

    let code = tables_body["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        let msg = tables_body["msg"].as_str().unwrap_or("未知错误");
        if code == 99991663 || code == 99991668 || code == 1254003 || code == 1254005 {
            return Err(format!(
                "FEISHU_NO_TABLE_PERMISSION: 飞书自建应用无权访问此多维表格。\n\n请按以下步骤授权：\n1. 打开该飞书多维表格\n2. 点击右上角「···」菜单\n3. 选择「添加文档应用」/「协作者」\n4. 搜索并添加您创建的自建应用 (App ID)\n\n(错误码: {}, 原始信息: {})",
                code, msg
            ));
        }
        return Err(format!("获取飞书工作表失败 [{}]: {}", code, msg));
    }

    let raw_tables = tables_body["data"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    if raw_tables.is_empty() {
        return Err("该多维表格中没有任何工作表 (Table)".to_string());
    }

    let mut tables_meta = Vec::new();
    for t in &raw_tables {
        if let (Some(tid), Some(tname)) = (t["table_id"].as_str(), t["name"].as_str()) {
            tables_meta.push(FeishuTableMeta {
                table_id: tid.to_string(),
                name: tname.to_string(),
                revision: t["revision"].as_i64(),
            });
        }
    }

    // 确定当前选中的 table_id
    let selected_table_meta = if let Some(ref tid) = target_table_id {
        tables_meta
            .iter()
            .find(|t| &t.table_id == tid)
            .cloned()
            .unwrap_or_else(|| tables_meta[0].clone())
    } else {
        tables_meta[0].clone()
    };

    let active_table_id = selected_table_meta.table_id.clone();
    let active_table_name = selected_table_meta.name.clone();

    // 3. 获取该表的字段列表 (fields)
    let fields_url = format!(
        "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/fields",
        app_token, active_table_id
    );
    let fields_resp = client
        .get(&fields_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("请求飞书字段列表失败: {}", e))?;

    let fields_body: serde_json::Value = fields_resp
        .json()
        .await
        .map_err(|e| format!("解析字段列表失败: {}", e))?;

    let raw_fields = fields_body["data"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut field_names = Vec::new();
    for f in &raw_fields {
        if let Some(fname) = f["field_name"].as_str() {
            field_names.push(fname.to_string());
        }
    }

    // 4. 获取前 100 条记录用于样本与预览
    let records_url = format!(
        "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/records?page_size=100",
        app_token, active_table_id
    );
    let records_resp = client
        .get(&records_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("请求飞书记录数据失败: {}", e))?;

    let records_body: serde_json::Value = records_resp
        .json()
        .await
        .map_err(|e| format!("解析记录数据失败: {}", e))?;

    let total_records = records_body["data"]["total"].as_i64().unwrap_or(0) as usize;
    let raw_records = records_body["data"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    // 收集每列的样本数据
    let mut columns = Vec::new();
    for (col_idx, field_name) in field_names.iter().enumerate() {
        let mut sample_values = Vec::new();
        for rec in &raw_records {
            if let Some(val) = rec["fields"].get(field_name) {
                let s = convert_feishu_val_to_string(val);
                if !s.is_empty() && !sample_values.contains(&s) {
                    sample_values.push(s);
                    if sample_values.len() >= 3 {
                        break;
                    }
                }
            }
        }

        let (suggested_field, confidence) = match_field_confidence(field_name);

        columns.push(ColumnMappingRecommendation {
            column_index: col_idx,
            excel_header: field_name.clone(),
            sample_values,
            suggested_field,
            confidence,
        });
    }

    // 构造前 10 行预览
    let mut preview_rows = Vec::new();
    for rec in raw_records.iter().take(10) {
        let mut row_map = HashMap::new();
        let fields = &rec["fields"];
        for field_name in &field_names {
            let val_str = fields
                .get(field_name)
                .map(convert_feishu_val_to_string)
                .unwrap_or_default();
            row_map.insert(field_name.clone(), serde_json::Value::String(val_str));
        }
        preview_rows.push(row_map);
    }

    Ok(FeishuBitableInspectResult {
        app_token,
        table_id: active_table_id,
        table_name: active_table_name,
        tables: tables_meta,
        total_records: if total_records > 0 {
            total_records
        } else {
            raw_records.len()
        },
        columns,
        preview_rows,
    })
}

/// 批量导入飞书多维表格案件入库（单事务安全写入与清洗查重）
#[tauri::command]
pub async fn feishu_import_bitable_cases(
    app_token: String,
    table_id: String,
    config: CaseImportConfig,
) -> Result<CaseImportReport, String> {
    let (client, token) = get_feishu_client_and_token()
        .await
        .map_err(|e| e.to_string())?;

    // 1. 全量翻页拉取该表的所有记录
    let mut all_records = Vec::new();
    let mut page_token: Option<String> = None;
    let mut limiter = RateLimiter::new(5.0);

    loop {
        let mut url = format!(
            "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/records?page_size=200",
            app_token, table_id
        );
        if let Some(ref pt) = page_token {
            url.push_str(&format!("&page_token={}", pt));
        }

        limiter.acquire().await;

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("拉取飞书记录网络异常: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("解析飞书记录 JSON 失败: {}", e))?;

        if body["code"].as_i64().unwrap_or(-1) != 0 {
            let msg = body["msg"].as_str().unwrap_or("未知错误");
            return Err(format!("拉取飞书记录失败: {}", msg));
        }

        let items = body["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if items.is_empty() {
            break;
        }

        all_records.extend(items);

        let has_more = body["data"]["has_more"].as_bool().unwrap_or(false);
        if !has_more {
            break;
        }
        page_token = body["data"]["page_token"].as_str().map(|s| s.to_string());
    }

    // 2. 获取该表的字段列表以对齐 column_index
    let fields_url = format!(
        "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/fields",
        app_token, table_id
    );
    let fields_resp = client
        .get(&fields_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("获取字段列表网络异常: {}", e))?;
    let fields_body: serde_json::Value = fields_resp
        .json()
        .await
        .map_err(|e| format!("解析字段列表 JSON 失败: {}", e))?;
    let field_names: Vec<String> = fields_body["data"]["items"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f["field_name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // 3. 在阻塞线程中开启数据库单事务，执行清洗、查重与批量入库
    run_blocking(move || {
        let mut conn = db::open_db().map_err(|e| anyhow!("打开数据库失败: {}", e))?;
        let tx = conn.transaction()?;

        let mut report = CaseImportReport {
            total_rows_processed: 0,
            created_count: 0,
            updated_count: 0,
            skipped_count: 0,
            failed_count: 0,
            errors: Vec::new(),
            imported_case_ids: Vec::new(),
        };

        let mut last_forward_fill_vals: HashMap<usize, String> = HashMap::new();

        for (rel_idx, record) in all_records.iter().enumerate() {
            let row_line = rel_idx + 1;
            let record_fields = &record["fields"];

            let mut extracted: HashMap<String, String> = HashMap::new();
            let mut notes_collected: Vec<String> = Vec::new();
            let mut raw_trial_dates_collected: Vec<String> = Vec::new();
            let mut row_is_empty = true;

            for (col_idx, field_name) in config.column_mappings.iter() {
                if let Some(feishu_header) = field_names.get(*col_idx) {
                    let raw_val = record_fields
                        .get(feishu_header)
                        .map(convert_feishu_val_to_string)
                        .unwrap_or_default();

                    let final_val = if raw_val.is_empty() {
                        if config.forward_fill_columns.contains(col_idx) {
                            last_forward_fill_vals.get(col_idx).cloned().unwrap_or_default()
                        } else {
                            String::new()
                        }
                    } else {
                        if config.forward_fill_columns.contains(col_idx) {
                            last_forward_fill_vals.insert(*col_idx, raw_val.clone());
                        }
                        raw_val
                    };

                    if !final_val.is_empty() {
                        row_is_empty = false;
                    }

                    if field_name == "notes" {
                        if !final_val.is_empty() {
                            notes_collected.push(format!("[{}] {}", feishu_header, final_val));
                        }
                    } else if field_name == "trialDate" || field_name == "trial2Date" || field_name == "trial3Date" {
                        if !final_val.is_empty() {
                            for d in extract_dates_list(&final_val) {
                                if !raw_trial_dates_collected.contains(&d) {
                                    raw_trial_dates_collected.push(d);
                                }
                            }
                        }
                    } else {
                        extracted.insert(field_name.clone(), final_val);
                    }
                }
            }

            if row_is_empty {
                continue;
            }

            report.total_rows_processed += 1;

            let case_name = extracted.get("caseName").map(|s| s.trim().to_string()).unwrap_or_default();
            let case_no = extracted.get("caseNo").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            let client_name = extracted.get("clientName").map(|s| s.trim().to_string()).unwrap_or_default();
            let opponent_name = extracted.get("opponentName").map(|s| s.trim().to_string()).unwrap_or_default();
            let cause_action = extracted.get("causeAction").map(|s| s.trim().to_string()).unwrap_or_default();
            let patent_name = extracted.get("patentName").map(|s| s.trim().to_string()).unwrap_or_default();

            if case_name.is_empty()
                && case_no.is_none()
                && patent_name.is_empty()
                && (client_name.is_empty() || opponent_name.is_empty())
            {
                report.skipped_count += 1;
                continue;
            }

            let final_case_name = if !case_name.is_empty() {
                case_name
            } else if !client_name.is_empty() && !opponent_name.is_empty() && !cause_action.is_empty() {
                let c_lead = client_name.lines().next().unwrap_or(&client_name).trim();
                let o_lead = opponent_name.lines().next().unwrap_or(&opponent_name).trim();
                format!("{}诉{}{}", c_lead, o_lead, cause_action)
            } else if !patent_name.is_empty() {
                let p_lead = patent_name.lines().next().unwrap_or(&patent_name).trim();
                if !cause_action.is_empty() {
                    format!("{}「{}」案", cause_action, p_lead)
                } else {
                    p_lead.to_string()
                }
            } else {
                case_no.clone().unwrap_or_else(|| format!("未命名案件-{}", row_line))
            };

            let final_client_name = if client_name.is_empty() {
                "待补充委托人".to_string()
            } else {
                client_name
            };

            let (inferred_track, raw_track, case_route) = infer_track_and_route(
                config.default_track.as_deref(),
                extracted.get("track").map(|s| s.as_str()),
                None,
                &final_case_name,
                extracted.get("causeAction").map(|s| s.as_str()),
            );

            let track = inferred_track;
            let filing_date = extracted.get("filingDate").and_then(|s| clean_date_str(s));
            let trial_date = raw_trial_dates_collected.first().cloned();
            let trial2_date = raw_trial_dates_collected.get(1).cloned();
            let trial3_date = raw_trial_dates_collected.get(2).cloned();
            for (extra_idx, extra_d) in raw_trial_dates_collected.iter().skip(3).enumerate() {
                notes_collected.push(format!("[第 {} 次开庭] {}", extra_idx + 4, extra_d));
            }
            let verdict_date = extracted
                .get("verdictDate")
                .and_then(|s| clean_date_str(s))
                .or_else(|| {
                    extracted
                        .get("caseResult")
                        .and_then(|s| clean_date_str(s))
                });
            let completed_text = extracted.get("completedText").cloned();
            let stay_date = extracted.get("stayDate").and_then(|s| clean_date_str(s));
            let relief_deadline = extracted.get("reliefDeadline").and_then(|s| clean_date_str(s));
            let attorneys = extracted.get("attorneys").and_then(|s| clean_array_to_json(s));
            let final_notes = if !notes_collected.is_empty() {
                Some(notes_collected.join("\n"))
            } else {
                extracted.get("notes").cloned().filter(|s| !s.is_empty())
            };

            // 查重判断
            let existing_case_id: Option<String> = if let Some(no) = &case_no {
                tx.query_row(
                    "SELECT id FROM cases WHERE case_no = ?1 LIMIT 1",
                    rusqlite::params![no],
                    |r| r.get(0),
                )
                .ok()
            } else {
                tx.query_row(
                    "SELECT id FROM cases WHERE case_name = ?1 LIMIT 1",
                    rusqlite::params![final_case_name],
                    |r| r.get(0),
                )
                .ok()
            };

            match (existing_case_id, config.conflict_strategy.as_str()) {
                (Some(_exist_id), "skip") => {
                    report.skipped_count += 1;
                    continue;
                }
                (Some(exist_id), "update") => {
                    let update_res = tx.execute(
                        "UPDATE cases SET
                            case_name = COALESCE(NULLIF(?1, ''), case_name),
                            client_name = COALESCE(NULLIF(?2, ''), client_name),
                            opponent_name = COALESCE(NULLIF(?3, ''), opponent_name),
                            court = COALESCE(NULLIF(?4, ''), court),
                            attorneys = COALESCE(?5, attorneys),
                            filing_date = COALESCE(?6, filing_date),
                            trial_date = COALESCE(?7, trial_date),
                            trial2_date = COALESCE(?8, trial2_date),
                            trial3_date = COALESCE(?9, trial3_date),
                            verdict_date = COALESCE(?10, verdict_date),
                            completed_text = COALESCE(NULLIF(?11, ''), completed_text),
                            judge_panel = COALESCE(NULLIF(?12, ''), judge_panel),
                            clerk = COALESCE(NULLIF(?13, ''), clerk),
                            stay_date = COALESCE(NULLIF(?14, ''), stay_date),
                            notes = CASE
                                WHEN notes IS NULL OR notes = '' THEN ?15
                                WHEN ?15 IS NULL OR ?15 = '' THEN notes
                                WHEN notes = ?15 THEN notes
                                ELSE notes || char(10) || ?15
                            END,
                            updated_at = datetime('now', 'localtime')
                        WHERE id = ?16",
                        rusqlite::params![
                            final_case_name,
                            final_client_name,
                            extracted.get("opponentName").cloned().unwrap_or_default(),
                            extracted.get("court").cloned(),
                            attorneys,
                            filing_date,
                            trial_date,
                            trial2_date,
                            trial3_date,
                            verdict_date,
                            completed_text,
                            extracted.get("judgePanel"),
                            extracted.get("clerk"),
                            stay_date,
                            final_notes,
                            exist_id
                        ],
                    );

                    match update_res {
                        Ok(_) => {
                            // 自动同步至 hearings 庭审分表 (支持无限次开庭)
                            for (h_idx, h_date) in raw_trial_dates_collected.iter().enumerate() {
                                let exists: bool = tx
                                    .query_row(
                                        "SELECT COUNT(*) > 0 FROM hearings WHERE case_id = ?1 AND hearing_date = ?2",
                                        rusqlite::params![&exist_id, h_date],
                                        |r| r.get(0),
                                    )
                                    .unwrap_or(false);

                                if !exists {
                                    let h_id = db::new_id();
                                    let h_name = format!("第 {} 次开庭/口审", h_idx + 1);
                                    let judges_json = extracted.get("judgePanel").and_then(|s| clean_array_to_json(s));
                                    let _ = tx.execute(
                                        "INSERT INTO hearings (
                                            id, case_id, hearing_record, hearing_name, hearing_date,
                                            court, case_level, judges, contact_info, actual_status, created_at
                                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '未开', datetime('now', 'localtime'))",
                                        rusqlite::params![
                                            h_id,
                                            &exist_id,
                                            final_case_name,
                                            h_name,
                                            h_date,
                                            extracted.get("court"),
                                            extracted.get("caseLevel"),
                                            judges_json,
                                            extracted.get("clerk"),
                                        ],
                                    );
                                }
                            }

                            report.updated_count += 1;
                            report.imported_case_ids.push(exist_id);
                        }
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 条记录更新失败: {}", row_line, e));
                        }
                    }
                }
                _ => {
                    let new_case_id = db::new_id();
                    let opponent_name = extracted.get("opponentName").cloned().unwrap_or_default();
                    let our_role = extracted.get("ourRole").cloned();
                    let opponent_role = extracted.get("opponentRole").cloned();
                    let case_level = extracted.get("caseLevel").cloned();
                    let case_progress = extracted.get("caseProgress").cloned();
                    let case_result = extracted.get("caseResult").cloned();
                    let cause_action = extracted.get("causeAction").cloned();
                    let court = extracted.get("court").cloned();
                    let judge_panel = extracted.get("judgePanel").cloned();
                    let clerk = extracted.get("clerk").cloned();
                    let internal_no = extracted.get("internalNo").cloned();
                    let patent_name = extracted.get("patentName").cloned();
                    let patent_app_no = extracted.get("patentAppNo").cloned();

                    let insert_res = tx.execute(
                        "INSERT INTO cases (
                            id, track, raw_track, case_name, case_no, internal_no, cause_action,
                            client_name, our_role, opponent_name, opponent_role, court, judge_panel, clerk, attorneys,
                            case_level, case_progress, case_result,
                            patent_name, patent_app_no,
                            filing_date, trial_date, trial2_date, trial3_date, verdict_date, completed_text,
                            stay_date, relief_deadline, case_route, notes, created_at, updated_at
                        ) VALUES (
                            ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                            ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                            ?16, ?17, ?18,
                            ?19, ?20,
                            ?21, ?22, ?23, ?24, ?25, ?26,
                            ?27, ?28, ?29, ?30, datetime('now', 'localtime'), datetime('now', 'localtime')
                        )",
                        rusqlite::params![
                            new_case_id,
                            track,
                            raw_track,
                            final_case_name,
                            case_no,
                            internal_no,
                            cause_action,
                            final_client_name,
                            our_role,
                            opponent_name,
                            opponent_role,
                            court,
                            judge_panel,
                            clerk,
                            attorneys,
                            case_level,
                            case_progress,
                            case_result,
                            patent_name,
                            patent_app_no,
                            filing_date,
                            trial_date,
                            trial2_date,
                            trial3_date,
                            verdict_date,
                            completed_text,
                            stay_date,
                            relief_deadline,
                            case_route,
                            final_notes,
                        ],
                    );

                    match insert_res {
                        Ok(_) => {
                            // 自动同步至 hearings 庭审分表 (支持无限次开庭)
                            for (h_idx, h_date) in raw_trial_dates_collected.iter().enumerate() {
                                let h_id = db::new_id();
                                let h_name = format!("第 {} 次开庭/口审", h_idx + 1);
                                let judges_json = extracted.get("judgePanel").and_then(|s| clean_array_to_json(s));
                                let _ = tx.execute(
                                    "INSERT INTO hearings (
                                        id, case_id, hearing_record, hearing_name, hearing_date,
                                        court, case_level, judges, contact_info, actual_status, created_at
                                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '未开', datetime('now', 'localtime'))",
                                    rusqlite::params![
                                        h_id,
                                        &new_case_id,
                                        final_case_name,
                                        h_name,
                                        h_date,
                                        extracted.get("court"),
                                        extracted.get("caseLevel"),
                                        judges_json,
                                        extracted.get("clerk"),
                                    ],
                                );
                            }
                            if !final_client_name.is_empty() && final_client_name != "待补充委托人" {
                                let _ = tx.execute(
                                    "INSERT OR IGNORE INTO clients (id, name, created_at, updated_at)
                                     VALUES (?1, ?2, datetime('now', 'localtime'), datetime('now', 'localtime'))",
                                    rusqlite::params![db::new_id(), final_client_name],
                                );
                            }

                            report.created_count += 1;
                            report.imported_case_ids.push(new_case_id);
                        }
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 条记录插入失败: {}", row_line, e));
                        }
                    }
                }
            }
        }

        tx.commit()?;
        Ok(report)
    })
    .await
}

/// 飞书多维表格关联分表批量导入 (任务分表 / 庭审分表 / 办案日志分表)
#[tauri::command]
pub async fn feishu_import_bitable_subtable(
    app_token: String,
    table_id: String,
    config: SubtableImportConfig,
) -> Result<SubtableImportReport, String> {
    let (client, token) = get_feishu_client_and_token().await.map_err(|e| e.to_string())?;

    // 1. 分页全量拉取记录
    let mut all_records = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut url = format!(
            "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/records?page_size=100",
            app_token, table_id
        );
        if let Some(pt) = &page_token {
            url.push_str(&format!("&page_token={}", pt));
        }

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("请求飞书记录列表网络异常: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("解析飞书记录 JSON 失败: {}", e))?;

        let code = body["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            let msg = body["msg"].as_str().unwrap_or("未知错误");
            return Err(format!("读取飞书多维表格记录失败: {} (code: {})", msg, code));
        }

        let items = body["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if items.is_empty() {
            break;
        }

        all_records.extend(items);

        let has_more = body["data"]["has_more"].as_bool().unwrap_or(false);
        if !has_more {
            break;
        }
        page_token = body["data"]["page_token"].as_str().map(|s| s.to_string());
    }

    // 2. 获取该表的字段列表以对齐 column_index
    let fields_url = format!(
        "https://open.feishu.cn/open-apis/bitable/v1/apps/{}/tables/{}/fields",
        app_token, table_id
    );
    let fields_resp = client
        .get(&fields_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("获取字段列表网络异常: {}", e))?;
    let fields_body: serde_json::Value = fields_resp
        .json()
        .await
        .map_err(|e| format!("解析字段列表 JSON 失败: {}", e))?;
    let field_names: Vec<String> = fields_body["data"]["items"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f["field_name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    run_blocking(move || {
        let mut report = SubtableImportReport {
            target_entity: config.target_entity.clone(),
            total_rows_processed: 0,
            created_count: 0,
            linked_cases_count: 0,
            unlinked_count: 0,
            failed_count: 0,
            errors: Vec::new(),
        };

        let mut conn = db::open_db().map_err(|e| anyhow!("打开数据库失败: {}", e))?;
        let tx = conn.transaction()?;

        let mut last_forward_fill_vals: HashMap<usize, String> = HashMap::new();

        for (rel_idx, record) in all_records.iter().enumerate() {
            let row_line = rel_idx + 1;
            let record_fields = &record["fields"];

            let mut extracted: HashMap<String, String> = HashMap::new();
            let mut row_is_empty = true;

            for (col_idx, field_name) in config.column_mappings.iter() {
                if let Some(feishu_header) = field_names.get(*col_idx) {
                    let raw_val = record_fields
                        .get(feishu_header)
                        .map(convert_feishu_val_to_string)
                        .unwrap_or_default();

                    let final_val = if raw_val.is_empty() {
                        if config.forward_fill_columns.contains(col_idx) {
                            last_forward_fill_vals.get(col_idx).cloned().unwrap_or_default()
                        } else {
                            String::new()
                        }
                    } else {
                        if config.forward_fill_columns.contains(col_idx) {
                            last_forward_fill_vals.insert(*col_idx, raw_val.clone());
                        }
                        raw_val
                    };

                    if !final_val.is_empty() {
                        row_is_empty = false;
                    }
                    extracted.insert(field_name.clone(), final_val);
                }
            }

            if row_is_empty {
                continue;
            }

            report.total_rows_processed += 1;

            let case_no = extracted.get("caseNo").map(|s| s.trim()).filter(|s| !s.is_empty());
            let case_name = extracted.get("caseName").map(|s| s.trim()).filter(|s| !s.is_empty());

            let matched_case_id: Option<String> = if let Some(no) = case_no {
                tx.query_row("SELECT id FROM cases WHERE case_no = ?1 LIMIT 1", rusqlite::params![no], |r| r.get(0)).ok()
            } else if let Some(name) = case_name {
                tx.query_row("SELECT id FROM cases WHERE case_name = ?1 LIMIT 1", rusqlite::params![name], |r| r.get(0)).ok()
            } else {
                None
            };

            if matched_case_id.is_some() {
                report.linked_cases_count += 1;
            } else {
                report.unlinked_count += 1;
            }

            match config.target_entity.as_str() {
                "tasks" => {
                    let task_name = extracted.get("taskName")
                        .or_else(|| extracted.get("name"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    if task_name.is_empty() {
                        continue;
                    }

                    let t_id = db::new_id();
                    let deadline = extracted.get("deadline").and_then(|s| clean_date_str(s));
                    let description = extracted.get("description").cloned();
                    let clean_priority = match extracted.get("priority").map(|s| s.trim()) {
                        Some("高") | Some("紧急") | Some("high") | Some("urgent_important") => "urgent_important",
                        Some("重要") | Some("important") => "important",
                        Some("次要") | Some("低") | Some("low") => "normal",
                        _ => "normal",
                    };
                    let created_date = chrono::Local::now().format("%Y-%m-%d").to_string();
                    let assignee = extracted.get("assignee").cloned();
                    let completed = match extracted.get("completed").map(|s| s.as_str()) {
                        Some("已完成") | Some("1") | Some("true") | Some("是") | Some("完成") => 1,
                        _ => 0,
                    };

                    let res = tx.execute(
                        "INSERT INTO tasks (
                            id, case_id, task_name, description, created_date, deadline, due_date, priority, completed, assignee, created_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8, ?9, datetime('now', 'localtime'))",
                        rusqlite::params![
                            t_id,
                            matched_case_id,
                            task_name,
                            description,
                            created_date,
                            deadline,
                            clean_priority,
                            completed,
                            assignee
                        ],
                    );

                    match res {
                        Ok(_) => report.created_count += 1,
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 条记录任务插入失败: {}", row_line, e));
                        }
                    }
                }
                "hearings" => {
                    let hearing_date = extracted.get("hearingDate")
                        .or_else(|| extracted.get("trialDate"))
                        .and_then(|s| clean_date_str(s));

                    if hearing_date.is_none() {
                        continue;
                    }

                    let h_id = db::new_id();
                    let hearing_name = extracted.get("hearingName").cloned().unwrap_or_else(|| "开庭/口审".to_string());
                    let court = extracted.get("court").cloned();
                    let case_level = extracted.get("caseLevel").cloned();
                    let judges_json = extracted.get("judgePanel").or_else(|| extracted.get("judges")).and_then(|s| clean_array_to_json(s));
                    let clerk = extracted.get("clerk").or_else(|| extracted.get("contactInfo")).cloned();
                    let actual_status = extracted.get("actualStatus").cloned();

                    let res = tx.execute(
                        "INSERT INTO hearings (
                            id, case_id, hearing_record, hearing_name, hearing_date,
                            court, case_level, judges, contact_info, actual_status, created_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now', 'localtime'))",
                        rusqlite::params![
                            h_id,
                            matched_case_id.unwrap_or_default(),
                            case_name.unwrap_or_default(),
                            hearing_name,
                            hearing_date,
                            court,
                            case_level,
                            judges_json,
                            clerk,
                            actual_status,
                        ],
                    );

                    match res {
                        Ok(_) => report.created_count += 1,
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 条记录开庭插入失败: {}", row_line, e));
                        }
                    }
                }
                "case_logs" => {
                    let content = extracted.get("content")
                        .or_else(|| extracted.get("notes"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    if content.is_empty() {
                        continue;
                    }

                    let l_id = db::new_id();
                    let event_date = extracted.get("eventDate").and_then(|s| clean_date_str(s)).unwrap_or_else(|| "2026-08-31".to_string());
                    let event_type = extracted.get("eventType").cloned().unwrap_or_else(|| "办案日志".to_string());
                    let operator = extracted.get("operator").cloned();

                    let res = tx.execute(
                        "INSERT INTO case_logs (
                            id, case_id, event_date, event_type, content, operator, created_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now', 'localtime'))",
                        rusqlite::params![
                            l_id,
                            matched_case_id.unwrap_or_default(),
                            event_date,
                            event_type,
                            content,
                            operator,
                        ],
                    );

                    match res {
                        Ok(_) => report.created_count += 1,
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 条记录日志插入失败: {}", row_line, e));
                        }
                    }
                }
                _ => {}
            }
        }

        tx.commit()?;
        Ok(report)
    })
    .await
}

// ============================================================
// 遗留的 Feishu Dump 导入兼容 (保留供老脚本使用)
// ============================================================

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct FeishuDump {
    tables: Option<FeishuTables>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct FeishuTables {
    cases: Option<FeishuTable>,
    #[allow(dead_code)]
    case_logs: Option<FeishuTable>,
    #[allow(dead_code)]
    hearings: Option<FeishuTable>,
    #[allow(dead_code)]
    tasks: Option<FeishuTable>,
    #[allow(dead_code)]
    officials: Option<FeishuTable>,
}

#[derive(Debug, Deserialize)]
struct FeishuTable {
    records: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub cases: usize,
    pub logs: usize,
    pub hearings: usize,
    pub tasks: usize,
    pub officials: usize,
    pub errors: Vec<String>,
}

pub fn import_feishu_dump(
    conn: &mut rusqlite::Connection,
    json_path: &Path,
) -> Result<ImportReport> {
    let data = std::fs::read_to_string(json_path)?;
    let dump: FeishuDump = serde_json::from_str(&data)?;

    let mut report = ImportReport {
        cases: 0,
        logs: 0,
        hearings: 0,
        tasks: 0,
        officials: 0,
        errors: Vec::new(),
    };

    let tables = match dump.tables {
        Some(t) => t,
        None => return Ok(report),
    };

    let tx = conn.transaction()?;

    if let Some(table) = tables.cases {
        for record in &table.records {
            let feishu_id = record["record_id"].as_str().unwrap_or_default();
            let fields = &record["fields"];
            let case_name = fields["案件信息"].as_str().unwrap_or("").trim();
            if !case_name.is_empty() {
                let _ = tx.execute(
                    "INSERT OR IGNORE INTO cases (id, track, case_name, client_name, opponent_name, created_at, updated_at)
                     VALUES (?1, 'civil_tort', ?2, ?3, ?4, datetime('now', 'localtime'), datetime('now', 'localtime'))",
                    rusqlite::params![
                        feishu_id,
                        case_name,
                        fields["客户名称"].as_str().unwrap_or(""),
                        fields["对方名称"].as_str().unwrap_or("")
                    ],
                );
                report.cases += 1;
            }
        }
    }

    tx.commit()?;
    Ok(report)
}

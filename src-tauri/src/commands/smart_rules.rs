//! Smart Rules（W5 · DEVONthink 式本地自动化）—— CRUD + 规则引擎
//!
//! 匹配语义：对 match_field（filename / ocr_text）做大小写不敏感的子串匹配。
//! 动作：
//!
//! - set_category: action_payload 写入 case_files.category（须为合法枚举）
//! - mark_urgent: 给案件写一条 urgent 通知（通知中心）
//! - add_keyword: 追加到 case_files.knowledge_keywords
//!
//! OCR 执行：提交持久文档队列，由本地 Rust 文档引擎处理。
use rusqlite::params;
use serde::Serialize;

use super::run_blocking;
use crate::db;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SmartRuleDto {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub match_field: String,
    pub match_pattern: String,
    pub action_type: String,
    pub action_payload: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SmartRuleApplyResult {
    pub file_id: String,
    pub matched_rules: Vec<String>,
    pub actions_applied: Vec<String>,
}

fn row_to_rule(row: &rusqlite::Row) -> rusqlite::Result<SmartRuleDto> {
    Ok(SmartRuleDto {
        id: row.get("id")?,
        name: row.get("name")?,
        enabled: row.get::<_, i32>("enabled")? != 0,
        match_field: row.get("match_field")?,
        match_pattern: row.get("match_pattern")?,
        action_type: row.get("action_type")?,
        action_payload: row.get("action_payload")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

const RULE_COLS: &str =
    "id, name, enabled, match_field, match_pattern, action_type, action_payload, created_at, updated_at";

const VALID_CATEGORIES: [&str; 7] = [
    "summons",
    "evidence",
    "submitted",
    "received",
    "internal",
    "correspondence",
    "other",
];

#[tauri::command]
pub async fn list_smart_rules() -> Result<Vec<SmartRuleDto>, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {RULE_COLS} FROM smart_rules ORDER BY created_at ASC"
        ))?;
        let rows = stmt.query_map([], row_to_rule)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

#[tauri::command]
pub async fn upsert_smart_rule(
    id: Option<String>,
    name: String,
    enabled: bool,
    match_field: String,
    match_pattern: String,
    action_type: String,
    action_payload: String,
) -> Result<String, String> {
    run_blocking(move || {
        if !["filename", "ocr_text"].contains(&match_field.as_str()) {
            return Err(anyhow::anyhow!("无效的匹配字段: {match_field}"));
        }
        if !["set_category", "mark_urgent", "add_keyword"].contains(&action_type.as_str()) {
            return Err(anyhow::anyhow!("无效的动作类型: {action_type}"));
        }
        if action_type == "set_category" && !VALID_CATEGORIES.contains(&action_payload.as_str()) {
            return Err(anyhow::anyhow!(
                "set_category 的载荷必须是合法分类: {}",
                VALID_CATEGORIES.join("/")
            ));
        }
        if match_pattern.trim().is_empty() {
            return Err(anyhow::anyhow!("匹配模式不能为空"));
        }
        let conn = db::open_db()?;
        match &id {
            Some(rid) => {
                let n = conn.execute(
                    "UPDATE smart_rules SET name=?2, enabled=?3, match_field=?4, match_pattern=?5,
                     action_type=?6, action_payload=?7, updated_at=datetime('now','localtime')
                     WHERE id=?1",
                    params![rid, name, if enabled { 1 } else { 0 }, match_field,
                            match_pattern, action_type, action_payload],
                )?;
                if n == 0 {
                    return Err(anyhow::anyhow!("规则不存在: {rid}"));
                }
                Ok(rid.clone())
            }
            None => {
                let new_id = db::new_id();
                conn.execute(
                    "INSERT INTO smart_rules (id, name, enabled, match_field, match_pattern, action_type, action_payload)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![new_id, name, if enabled { 1 } else { 0 }, match_field,
                            match_pattern, action_type, action_payload],
                )?;
                Ok(new_id)
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn delete_smart_rule(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM smart_rules WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}

/// 对单个文件执行全部启用规则（文件登记 / OCR 完成时自动调用，也可手动触发）
#[tauri::command]
pub async fn apply_smart_rules(file_id: String) -> Result<SmartRuleApplyResult, String> {
    run_blocking(move || apply_rules_inner(&file_id)).await
}

/// 对全部已登记文件批量执行规则
#[tauri::command]
pub async fn run_smart_rules_for_all() -> Result<i64, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare("SELECT id FROM case_files WHERE deleted_at IS NULL")?;
        let ids: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        drop(conn);
        let mut applied = 0i64;
        let mut failures = Vec::new();
        for id in ids {
            // 单文件失败不中止整批（与 ocr_all_pending 容错语义一致）
            match apply_rules_inner(&id) {
                Ok(res) => {
                    if !res.matched_rules.is_empty() {
                        applied += 1;
                    }
                }
                Err(e) => failures.push(format!("{id}: {e}")),
            }
        }
        if !failures.is_empty() {
            anyhow::bail!("批量规则部分失败：已匹配 {} 个文件，失败 {} 个。成功修改已保留，请核对后重试。{}", applied, failures.len(), failures.join("; "));
        }
        Ok(applied)
    })
    .await
}

pub(crate) fn apply_rules_inner(file_id: &str) -> Result<SmartRuleApplyResult, anyhow::Error> {
    let mut connection = db::open_db()?;
    let conn = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let (file_name, ocr_text, case_id): (String, Option<String>, String) = conn.query_row(
        "SELECT file_name, ocr_text, case_id FROM case_files WHERE id = ?1 AND deleted_at IS NULL",
        params![file_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {RULE_COLS} FROM smart_rules WHERE enabled = 1 ORDER BY created_at ASC"
    ))?;
    let rules: Vec<SmartRuleDto> = stmt
        .query_map([], row_to_rule)?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    let mut matched = Vec::new();
    let mut applied = Vec::new();
    for rule in rules {
        let haystack = match rule.match_field.as_str() {
            "filename" => file_name.to_lowercase(),
            "ocr_text" => ocr_text.clone().unwrap_or_default().to_lowercase(),
            _ => continue,
        };
        if haystack.contains(&rule.match_pattern.to_lowercase()) {
            matched.push(rule.name.clone());
            match rule.action_type.as_str() {
                "set_category" => {
                    conn.execute(
                        "UPDATE case_files SET category = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
                        params![file_id, rule.action_payload],
                    )?;
                    applied.push(format!("分类 → {}", rule.action_payload));
                }
                "mark_urgent" => {
                    // 幂等：同文件同规则存在未处理通知则跳过（dismiss 后允许再次提醒）
                    let payload =
                        serde_json::json!({"caseId": case_id, "fileId": file_id, "rule": rule.name}).to_string();
                    let existing: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM notifications
                         WHERE dismissed_at IS NULL AND type = 'smart_rule'
                           AND payload_json LIKE ?1",
                        params![format!("%\"fileId\":\"{file_id}\"%")],
                        |r| r.get(0),
                    )?;
                    if existing == 0 {
                        conn.execute(
                            "INSERT INTO notifications (id, type, title, body, payload_json)
                             VALUES (?1, 'smart_rule', ?2, ?3, ?4)",
                            params![
                                db::new_id(),
                                format!("⚡ 紧急文件命中规则「{}」", rule.name),
                                format!("文件 {} 匹配「{}」", file_name, rule.match_pattern),
                                payload
                            ],
                        )?;
                        applied.push("已推送紧急通知".to_string());
                    }
                }
                "add_keyword" => {
                    let existing: Option<String> = conn
                        .query_row(
                            "SELECT knowledge_keywords FROM case_files WHERE id = ?1",
                            params![file_id],
                            |r| r.get(0),
                        )?;
                    let mut kws: Vec<String> = existing
                        .map(|s| {
                            s.split(',')
                                .map(|x| x.trim().to_string())
                                .filter(|x| !x.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();
                    if !kws.contains(&rule.action_payload) {
                        kws.push(rule.action_payload.clone());
                        conn.execute(
                            "UPDATE case_files SET knowledge_keywords = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
                            params![file_id, kws.join(",")],
                        )?;
                    }
                    applied.push(format!("关键词 +{}", rule.action_payload));
                }
                _ => {}
            }
        }
    }
    conn.commit()?;
    Ok(SmartRuleApplyResult {
        file_id: file_id.to_string(),
        matched_rules: matched,
        actions_applied: applied,
    })
}

/// 列出待 OCR 的扫描件（pdf_extractor 标记 NeedsOCR / ocr_status = 'pending'）
#[tauri::command]
pub async fn list_pending_ocr_files() -> Result<Vec<(String, String, String)>, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, file_name, case_id FROM case_files
             WHERE deleted_at IS NULL AND ocr_status = 'pending' AND (file_type LIKE '%pdf%' OR file_name LIKE '%.pdf'
                   OR file_name LIKE '%.png' OR file_name LIKE '%.jpg' OR file_name LIKE '%.jpeg')
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

// OCR entry points share the durable, single-worker document queue.

/// 是否 OCR 候选文件（pdf / 常见图片）
fn is_ocr_candidate(file_name: &str, file_type: Option<&str>) -> bool {
    if crate::document_pipeline::supports_path(std::path::Path::new(file_name)) { return true; }
    let ext = file_type
        .map(|s| s.trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::path::Path::new(file_name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
        });
    matches!(
        ext.as_deref(),
        Some("pdf" | "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" | "gif")
    )
}

/// 登记自动接线（供 commands/files.rs 调用）：
/// pdf/图片登记成功后立即套用 Smart Rules（filename 类规则即时生效）。
/// 失败仅记录日志，绝不影响文件登记主流程。
pub fn auto_rules_on_register(file_id: &str, file_name: &str, file_type: Option<&str>) {
    if !is_ocr_candidate(file_name, file_type) {
        return;
    }
    if let Err(e) = apply_rules_inner(file_id) {
        log::warn!("smart rules on register failed for {file_id}: {e}");
    }
}

#[tauri::command]
pub async fn ocr_case_file(file_id: String) -> Result<String, String> {
    let job = super::document_intelligence::queue_document_processing(file_id).await?;
    Ok(if job.status == "completed" {
        "OCR 完成，已复用本地识别结果".into()
    } else {
        "已加入本地文档处理队列".into()
    })
}

/// Return newly queued files; failed and cancelled jobs require an explicit retry.
#[tauri::command]
pub async fn ocr_all_pending() -> Result<i64, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let candidates = {
            let mut stmt = conn.prepare(
                "SELECT id,file_name,file_type FROM case_files f
                 WHERE deleted_at IS NULL AND ocr_status='pending' AND NOT EXISTS
                 (SELECT 1 FROM document_processing_jobs j WHERE j.file_id=f.id)
                 AND (lower(file_path) LIKE '%.md' OR lower(file_path) LIKE '%.markdown'
                   OR lower(file_path) LIKE '%.txt' OR lower(file_path) LIKE '%.doc'
                   OR lower(file_path) LIKE '%.docx' OR lower(file_path) LIKE '%.docm'
                   OR lower(file_path) LIKE '%.rtf' OR lower(file_path) LIKE '%.odt'
                   OR lower(file_path) LIKE '%.pdf' OR lower(file_path) LIKE '%.png'
                   OR lower(file_path) LIKE '%.jpg' OR lower(file_path) LIKE '%.jpeg'
                   OR lower(file_path) LIKE '%.tif' OR lower(file_path) LIKE '%.tiff'
                   OR lower(file_path) LIKE '%.bmp' OR lower(file_path) LIKE '%.webp'
                   OR lower(file_path) LIKE '%.gif')
                 ORDER BY created_at LIMIT 32",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let mut queued = 0;
        for (id, name, kind) in candidates {
            if !is_ocr_candidate(&name, kind.as_deref()) {
                continue;
            }
            match super::document_intelligence::queue_file(&mut *db::open_db()?, &id) {
                Ok(_) => queued += 1,
                Err(error) => {
                    conn.execute(
                        "UPDATE case_files SET ocr_status='failed',ocr_error=?2 WHERE id=?1",
                        params![id, error.to_string()],
                    )?;
                    log::warn!("auto OCR queue failed for {id}: {error}");
                }
            }
        }
        Ok(queued)
    })
    .await
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileOcrStateDto {
    pub file_id: String,
    pub ocr_status: String,
    pub has_text: bool,
}

/// 查询案件全部文件的 OCR 状态（文件列表徽标用；不搬运 ocr_text 正文）
#[tauri::command]
pub async fn list_case_ocr_states(case_id: String) -> Result<Vec<FileOcrStateDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, ocr_status, (ocr_text IS NOT NULL AND LENGTH(ocr_text) > 0)
             FROM case_files WHERE case_id = ?1 AND deleted_at IS NULL",
        )?;
        let rows = stmt.query_map(params![case_id], |r| {
            Ok(FileOcrStateDto {
                file_id: r.get(0)?,
                ocr_status: r
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| "pending".to_string()),
                has_text: r.get::<_, i32>(2)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

/// 读取单文件的 OCR 文本（「查看文本」弹窗用）
#[tauri::command]
pub async fn get_file_ocr_text(file_id: String) -> Result<Option<String>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let text = conn
            .query_row(
                "SELECT ocr_text FROM case_files WHERE id = ?1 AND deleted_at IS NULL",
                params![file_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        Ok(text)
    })
    .await
}

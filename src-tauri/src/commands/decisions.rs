//! 决策记录命令（设计哲学 §11.6：推荐引擎的决策留痕）

use super::run_blocking;
use crate::db;

/// 记录一条决策
/// decision_type 支持 recommend_today / recommend_priority / recommend_estimate 等（见 decisions 表 CHECK）
/// review_due 为可选复核日期（ISO YYYY-MM-DD），到期后由决策复核调度主动提醒（设计哲学 §11.7）
#[tauri::command]
pub async fn record_decision(
    entity_type: String,
    entity_id: String,
    decision_type: String,
    decision: String,
    basis: Option<String>,
    source_ref: Option<String>,
    status: Option<String>,
    review_due: Option<String>,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let status = status.unwrap_or_else(|| "proposed".to_string());

        conn.execute(
            "INSERT INTO decisions (id, entity_type, entity_id, decision_type, decision, basis, source_ref, status, review_due)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id,
                entity_type,
                entity_id,
                decision_type,
                decision,
                basis,
                source_ref,
                status,
                review_due,
            ],
        )?;

        Ok(serde_json::json!({ "id": id }))
    })
    .await
}

/// 查询到期待复核决策（设计哲学 §11.7）：
/// review_due <= 今天 且 status='confirmed' 且尚未 reviewed
/// 供 get_pending_decision_reviews 命令与 lib.rs 每日 08:30 调度共用
pub fn pending_decision_reviews(
    conn: &rusqlite::Connection,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let today = db::today();
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, entity_id, decision_type, decision, basis, review_due, created_at
         FROM decisions
         WHERE status = 'confirmed' AND reviewed_at IS NULL
           AND review_due IS NOT NULL AND review_due != '' AND review_due <= ?1
         ORDER BY review_due ASC",
    )?;

    let rows = stmt
        .query_map(rusqlite::params![today], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "entityType": row.get::<_, String>(1)?,
                "entityId": row.get::<_, String>(2)?,
                "decisionType": row.get::<_, String>(3)?,
                "decision": row.get::<_, String>(4)?,
                "basis": row.get::<_, Option<String>>(5)?,
                "reviewDue": row.get::<_, Option<String>>(6)?,
                "createdAt": row.get::<_, Option<String>>(7)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// 获取到期待复核决策列表（设计哲学 §11.7："该决策仍有效吗？"）
#[tauri::command]
pub async fn get_pending_decision_reviews() -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        pending_decision_reviews(&conn)
    })
    .await
}

/// 复核决策（设计哲学 §11.7）
/// still_valid=true → 仅写 reviewed_at；false → status='voided'，可附复核说明（追加到 basis）
#[tauri::command]
pub async fn mark_decision_reviewed(
    id: String,
    still_valid: bool,
    note: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        if still_valid {
            conn.execute(
                "UPDATE decisions SET reviewed_at = datetime('now','localtime') WHERE id = ?1",
                rusqlite::params![id],
            )?;
        } else {
            match note.filter(|n| !n.trim().is_empty()) {
                Some(n) => {
                    let note_line = format!("复核作废说明: {}", n);
                    conn.execute(
                        "UPDATE decisions SET status = 'voided', reviewed_at = datetime('now','localtime'),
                           basis = CASE WHEN basis IS NULL OR basis = '' THEN ?2
                                        ELSE basis || char(10) || ?2 END
                         WHERE id = ?1",
                        rusqlite::params![id, note_line],
                    )?;
                }
                None => {
                    conn.execute(
                        "UPDATE decisions SET status = 'voided', reviewed_at = datetime('now','localtime')
                         WHERE id = ?1",
                        rusqlite::params![id],
                    )?;
                }
            }
        }

        Ok(())
    })
    .await
}

/// 查询决策记录
#[tauri::command]
pub async fn list_decisions(
    entity_type: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let limit = limit.unwrap_or(50).clamp(1, 500);

        let mut sql = String::from(
            "SELECT id, entity_type, entity_id, decision_type, decision, basis, ai_advice,
                    ai_model, source_ref, status, created_at, updated_at
             FROM decisions WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(et) = &entity_type {
            if !et.is_empty() {
                sql.push_str(&format!(" AND entity_type = ?{}", idx));
                params.push(Box::new(et.clone()));
                idx += 1;
            }
        }
        if let Some(st) = &status {
            if !st.is_empty() {
                sql.push_str(&format!(" AND status = ?{}", idx));
                params.push(Box::new(st.clone()));
                idx += 1;
            }
        }
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{}", idx));
        params.push(Box::new(limit));

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "entityType": row.get::<_, String>(1)?,
                    "entityId": row.get::<_, String>(2)?,
                    "decisionType": row.get::<_, String>(3)?,
                    "decision": row.get::<_, String>(4)?,
                    "basis": row.get::<_, Option<String>>(5)?,
                    "aiAdvice": row.get::<_, Option<String>>(6)?,
                    "aiModel": row.get::<_, Option<String>>(7)?,
                    "sourceRef": row.get::<_, Option<String>>(8)?,
                    "status": row.get::<_, String>(9)?,
                    "createdAt": row.get::<_, Option<String>>(10)?,
                    "updatedAt": row.get::<_, Option<String>>(11)?,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    })
    .await
}

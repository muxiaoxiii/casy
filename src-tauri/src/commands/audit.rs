//! 实体变更留痕查询（A1：开庭改期链对用户可见）
//!
//! `audit_events` 由数据库触发器写入（开庭改期/白板事实/AI 工具执行等），
//! 此前只有写入没有读取入口，用户看不到"这个开庭日期为什么变了"。

use super::run_blocking;
use crate::db;
use rusqlite::params;

/// 单条变更记录
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityAuditEventDto {
    pub id: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    /// 触发器写入的 JSON：before/after/reason 等
    pub payload: Option<serde_json::Value>,
    /// user / ai / system / mcp / skill
    pub actor: String,
    pub created_at: String,
}

/// 读取变更留痕（倒序）。
///
/// `case_id` 为空时跨全部案件读取（A3 操作记录视图用）；`event_type` 可按
/// `hearing_updated` / `whiteboard_fact_*` / AI 工具执行等过滤。
#[tauri::command]
pub async fn list_entity_audit_events(
    case_id: Option<String>,
    event_type: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<EntityAuditEventDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let limit = limit.unwrap_or(50).clamp(1, 500);
        let case_id = case_id.filter(|value| !value.trim().is_empty());
        let kind = event_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let mut conditions = vec!["aggregate_type = 'case'".to_string()];
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(id) = case_id {
            conditions.push(format!("aggregate_id = ?{}", params.len() + 1));
            params.push(Box::new(id));
        }
        if let Some(kind) = kind {
            conditions.push(format!("event_type = ?{}", params.len() + 1));
            params.push(Box::new(kind));
        }
        conditions.push(format!("1 = ?{}", params.len() + 1));
        params.push(Box::new(1));
        let sql = format!(
            "SELECT id, aggregate_type, aggregate_id, event_type, payload, actor, created_at
             FROM audit_events WHERE {} ORDER BY created_at DESC LIMIT ?{}",
            conditions.join(" AND "),
            params.len() + 1
        );
        params.push(Box::new(limit));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let payload: Option<String> = row.get(4)?;
                Ok(EntityAuditEventDto {
                    id: row.get(0)?,
                    aggregate_type: row.get(1)?,
                    aggregate_id: row.get(2)?,
                    event_type: row.get(3)?,
                    payload: payload.and_then(|text| serde_json::from_str(&text).ok()),
                    actor: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    })
    .await
}

#[cfg(test)]
mod tests {
    fn test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        conn
    }

    #[test]
    fn hearing_reschedule_is_queryable_as_an_audit_chain() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO cases(id, case_name, client_name) VALUES('case-1','测试案件','测试客户')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO hearings(id, case_id, hearing_record, hearing_name, hearing_date, lifecycle_status) VALUES('h-1','case-1','第一次开庭','第一次开庭','2026-10-20 09:00:00','scheduled')",
            [],
        )
        .unwrap();
        // 改期：触发器应写入 before/after/reason
        conn.execute(
            "UPDATE hearings SET hearing_date='2026-11-05 14:00:00', change_reason='（2026）鲁02民初664号 传票改期' WHERE id='h-1'",
            [],
        )
        .unwrap();

        let mut stmt = conn
            .prepare("SELECT payload FROM audit_events WHERE aggregate_id='case-1' AND event_type='hearing_updated'")
            .unwrap();
        let payloads: Vec<String> = stmt
            .query_map([], |r| r.get::<_, Option<String>>(0))
            .unwrap()
            .map(|value| value.unwrap().unwrap_or_default())
            .collect();
        assert_eq!(payloads.len(), 1, "改期应产生一条留痕");
        let payload: serde_json::Value = serde_json::from_str(&payloads[0]).unwrap();
        assert_eq!(payload["before"]["date"], "2026-10-20 09:00:00");
        assert_eq!(payload["after"]["date"], "2026-11-05 14:00:00");
        assert!(payload["reason"].as_str().unwrap().contains("改期"));
    }
}

/// 某次开庭的一条提醒作业（H3：让用户看到"T-7 已发 / T-1 已发 / 已取消"）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HearingReminderJobDto {
    pub id: String,
    pub hearing_id: String,
    /// local / system / calendar / email_ics / feishu_message / feishu_task
    pub channel: String,
    /// 计划触发时间
    pub scheduled_at: String,
    /// 触发时的偏移快照（如 "开庭前 7 天"）
    pub offset_snapshot: Option<String>,
    /// pending / synced / sent / delivered / read / sync_failed /
    /// delivery_unknown / cancelled / dead_lettered
    pub status: String,
    pub last_error: Option<String>,
    /// 改期后被新作业取代时指向新作业
    pub supersedes_id: Option<String>,
}

/// 读取某案件全部开庭的提醒作业（倒序）。
#[tauri::command]
pub async fn list_hearing_reminder_jobs(case_id: String) -> Result<Vec<HearingReminderJobDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT j.id, j.entity_id, j.channel, j.scheduled_at, j.offset_snapshot, j.status, j.last_error, j.supersedes_id
             FROM reminder_jobs j
             JOIN hearings h ON h.id = j.entity_id
             WHERE j.entity_type = 'hearing' AND h.case_id = ?1
             ORDER BY j.scheduled_at DESC LIMIT 200",
        )?;
        let rows = stmt
            .query_map(params![case_id], |row| {
                Ok(HearingReminderJobDto {
                    id: row.get(0)?,
                    hearing_id: row.get(1)?,
                    channel: row.get(2)?,
                    scheduled_at: row.get(3)?,
                    offset_snapshot: row.get(4)?,
                    status: row.get(5)?,
                    last_error: row.get(6)?,
                    supersedes_id: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    })
    .await
}

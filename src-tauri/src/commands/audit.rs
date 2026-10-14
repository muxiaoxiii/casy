//! 实体变更留痕查询（A1：开庭改期链对用户可见）
//!
//! `audit_events` 由数据库触发器写入（开庭改期/白板事实/AI 工具执行等），
//! 此前只有写入没有读取入口，用户看不到"这个开庭日期为什么变了"。

use super::run_blocking;
use crate::db;

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

/// 按案件读取变更留痕（倒序，可在分页上限内取最近若干条）。
#[tauri::command]
pub async fn list_entity_audit_events(
    case_id: String,
    event_type: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<EntityAuditEventDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let limit = limit.unwrap_or(50).clamp(1, 200);
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match event_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(kind) => (
                "SELECT id, aggregate_type, aggregate_id, event_type, payload, actor, created_at
                 FROM audit_events WHERE aggregate_type = 'case' AND aggregate_id = ?1 AND event_type = ?2
                 ORDER BY created_at DESC LIMIT ?3"
                    .to_string(),
                vec![
                    Box::new(case_id) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(kind.to_string()),
                    Box::new(limit),
                ],
            ),
            None => (
                "SELECT id, aggregate_type, aggregate_id, event_type, payload, actor, created_at
                 FROM audit_events WHERE aggregate_type = 'case' AND aggregate_id = ?1
                 ORDER BY created_at DESC LIMIT ?2"
                    .to_string(),
                vec![
                    Box::new(case_id) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(limit),
                ],
            ),
        };
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

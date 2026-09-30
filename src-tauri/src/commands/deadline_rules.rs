//! 期限规则自定义 CRUD + 变更留痕（W3 · LawToolBox 式可审计规则引擎）
//!
//! 每次 create/update/toggle/delete 均写 deadline_rule_audit；
//! 写操作后由 deadline::engine 对受影响案件重算期限。
use rusqlite::params;
use serde::Serialize;

use super::run_blocking;
use crate::db;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineRuleDto {
    pub id: String,
    pub track: String,
    pub rule_name: String,
    pub legal_basis: String,
    pub trigger_field: String,
    pub offset_value: i64,
    pub offset_unit: String,
    pub calc_method: String,
    pub procedure_types: Option<String>,
    pub deadline_source: String,
    pub auto_calculate: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineRuleAuditDto {
    pub id: String,
    pub rule_id: String,
    pub action: String,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub actor: String,
    pub created_at: Option<String>,
}

fn row_to_rule(row: &rusqlite::Row) -> rusqlite::Result<DeadlineRuleDto> {
    Ok(DeadlineRuleDto {
        id: row.get("id")?,
        track: row.get("track")?,
        rule_name: row.get("rule_name")?,
        legal_basis: row.get("legal_basis")?,
        trigger_field: row.get("trigger_field")?,
        offset_value: row.get("offset_value")?,
        offset_unit: row.get("offset_unit")?,
        calc_method: row.get("calc_method")?,
        procedure_types: row.get("procedure_types")?,
        deadline_source: row.get("deadline_source")?,
        auto_calculate: row.get::<_, i32>("auto_calculate")? != 0,
        priority: row.get("priority")?,
    })
}

const RULE_COLS: &str = "id, track, rule_name, legal_basis, trigger_field, offset_value,
     offset_unit, calc_method, procedure_types, deadline_source, auto_calculate, priority";

fn get_rule(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<DeadlineRuleDto>, anyhow::Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {RULE_COLS} FROM deadline_rules WHERE id = ?1"
    ))?;
    let mut rows = stmt.query_map(params![id], row_to_rule)?;
    Ok(rows.next().transpose()?)
}

fn write_audit(
    conn: &rusqlite::Connection,
    rule_id: &str,
    action: &str,
    before: Option<&DeadlineRuleDto>,
    after: Option<&DeadlineRuleDto>,
) -> Result<(), anyhow::Error> {
    let to_json = |r: Option<&DeadlineRuleDto>| -> Option<String> {
        r.map(|r| {
            serde_json::json!({
                "id": r.id, "track": r.track, "ruleName": r.rule_name,
                "legalBasis": r.legal_basis, "triggerField": r.trigger_field,
                "offsetValue": r.offset_value, "offsetUnit": r.offset_unit,
                "calcMethod": r.calc_method, "procedureTypes": r.procedure_types,
                "deadlineSource": r.deadline_source, "autoCalculate": r.auto_calculate,
                "priority": r.priority,
            })
            .to_string()
        })
    };
    conn.execute(
        "INSERT INTO deadline_rule_audit (id, rule_id, action, before_json, after_json, actor)
         VALUES (?1, ?2, ?3, ?4, ?5, 'user')",
        params![
            db::new_id(),
            rule_id,
            action,
            to_json(before),
            to_json(after)
        ],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn list_deadline_rules() -> Result<Vec<DeadlineRuleDto>, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {RULE_COLS} FROM deadline_rules ORDER BY track, priority DESC"
        ))?;
        let rows = stmt.query_map([], row_to_rule)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

/// 新增或更新规则（id 为空 → 新建）；写审计留痕
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn upsert_deadline_rule(
    id: Option<String>,
    track: String,
    rule_name: String,
    legal_basis: String,
    trigger_field: String,
    offset_value: i64,
    offset_unit: String,
    calc_method: String,
    procedure_types: Option<String>,
    deadline_source: String,
    priority: i32,
) -> Result<String, String> {
    run_blocking(move || {
        let mut raw = db::open_db()?;
        let conn = raw.transaction()?;
        let valid_units = ["day", "calendar_month"];
        if !valid_units.contains(&offset_unit.as_str()) {
            return Err(anyhow::anyhow!("无效的偏移单位: {offset_unit}"));
        }
        match &id {
            Some(rid) => {
                let before =
                    get_rule(&conn, rid)?.ok_or_else(|| anyhow::anyhow!("规则不存在: {rid}"))?;
                conn.execute(
                    "UPDATE deadline_rules SET track=?2, rule_name=?3, legal_basis=?4,
                     trigger_field=?5, offset_value=?6, offset_unit=?7, calc_method=?8,
                     procedure_types=?9, deadline_source=?10, priority=?11 WHERE id=?1",
                    params![
                        rid,
                        track,
                        rule_name,
                        legal_basis,
                        trigger_field,
                        offset_value,
                        offset_unit,
                        calc_method,
                        procedure_types,
                        deadline_source,
                        priority
                    ],
                )?;
                let after = get_rule(&conn, rid)?;
                write_audit(&conn, rid, "update", Some(&before), after.as_ref())?;
                // 规则变更即重算（新旧 track 都重算，防止漏调导致期限静默过期）
                crate::deadline::recalc::recalc_track_inner(&conn, &track)?;
                if before.track != track {
                    crate::deadline::recalc::recalc_track_inner(&conn, &before.track)?;
                }
                conn.commit()?;
                Ok(rid.clone())
            }
            None => {
                let new_id = db::new_id();
                conn.execute(
                    "INSERT INTO deadline_rules (id, track, rule_name, legal_basis, trigger_field,
                     offset_value, offset_unit, calc_method, procedure_types, deadline_source,
                     auto_calculate, priority)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
                    params![
                        new_id,
                        track,
                        rule_name,
                        legal_basis,
                        trigger_field,
                        offset_value,
                        offset_unit,
                        calc_method,
                        procedure_types,
                        deadline_source,
                        priority
                    ],
                )?;
                let after = get_rule(&conn, &new_id)?;
                write_audit(&conn, &new_id, "create", None, after.as_ref())?;
                crate::deadline::recalc::recalc_track_inner(&conn, &track)?;
                conn.commit()?;
                Ok(new_id)
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn toggle_deadline_rule(id: String, enabled: bool) -> Result<(), String> {
    run_blocking(move || {
        let mut raw = db::open_db()?;
        let conn = raw.transaction()?;
        let before = get_rule(&conn, &id)?.ok_or_else(|| anyhow::anyhow!("规则不存在: {id}"))?;
        conn.execute(
            "UPDATE deadline_rules SET auto_calculate = ?2 WHERE id = ?1",
            params![id, if enabled { 1 } else { 0 }],
        )?;
        let after = get_rule(&conn, &id)?;
        write_audit(&conn, &id, "toggle", Some(&before), after.as_ref())?;
        crate::deadline::recalc::recalc_track_inner(&conn, &before.track)?;
        conn.commit()?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_deadline_rule(id: String) -> Result<(), String> {
    run_blocking(move || {
        let mut raw = db::open_db()?;
        let conn = raw.transaction()?;
        let before = get_rule(&conn, &id)?.ok_or_else(|| anyhow::anyhow!("规则不存在: {id}"))?;
        // case_deadlines.rule_id 无 ON DELETE，先解绑避免 RESTRICT 中断删除。
        conn.execute(
            "UPDATE case_deadlines SET rule_id = NULL WHERE rule_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM deadline_rules WHERE id = ?1", params![id])?;
        write_audit(&conn, &id, "delete", Some(&before), None)?;
        crate::deadline::recalc::recalc_track_inner(&conn, &before.track)?;
        conn.commit()?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_deadline_rule_audit(
    rule_id: Option<String>,
) -> Result<Vec<DeadlineRuleAuditDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let (sql, id_param): (String, Option<String>) = match rule_id {
            Some(rid) => (
                "SELECT id, rule_id, action, before_json, after_json, actor, created_at
                 FROM deadline_rule_audit WHERE rule_id = ?1 ORDER BY created_at DESC"
                    .to_string(),
                Some(rid),
            ),
            None => (
                "SELECT id, rule_id, action, before_json, after_json, actor, created_at
                 FROM deadline_rule_audit ORDER BY created_at DESC LIMIT 200"
                    .to_string(),
                None,
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let map = |row: &rusqlite::Row| -> rusqlite::Result<DeadlineRuleAuditDto> {
            Ok(DeadlineRuleAuditDto {
                id: row.get("id")?,
                rule_id: row.get("rule_id")?,
                action: row.get("action")?,
                before_json: row.get("before_json")?,
                after_json: row.get("after_json")?,
                actor: row.get("actor")?,
                created_at: row.get("created_at")?,
            })
        };
        let rows = match id_param {
            Some(rid) => stmt.query_map(params![rid], map)?,
            None => stmt.query_map([], map)?,
        };
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

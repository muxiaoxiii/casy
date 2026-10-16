//! 待确认建议与冲突确认（UPDATE_PLAN 0.2.0 Phase 1 第三步）
//!
//! 文书抽取出的开庭/期限事实**不直接建日程**，而是生成待确认建议；命中冲突时
//! 必须由用户裁决。铁律：低于置信度阈值只进待确认，绝不自动建日程——日程建错
//! 的代价高于漏建。
//!
//! C1 冲突（同案已有开庭且日期不同）是用户点名的核心交互：
//! 「先后下载了两个传票，指向同一个案件，但开庭不同——是真的两个开庭，还是改期了？」
//! 三种裁决：两个开庭都建 / 改期（旧排期标记 rescheduled 并留痕）/ 都不是开庭。

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use specta::Type;

/// 低于该置信度的建议只进待确认，不参与任何自动路径。
pub const AUTO_CONFIRM_THRESHOLD: f64 = 0.95;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntakeProposal {
    pub id: String,
    pub ref_type: String,
    pub ref_id: String,
    /// create_hearing / create_deadline / link_case
    pub kind: String,
    pub payload: serde_json::Value,
    /// 支撑该建议的事实（含页码出处），前端必须展示
    pub evidence: Vec<serde_json::Value>,
    pub confidence: f64,
    /// 命中冲突时为 Some；前端据此渲染三选一
    pub conflict: Option<ConflictCase>,
    /// pending / confirmed / rejected / superseded / expired
    pub status: String,
    pub resolution: Option<serde_json::Value>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// 冲突用例。`kind` 决定前端渲染哪些选项。
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictCase {
    /// same_case_hearing_date / same_day_collision / duplicate_document /
    /// uncertain_case / doubtful_doc_type
    pub kind: String,
    /// 既有实体摘要（前端展示"已存在的排期"）
    pub existing: Vec<serde_json::Value>,
    /// 建议的新值
    pub proposed: serde_json::Value,
    /// 用户可选的裁决（前端按此渲染按钮）
    pub options: Vec<ConflictOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictOption {
    pub value: String,
    pub label: String,
    /// true 表示该选项会写库（其余为"不改动"类）
    pub writes: bool,
}

/// C1 的三个选项（同案开庭日期不一致）。
pub fn c1_options() -> Vec<ConflictOption> {
    vec![
        ConflictOption { value: "two_hearings".into(), label: "两个开庭都建立".into(), writes: true },
        ConflictOption { value: "reschedule".into(), label: "改期：旧排期标记改期并留痕".into(), writes: true },
        ConflictOption { value: "not_hearing".into(), label: "都不是开庭（文书类型误判）".into(), writes: false },
    ]
}

/// C1：同案已有开庭且日期与建议不同。
fn detect_c1(
    conn: &rusqlite::Connection,
    case_id: &str,
    proposed_date: &str,
) -> anyhow::Result<Option<ConflictCase>> {
    // 只看未取消、未开过的排期（已取消/已开的不构成冲突）
    let mut stmt = conn.prepare(
        "SELECT id, hearing_name, hearing_date, venue, court FROM hearings
         WHERE case_id = ?1 AND lifecycle_status = 'scheduled'
         ORDER BY hearing_date ASC LIMIT 20",
    )?;
    let existing: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![case_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, Option<String>>(1)?,
                "date": row.get::<_, String>(2)?,
                "venue": row.get::<_, Option<String>>(3)?,
                "court": row.get::<_, Option<String>>(4)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // 同日同学期不算冲突（同一次开庭的不同来源）
    let conflicts = existing
        .iter()
        .any(|item| item["date"].as_str().unwrap_or("").starts_with(proposed_date));
    if conflicts {
        return Ok(None);
    }
    if existing.is_empty() {
        return Ok(None);
    }
    Ok(Some(ConflictCase {
        kind: "same_case_hearing_date".into(),
        existing,
        proposed: serde_json::json!({ "date": proposed_date }),
        options: c1_options(),
    }))
}

/// 取建议关联的案件。case_file 走 case_files.case_id；inbox_item 走 linked_case_id。
fn resolve_case_id(conn: &rusqlite::Connection, ref_type: &str, ref_id: &str) -> anyhow::Result<Option<String>> {
    let sql = match ref_type {
        "case_file" => "SELECT case_id FROM case_files WHERE id=?1 AND deleted_at IS NULL",
        "inbox_item" => "SELECT linked_case_id FROM inbox_items WHERE id=?1",
        _ => anyhow::bail!("不支持的引用类型: {ref_type}"),
    };
    Ok(conn.query_row(sql, [ref_id], |row| row.get::<_, Option<String>>(0)).optional()?.flatten())
}

/// 从已抽取的事实生成建议（幂等：同一 ref 重复生成会先作废旧 pending 建议）。
///
/// 只有开庭事实会生成 create_hearing 建议；期限事实留待 Phase 3 统一入口。
pub fn generate_proposals(
    conn: &rusqlite::Connection,
    ref_type: &str,
    ref_id: &str,
) -> anyhow::Result<Vec<IntakeProposal>> {
    let facts = super::document_facts::read_facts_row(conn, ref_type, ref_id)
        .map_err(|_| anyhow::anyhow!("尚未抽取事实，请先 extract_document_facts"))?;
    if facts.hearings.is_empty() {
        return Ok(Vec::new());
    }
    let case_id = resolve_case_id(conn, ref_type, ref_id)?;

    // 旧 pending 建议作废（重跑覆盖），避免同一文书反复生成
    conn.execute(
        "UPDATE intake_proposals SET status='superseded', resolved_at=datetime('now','localtime')
         WHERE ref_type=?1 AND ref_id=?2 AND status='pending'",
        rusqlite::params![ref_type, ref_id],
    )?;

    let mut out = Vec::new();
    for hearing in &facts.hearings {
        let conflict = match &case_id {
            Some(case_id) => detect_c1(conn, case_id, &hearing.date)?,
            // 没有案件 → C4 归属未定：建议照常生成，但 payload 不带 case_id，
            // 确认时由用户选择归入哪个案件
            None => Some(ConflictCase {
                kind: "uncertain_case".into(),
                existing: Vec::new(),
                proposed: serde_json::json!({ "date": hearing.date }),
                options: vec![ConflictOption {
                    value: "assign_case".into(),
                    label: "选择归卷案件".into(),
                    writes: true,
                }],
            }),
        };
        let id = crate::db::new_id();
        let payload = serde_json::json!({
            "caseId": case_id,
            "hearingName": "开庭",
            "hearingDate": hearing.date,
            "venue": hearing.venue,
            "court": facts.court,
        });
        let evidence = vec![serde_json::json!({
            "docType": facts.doc_type,
            "caseNo": facts.case_no,
            "court": facts.court,
            "date": hearing.date,
            "page": hearing.page,
            "confidence": hearing.confidence,
        })];
        let conflict_json = conflict
            .as_ref()
            .map(|c| serde_json::to_string(c))
            .transpose()?;
        conn.execute(
            "INSERT INTO intake_proposals
               (id, ref_type, ref_id, kind, payload_json, evidence_json, confidence, conflict_json, status, created_at)
             VALUES (?1, ?2, ?3, 'create_hearing', ?4, ?5, ?6, ?7, 'pending', datetime('now','localtime'))",
            rusqlite::params![
                id,
                ref_type,
                ref_id,
                serde_json::to_string(&payload)?,
                serde_json::to_string(&evidence)?,
                hearing.confidence,
                conflict_json,
            ],
        )?;
        out.push(IntakeProposal {
            id,
            ref_type: ref_type.to_string(),
            ref_id: ref_id.to_string(),
            kind: "create_hearing".into(),
            payload,
            evidence,
            confidence: hearing.confidence,
            conflict,
            status: "pending".into(),
            resolution: None,
            created_at: crate::db::now_local(),
            resolved_at: None,
        });
    }
    Ok(out)
}

fn row_to_proposal(row: &rusqlite::Row<'_>) -> rusqlite::Result<IntakeProposal> {
    let payload: String = row.get(3)?;
    let evidence: String = row.get(4)?;
    let conflict: Option<String> = row.get(6)?;
    let resolution: Option<String> = row.get(8)?;
    Ok(IntakeProposal {
        id: row.get(0)?,
        ref_type: row.get(1)?,
        ref_id: row.get(2)?,
        kind: row.get(9)?,
        payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
        evidence: serde_json::from_str(&evidence).unwrap_or_default(),
        confidence: row.get(5)?,
        conflict: conflict.and_then(|c| serde_json::from_str(&c).ok()),
        status: row.get(7)?,
        resolution: resolution.and_then(|r| serde_json::from_str(&r).ok()),
        created_at: row.get(10)?,
        resolved_at: row.get(11)?,
    })
}

const PROPOSAL_COLUMNS: &str = "id, ref_type, ref_id, payload_json, evidence_json, confidence, conflict_json, status, resolution_json, kind, created_at, resolved_at";

/// 列出建议（默认 pending，新的在前）。
pub fn list_proposals(conn: &rusqlite::Connection, status: Option<&str>) -> anyhow::Result<Vec<IntakeProposal>> {
    let sql = format!(
        "SELECT {PROPOSAL_COLUMNS} FROM intake_proposals WHERE (?1 IS NULL OR status = ?1) ORDER BY created_at DESC LIMIT 200"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![status], row_to_proposal)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

/// 确认一条建议。
///
/// `resolution` 至少含 `option`（冲突选项值）；C1 选 `reschedule` 时还需
/// `supersedeHearingId`。确认后写 hearings + calendar_events +
/// schedule_change_log，全部带证据指针。
pub fn confirm_proposal(
    conn: &mut rusqlite::Connection,
    id: &str,
    resolution: &serde_json::Value,
) -> anyhow::Result<IntakeProposal> {
    let tx = conn.transaction()?;
    let proposal = tx
        .query_row(
            &format!("SELECT {PROPOSAL_COLUMNS} FROM intake_proposals WHERE id=?1"),
            [id],
            row_to_proposal,
        )
        .map_err(|_| anyhow::anyhow!("建议不存在"))?;
    anyhow::ensure!(proposal.status == "pending", "该建议已处理");

    let option = resolution
        .get("option")
        .and_then(|v| v.as_str())
        .unwrap_or("confirm")
        .to_string();

    let mut hearing_id: Option<String> = None;
    if proposal.kind == "create_hearing" {
        match option.as_str() {
            // 都不是开庭：只留痕，不建任何日程
            "not_hearing" => {}
            "reschedule" => {
                let supersede = resolution
                    .get("supersedeHearingId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("改期需要指定要取代的排期"))?
                    .to_string();
                // 旧排期标记改期并留痕（不删除：改期是状态迁移）
                let old_date: String = tx.query_row(
                    "UPDATE hearings SET lifecycle_status='postponed', change_reason=?2 WHERE id=?1 RETURNING hearing_date",
                    rusqlite::params![supersede, resolution.get("reason").and_then(|v| v.as_str()).unwrap_or("传票改期")],
                    |r| r.get(0),
                )?;
                let new_id = insert_hearing_from_proposal(&tx, &proposal)?;
                write_change_log(&tx, "hearing", &new_id, "rescheduled",
                    Some(&serde_json::json!({ "date": old_date }).to_string()),
                    Some(&serde_json::json!({ "date": proposal.payload["hearingDate"] }).to_string()),
                    resolution.get("reason").and_then(|v| v.as_str()),
                    &proposal)?;
                hearing_id = Some(new_id);
            }
            // two_hearings / 无冲突直接确认 / assign_case
            _ => {
                let case_id = resolution
                    .get("caseId")
                    .and_then(|v| v.as_str())
                    .or_else(|| proposal.payload["caseId"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("请先选择归卷案件"))?
                    .to_string();
                let new_id = insert_hearing_from_proposal(&tx, &proposal)?;
                if proposal.payload["caseId"].is_null() {
                    tx.execute("UPDATE hearings SET case_id=?2 WHERE id=?1", rusqlite::params![new_id, case_id])?;
                }
                write_change_log(&tx, "hearing", &new_id, "created", None,
                    Some(&proposal.payload.to_string()), None, &proposal)?;
                hearing_id = Some(new_id);
            }
        }
    }

    tx.execute(
        "UPDATE intake_proposals SET status='confirmed', resolution_json=?2, resolved_at=datetime('now','localtime') WHERE id=?1",
        rusqlite::params![id, serde_json::to_string(resolution)?],
    )?;
    tx.commit()?;
    let _ = hearing_id;
    // 回读最新状态
    let conn = &*conn;
    let updated = conn
        .query_row(&format!("SELECT {PROPOSAL_COLUMNS} FROM intake_proposals WHERE id=?1"), [id], row_to_proposal)?;
    Ok(updated)
}

/// 从建议 payload 插入 hearings 行（复用 create_case_hearing 的字段口径）。
fn insert_hearing_from_proposal(
    tx: &rusqlite::Transaction<'_>,
    proposal: &IntakeProposal,
) -> anyhow::Result<String> {
    let id = crate::db::new_id();
    let payload = &proposal.payload;
    let case_id = payload["caseId"].as_str().unwrap_or_default().to_string();
    let hearing_date = payload["hearingDate"].as_str().unwrap_or_default().to_string();
    crate::commands::cases::validate_hearing_date(&hearing_date)?;
    let hearing_name = payload["hearingName"].as_str().unwrap_or("开庭").to_string();
    tx.execute(
        "INSERT INTO hearings (id, case_id, hearing_record, hearing_name, hearing_date, venue, court, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now','localtime'))",
        rusqlite::params![
            id,
            case_id,
            hearing_name,
            hearing_name,
            hearing_date,
            payload["venue"].as_str(),
            payload["court"].as_str(),
        ],
    )?;
    Ok(id)
}

/// 写日程变更留痕（带证据指针：哪份文书、哪个建议）。
fn write_change_log(
    tx: &rusqlite::Transaction<'_>,
    entity_type: &str,
    entity_id: &str,
    change_kind: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
    reason: Option<&str>,
    proposal: &IntakeProposal,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO schedule_change_log
           (id, entity_type, entity_id, change_kind, old_value_json, new_value_json, reason, evidence_ref_type, evidence_ref_id, proposal_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            crate::db::new_id(),
            entity_type,
            entity_id,
            change_kind,
            old_value,
            new_value,
            reason,
            proposal.ref_type,
            proposal.ref_id,
            proposal.id,
        ],
    )?;
    Ok(())
}

/// 拒绝一条建议（记录原因，供学习信号）。
pub fn reject_proposal(
    conn: &rusqlite::Connection,
    id: &str,
    reason: Option<&str>,
) -> anyhow::Result<()> {
    let rows = conn.execute(
        "UPDATE intake_proposals SET status='rejected', resolution_json=?2, resolved_at=datetime('now','localtime')
         WHERE id=?1 AND status='pending'",
        rusqlite::params![id, reason.map(|r| serde_json::json!({ "reason": r }).to_string())],
    )?;
    anyhow::ensure!(rows > 0, "建议不存在或已处理");
    Ok(())
}

/// 从已抽取的事实生成待确认建议（幂等重跑）。
#[tauri::command]
pub async fn generate_intake_proposals(ref_type: String, ref_id: String) -> Result<Vec<IntakeProposal>, String> {
    super::run_blocking(move || {
        let conn = crate::db::open_db()?;
        generate_proposals(&conn, &ref_type, &ref_id).map_err(Into::into)
    })
    .await
}

/// 列出建议（status 为空 = 全部，新的在前）。
#[tauri::command]
pub async fn list_intake_proposals(status: Option<String>) -> Result<Vec<IntakeProposal>, String> {
    super::run_blocking(move || {
        let conn = crate::db::open_db()?;
        list_proposals(&conn, status.as_deref()).map_err(Into::into)
    })
    .await
}

/// 确认一条建议。`resolution` 至少含 `option`；C1 选 reschedule 时需
/// `supersedeHearingId` 与 `reason`；无案件时需 `caseId`。
#[tauri::command]
pub async fn confirm_intake_proposal(id: String, resolution: serde_json::Value) -> Result<IntakeProposal, String> {
    super::run_blocking(move || {
        let mut conn = crate::db::open_db()?;
        confirm_proposal(&mut conn, &id, &resolution).map_err(Into::into)
    })
    .await
}

/// 拒绝一条建议（记录原因）。
#[tauri::command]
pub async fn reject_intake_proposal(id: String, reason: Option<String>) -> Result<(), String> {
    super::run_blocking(move || {
        let conn = crate::db::open_db()?;
        reject_proposal(&conn, &id, reason.as_deref()).map_err(Into::into)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        conn
    }

    fn seed(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO cases(id, case_name, client_name) VALUES('case-1','测试案件','客户')",
            [],
        )
        .unwrap();
    }

    fn facts_with_hearing(conn: &rusqlite::Connection, date: &str) {
        conn.execute(
            "INSERT INTO document_facts (id, ref_type, ref_id, doc_type, case_no, hearings_json, overall_confidence, extractor_version)
             VALUES ('f1','case_file','file-1','summons','（2026）鲁02民初664号',?1,0.95,'facts-1')",
            rusqlite::params![serde_json::to_string(&vec![serde_json::json!({
                "date": date, "venue": "第五法庭", "page": 2, "confidence": 0.95
            })]).unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO case_files (id, case_id, file_name, file_path, file_size, category) VALUES ('file-1','case-1','传票.pdf','/tmp/x.pdf',10,'summons')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn generates_hearing_proposal_with_page_evidence() {
        let conn = conn();
        seed(&conn);
        facts_with_hearing(&conn, "2026-10-26 14:00");
        let proposals = generate_proposals(&conn, "case_file", "file-1").unwrap();
        assert_eq!(proposals.len(), 1);
        let p = &proposals[0];
        assert_eq!(p.kind, "create_hearing");
        assert_eq!(p.payload["hearingDate"], "2026-10-26 14:00");
        // 证据必须带页码出处
        assert_eq!(p.evidence[0]["page"], 2);
        assert!(p.conflict.is_none(), "没有既有排期时不应有冲突");
    }

    #[test]
    fn conflicting_hearing_date_requires_user_decision() {
        let conn = conn();
        seed(&conn);
        facts_with_hearing(&conn, "2026-11-05 14:00");
        conn.execute(
            "INSERT INTO hearings (id, case_id, hearing_record, hearing_name, hearing_date, lifecycle_status)
             VALUES ('h-old','case-1','第一次开庭','第一次开庭','2026-10-20 09:00','scheduled')",
            [],
        )
        .unwrap();
        let proposals = generate_proposals(&conn, "case_file", "file-1").unwrap();
        let conflict = proposals[0].conflict.as_ref().expect("日期不同必须触发 C1");
        assert_eq!(conflict.kind, "same_case_hearing_date");
        assert_eq!(conflict.options.len(), 3, "两个开庭 / 改期 / 类型误判");
        assert_eq!(conflict.options[1].value, "reschedule");
    }

    #[test]
    fn same_date_is_not_a_conflict() {
        let conn = conn();
        seed(&conn);
        facts_with_hearing(&conn, "2026-10-26 14:00");
        conn.execute(
            "INSERT INTO hearings (id, case_id, hearing_record, hearing_name, hearing_date, lifecycle_status)
             VALUES ('h-same','case-1','第一次开庭','第一次开庭','2026-10-26 14:00','scheduled')",
            [],
        )
        .unwrap();
        let proposals = generate_proposals(&conn, "case_file", "file-1").unwrap();
        assert!(proposals[0].conflict.is_none(), "同日不构成冲突");
    }

    #[test]
    fn confirming_reschedule_marks_old_and_logs_both() {
        let conn = conn();
        seed(&conn);
        facts_with_hearing(&conn, "2026-11-05 14:00");
        conn.execute(
            "INSERT INTO hearings (id, case_id, hearing_record, hearing_name, hearing_date, lifecycle_status)
             VALUES ('h-old','case-1','第一次开庭','第一次开庭','2026-10-20 09:00','scheduled')",
            [],
        )
        .unwrap();
        let proposals = generate_proposals(&conn, "case_file", "file-1").unwrap();
        let mut conn = conn;
        let done = confirm_proposal(
            &mut conn,
            &proposals[0].id,
            &serde_json::json!({ "option": "reschedule", "supersedeHearingId": "h-old", "reason": "传票改期" }),
        )
        .unwrap();
        assert_eq!(done.status, "confirmed");
        // 旧排期标记改期，新排期建立
        let old_status: String = conn
            .query_row("SELECT lifecycle_status FROM hearings WHERE id='h-old'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(old_status, "postponed");
        let new_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM hearings WHERE case_id='case-1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(new_count, 2);
        // 留痕：旧→新 + 证据指针
        let logs: i64 = conn
            .query_row("SELECT COUNT(*) FROM schedule_change_log WHERE entity_type='hearing'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(logs, 1);
        let evidence: String = conn
            .query_row("SELECT evidence_ref_id FROM schedule_change_log LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(evidence, "file-1", "留痕必须带证据指针");
    }

    #[test]
    fn rejecting_records_reason_and_blocks_reconfirm() {
        let conn = conn();
        seed(&conn);
        facts_with_hearing(&conn, "2026-10-26 14:00");
        let proposals = generate_proposals(&conn, "case_file", "file-1").unwrap();
        reject_proposal(&conn, &proposals[0].id, Some("日期看错了")).unwrap();
        let mut conn = conn;
        let err = confirm_proposal(&mut conn, &proposals[0].id, &serde_json::json!({"option":"confirm"}));
        assert!(err.is_err(), "已拒绝的建议不能重复确认");
    }

    #[test]
    fn regenerating_supersedes_previous_pending() {
        let conn = conn();
        seed(&conn);
        facts_with_hearing(&conn, "2026-10-26 14:00");
        generate_proposals(&conn, "case_file", "file-1").unwrap();
        generate_proposals(&conn, "case_file", "file-1").unwrap();
        let pending = list_proposals(&conn, Some("pending")).unwrap();
        assert_eq!(pending.len(), 1, "重跑后只应有一条 pending");
        let superseded = list_proposals(&conn, Some("superseded")).unwrap();
        assert_eq!(superseded.len(), 1);
    }
}

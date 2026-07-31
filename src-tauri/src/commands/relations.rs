use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::run_blocking;
use crate::db;

/// 关系记录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseRelation {
    pub id: String,
    pub source_case_id: String,
    pub target_case_id: String,
    pub relation_type: String,
    pub label: Option<String>,
    pub created_at: Option<String>,
}

/// 关联案件（含案件摘要信息）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCase {
    pub relation_id: String,
    pub relation_type: String,
    pub label: Option<String>,
    pub case_id: String,
    pub case_name: String,
    pub case_no: Option<String>,
    pub case_status: Option<String>,
    pub client_name: String,
    pub track: String,
}

/// 添加关系
#[tauri::command]
pub async fn add_relation(
    case_id: String,
    related_id: String,
    relation_type: String,
    label: Option<String>,
) -> Result<CaseRelation, String> {
    let valid_types = ["same_patent", "same_party", "appeal_of", "cross_reference"];
    if !valid_types.contains(&relation_type.as_str()) {
        return Err(format!("无效的关系类型: {}", relation_type));
    }
    if case_id == related_id {
        return Err("不能与自身建立关系".to_string());
    }
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let now = db::now_local();
        conn.execute(
            "INSERT INTO case_relations (id, source_case_id, target_case_id, relation_type, label, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, case_id, related_id, relation_type, label, now],
        )
        .map_err(|e| anyhow::anyhow!(if e.to_string().contains("UNIQUE") { "该关系已存在".to_string() } else { e.to_string() }))?;
        Ok(CaseRelation {
            id,
            source_case_id: case_id,
            target_case_id: related_id,
            relation_type,
            label,
            created_at: Some(now),
        })
    })
    .await
}

/// 获取某案件的所有关系（含关联案件摘要）
#[tauri::command]
pub async fn get_relations(case_id: String) -> Result<Vec<RelatedCase>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.relation_type, r.label,
                    c.id, c.case_name, c.case_no, c.case_status, c.client_name, c.track
             FROM case_relations r
             JOIN cases c ON c.id = r.target_case_id
             WHERE r.source_case_id = ?1
             ORDER BY r.created_at DESC"
        )?;
        let rows = stmt.query_map(params![case_id], |row| {
            Ok(RelatedCase {
                relation_id: row.get(0)?,
                relation_type: row.get(1)?,
                label: row.get(2)?,
                case_id: row.get(3)?,
                case_name: row.get(4)?,
                case_no: row.get(5)?,
                case_status: row.get(6)?,
                client_name: row.get(7)?,
                track: row.get(8)?,
            })
        })?;
        let mut results: Vec<RelatedCase> = Vec::new();
        for row in rows {
            results.push(row?);
        }
        // 反向关系
        let mut stmt2 = conn.prepare(
            "SELECT r.id, r.relation_type, r.label,
                    c.id, c.case_name, c.case_no, c.case_status, c.client_name, c.track
             FROM case_relations r
             JOIN cases c ON c.id = r.source_case_id
             WHERE r.target_case_id = ?1
             ORDER BY r.created_at DESC"
        )?;
        let rows2 = stmt2.query_map(params![case_id], |row| {
            Ok(RelatedCase {
                relation_id: row.get(0)?,
                relation_type: row.get(1)?,
                label: row.get(2)?,
                case_id: row.get(3)?,
                case_name: row.get(4)?,
                case_no: row.get(5)?,
                case_status: row.get(6)?,
                client_name: row.get(7)?,
                track: row.get(8)?,
            })
        })?;
        for row in rows2 {
            results.push(row?);
        }
        Ok(results)
    })
    .await
}

/// 删除关系
#[tauri::command]
pub async fn remove_relation(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM case_relations WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}

/// 自动检测关系
#[tauri::command]
pub async fn detect_relations(case_id: String) -> Result<Vec<CaseRelation>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let case = db::cases::get_case(&conn, &case_id)?;
        let mut found: Vec<CaseRelation> = Vec::new();

        // 1. 同专利号 → same_patent
        if let Some(ref app_no) = case.patent_app_no {
            if !app_no.is_empty() {
                let mut stmt = conn.prepare(
                    "SELECT id FROM cases WHERE patent_app_no = ?1 AND id != ?2"
                )?;
                let ids: Vec<String> = stmt
                    .query_map(params![app_no, case_id], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                for related_id in ids {
                    if let Some(rel) = try_insert_relation(&conn, &case_id, &related_id, "same_patent", Some("同专利号"))? {
                        found.push(rel);
                    }
                }
            }
        }

        // 2. 同客户 → same_party
        if !case.client_name.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id FROM cases WHERE client_name = ?1 AND id != ?2"
            )?;
            let ids: Vec<String> = stmt
                .query_map(params![case.client_name, case_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            for related_id in ids {
                if let Some(rel) = try_insert_relation(&conn, &case_id, &related_id, "same_party", Some("同客户"))? {
                    found.push(rel);
                }
            }
        }

        // 3. 审级关联 → appeal_of
        if let Some(ref level) = case.case_level {
            if level == "二审" || level == "再审" {
                if let Some(ref case_no) = case.case_no {
                    if !case_no.is_empty() {
                        let base_no = extract_base_case_no(case_no);
                        let mut stmt = conn.prepare(
                            "SELECT id FROM cases WHERE case_no LIKE ?1 AND id != ?2 AND case_level = '一审'"
                        )?;
                        let pattern = format!("%{}%", base_no);
                        let ids: Vec<String> = stmt
                            .query_map(params![pattern, case_id], |row| row.get(0))?
                            .filter_map(|r| r.ok())
                            .collect();
                        for related_id in ids {
                            let label = if level == "二审" { "二审关联" } else { "再审关联" };
                            if let Some(rel) = try_insert_relation(&conn, &case_id, &related_id, "appeal_of", Some(label))? {
                                found.push(rel);
                            }
                        }
                    }
                }
            }
        }

        Ok(found)
    })
    .await
}

/// 尝试插入关系（忽略已存在的 UNIQUE 冲突）
fn try_insert_relation(
    conn: &rusqlite::Connection,
    source: &str,
    target: &str,
    rel_type: &str,
    label: Option<&str>,
) -> anyhow::Result<Option<CaseRelation>> {
    let id = db::new_id();
    let now = db::now_local();
    match conn.execute(
        "INSERT OR IGNORE INTO case_relations (id, source_case_id, target_case_id, relation_type, label, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, source, target, rel_type, label, now],
    ) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(CaseRelation {
            id,
            source_case_id: source.to_string(),
            target_case_id: target.to_string(),
            relation_type: rel_type.to_string(),
            label: label.map(|s| s.to_string()),
            created_at: Some(now),
        })),
        Err(e) => Err(e.into()),
    }
}

/// 从案号中提取基础部分（去掉审级标识）
fn extract_base_case_no(case_no: &str) -> String {
    let s = if let Some(idx) = case_no.find("之一") {
        &case_no[..idx]
    } else if let Some(idx) = case_no.find("之二") {
        &case_no[..idx]
    } else {
        case_no
    };
    s.trim().to_string()
}

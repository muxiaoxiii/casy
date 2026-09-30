//! 对象化实体（W6 · Capacities 式单一事实源）
//! 法官/客户/对方律师/法院为独立对象；更新一处，所有挂载案件同步可见。
use rusqlite::params;
use serde::Serialize;

use super::run_blocking;
use crate::db;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PersonDto {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub org: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub preferences: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// 关联案件数（列表页展示）
    pub case_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CasePersonDto {
    pub link_id: String,
    pub person: PersonDto,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PersonCaseDto {
    pub link_id: String,
    pub case_id: String,
    pub case_name: String,
    pub case_no: Option<String>,
    pub role: Option<String>,
}

const VALID_KINDS: [&str; 5] = ["judge", "client", "opposing_counsel", "court", "contact"];

fn row_to_person(row: &rusqlite::Row) -> rusqlite::Result<PersonDto> {
    Ok(PersonDto {
        id: row.get("id")?,
        kind: row.get("kind")?,
        name: row.get("name")?,
        org: row.get("org")?,
        phone: row.get("phone")?,
        email: row.get("email")?,
        preferences: row.get("preferences")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        case_count: None,
    })
}

const PERSON_COLS: &str =
    "id, kind, name, org, phone, email, preferences, notes, created_at, updated_at";

#[tauri::command]
pub async fn list_persons(
    kind: Option<String>,
    keyword: Option<String>,
) -> Result<Vec<PersonDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut sql = format!(
            "SELECT {PERSON_COLS}, (SELECT COUNT(*) FROM case_persons cp WHERE cp.person_id = persons.id) AS case_count
             FROM persons WHERE 1=1"
        );
        let mut vals: Vec<String> = Vec::new();
        if let Some(k) = &kind {
            sql.push_str(" AND kind = ?");
            vals.push(k.clone());
        }
        if let Some(kw) = &keyword {
            sql.push_str(" AND (name LIKE ? OR org LIKE ?)");
            vals.push(format!("%{kw}%"));
            vals.push(format!("%{kw}%"));
        }
        sql.push_str(" ORDER BY updated_at DESC");
        let mut stmt = conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::ToSql> =
            vals.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            let mut p = row_to_person(row)?;
            p.case_count = Some(row.get("case_count")?);
            Ok(p)
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

/// 新增或更新（id 为空 → 新建）
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn upsert_person(
    id: Option<String>,
    kind: String,
    name: String,
    org: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    preferences: Option<String>,
    notes: Option<String>,
) -> Result<String, String> {
    run_blocking(move || {
        if !VALID_KINDS.contains(&kind.as_str()) {
            return Err(anyhow::anyhow!("无效的实体类型: {kind}"));
        }
        if name.trim().is_empty() {
            return Err(anyhow::anyhow!("名称不能为空"));
        }
        let conn = db::open_db()?;
        match &id {
            Some(pid) => {
                let n = conn.execute(
                    "UPDATE persons SET kind=?2, name=?3, org=?4, phone=?5, email=?6,
                     preferences=?7, notes=?8, updated_at=datetime('now','localtime')
                     WHERE id=?1",
                    params![pid, kind, name, org, phone, email, preferences, notes],
                )?;
                if n == 0 {
                    return Err(anyhow::anyhow!("实体不存在: {pid}"));
                }
                Ok(pid.clone())
            }
            None => {
                let new_id = db::new_id();
                conn.execute(
                    "INSERT INTO persons (id, kind, name, org, phone, email, preferences, notes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![new_id, kind, name, org, phone, email, preferences, notes],
                )?;
                Ok(new_id)
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn delete_person(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM persons WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn attach_person_to_case(
    case_id: String,
    person_id: String,
    role: Option<String>,
) -> Result<String, String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        let role = role.map(|value| value.trim().to_owned()).filter(|value| !value.is_empty());
        // SQLite UNIQUE permits repeated NULL roles. Serialize check and insert.
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM case_persons WHERE case_id=?1 AND person_id=?2 AND COALESCE(TRIM(role),'')=COALESCE(?3,''))",
            params![case_id, person_id, role], |row| row.get(0),
        )?;
        if exists {
            anyhow::bail!("该实体已以相同角色挂载到本案");
        }
        let id = db::new_id();
        tx.execute(
            "INSERT INTO case_persons (id, case_id, person_id, role) VALUES (?1, ?2, ?3, ?4)",
            params![id, case_id, person_id, role],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                anyhow::anyhow!("该实体已以相同角色挂载到本案")
            } else {
                anyhow::anyhow!(e.to_string())
            }
        })?;
        tx.commit()?;
        Ok(id)
    })
    .await
}

#[tauri::command]
pub async fn detach_person_from_case(link_id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM case_persons WHERE id = ?1", params![link_id])?;
        Ok(())
    })
    .await
}

/// 案件侧：本案挂载的全部实体
#[tauri::command]
pub async fn list_case_persons(case_id: String) -> Result<Vec<CasePersonDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT cp.id AS link_id, cp.role, persons.id, persons.kind, persons.name,
                    persons.org, persons.phone, persons.email, persons.preferences,
                    persons.notes, persons.created_at, persons.updated_at
             FROM case_persons cp JOIN persons ON persons.id = cp.person_id
             WHERE cp.case_id = ?1
             ORDER BY persons.kind, persons.name",
        )?;
        let rows = stmt.query_map(params![case_id], |row| {
            Ok(CasePersonDto {
                link_id: row.get("link_id")?,
                role: row.get("role")?,
                person: row_to_person(row)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

/// 实体侧：该法官/客户/机构涉及的全部案件（单一事实源验证面）
#[tauri::command]
pub async fn list_person_cases(person_id: String) -> Result<Vec<PersonCaseDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT cp.id AS link_id, c.id AS case_id, c.case_name, c.case_no, cp.role
             FROM case_persons cp JOIN cases c ON c.id = cp.case_id
             WHERE cp.person_id = ?1
             ORDER BY c.updated_at DESC",
        )?;
        let rows = stmt.query_map(params![person_id], |row| {
            Ok(PersonCaseDto {
                link_id: row.get("link_id")?,
                case_id: row.get("case_id")?,
                case_name: row.get("case_name")?,
                case_no: row.get("case_no")?,
                role: row.get("role")?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

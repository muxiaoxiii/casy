use super::run_blocking;
use crate::db;
use serde::{Deserialize, Serialize};

/// 草稿实体（docs 域）
///
/// 契约（审查 P1-1）：Tauri v2 **只对入参**做 camelCase→snake_case 自动转换，
/// 返回值序列化完全由 serde 决定。缺 rename_all 时下行 JSON 为 snake_case
/// （case_id / template_path / created_at / updated_at），而前端 `core/services/docs.ts`
/// 按 camelCase（caseId / templatePath / createdAt / updatedAt）断言与消费，导致
/// 草稿字段在编辑器里恒为 undefined。与 docs.rs 的 RenderResponse / ExportResponse 同源问题。
#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub id: String,
    pub case_id: Option<String>,
    pub title: String,
    pub content: Option<String>,
    pub template_path: Option<String>,
    pub status: String,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// 新建草稿
#[tauri::command]
pub async fn create_draft(
    title: String,
    content: Option<String>,
    case_id: Option<String>,
    template_path: Option<String>,
) -> Result<Draft, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let now = db::now_local();

        conn.execute(
            "INSERT INTO drafts (id, case_id, title, content, template_path, status, version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'draft', 1, ?6, ?6)",
            rusqlite::params![id, case_id, title, content, template_path, now],
        )?;

        Ok(Draft {
            id,
            case_id,
            title,
            content,
            template_path,
            status: "draft".to_string(),
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        })
    })
    .await
}

/// 列出所有草稿
#[tauri::command]
pub async fn list_drafts() -> Result<Vec<Draft>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let mut stmt = conn.prepare(
            "SELECT id, case_id, title, content, template_path, status, version, created_at, updated_at
             FROM drafts ORDER BY updated_at DESC",
        )?;

        let drafts = stmt
            .query_map([], |row| {
                Ok(Draft {
                    id: row.get(0)?,
                    case_id: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    template_path: row.get(4)?,
                    status: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "draft".to_string()),
                    version: row.get::<_, Option<i32>>(6)?.unwrap_or(1),
                    created_at: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    updated_at: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(drafts)
    })
    .await
}

/// 获取单个草稿
#[tauri::command]
pub async fn get_draft(id: String) -> Result<Draft, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let draft = conn.query_row(
            "SELECT id, case_id, title, content, template_path, status, version, created_at, updated_at
             FROM drafts WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(Draft {
                    id: row.get(0)?,
                    case_id: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    template_path: row.get(4)?,
                    status: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "draft".to_string()),
                    version: row.get::<_, Option<i32>>(6)?.unwrap_or(1),
                    created_at: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    updated_at: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                })
            },
        )?;

        Ok(draft)
    })
    .await
}

/// 更新草稿
#[tauri::command]
pub async fn update_draft(
    id: String,
    title: Option<String>,
    content: Option<String>,
    status: Option<String>,
    case_id: Option<String>,
    expected_version: Option<i32>,
) -> Result<Draft, String> {
    run_blocking(move || {
        let mut connection = db::open_db()?;
        let conn = connection.transaction()?;

        // 先获取当前草稿
        let current = conn.query_row(
            "SELECT title, content, status, case_id, version FROM drafts WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i32>(4)?,
                ))
            },
        )?;
        anyhow::ensure!(expected_version.is_none_or(|version| version == current.4), "EDIT_CONFLICT: 文书已在其他页面修改，当前内容已保留，请重新打开后核对");

        let new_title = title.unwrap_or(current.0);
        let new_content = content.or(current.1);
        let new_status = status.unwrap_or(current.2);
        let new_case_id = case_id.or(current.3);

        conn.execute(
            "UPDATE drafts SET title = ?1, content = ?2, status = ?3, case_id = ?4, version = version + 1
             WHERE id = ?5",
            rusqlite::params![new_title, new_content, new_status, new_case_id, id],
        )?;

        // 返回更新后的草稿
        let draft = conn.query_row(
            "SELECT id, case_id, title, content, template_path, status, version, created_at, updated_at
             FROM drafts WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(Draft {
                    id: row.get(0)?,
                    case_id: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    template_path: row.get(4)?,
                    status: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "draft".to_string()),
                    version: row.get::<_, Option<i32>>(6)?.unwrap_or(1),
                    created_at: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    updated_at: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                })
            },
        )?;
        conn.commit()?;
        Ok(draft)
    })
    .await
}

/// 删除草稿
#[tauri::command]
pub async fn delete_draft(id: String) -> Result<bool, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let rows = conn.execute("DELETE FROM drafts WHERE id = ?1", rusqlite::params![id])?;

        Ok(rows > 0)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 契约（P1-1）：Draft 下行 JSON 必须为 camelCase，
    /// 与前端 `core/services/docs.ts` 的 caseId / templatePath / createdAt / updatedAt 对齐。
    #[test]
    fn draft_serializes_to_camel_case() {
        let draft = Draft {
            id: "d1".into(),
            case_id: Some("c1".into()),
            title: "起诉状".into(),
            content: Some("<p>正文</p>".into()),
            template_path: Some("tpl/起诉状.docx".into()),
            status: "draft".into(),
            version: 3,
            created_at: "2026-09-01 10:00:00".into(),
            updated_at: "2026-09-02 11:30:00".into(),
        };

        let value = serde_json::to_value(&draft).expect("serialize draft");
        let obj = value.as_object().expect("object");

        // camelCase 键必须存在
        assert_eq!(obj.get("caseId").and_then(|v| v.as_str()), Some("c1"));
        assert_eq!(obj.get("templatePath").and_then(|v| v.as_str()), Some("tpl/起诉状.docx"));
        assert_eq!(obj.get("createdAt").and_then(|v| v.as_str()), Some("2026-09-01 10:00:00"));
        assert_eq!(obj.get("updatedAt").and_then(|v| v.as_str()), Some("2026-09-02 11:30:00"));

        // snake_case 键不得泄漏
        assert!(obj.get("case_id").is_none(), "snake_case case_id 不得出现");
        assert!(obj.get("template_path").is_none(), "snake_case template_path 不得出现");
        assert!(obj.get("created_at").is_none(), "snake_case created_at 不得出现");
        assert!(obj.get("updated_at").is_none(), "snake_case updated_at 不得出现");
    }
}

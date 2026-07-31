use super::run_blocking;
use crate::db;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeFilter {
    pub category: Option<String>,
    pub case_id: Option<String>,
    pub search: Option<String>,
    pub law_name: Option<String>,
}

#[tauri::command]
pub async fn list_knowledge(filter: Option<KnowledgeFilter>) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut sql = String::from("SELECT * FROM knowledge_items WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(f) = &filter {
            if let Some(cat) = &f.category {
                if !cat.is_empty() {
                    sql.push_str(&format!(" AND category = ?{}", idx));
                    params.push(Box::new(cat.clone()));
                    idx += 1;
                }
            }
            if let Some(case_id) = &f.case_id {
                if !case_id.is_empty() {
                    sql.push_str(&format!(" AND linked_case_id = ?{}", idx));
                    params.push(Box::new(case_id.clone()));
                    idx += 1;
                }
            }
            if let Some(law) = &f.law_name {
                if !law.is_empty() {
                    sql.push_str(&format!(" AND law_name = ?{}", idx));
                    params.push(Box::new(law.clone()));
                    idx += 1;
                }
            }
            if let Some(search) = &f.search {
                if !search.is_empty() {
                    sql.push_str(&format!(
                        " AND (title LIKE ?{0} OR content LIKE ?{0} OR tags LIKE ?{0})",
                        idx
                    ));
                    params.push(Box::new(format!("%{}%", search)));
                }
            }
        }

        sql.push_str(" ORDER BY updated_at DESC LIMIT 200");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let items: Vec<serde_json::Value> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>("id")?,
                    "title": row.get::<_, String>("title")?,
                    "category": row.get::<_, String>("category")?,
                    "content": row.get::<_, String>("content")?,
                    "tags": row.get::<_, Option<String>>("tags")?,
                    "sourceType": row.get::<_, Option<String>>("source_type")?,
                    "linkedCaseId": row.get::<_, Option<String>>("linked_case_id")?,
                    "lawName": row.get::<_, Option<String>>("law_name")?,
                    "articleNo": row.get::<_, Option<String>>("article_no")?,
                    "effectiveDate": row.get::<_, Option<String>>("effective_date")?,
                    "status": row.get::<_, Option<String>>("status")?,
                    "createdAt": row.get::<_, Option<String>>("created_at")?,
                    "updatedAt": row.get::<_, Option<String>>("updated_at")?,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(items)
    })
    .await
}

#[tauri::command]
pub async fn create_knowledge(data: serde_json::Value) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let now = db::now_local();

        conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content, tags, source_type, source_id,
             linked_case_id, law_name, article_no, effective_date, status, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13)",
            rusqlite::params![
                id,
                data["title"].as_str().unwrap_or(""),
                data["category"].as_str().unwrap_or("other"),
                data["content"].as_str().unwrap_or(""),
                data["tags"].as_str(),
                data["sourceType"].as_str(),
                data["sourceId"].as_str(),
                data["linkedCaseId"].as_str(),
                data["lawName"].as_str(),
                data["articleNo"].as_str(),
                data["effectiveDate"].as_str(),
                data["status"].as_str().unwrap_or("current"),
                now,
            ],
        )?;

        Ok(id)
    })
    .await
}

#[tauri::command]
pub async fn update_knowledge(id: String, data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 保存版本历史
        if let Ok(old) = conn.query_row(
            "SELECT content FROM knowledge_items WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, String>(0),
        ) {
            let _ = conn.execute(
                "INSERT INTO knowledge_versions (id, item_id, content, change_reason) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![db::new_id(), id, old, "edit"],
            );
        }

        let mut sql = String::from("UPDATE knowledge_items SET updated_at = datetime('now','localtime')");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        let fields = [
            ("title", "title"), ("category", "category"), ("content", "content"),
            ("tags", "tags"), ("lawName", "law_name"), ("articleNo", "article_no"),
            ("effectiveDate", "effective_date"), ("status", "status"),
            ("linkedCaseId", "linked_case_id"),
        ];

        let mut idx = 1;
        for (json_key, db_col) in &fields {
            if let Some(val) = data.get(*json_key) {
                sql.push_str(&format!(", {} = ?{}", db_col, idx));
                match val {
                    serde_json::Value::String(s) => params.push(Box::new(s.clone())),
                    serde_json::Value::Null => params.push(Box::new(rusqlite::types::Null)),
                    _ => params.push(Box::new(val.to_string())),
                }
                idx += 1;
            }
        }

        sql.push_str(&format!(" WHERE id = ?{}", idx));
        params.push(Box::new(id));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, param_refs.as_slice())?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_knowledge(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM knowledge_items WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn search_knowledge(query: String) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT ki.* FROM knowledge_fts f JOIN knowledge_items ki ON ki.rowid = f.rowid
             WHERE knowledge_fts MATCH ?1 ORDER BY rank LIMIT 50"
        )?;
        let items: Vec<serde_json::Value> = stmt
            .query_map(rusqlite::params![query], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>("id")?,
                    "title": row.get::<_, String>("title")?,
                    "category": row.get::<_, String>("category")?,
                    "content": row.get::<_, String>("content")?,
                    "tags": row.get::<_, Option<String>>("tags")?,
                    "lawName": row.get::<_, Option<String>>("law_name")?,
                    "articleNo": row.get::<_, Option<String>>("article_no")?,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    })
    .await
}

#[tauri::command]
pub async fn knowledge_stats() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM knowledge_items", [], |r| r.get(0))?;

        let mut stmt = conn.prepare("SELECT category, COUNT(*) FROM knowledge_items GROUP BY category")?;
        let by_category: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(serde_json::json!({
            "total": total,
            "byCategory": by_category,
        }))
    })
    .await
}

/// 获取知识条目的版本历史
#[tauri::command]
pub async fn list_knowledge_versions(item_id: String) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, content, changed_at, change_reason FROM knowledge_versions
             WHERE item_id = ?1 ORDER BY changed_at DESC LIMIT 50"
        )?;
        let versions: Vec<serde_json::Value> = stmt
            .query_map(rusqlite::params![item_id], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>("id")?,
                    "content": row.get::<_, String>("content")?,
                    "changedAt": row.get::<_, Option<String>>("changed_at")?,
                    "changeReason": row.get::<_, Option<String>>("change_reason")?,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(versions)
    })
    .await
}

/// 对比两个版本的差异
#[tauri::command]
pub async fn diff_knowledge_versions(
    version_id_1: String,
    version_id_2: String,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let (id1, content1, changed_at1, reason1): (String, String, Option<String>, Option<String>) = conn.query_row(
            "SELECT id, content, changed_at, change_reason FROM knowledge_versions WHERE id = ?1",
            rusqlite::params![version_id_1],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).map_err(|e| anyhow::anyhow!("版本1不存在: {}", e))?;

        let (id2, content2, changed_at2, reason2): (String, String, Option<String>, Option<String>) = conn.query_row(
            "SELECT id, content, changed_at, change_reason FROM knowledge_versions WHERE id = ?1",
            rusqlite::params![version_id_2],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).map_err(|e| anyhow::anyhow!("版本2不存在: {}", e))?;

        // 简单逐行差异对比
        let lines1: Vec<&str> = content1.lines().collect();
        let lines2: Vec<&str> = content2.lines().collect();
        let mut diffs = Vec::new();

        let max_len = lines1.len().max(lines2.len());
        for i in 0..max_len {
            let old_line = lines1.get(i).copied();
            let new_line = lines2.get(i).copied();
            match (old_line, new_line) {
                (Some(o), Some(n)) if o == n => {
                    diffs.push(serde_json::json!({ "type": "equal", "line": i + 1, "text": o }));
                }
                (Some(o), Some(n)) => {
                    diffs.push(serde_json::json!({ "type": "removed", "line": i + 1, "text": o }));
                    diffs.push(serde_json::json!({ "type": "added", "line": i + 1, "text": n }));
                }
                (Some(o), None) => {
                    diffs.push(serde_json::json!({ "type": "removed", "line": i + 1, "text": o }));
                }
                (None, Some(n)) => {
                    diffs.push(serde_json::json!({ "type": "added", "line": i + 1, "text": n }));
                }
                _ => {}
            }
        }

        Ok(serde_json::json!({
            "version1": { "id": id1, "changedAt": changed_at1, "changeReason": reason1 },
            "version2": { "id": id2, "changedAt": changed_at2, "changeReason": reason2 },
            "diffs": diffs,
        }))
    })
    .await
}

/// 从选中文本创建知识条目
#[tauri::command]
pub async fn create_knowledge_from_selection(
    text: String,
    source: Option<String>,
    tags: Option<String>,
    category: Option<String>,
    case_id: Option<String>,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let now = db::now_local();

        let title = if text.chars().count() > 50 {
            text.chars().take(50).collect::<String>()
        } else {
            text.clone()
        };

        conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content, tags, source_type,
             linked_case_id, status, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
            rusqlite::params![
                id,
                title,
                category.as_deref().unwrap_or("other"),
                text,
                tags,
                source.unwrap_or_else(|| "editor".to_string()),
                case_id,
                "current",
                now,
            ],
        )?;

        Ok(id)
    })
    .await
}

/// 关联知识条目到案件
#[tauri::command]
pub async fn link_knowledge_to_case(
    knowledge_id: String,
    case_id: String,
    relation_type: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 更新 knowledge_items 的 linked_case_id
        conn.execute(
            "UPDATE knowledge_items SET linked_case_id = ?1, updated_at = datetime('now','localtime')
             WHERE id = ?2",
            rusqlite::params![case_id, knowledge_id],
        )?;

        // 同时在 knowledge_relations 中记录关系（如果 source 和 target 都存在）
        let rel_type = relation_type.unwrap_or_else(|| "related".to_string());
        let rel_id = db::new_id();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO knowledge_relations (id, source_id, target_id, relation_type)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![rel_id, knowledge_id, case_id, rel_type],
        );

        Ok(())
    })
    .await
}

/// 关联知识条目到法条
#[tauri::command]
pub async fn link_knowledge_to_law(
    knowledge_id: String,
    law_name: String,
    article_no: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "UPDATE knowledge_items SET law_name = ?1, article_no = ?2, updated_at = datetime('now','localtime')
             WHERE id = ?3",
            rusqlite::params![law_name, article_no, knowledge_id],
        )?;
        Ok(())
    })
    .await
}

/// 对比版本与当前内容的差异
#[tauri::command]
pub async fn diff_knowledge_with_current(
    version_id: String,
    item_id: String,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let (vid, version_content, changed_at, reason): (String, String, Option<String>, Option<String>) = conn.query_row(
            "SELECT id, content, changed_at, change_reason FROM knowledge_versions WHERE id = ?1",
            rusqlite::params![version_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).map_err(|e| anyhow::anyhow!("版本不存在: {}", e))?;

        let current_content: String = conn.query_row(
            "SELECT content FROM knowledge_items WHERE id = ?1",
            rusqlite::params![item_id],
            |r| r.get(0),
        ).map_err(|e| anyhow::anyhow!("知识条目不存在: {}", e))?;

        // 逐行差异对比
        let lines_old: Vec<&str> = version_content.lines().collect();
        let lines_new: Vec<&str> = current_content.lines().collect();
        let mut diffs = Vec::new();

        let max_len = lines_old.len().max(lines_new.len());
        for i in 0..max_len {
            let old_line = lines_old.get(i).copied();
            let new_line = lines_new.get(i).copied();
            match (old_line, new_line) {
                (Some(o), Some(n)) if o == n => {
                    diffs.push(serde_json::json!({ "type": "equal", "line": i + 1, "text": o }));
                }
                (Some(o), Some(n)) => {
                    diffs.push(serde_json::json!({ "type": "removed", "line": i + 1, "text": o }));
                    diffs.push(serde_json::json!({ "type": "added", "line": i + 1, "text": n }));
                }
                (Some(o), None) => {
                    diffs.push(serde_json::json!({ "type": "removed", "line": i + 1, "text": o }));
                }
                (None, Some(n)) => {
                    diffs.push(serde_json::json!({ "type": "added", "line": i + 1, "text": n }));
                }
                _ => {}
            }
        }

        Ok(serde_json::json!({
            "version": { "id": vid, "changedAt": changed_at, "changeReason": reason },
            "currentContent": current_content,
            "diffs": diffs,
        }))
    })
    .await
}

use super::run_blocking;
use crate::db;
use serde::Serialize;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSearchResult {
    pub item_type: String, // "knowledge" | "file"
    pub id: String,
    pub title: String,
    pub category: String,
    pub snippet: Option<String>,
}

#[tauri::command]
pub async fn global_search(query: String) -> Result<Vec<GlobalSearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    run_blocking(move || {
        let conn = db::open_db()?;
        
        let sql = r#"
            SELECT 'knowledge' as item_type, ki.id, ki.title, ki.category, snippet(knowledge_fts, -1, '<b>', '</b>', '...', 64) as snippet, f.rank
            FROM knowledge_fts f 
            JOIN knowledge_items ki ON ki.rowid = f.rowid
            WHERE knowledge_fts MATCH ?1
            
            UNION ALL
            
            SELECT 'file' as item_type, cf.id, cf.file_name as title, cf.category, snippet(files_fts, -1, '<b>', '</b>', '...', 64) as snippet, f.rank
            FROM files_fts f 
            JOIN case_files cf ON cf.rowid = f.rowid
            WHERE files_fts MATCH ?1
            
            ORDER BY rank LIMIT 50
        "#;
        
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(rusqlite::params![query], |row| {
            Ok(GlobalSearchResult {
                item_type: row.get(0)?,
                id: row.get(1)?,
                title: row.get(2)?,
                category: row.get(3)?,
                snippet: row.get(4)?,
            })
        })?;
        
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        
        Ok(results)
    })
    .await
}

#[tauri::command]
pub async fn reasoning_search(
    app: tauri::AppHandle,
    query: String,
    scope: Vec<String>,
) -> Result<String, String> {
    crate::ai::page_index::navigate_and_reason_search(app, &query, scope).await
}

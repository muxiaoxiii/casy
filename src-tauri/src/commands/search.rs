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
    pub case_id: Option<String>,
}

fn global_search_inner(
    conn: &rusqlite::Connection,
    query: &str,
) -> anyhow::Result<Vec<GlobalSearchResult>> {
    let fts_query = to_safe_fts_phrase(query);
    let sql = r#"
        SELECT 'knowledge' as item_type, ki.id, ki.title, ki.category, snippet(knowledge_fts, -1, '<b>', '</b>', '...', 64) as snippet, f.rank, NULL as case_id
        FROM knowledge_fts f
        JOIN knowledge_items ki ON ki.rowid = f.rowid
        WHERE knowledge_fts MATCH ?1 AND COALESCE(ki.status,'current')='current'

        UNION ALL

        SELECT 'file' as item_type, cf.id, cf.file_name as title, cf.category, snippet(files_fts, -1, '<b>', '</b>', '...', 64) as snippet, f.rank, cf.case_id
        FROM files_fts f
        JOIN case_files cf ON cf.rowid = f.rowid
        WHERE files_fts MATCH ?1 AND cf.deleted_at IS NULL

        UNION ALL

        SELECT 'file' as item_type, cf.id, cf.file_name as title, cf.category,
               '[' || CASE WHEN j.engine='text-document' THEN 's' ELSE 'p' END || dp.page_number || '] ' || snippet(document_pages_fts, -1, '<b>', '</b>', '...', 64) as snippet,
               pf.rank as rank, cf.case_id
        FROM document_pages_fts pf
        JOIN document_pages dp ON dp.rowid = pf.rowid
        JOIN case_files cf ON cf.id = dp.file_id
        JOIN document_processing_jobs j ON j.id = dp.job_id
        WHERE document_pages_fts MATCH ?1 AND cf.deleted_at IS NULL
          AND j.status = 'completed'
          AND j.id = (
            SELECT j2.id
            FROM document_processing_jobs j2
            WHERE j2.file_id = cf.id AND j2.status = 'completed'
            ORDER BY j2.rowid DESC LIMIT 1
          )


        UNION ALL
        SELECT 'knowledge',ki.id,ki.title,ki.category,substr(ki.content,max(1,instr(lower(ki.content),lower(?2))-30),180),100,NULL
        FROM knowledge_items ki WHERE COALESCE(ki.status,'current')='current' AND (instr(lower(ki.title),lower(?2))>0 OR instr(lower(ki.content),lower(?2))>0)
        UNION ALL
        SELECT 'file',cf.id,cf.file_name,cf.category,substr(cf.ocr_text,max(1,instr(lower(cf.ocr_text),lower(?2))-30),180),100,cf.case_id
        FROM case_files cf WHERE cf.deleted_at IS NULL AND (instr(lower(cf.file_name),lower(?2))>0 OR instr(lower(COALESCE(cf.ocr_text,'')),lower(?2))>0)
        UNION ALL
        SELECT 'file',cf.id,cf.file_name,cf.category,'[' || dp.page_number || '] ' || substr(dp.plain_text,max(1,instr(lower(dp.plain_text),lower(?2))-30),180),100,cf.case_id
        FROM document_pages dp JOIN case_files cf ON cf.id=dp.file_id JOIN document_processing_jobs j ON j.id=dp.job_id
        WHERE cf.deleted_at IS NULL AND j.status='completed' AND j.id=(SELECT j2.id FROM document_processing_jobs j2 WHERE j2.file_id=cf.id AND j2.status='completed' ORDER BY j2.rowid DESC LIMIT 1)
        AND (instr(lower(dp.plain_text),lower(?2))>0 OR instr(lower(dp.markdown),lower(?2))>0)
        ORDER BY rank LIMIT 50
    "#;

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params![fts_query,query], |row| {
        Ok(GlobalSearchResult {
            item_type: row.get(0)?,
            id: row.get(1)?,
            title: row.get(2)?,
            category: row.get(3)?,
            snippet: row.get(4)?,
            case_id: row.get(6)?,
        })
    })?;

    let mut results = Vec::new();
    let mut seen=std::collections::HashSet::new();
    for row in rows {
        let row=row?;
        if seen.insert((row.item_type.clone(),row.id.clone())){results.push(row);}
    }

    Ok(results)
}

fn to_safe_fts_phrase(query: &str) -> String {
    let normalized = query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('"', "\"\"");
    format!("\"{}\"", normalized)
}

#[tauri::command]
pub async fn global_search(query: String) -> Result<Vec<GlobalSearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    run_blocking(move || {
        let conn = db::open_db()?;
        global_search_inner(&conn, &query)
    })
    .await
}

#[tauri::command]
pub async fn search_document_passages(
    query: String,
    scope: Vec<String>,
) -> Result<Vec<crate::ai::retrieval::DocumentPassage>, String> {
    run_blocking(move || crate::ai::retrieval::search(&*db::open_db()?, &query, &scope, 30)).await
}

#[tauri::command]
pub async fn reasoning_search(
    app: tauri::AppHandle,
    query: String,
    scope: Vec<String>,
) -> Result<String, String> {
    crate::ai::page_index::navigate_and_reason_search(app, &query, scope).await
}

#[cfg(test)]
mod tests {
    use super::{global_search_inner, to_safe_fts_phrase};

    #[test]
    fn global_search_reads_completed_document_pages() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        conn.execute_batch(
            "INSERT INTO cases (id, case_name, client_name, opponent_name)
             VALUES ('c1', '测试案件', '委托人', '相对方');
             INSERT INTO case_files (id, case_id, file_name, file_path, category)
             VALUES ('f1', 'c1', '扫描卷宗.pdf', '/tmp/scan.pdf', 'evidence');
             INSERT INTO document_processing_jobs (id, file_id, source_sha256, status, completed_at, updated_at)
             VALUES ('j1', 'f1', 'sha', 'completed', '2026-09-04 10:00:00', '2026-09-04 10:00:00');
             INSERT INTO document_pages (job_id, file_id, page_number, plain_text, markdown)
             VALUES ('j1', 'f1', 3, '第三页记载行政裁决与侵权事实并行处理', '');",
        )
        .unwrap();

        let results = global_search_inner(&conn, "行政裁决").unwrap();
        assert!(results.iter().any(|item| {
            item.item_type == "file"
                && item.id == "f1"
                && item.snippet.as_deref().unwrap_or_default().contains("[p3]")
        }));
        conn.execute_batch(
            "INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,completed_at,updated_at)
             VALUES('j2','f1','new-sha','completed','2026-09-04 10:00:00','2026-09-04 10:00:00');
             INSERT INTO document_pages(job_id,file_id,page_number,plain_text,markdown)
             VALUES('j2','f1',1,'新版本开庭通知','');"
        ).unwrap();
        assert!(global_search_inner(&conn, "行政裁决").unwrap().is_empty());
        assert!(global_search_inner(&conn, "开庭通知")
            .unwrap()
            .iter()
            .any(|item| item.id == "f1"));
    }

    #[test]
    fn global_search_reads_document_page_markdown() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        conn.execute_batch(
            "INSERT INTO cases (id, case_name, client_name, opponent_name)
             VALUES ('c1', '测试案件', '委托人', '相对方');
             INSERT INTO case_files (id, case_id, file_name, file_path, category)
             VALUES ('f1', 'c1', '扫描卷宗.pdf', '/tmp/scan.pdf', 'evidence');
             INSERT INTO document_processing_jobs (id, file_id, source_sha256, status, completed_at, updated_at)
             VALUES ('j1', 'f1', 'sha', 'completed', '2026-09-04 10:00:00', '2026-09-04 10:00:00');
             INSERT INTO document_pages (job_id, file_id, page_number, plain_text, markdown)
             VALUES ('j1', 'f1', 4, '', '## 行政裁决\n民事侵权事实同步审理');",
        )
        .unwrap();

        let results = global_search_inner(&conn, "行政裁决").unwrap();
        assert!(results.iter().any(|item| {
            item.id == "f1" && item.snippet.as_deref().unwrap_or_default().contains("[p4]")
        }));
    }

    #[test]
    fn global_search_escapes_fts_special_characters() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        conn.execute_batch(
            "INSERT INTO knowledge_items (id, title, category, content)
             VALUES ('k1', '案件号', 'note', '案号包含 abc \"123\" (2026)');",
        )
        .unwrap();

        let results = global_search_inner(&conn, "abc \"123\" (2026)").unwrap();
        assert!(results.iter().any(|item| item.id == "k1"));
        assert_eq!(to_safe_fts_phrase("abc \"123\""), "\"abc \"\"123\"\"\"");
    }
}

use super::run_blocking;
use crate::db;
use serde_json::Value;

fn process_session() -> &'static str {
    static SESSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SESSION.get_or_init(|| uuid::Uuid::new_v4().to_string())
}

fn session_key(session_id: &str) -> anyhow::Result<String> {
    anyhow::ensure!(uuid::Uuid::parse_str(session_id).is_ok(), "编辑会话无效");
    Ok(format!("editor_recovery:{session_id}"))
}

#[tauri::command]
pub async fn save_editor_recovery(session_id: String, draft: Option<Value>) -> Result<(), String> {
    run_blocking(move || {
        let key = session_key(&session_id)?;
        let conn = db::open_db()?;
        match draft {
            Some(draft) => {
                anyhow::ensure!(draft["id"].as_str().is_some() && draft["content"].as_str().is_some(), "编辑草稿格式无效");
                let payload = serde_json::json!({"draft":draft,"processId":std::process::id(),"processSession":process_session(),"savedAt":db::now_local()});
                db::set_setting(&conn, &key, &payload.to_string())?;
            }
            None => { conn.execute("DELETE FROM settings WHERE key=?1", [&key])?; }
        }
        Ok(())
    }).await
}

#[tauri::command]
pub async fn recover_editor_drafts() -> Result<usize, String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;
        let mut stmt = tx.prepare("SELECT key,value FROM settings WHERE key LIKE 'editor_recovery:%'")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        let mut count = 0;
        for (key, value) in rows {
            let payload: Value = serde_json::from_str(&value)?;
            if payload["processSession"].as_str() == Some(process_session()) { continue; }
            let draft = &payload["draft"];
            let content = draft["content"].as_str().unwrap_or("");
            let id = draft["id"].as_str().unwrap_or("");
            if draft["kind"].as_str() == Some("document") {
                let title = draft["title"].as_str().unwrap_or("未命名文书");
                let same: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM drafts WHERE id=?1 AND content=?2 AND title=?3)",rusqlite::params![id,content,title],|r|r.get(0))?;
                if !same {
                    tx.execute("INSERT INTO drafts(id,title,content,status,version) VALUES(?1,?2,?3,'draft',1)",rusqlite::params![db::new_id(),format!("{title}（恢复草稿）"),content])?;
                    count += 1;
                }
                tx.execute("DELETE FROM settings WHERE key=?1", [&key])?;
                continue;
            }
            let same: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_items WHERE id=?1 AND content=?2 AND title=?3)", rusqlite::params![id,content,draft["title"].as_str().unwrap_or("")], |r|r.get(0))?;
            if !same {
                let restored_id = db::new_id();
                let title = format!("{}（恢复草稿）", draft["title"].as_str().unwrap_or("未命名"));
                tx.execute("INSERT INTO knowledge_items(id,title,content,category,block_type,status) VALUES(?1,?2,?3,'reference','page','current')", rusqlite::params![restored_id,title,content])?;
                super::knowledge::sync_wiki_links(&tx, &restored_id, content)?;
                count += 1;
            }
            tx.execute("DELETE FROM settings WHERE key=?1", [&key])?;
        }
        tx.commit()?;
        Ok(count)
    }).await
}

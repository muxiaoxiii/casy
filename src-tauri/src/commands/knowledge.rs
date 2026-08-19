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
                    "parentId": row.get::<_, Option<String>>("parent_id")?,
                    "blockType": row.get::<_, Option<String>>("block_type")?,
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

        // 块级化（§8.2）：block_type 限 page/block/reference，缺省 'page'
        let block_type = match data["blockType"].as_str() {
            Some("block") => "block",
            Some("reference") => "reference",
            _ => "page",
        };

        conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content, tags, source_type, source_id,
             linked_case_id, law_name, article_no, effective_date, status, parent_id, block_type,
             created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
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
                data["parentId"].as_str(),
                block_type,
                now,
            ],
        )?;

        Ok(id)
    })
    .await
}

/// 列出某知识条目下的块（§8.2 知识块级化）
#[tauri::command]
pub async fn list_knowledge_blocks(parent_id: String) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, category, content, tags, block_type, created_at, updated_at
             FROM knowledge_items WHERE parent_id = ?1 ORDER BY created_at ASC",
        )?;
        let blocks: Vec<serde_json::Value> = stmt
            .query_map(rusqlite::params![parent_id], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "category": row.get::<_, String>(2)?,
                    "content": row.get::<_, String>(3)?,
                    "tags": row.get::<_, Option<String>>(4)?,
                    "blockType": row.get::<_, Option<String>>(5)?,
                    "createdAt": row.get::<_, Option<String>>(6)?,
                    "updatedAt": row.get::<_, Option<String>>(7)?,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(blocks)
    })
    .await
}

/// 获取知识条目及其块树（§8.2；深度上限 5 层、总块数上限 100，防自引用环）
#[tauri::command]
pub async fn get_knowledge_with_blocks(id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let item = conn
            .query_row(
                "SELECT id, title, category, content, tags, source_type, linked_case_id,
                        parent_id, block_type, created_at, updated_at
                 FROM knowledge_items WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    Ok(serde_json::json!({
                        "id": row.get::<_, String>(0)?,
                        "title": row.get::<_, String>(1)?,
                        "category": row.get::<_, String>(2)?,
                        "content": row.get::<_, String>(3)?,
                        "tags": row.get::<_, Option<String>>(4)?,
                        "sourceType": row.get::<_, Option<String>>(5)?,
                        "linkedCaseId": row.get::<_, Option<String>>(6)?,
                        "parentId": row.get::<_, Option<String>>(7)?,
                        "blockType": row.get::<_, Option<String>>(8)?,
                        "createdAt": row.get::<_, Option<String>>(9)?,
                        "updatedAt": row.get::<_, Option<String>>(10)?,
                    }))
                },
            )
            .map_err(|e| anyhow::anyhow!("知识条目不存在: {}", e))?;

        // 逐层展开块树（BFS，带深度与总量上限；已访问集合防环）
        const MAX_DEPTH: usize = 5;
        const MAX_BLOCKS: usize = 100;
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        visited.insert(id.clone());
        let mut blocks: Vec<serde_json::Value> = Vec::new();
        let mut frontier = vec![id.clone()];

        for _ in 0..MAX_DEPTH {
            if frontier.is_empty() || blocks.len() >= MAX_BLOCKS {
                break;
            }
            let mut next_frontier: Vec<String> = Vec::new();
            for pid in &frontier {
                let mut stmt = conn.prepare(
                    "SELECT id, title, category, content, tags, block_type, parent_id, created_at, updated_at
                     FROM knowledge_items WHERE parent_id = ?1 ORDER BY created_at ASC",
                )?;
                let rows: Vec<serde_json::Value> = stmt
                    .query_map(rusqlite::params![pid], |row| {
                        Ok(serde_json::json!({
                            "id": row.get::<_, String>(0)?,
                            "title": row.get::<_, String>(1)?,
                            "category": row.get::<_, String>(2)?,
                            "content": row.get::<_, String>(3)?,
                            "tags": row.get::<_, Option<String>>(4)?,
                            "blockType": row.get::<_, Option<String>>(5)?,
                            "parentId": row.get::<_, Option<String>>(6)?,
                            "createdAt": row.get::<_, Option<String>>(7)?,
                            "updatedAt": row.get::<_, Option<String>>(8)?,
                        }))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();
                for block in rows {
                    if blocks.len() >= MAX_BLOCKS {
                        break;
                    }
                    if let Some(bid) = block["id"].as_str() {
                        if visited.insert(bid.to_string()) {
                            next_frontier.push(bid.to_string());
                            blocks.push(block);
                        }
                    }
                }
            }
            frontier = next_frontier;
        }

        Ok(serde_json::json!({
            "item": item,
            "blocks": blocks,
        }))
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

/// 知识图谱数据：真实节点与边（禁止随机生成）
///
/// - nodes：知识条目（最近更新 N=100）、被知识引用的案件、关联知识的任务
/// - edges：knowledge_relations 知识↔知识关系、
///   knowledge_items.linked_case_id 知识→案件、tasks.knowledge_id 任务→知识
#[tauri::command]
pub async fn get_knowledge_graph(limit: Option<usize>) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let limit = limit.unwrap_or(100).clamp(1, 500);

        let mut nodes: Vec<serde_json::Value> = Vec::new();
        let mut edges: Vec<serde_json::Value> = Vec::new();
        let mut knowledge_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut case_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        // 知识节点（最近更新优先）
        {
            let mut stmt = conn.prepare(
                "SELECT id, title, category, linked_case_id FROM knowledge_items
                 ORDER BY updated_at DESC LIMIT ?1",
            )?;
            let items: Vec<(String, String, String, Option<String>)> = stmt
                .query_map([limit as i64], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            for (id, title, category, linked_case_id) in items {
                // 知识 → 案件 边（linked_case_id 外键）
                if let Some(cid) = linked_case_id {
                    if !cid.is_empty() {
                        edges.push(serde_json::json!({
                            "source": format!("k-{}", id),
                            "target": format!("c-{}", cid),
                            "type": "linked_case",
                        }));
                        case_ids.insert(cid);
                    }
                }
                nodes.push(serde_json::json!({
                    "id": format!("k-{}", id),
                    "name": title,
                    "type": "knowledge",
                    "category": category,
                }));
                knowledge_ids.insert(id);
            }
        }

        // 知识 ↔ 知识 边（knowledge_relations 表，两端都需在节点集内）
        {
            let mut stmt = conn.prepare(
                "SELECT source_id, target_id, relation_type FROM knowledge_relations",
            )?;
            let rels: Vec<(String, String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (source, target, rel_type) in rels {
                if knowledge_ids.contains(&source) && knowledge_ids.contains(&target) {
                    edges.push(serde_json::json!({
                        "source": format!("k-{}", source),
                        "target": format!("k-{}", target),
                        "type": rel_type,
                    }));
                }
            }
        }

        // 案件节点（仅被知识条目引用的）
        if !case_ids.is_empty() {
            let mut sorted: Vec<&String> = case_ids.iter().collect();
            sorted.sort();
            let placeholders = sorted.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT id, case_name FROM cases WHERE id IN ({})",
                placeholders
            );
            let mut stmt = conn.prepare(&sql)?;
            let cases: Vec<(String, String)> = stmt
                .query_map(rusqlite::params_from_iter(sorted.iter()), |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (id, name) in cases {
                nodes.push(serde_json::json!({
                    "id": format!("c-{}", id),
                    "name": name,
                    "type": "case",
                }));
            }
        }

        // 任务节点 + 任务 → 知识 边（tasks.knowledge_id 外键）
        {
            let mut stmt = conn.prepare(
                "SELECT id, task_name, knowledge_id FROM tasks WHERE knowledge_id IS NOT NULL",
            )?;
            let tasks: Vec<(String, String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (id, name, kid) in tasks {
                if knowledge_ids.contains(&kid) {
                    nodes.push(serde_json::json!({
                        "id": format!("t-{}", id),
                        "name": name,
                        "type": "task",
                    }));
                    edges.push(serde_json::json!({
                        "source": format!("t-{}", id),
                        "target": format!("k-{}", kid),
                        "type": "references",
                    }));
                }
            }
        }

        Ok(serde_json::json!({ "nodes": nodes, "edges": edges }))
    })
    .await
}

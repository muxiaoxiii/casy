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
pub async fn list_knowledge(
    filter: Option<KnowledgeFilter>,
) -> Result<Vec<serde_json::Value>, String> {
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

        sql.push_str(" ORDER BY updated_at DESC LIMIT 2000");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
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

/// 列出某知识条目下的块（§8.2 知识块级化）
#[tauri::command]
pub async fn list_knowledge_blocks(parent_id: String) -> Result<Vec<KnowledgeBlockDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, category, content, tags, block_type, created_at, updated_at
             FROM knowledge_items WHERE parent_id = ?1 ORDER BY created_at ASC",
        )?;
        let blocks: Vec<KnowledgeBlockDto> = stmt
            .query_map(rusqlite::params![parent_id], |row| {
                Ok(KnowledgeBlockDto {
                    id: row.get::<_, String>(0)?,
                    title: row.get::<_, String>(1)?,
                    category: row.get::<_, String>(2)?,
                    content: row.get::<_, String>(3)?,
                    tags: row.get::<_, Option<String>>(4)?,
                    block_type: row.get::<_, Option<String>>(5)?,
                    created_at: row.get::<_, Option<String>>(6)?,
                    updated_at: row.get::<_, Option<String>>(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(blocks)
    })
    .await
}

/// 获取知识条目及其块树（§8.2；深度上限 5 层、总块数上限 100，防自引用环）
#[tauri::command]
pub async fn get_knowledge_with_blocks(id: String) -> Result<KnowledgeWithBlocksDto, String> {
    run_blocking(move || {
        const MAX_DEPTH: usize = 5;
        const MAX_BLOCKS: usize = 100;
        let conn = db::open_db()?;

        let item = conn
            .query_row(
                "SELECT id, title, category, content, tags, source_type, linked_case_id,
                        parent_id, block_type, created_at, updated_at
                 FROM knowledge_items WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    Ok(KnowledgeItemDto {
                        id: row.get::<_, String>(0)?,
                        title: row.get::<_, String>(1)?,
                        category: row.get::<_, String>(2)?,
                        content: row.get::<_, String>(3)?,
                        tags: row.get::<_, Option<String>>(4)?,
                        source_type: row.get::<_, Option<String>>(5)?,
                        linked_case_id: row.get::<_, Option<String>>(6)?,
                        parent_id: row.get::<_, Option<String>>(7)?,
                        block_type: row.get::<_, Option<String>>(8)?,
                        created_at: row.get::<_, Option<String>>(9)?,
                        updated_at: row.get::<_, Option<String>>(10)?,
                    })
                },
            )
            .map_err(|e| anyhow::anyhow!("知识条目不存在: {}", e))?;

        let mut blocks: Vec<KnowledgeTreeBlockDto> = Vec::new();
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        visited.insert(item.id.clone());

        let mut frontier: Vec<String> = vec![id.clone()];
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
                let rows: Vec<KnowledgeTreeBlockDto> = stmt
                    .query_map(rusqlite::params![pid], |row| {
                        Ok(KnowledgeTreeBlockDto {
                            id: row.get::<_, String>(0)?,
                            title: row.get::<_, String>(1)?,
                            category: row.get::<_, String>(2)?,
                            content: row.get::<_, String>(3)?,
                            tags: row.get::<_, Option<String>>(4)?,
                            block_type: row.get::<_, Option<String>>(5)?,
                            parent_id: row.get::<_, Option<String>>(6)?,
                            created_at: row.get::<_, Option<String>>(7)?,
                            updated_at: row.get::<_, Option<String>>(8)?,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                for block in rows {
                    if blocks.len() >= MAX_BLOCKS {
                        break;
                    }
                    if visited.insert(block.id.clone()) {
                        next_frontier.push(block.id.clone());
                        blocks.push(block);
                    }
                }
            }
            frontier = next_frontier;
        }

        Ok(KnowledgeWithBlocksDto { item, blocks })
    })
    .await
}

/// 编辑会话快照：内容真正变化时才记录；5 分钟窗口只计 edit_session，
/// before_restore 快照不占用窗口（恢复后立即编辑仍会留存恢复态快照）。
fn maybe_snapshot_edit(
    conn: &rusqlite::Connection,
    id: &str,
    next_content: &str,
) -> anyhow::Result<()> {
    let old: String = conn.query_row(
        "SELECT content FROM knowledge_items WHERE id = ?1",
        [id],
        |r| r.get(0),
    )?;
    if next_content == old {
        return Ok(());
    }
    let recent: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_versions
         WHERE item_id=?1 AND change_reason='edit_session'
           AND changed_at >= datetime('now','localtime','-5 minutes'))",
        [id],
        |row| row.get(0),
    )?;
    if recent == 0 {
        conn.execute(
            "INSERT INTO knowledge_versions (id, item_id, content, change_reason) VALUES (?1, ?2, ?3, 'edit_session')",
            rusqlite::params![db::new_id(), id, old],
        )?;
    }
    Ok(())
}

/// 上级笔记校验：必须存在、不能是自己、不能形成祖先循环。
/// CTE 用 UNION（去重）而非 UNION ALL：即使历史脏数据已存在环也能终止。
fn validate_parent(conn: &rusqlite::Connection, id: &str, parent: &str) -> anyhow::Result<()> {
    if parent == id {
        return Err(anyhow::anyhow!("笔记不能把自己设为上级笔记"));
    }
    let parent_exists: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_items WHERE id=?1)",
        [parent],
        |row| row.get(0),
    )?;
    if parent_exists == 0 {
        return Err(anyhow::anyhow!("上级笔记不存在"));
    }
    let creates_cycle: i64 = conn.query_row(
        "WITH RECURSIVE ancestors(id) AS (
           SELECT parent_id FROM knowledge_items WHERE id = ?1
           UNION
           SELECT k.parent_id FROM knowledge_items k JOIN ancestors a ON k.id = a.id
           WHERE k.parent_id IS NOT NULL
         ) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id = ?2)",
        rusqlite::params![parent, id],
        |row| row.get(0),
    )?;
    if creates_cycle != 0 {
        return Err(anyhow::anyhow!("该上级笔记会形成循环层级"));
    }
    Ok(())
}

/// 解析正文中的 [[笔记标题]]（标题内不允许 [ 或 ]，与 Obsidian 一致）
fn parse_wiki_titles(content: &str) -> Vec<String> {
    let re = regex::Regex::new(r"\[\[([^\[\]]+)\]\]").unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for cap in re.captures_iter(content) {
        let title = cap[1].trim().to_string();
        if !title.is_empty() && seen.insert(title.to_lowercase()) {
            out.push(title);
        }
    }
    out
}

/// Wiki 双链权威同步（后端）：以正文 [[标题]] 为事实源，
/// 只管辖 anchor 以 "wiki:" 开头的自动链接；手动关联与其他系统链接（pageindex:structure 等）不受影响。
pub(crate) fn sync_wiki_links(
    conn: &rusqlite::Connection,
    item_id: &str,
    content: &str,
) -> anyhow::Result<()> {
    let titles = parse_wiki_titles(content);
    // 标题 → 目标笔记（大小写不敏感，排除自身与块；同名取最早创建者，与前端行为一致）
    let mut desired: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for title in titles {
        let hit: Option<String> = conn
            .query_row(
                "SELECT id FROM knowledge_items
                 WHERE LOWER(title) = LOWER(?1) AND id != ?2
                   AND COALESCE(block_type, 'page') != 'block'
                 ORDER BY created_at ASC LIMIT 1",
                rusqlite::params![title, item_id],
                |r| r.get(0),
            )
            .ok();
        if let Some(target) = hit {
            desired.entry(target).or_insert(title);
        }
    }

    let mut stmt = conn.prepare(
        "SELECT id, target_id FROM links
         WHERE source_type='knowledge' AND source_id=?1
           AND target_type='knowledge' AND anchor LIKE 'wiki:%'",
    )?;
    let automatic: Vec<(String, String)> = stmt
        .query_map([item_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    for (link_id, target_id) in &automatic {
        if !desired.contains_key(target_id) {
            conn.execute("DELETE FROM links WHERE id = ?1", [link_id])?;
        }
    }
    for (target_id, title) in &desired {
        if automatic.iter().any(|(_, t)| t == target_id) {
            continue;
        }
        conn.execute(
            "INSERT INTO links (id, source_type, source_id, target_type, target_id, anchor, label)
             VALUES (?1, 'knowledge', ?2, 'knowledge', ?3, ?4, ?5)",
            rusqlite::params![
                db::new_id(),
                item_id,
                target_id,
                format!("wiki:{title}"),
                title
            ],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub async fn update_knowledge(id: String, data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;

        // 不存在即报错：旧实现对不存在的 id 静默成功，前端会误显示"已保存"
        let exists: i64 = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM knowledge_items WHERE id=?1)",
            [&id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(anyhow::anyhow!("知识条目不存在: {id}"));
        }

        if let Some(expected) = data.get("expectedContent").and_then(|v| v.as_str()) {
            let current: String = tx.query_row("SELECT content FROM knowledge_items WHERE id=?1", [&id], |r| r.get(0))?;
            if current != expected {
                return Err(anyhow::anyhow!("EDIT_CONFLICT: 此笔记已在其他窗口修改，当前修改已保留为恢复草稿，请重新打开原笔记后合并"));
            }
        }

        if let Some(next_content) = data.get("content").and_then(|value| value.as_str()) {
            maybe_snapshot_edit(&tx, &id, next_content)?;
        }

        if let Some(parent) = data.get("parentId").and_then(|value| value.as_str()) {
            validate_parent(&tx, &id, parent)?;
        }

        let mut sql =
            String::from("UPDATE knowledge_items SET updated_at = datetime('now','localtime')");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        let fields = [
            ("title", "title"),
            ("category", "category"),
            ("content", "content"),
            ("tags", "tags"),
            ("lawName", "law_name"),
            ("articleNo", "article_no"),
            ("effectiveDate", "effective_date"),
            ("status", "status"),
            ("linkedCaseId", "linked_case_id"),
            ("parentId", "parent_id"),
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
        params.push(Box::new(id.clone()));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        tx.execute(&sql, param_refs.as_slice())?;

        // Wiki 双链权威同步：任何写入路径（笔记本/MCP/AI）都保持一致
        if let Some(content) = data.get("content").and_then(|value| value.as_str()) {
            sync_wiki_links(&tx, &id, content)?;
        }

        tx.commit()?;
        Ok(())
    })
    .await
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDocumentSourceDto {
    pub file_id: String,
    pub case_id: String,
    pub file_name: String,
    pub case_name: String,
    pub total_pages: i64,
    pub markdown_path: Option<String>,
    pub searchable_pdf_path: Option<String>,
    pub imported_knowledge_id: Option<String>,
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PageIndexImportResultDto {
    pub knowledge_id: String,
    pub child_count: usize,
    pub reused: bool,
}

/// 已完成 OCR/PageIndex 的卷宗来源，供知识库选择性沉淀。
#[tauri::command]
pub async fn list_knowledge_document_sources() -> Result<Vec<KnowledgeDocumentSourceDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT cf.id, cf.case_id, cf.file_name, c.case_name,
                    j.total_pages, j.markdown_path, j.searchable_pdf_path,
                    (SELECT ki.id FROM knowledge_items ki
                     WHERE ki.source_type = 'ocr-pageindex' AND ki.source_id = cf.id
                       AND EXISTS (SELECT 1 FROM links l WHERE l.source_type='knowledge'
                         AND l.source_id=ki.id AND l.target_type='file' AND l.target_id=cf.id
                         AND l.anchor='source:document-job:' || j.id)
                     ORDER BY ki.created_at DESC LIMIT 1)
             FROM case_files cf
             JOIN cases c ON c.id = cf.case_id
             JOIN document_processing_jobs j ON j.id = (
               SELECT j2.id FROM document_processing_jobs j2
               WHERE j2.file_id = cf.id AND j2.status = 'completed'
               ORDER BY j2.rowid DESC LIMIT 1
             )
             WHERE cf.deleted_at IS NULL ORDER BY j.updated_at DESC LIMIT 200",
        )?;
        let items = stmt
            .query_map([], |row| {
                Ok(KnowledgeDocumentSourceDto {
                    file_id: row.get(0)?,
                    case_id: row.get(1)?,
                    file_name: row.get(2)?,
                    case_name: row.get(3)?,
                    total_pages: row.get(4)?,
                    markdown_path: row.get(5)?,
                    searchable_pdf_path: row.get(6)?,
                    imported_knowledge_id: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    })
    .await
}

/// 将 OCR Markdown 全文作为根笔记，并把 PageIndex 结构节点作为子笔记沉淀（可测内层；
/// pub 供 examples/real_db_regression.rs 对真实库副本做端到端回归）。
/// 同一处理版本复用既有根笔记；新版生成独立快照，保留旧笔记及人工修改。
pub fn import_pageindex_inner(
    conn: &mut rusqlite::Connection,
    file_id: &str,
) -> anyhow::Result<PageIndexImportResultDto> {
    let tx = conn.transaction()?;

    let (case_id, case_name, file_name, markdown_path, searchable_pdf_path, latest_job): (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    ) = tx
        .query_row(
            "SELECT cf.case_id,c.case_name,cf.file_name,j.markdown_path,j.searchable_pdf_path,j.id
             FROM case_files cf JOIN cases c ON c.id=cf.case_id
             JOIN document_processing_jobs j ON j.id=(
               SELECT j2.id FROM document_processing_jobs j2
               WHERE j2.file_id=cf.id AND j2.status='completed'
               ORDER BY j2.rowid DESC LIMIT 1)
             WHERE cf.id=?1 AND cf.deleted_at IS NULL",
            [file_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .map_err(|_| anyhow::anyhow!("该文件尚未完成正文提取与索引"))?;

    let revision_anchor = format!("source:document-job:{latest_job}");
    use rusqlite::OptionalExtension;
    if let Some(existing) = tx.query_row(
        "SELECT ki.id FROM knowledge_items ki
         JOIN links l ON l.source_type='knowledge' AND l.source_id=ki.id
           AND l.target_type='file' AND l.target_id=?1 AND l.anchor=?2
         WHERE ki.source_type='ocr-pageindex' AND ki.source_id=?1
         ORDER BY ki.rowid DESC LIMIT 1",
        rusqlite::params![file_id, revision_anchor],
        |row| row.get::<_, String>(0),
    ).optional()? {
        let child_count = tx.query_row(
            "WITH RECURSIVE children(id) AS (
               SELECT id FROM knowledge_items WHERE parent_id=?1
               UNION SELECT ki.id FROM knowledge_items ki JOIN children c ON ki.parent_id=c.id
             ) SELECT COUNT(*) FROM children",
            [&existing], |row| row.get::<_, i64>(0),
        )? as usize;
        tx.commit()?;
        return Ok(PageIndexImportResultDto { knowledge_id: existing, child_count, reused: true });
    }

    let full_markdown = match markdown_path.as_deref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .filter(|text| !text.trim().is_empty()) {
        Some(text) => text,
        None => {
            let mut output = String::new();
            let mut pages = tx.prepare(
                "SELECT page_number,markdown,plain_text FROM document_pages WHERE job_id=?1 ORDER BY page_number"
            )?;
            let rows = pages.query_map([latest_job.as_str()], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })?;
            for row in rows {
                let (page, markdown, text) = row?;
                let content = if markdown.trim().is_empty() { text } else { markdown };
                output.push_str(&format!("\n\n<!-- page:{page} -->\n\n{content}"));
            }
            output
        }
    };
    let full_markdown = if let Some(root) = markdown_path.as_deref().and_then(|path| std::path::Path::new(path).parent()).filter(|root|root.join(crate::document_pipeline::assets::MANIFEST).exists()) {
        let manifest=crate::document_pipeline::assets::load(root)?;
        crate::document_pipeline::assets::inline(&full_markdown,root,&manifest)?
    } else { full_markdown };
    if full_markdown.trim().is_empty() {
        return Err(anyhow::anyhow!("文档处理已完成，但没有可导入的 Markdown 内容"));
    }

    // Only clean structures whose snapshot root has disappeared. Live older snapshots remain editable.
    let orphans: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT ki.id FROM knowledge_items ki
             JOIN links structure ON structure.source_type='knowledge' AND structure.source_id=ki.id
               AND structure.target_type='knowledge' AND structure.anchor='pageindex:structure'
             JOIN links origin ON origin.source_type='knowledge' AND origin.source_id=structure.target_id
               AND origin.target_type='file' AND origin.target_id=?1
             WHERE ki.source_type='pageindex-node'
               AND NOT EXISTS(SELECT 1 FROM knowledge_items root WHERE root.id=structure.target_id)",
        )?;
        let rows = stmt.query_map([file_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    for id in orphans {
        tx.execute("DELETE FROM links WHERE (source_type='knowledge' AND source_id=?1)
                    OR (target_type='knowledge' AND target_id=?1)", [&id])?;
        tx.execute("DELETE FROM knowledge_items WHERE id=?1", [&id])?;
    }

    let root_id = db::new_id();
    let root_title = format!("[卷宗] {}", file_name);
    let source_meta = format!(
        "> 来源案件：{}\n> 原始文件：{}\n> 可搜索 PDF：{}\n\n",
        case_name,
        file_name,
        searchable_pdf_path.as_deref().unwrap_or("文本来源，按段索引")
    );
    tx.execute(
        "INSERT INTO knowledge_items(id,title,category,content,tags,source_type,source_id,linked_case_id,status,block_type)
         VALUES(?1,?2,'document_summary',?3,'OCR,PageIndex,卷宗沉淀','ocr-pageindex',?4,?5,'current','page')",
        rusqlite::params![root_id, root_title, format!("{}{}", source_meta, full_markdown), file_id, case_id],
    )?;
    tx.execute(
        "INSERT INTO links(id,source_type,source_id,target_type,target_id,anchor,label)
         VALUES(?1,'knowledge',?2,'file',?3,?4,'原始文档')",
        rusqlite::params![db::new_id(), root_id, file_id, revision_anchor],
    )?;

    let mut nodes_stmt = tx.prepare(
        "SELECT id,parent_id,title,summary,level,page_start,page_end
         FROM page_index_nodes WHERE file_id=?1 AND level>0 ORDER BY page_start,level LIMIT 500",
    )?;
    let nodes = nodes_stmt
        .query_map([file_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(nodes_stmt);
    let mut id_map = std::collections::HashMap::<String, String>::new();
    let mut child_count = 0usize;
    for (node_id, parent_node_id, title, summary, _level, page_start, page_end) in nodes {
        let knowledge_id = db::new_id();
        let parent_id = parent_node_id
            .as_ref()
            .and_then(|id| id_map.get(id))
            .cloned()
            .unwrap_or_else(|| root_id.clone());
        // 结构归属由 links 表（anchor='pageindex:structure'）承载；正文只用纯文本注明来源，
        // 不用 [[根标题]]：根标题含方括号，Wiki 语法无法解析，且该链接不应被 Wiki 自动回收管辖。
        let location = if searchable_pdf_path.is_some() { "页码" } else { "段落" };
        let content = format!(
            "> 来源卷宗：{}\n> {}：{}-{}\n\n{}",
            root_title, location, page_start, page_end, summary
        );
        tx.execute(
            "INSERT INTO knowledge_items(id,title,category,content,tags,source_type,source_id,linked_case_id,status,parent_id,block_type)
             VALUES(?1,?2,'document_summary',?3,'PageIndex,结构节点','pageindex-node',?4,?5,'current',?6,'page')",
            rusqlite::params![knowledge_id,title,content,node_id,case_id,parent_id],
        )?;
        tx.execute(
            "INSERT INTO links(id,source_type,source_id,target_type,target_id,anchor,label)
             VALUES(?1,'knowledge',?2,'knowledge',?3,'pageindex:structure',?4)",
            rusqlite::params![db::new_id(), knowledge_id, root_id, root_title],
        )?;
        id_map.insert(node_id, knowledge_id);
        child_count += 1;
    }
    tx.commit()?;
    Ok(PageIndexImportResultDto {
        knowledge_id: root_id,
        child_count,
        reused: false,
    })
}

/// 将 OCR Markdown 全文作为根笔记，并把 PageIndex 结构节点作为子笔记沉淀。
/// 同一处理版本复用既有笔记；新版保留为独立快照。
#[tauri::command]
pub async fn import_pageindex_to_knowledge(
    file_id: String,
) -> Result<PageIndexImportResultDto, String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        import_pageindex_inner(&mut conn, &file_id)
    })
    .await
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeExportDto {
    pub output_path: String,
    pub file_size: u64,
    pub exported_at: String,
}

fn export_knowledge_markdown_inner(
    conn: &rusqlite::Connection,
    item_id: &str,
    output_path: &str,
) -> anyhow::Result<KnowledgeExportDto> {
    // 安全（审查 P0-5）：原本仅校验「绝对路径 + .md 扩展名」，未消除 `..` 穿越，
    // 也未校验目录归属。统一走导出路径校验：绝对路径 + 扩展名白名单 +
    // canonicalize 父目录后重建完整路径。
    let path = crate::docsy_engine::output_path::resolve_explicit_output_path(
        output_path,
        &["md", "markdown"],
    )?;

    let (title, content): (String, String) = conn
        .query_row(
            "SELECT title, content FROM knowledge_items WHERE id=?1",
            [item_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| anyhow::anyhow!("知识条目不存在: {item_id}"))?;

    let markdown = if content.trim().is_empty() {
        format!("# {}\n", title.trim())
    } else {
        format!("{}\n", content.trim_end())
    };
    let destination = path.parent().ok_or_else(|| anyhow::anyhow!("导出目录无效"))?;
    let (markdown, assets) = super::markdown_export::externalize_images(&markdown, destination)?;
    use std::io::Write;
    let mut stage = tempfile::NamedTempFile::new_in(destination)?;
    stage.write_all(markdown.as_bytes())?;
    stage.as_file().sync_all()?;
    stage.persist(&path)?;
    if let Some(assets) = assets { let _ = assets.keep(); }
    let metadata = std::fs::metadata(&path)?;
    Ok(KnowledgeExportDto {
        output_path: path.to_string_lossy().into_owned(),
        file_size: metadata.len(),
        exported_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

/// 导出单篇知识笔记 Markdown。
///
/// 正文始终从已保存的数据库读取，前端必须先 flush/save 当前编辑事务，避免导出旧内容。
#[tauri::command]
pub async fn export_knowledge_markdown(
    item_id: String,
    output_path: String,
) -> Result<KnowledgeExportDto, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        export_knowledge_markdown_inner(&conn, &item_id, &output_path)
    })
    .await
}

/// 恢复历史版本（可测内层）：归属校验 + before_restore 快照 + Wiki 双链重同步
fn restore_version_inner(
    conn: &mut rusqlite::Connection,
    item_id: &str,
    version_id: &str,
) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    let historical: String = tx
        .query_row(
            "SELECT content FROM knowledge_versions WHERE id=?1 AND item_id=?2",
            rusqlite::params![version_id, item_id],
            |row| row.get(0),
        )
        .map_err(|_| anyhow::anyhow!("历史版本不存在或不属于该笔记"))?;
    let current: String = tx.query_row(
        "SELECT content FROM knowledge_items WHERE id=?1",
        [item_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO knowledge_versions(id,item_id,content,change_reason) VALUES(?1,?2,?3,'before_restore')",
        rusqlite::params![db::new_id(), item_id, current],
    )?;
    tx.execute(
        "UPDATE knowledge_items SET content=?1,updated_at=datetime('now','localtime') WHERE id=?2",
        rusqlite::params![historical, item_id],
    )?;
    // 正文变了，Wiki 自动链接同步收敛（后端权威，前端无需再 diff）
    sync_wiki_links(&tx, item_id, &historical)?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub async fn restore_knowledge_version(item_id: String, version_id: String) -> Result<(), String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        restore_version_inner(&mut conn, &item_id, &version_id)
    })
    .await
}

/// 删除笔记：连带清理 links 孤儿行，并把子笔记重挂到被删笔记的上级（Trilium 式提升）。
#[tauri::command]
pub async fn delete_knowledge(id: String) -> Result<(), String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;
        let parent: Option<String> = tx
            .query_row(
                "SELECT parent_id FROM knowledge_items WHERE id = ?1",
                [&id],
                |r| r.get(0),
            )
            .map_err(|_| anyhow::anyhow!("知识条目不存在: {id}"))?;
        // 子笔记重挂到祖父级（或回到根），避免 parent_id 悬空
        tx.execute(
            "UPDATE knowledge_items SET parent_id = ?2 WHERE parent_id = ?1",
            rusqlite::params![id, parent],
        )?;
        // links 表无 FK，显式清理双向孤儿行
        tx.execute(
            "DELETE FROM links WHERE (source_type='knowledge' AND source_id=?1)
             OR (target_type='knowledge' AND target_id=?1)",
            [&id],
        )?;
        tx.execute("DELETE FROM knowledge_items WHERE id = ?1", [&id])?;
        tx.commit()?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn search_knowledge(query: String) -> Result<Vec<SearchKnowledgeDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        search_knowledge_inner(&conn, &query)
    })
    .await
}

fn safe_fts_phrase(query: &str) -> String {
    let normalized = query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('"', "\"\"");
    format!("\"{}\"", normalized)
}

fn read_search_knowledge_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchKnowledgeDto> {
    Ok(SearchKnowledgeDto {
        id: row.get::<_, String>("id")?,
        title: row.get::<_, String>("title")?,
        category: row.get::<_, String>("category")?,
        content: row.get::<_, String>("content")?,
        tags: row.get::<_, Option<String>>("tags")?,
        law_name: row.get::<_, Option<String>>("law_name")?,
        article_no: row.get::<_, Option<String>>("article_no")?,
    })
}

fn search_knowledge_inner(
    conn: &rusqlite::Connection,
    query: &str,
) -> anyhow::Result<Vec<SearchKnowledgeDto>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut items: Vec<SearchKnowledgeDto> = {
        let mut stmt = conn.prepare(
            "SELECT ki.* FROM knowledge_fts f JOIN knowledge_items ki ON ki.rowid = f.rowid
             WHERE knowledge_fts MATCH ?1 ORDER BY rank LIMIT 50",
        )?;
        let fts_query = safe_fts_phrase(query);
        let rows = stmt.query_map(rusqlite::params![fts_query], read_search_knowledge_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    if items.len() >= 50 {
        return Ok(items);
    }

    let mut seen: std::collections::HashSet<String> =
        items.iter().map(|item| item.id.clone()).collect();
    let like_query = format!("%{}%", query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn.prepare(
        "SELECT * FROM knowledge_items
         WHERE title LIKE ?1 ESCAPE '\\'
            OR content LIKE ?1 ESCAPE '\\'
            OR COALESCE(tags, '') LIKE ?1 ESCAPE '\\'
            OR COALESCE(law_name, '') LIKE ?1 ESCAPE '\\'
            OR COALESCE(article_no, '') LIKE ?1 ESCAPE '\\'
         ORDER BY updated_at DESC LIMIT 50",
    )?;
    let fallback_rows = stmt.query_map(rusqlite::params![like_query], read_search_knowledge_row)?;
    for row in fallback_rows {
        let item = row?;
        if seen.insert(item.id.clone()) {
            items.push(item);
            if items.len() >= 50 {
                break;
            }
        }
    }

    Ok(items)
}

#[tauri::command]
pub async fn knowledge_stats() -> Result<KnowledgeStatsDto, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let total: i64 =
            conn.query_row("SELECT COUNT(*) FROM knowledge_items", [], |r| r.get(0))?;

        let mut stmt =
            conn.prepare("SELECT category, COUNT(*) FROM knowledge_items GROUP BY category")?;
        let by_category: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(KnowledgeStatsDto { total, by_category })
    })
    .await
}

/// 获取知识条目的版本历史
#[tauri::command]
pub async fn list_knowledge_versions(item_id: String) -> Result<Vec<KnowledgeVersionDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, content, changed_at, change_reason FROM knowledge_versions
             WHERE item_id = ?1 ORDER BY changed_at DESC LIMIT 50",
        )?;
        let versions: Vec<KnowledgeVersionDto> = stmt
            .query_map(rusqlite::params![item_id], |row| {
                Ok(KnowledgeVersionDto {
                    id: row.get::<_, String>("id")?,
                    content: row.get::<_, String>("content")?,
                    changed_at: row.get::<_, Option<String>>("changed_at")?,
                    change_reason: row.get::<_, Option<String>>("change_reason")?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(versions)
    })
    .await
}

/// 对比两个版本的差异
#[tauri::command]
pub async fn diff_knowledge_versions(
    // Tauri camelCase mapping: versionId1 → version_id1 (not version_id_1).
    version_id1: String,
    version_id2: String,
) -> Result<KnowledgeDiffVersionsResult, String> {
    let version_id_1 = version_id1;
    let version_id_2 = version_id2;
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

        let diffs = line_diff(&content1,&content2);

        Ok(KnowledgeDiffVersionsResult {
            version1: VersionMetaDto { id: id1, changed_at: changed_at1, change_reason: reason1 },
            version2: VersionMetaDto { id: id2, changed_at: changed_at2, change_reason: reason2 },
            diffs,
        })
    })
    .await
}

/// 创建知识条目输入（B1 类型化；全部可选，缺省口径与原 Value 版一致）
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateKnowledgeInput {
    pub block_type: String,
    pub title: String,
    pub category: String,
    pub content: String,
    pub tags: Option<String>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub linked_case_id: Option<String>,
    pub law_name: Option<String>,
    pub article_no: Option<String>,
    pub effective_date: Option<String>,
    pub status: String,
    pub parent_id: Option<String>,
}

impl Default for CreateKnowledgeInput {
    fn default() -> Self {
        Self {
            block_type: "page".to_string(),
            title: String::new(),
            category: "other".to_string(),
            content: String::new(),
            tags: None,
            source_type: None,
            source_id: None,
            linked_case_id: None,
            law_name: None,
            article_no: None,
            effective_date: None,
            status: "current".to_string(),
            parent_id: None,
        }
    }
}

// ── B1 返回侧 Dto（knowledge 域）──

/// 知识块（列表视图，8 字段）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBlockDto {
    pub id: String,
    pub title: String,
    pub category: String,
    pub content: String,
    pub tags: Option<String>,
    pub block_type: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// 知识树块（含 parent_id，9 字段）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeTreeBlockDto {
    pub id: String,
    pub title: String,
    pub category: String,
    pub content: String,
    pub tags: Option<String>,
    pub block_type: Option<String>,
    pub parent_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// 知识条目主体
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeItemDto {
    pub id: String,
    pub title: String,
    pub category: String,
    pub content: String,
    pub tags: Option<String>,
    pub source_type: Option<String>,
    pub linked_case_id: Option<String>,
    pub parent_id: Option<String>,
    pub block_type: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// 条目 + 块树
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeWithBlocksDto {
    pub item: KnowledgeItemDto,
    pub blocks: Vec<KnowledgeTreeBlockDto>,
}

/// 全文检索命中项
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchKnowledgeDto {
    pub id: String,
    pub title: String,
    pub category: String,
    pub content: String,
    pub tags: Option<String>,
    pub law_name: Option<String>,
    pub article_no: Option<String>,
}

/// 统计（byCategory 保持原 [分类,计数] 元组数组形状）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeStatsDto {
    pub total: i64,
    pub by_category: Vec<(String, i64)>,
}

/// 版本历史项
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeVersionDto {
    pub id: String,
    pub content: String,
    pub changed_at: Option<String>,
    pub change_reason: Option<String>,
}

/// 差异行
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffLineDto {
    #[serde(rename = "type")]
    pub diff_type: String,
    pub line: usize,
    pub text: String,
}

/// 版本元信息
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetaDto {
    pub id: String,
    pub changed_at: Option<String>,
    pub change_reason: Option<String>,
}

/// 双版本对比结果
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDiffVersionsResult {
    pub version1: VersionMetaDto,
    pub version2: VersionMetaDto,
    pub diffs: Vec<DiffLineDto>,
}

/// 版本 vs 当前 对比结果
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDiffCurrentResult {
    pub version: VersionMetaDto,
    pub current_content: String,
    pub diffs: Vec<DiffLineDto>,
}

/// 图谱节点
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphNodeDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub category: Option<String>,
}

/// 图谱边
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdgeDto {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

/// 知识图谱
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeGraphDto {
    pub nodes: Vec<GraphNodeDto>,
    pub edges: Vec<GraphEdgeDto>,
}

#[tauri::command]
pub async fn create_knowledge(data: CreateKnowledgeInput) -> Result<String, String> {
    run_blocking(move || {
        let mut raw = db::open_db()?;
        let conn = raw.transaction()?;
        let id = create_knowledge_in_transaction(&conn, data)?;
        conn.commit()?;
        Ok(id)
    })
    .await
}

pub(super) fn create_knowledge_in_transaction(conn: &rusqlite::Connection, data: CreateKnowledgeInput) -> anyhow::Result<String> {
        let id = db::new_id();
        let now = db::now_local();

        // 块级化（§8.2）：block_type 限 page/block/reference，缺省 'page'
        let block_type = match data.block_type.as_str() {
            "block" => "block",
            "reference" => "reference",
            _ => "page",
        };

        conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content, tags, source_type, source_id,
             linked_case_id, law_name, article_no, effective_date, status, parent_id, block_type,
             created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
            rusqlite::params![
                id,
                data.title.as_str(),
                data.category.as_str(),
                data.content.as_str(),
                data.tags.as_deref(),
                data.source_type.as_deref(),
                data.source_id.as_deref(),
                data.linked_case_id.as_deref(),
                data.law_name.as_deref(),
                data.article_no.as_deref(),
                data.effective_date.as_deref(),
                data.status.as_str(),
                data.parent_id.as_deref(),
                block_type,
                now,
            ],
        )?;

        sync_wiki_links(conn, &id, &data.content)?;
        Ok(id)
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
) -> Result<KnowledgeDiffCurrentResult, String> {
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

        let diffs = line_diff(&version_content,&current_content);

        Ok(KnowledgeDiffCurrentResult {
            version: VersionMetaDto { id: vid, changed_at, change_reason: reason },
            current_content,
            diffs,
        })
    })
    .await
}

/// 知识图谱数据：真实节点与边（禁止随机生成）
///
/// - nodes：知识条目（最近更新 N=100）、被知识引用的案件、关联知识的任务
/// - edges：knowledge_relations 知识↔知识关系、
///   knowledge_items.linked_case_id 知识→案件、tasks.knowledge_id 任务→知识
#[tauri::command]
pub async fn get_knowledge_graph(limit: Option<usize>) -> Result<KnowledgeGraphDto, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let limit = limit.unwrap_or(100).clamp(1, 500);

        let mut nodes: Vec<GraphNodeDto> = Vec::new();
        let mut edges: Vec<GraphEdgeDto> = Vec::new();
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
                if let Some(cid) = linked_case_id {
                    if !cid.is_empty() {
                        edges.push(GraphEdgeDto {
                            source: format!("k-{}", id),
                            target: format!("c-{}", cid),
                            edge_type: "linked_case".into(),
                        });
                        case_ids.insert(cid);
                    }
                }
                nodes.push(GraphNodeDto {
                    id: format!("k-{}", id),
                    name: title,
                    node_type: "knowledge".into(),
                    category: Some(category),
                });
                knowledge_ids.insert(id);
            }
        }

        // 知识 ↔ 知识 边（两端都需在节点集内）
        {
            let mut stmt = conn
                .prepare("SELECT source_id, target_id, relation_type FROM knowledge_relations")?;
            let rels: Vec<(String, String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (source, target, rel_type) in rels {
                if knowledge_ids.contains(&source) && knowledge_ids.contains(&target) {
                    edges.push(GraphEdgeDto {
                        source: format!("k-{}", source),
                        target: format!("k-{}", target),
                        edge_type: rel_type,
                    });
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
                nodes.push(GraphNodeDto {
                    id: format!("c-{}", id),
                    name,
                    node_type: "case".into(),
                    category: None,
                });
            }
        }

        // 任务节点 + 任务 → 知识 边（tasks.knowledge_id 外键）
        {
            let mut stmt = conn.prepare(
                "SELECT id, task_name, knowledge_id FROM tasks WHERE knowledge_id IS NOT NULL AND deleted_at IS NULL",
            )?;
            let tasks: Vec<(String, String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (id, name, kid) in tasks {
                if knowledge_ids.contains(&kid) {
                    nodes.push(GraphNodeDto {
                        id: format!("t-{}", id),
                        name,
                        node_type: "task".into(),
                        category: None,
                    });
                    edges.push(GraphEdgeDto {
                        source: format!("t-{}", id),
                        target: format!("k-{}", kid),
                        edge_type: "references".into(),
                    });
                }
            }
        }

        Ok(KnowledgeGraphDto { nodes, edges })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        conn
    }

    fn insert_note(conn: &rusqlite::Connection, id: &str, title: &str, content: &str) {
        conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content) VALUES (?1, ?2, 'reference', ?3)",
            rusqlite::params![id, title, content],
        )
        .unwrap();
    }

    #[test]
    fn search_knowledge_falls_back_for_chinese_phrase() {
        let conn = test_conn();
        insert_note(
            &conn,
            "k1",
            "并行程序备忘",
            "第三页记载行政裁决与民事侵权事实并行处理。",
        );

        let results = search_knowledge_inner(&conn, "行政裁决").unwrap();
        assert!(results.iter().any(|item| item.id == "k1"));
    }

    #[test]
    fn search_knowledge_treats_wildcards_and_backslashes_literally() {
        let conn=test_conn();
        insert_note(&conn,"literal","Literal",r"C:\evidence 50% a_b");
        insert_note(&conn,"other","Other","C:evidence 500 aXb");
        for query in [r"C:\evidence","50%","a_b"] {
            let results=search_knowledge_inner(&conn,query).unwrap();
            assert!(results.iter().any(|item|item.id=="literal"));
        }
    }

    // ── 层级校验 ─────────────────────────────────────────

    #[test]
    fn test_validate_parent_rejects_self_missing_and_cycle() {
        let conn = test_conn();
        insert_note(&conn, "a", "甲", "");
        insert_note(&conn, "b", "乙", "");
        insert_note(&conn, "c", "丙", "");
        conn.execute("UPDATE knowledge_items SET parent_id='a' WHERE id='b'", [])
            .unwrap();
        conn.execute("UPDATE knowledge_items SET parent_id='b' WHERE id='c'", [])
            .unwrap();

        assert!(validate_parent(&conn, "a", "a").is_err(), "自己不能作上级");
        assert!(
            validate_parent(&conn, "a", "ghost").is_err(),
            "不存在的上级应拒绝"
        );
        assert!(
            validate_parent(&conn, "a", "c").is_err(),
            "a→b→c 链上 a 以 c 为上级会成环"
        );
        assert!(
            validate_parent(&conn, "c", "a").is_ok(),
            "c 以 a 为上级（平移）不成环"
        );
    }

    // ── 编辑会话快照窗口 ─────────────────────────────────

    #[test]
    fn test_snapshot_only_on_change_and_once_per_session() {
        let conn = test_conn();
        insert_note(&conn, "n", "笔记", "v1");

        maybe_snapshot_edit(&conn, "n", "v1").unwrap(); // 无变化
        let c: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_versions WHERE item_id='n'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(c, 0, "内容未变不应产生快照");

        maybe_snapshot_edit(&conn, "n", "v2").unwrap();
        maybe_snapshot_edit(&conn, "n", "v3").unwrap(); // 同一会话窗口内
        let c: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_versions WHERE item_id='n'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(c, 1, "5 分钟编辑会话窗口内只保留一个会话前快照");
        let reason: String = conn
            .query_row(
                "SELECT change_reason FROM knowledge_versions WHERE item_id='n'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reason, "edit_session");
    }

    #[test]
    fn test_before_restore_snapshot_does_not_block_edit_window() {
        let conn = test_conn();
        insert_note(&conn, "n", "笔记", "v1");
        conn.execute(
            "INSERT INTO knowledge_versions (id, item_id, content, change_reason) VALUES ('vr', 'n', 'v1', 'before_restore')",
            [],
        )
        .unwrap();
        maybe_snapshot_edit(&conn, "n", "v2").unwrap();
        let c: i64 = conn.query_row(
            "SELECT COUNT(*) FROM knowledge_versions WHERE item_id='n' AND change_reason='edit_session'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(c, 1, "before_restore 快照不应占用编辑会话窗口");
    }

    // ── Wiki 双链权威同步 ────────────────────────────────

    fn wiki_links(conn: &rusqlite::Connection, src: &str) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT target_id, anchor FROM links WHERE source_type='knowledge' AND source_id=?1 ORDER BY anchor")
            .unwrap();
        stmt.query_map([src], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn test_sync_wiki_links_create_remove_and_preserve_others() {
        let conn = test_conn();
        insert_note(&conn, "a", "甲笔记", "");
        insert_note(&conn, "b", "乙笔记", "");
        // 手动关联与结构关联（非 wiki: 前缀）
        conn.execute(
            "INSERT INTO links (id, source_type, source_id, target_type, target_id, anchor, label) VALUES ('lm', 'knowledge', 'a', 'knowledge', 'b', 'manual', '手动')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO links (id, source_type, source_id, target_type, target_id, anchor, label) VALUES ('ls', 'knowledge', 'a', 'knowledge', 'b', 'pageindex:structure', '结构')",
            [],
        ).unwrap();

        // 大小写不敏感匹配；含方括号标题不可解析（与 Obsidian 一致）
        sync_wiki_links(&conn, "a", "参见 [[乙笔记]] 与 [[[卷宗] x.pdf]]").unwrap();
        let links = wiki_links(&conn, "a");
        assert_eq!(
            links.len(),
            3,
            "应新增 1 条 wiki 链，保留 manual 与 structure 链: {links:?}"
        );
        assert!(links.iter().any(|(_, an)| an == "wiki:乙笔记"));

        // 自身链接排除
        sync_wiki_links(&conn, "a", "[[甲笔记]]").unwrap();
        assert!(wiki_links(&conn, "a")
            .iter()
            .all(|(_, an)| an != "wiki:甲笔记"));

        // 正文移除后 wiki 链回收，其他链不受影响
        sync_wiki_links(&conn, "a", "没有链接了").unwrap();
        let links = wiki_links(&conn, "a");
        assert_eq!(links.len(), 2);
        assert!(links.iter().all(|(_, an)| !an.starts_with("wiki:")));
    }

    // ── 版本恢复 ─────────────────────────────────────────

    #[test]
    fn test_restore_version_ownership_snapshot_and_wiki_resync() {
        let mut conn = test_conn();
        insert_note(&conn, "a", "甲", "新正文 没有链接");
        insert_note(&conn, "b", "乙笔记", "");
        sync_wiki_links(&conn, "a", "新正文 [[乙笔记]]").unwrap();
        conn.execute(
            "UPDATE knowledge_items SET content='新正文 [[乙笔记]]' WHERE id='a'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO knowledge_versions (id, item_id, content, change_reason) VALUES ('v-old', 'a', '旧正文', 'edit_session')",
            [],
        ).unwrap();

        // 归属校验
        assert!(
            restore_version_inner(&mut conn, "b", "v-old").is_err(),
            "跨笔记恢复应拒绝"
        );

        restore_version_inner(&mut conn, "a", "v-old").unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM knowledge_items WHERE id='a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(content, "旧正文");
        let before_restore: i64 = conn.query_row(
            "SELECT COUNT(*) FROM knowledge_versions WHERE item_id='a' AND change_reason='before_restore'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(before_restore, 1, "恢复前应留存当前正文快照");
        // 恢复后正文不再有 [[乙笔记]]，wiki 链应被回收
        assert!(
            wiki_links(&conn, "a").is_empty(),
            "恢复后 Wiki 链应与正文一致"
        );
    }

    // ── PageIndex 沉淀 ───────────────────────────────────

    fn fixture_document(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "INSERT INTO cases (id, case_name, client_name) VALUES ('case1', '测试案件', '委托人');
             INSERT INTO case_files (id, case_id, file_name, file_path, category)
               VALUES ('f1', 'case1', '判决书.pdf', '/tmp/判决书.pdf', 'evidence');
             INSERT INTO document_processing_jobs (id, file_id, source_sha256, status, total_pages)
               VALUES ('j1', 'f1', 'sha', 'completed', 2);
             INSERT INTO document_pages (job_id, file_id, page_number, plain_text)
               VALUES ('j1', 'f1', 1, '第一页正文'), ('j1', 'f1', 2, '第二页正文');
             INSERT INTO page_index_nodes (id, file_id, parent_id, title, summary, level, page_start, page_end)
               VALUES
               ('pn-root', 'f1', NULL, '全文', '', 0, 1, 2),
               ('pn-1', 'f1', 'pn-root', '一审', '一审摘要', 1, 1, 1),
               ('pn-2', 'f1', 'pn-1', '事实认定', '事实摘要', 2, 1, 1);",
        )
        .unwrap();
    }

    #[test]
    fn test_import_pageindex_tree_dedup_and_links() {
        let mut conn = test_conn();
        fixture_document(&conn);

        let first = import_pageindex_inner(&mut conn, "f1").unwrap();
        assert!(!first.reused);
        assert_eq!(
            first.child_count, 2,
            "level>0 节点沉淀为子笔记（root 节点除外）"
        );

        // 根笔记内容与归属
        let (title, content, case_id): (String, String, String) = conn
            .query_row(
                "SELECT title, content, linked_case_id FROM knowledge_items WHERE id=?1",
                [&first.knowledge_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(title.contains("[卷宗]"));
        assert!(
            content.contains("第一页正文"),
            "Markdown 缺失时回退逐页 Page IR"
        );
        assert_eq!(case_id, "case1");

        // 链接：根→原始文件；子→根（pageindex:structure，非 wiki:，不受自动回收管辖）
        let file_link: i64 = conn.query_row(
            "SELECT COUNT(*) FROM links WHERE source_id=?1 AND target_type='file' AND anchor='source:document-job:j1'",
            [&first.knowledge_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(file_link, 1);
        let struct_links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM links WHERE target_id=?1 AND anchor='pageindex:structure'",
                [&first.knowledge_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(struct_links, 2);

        // 子笔记层级保持 PageIndex 父子关系
        let grandchild_parent: String = conn.query_row(
            "SELECT ki.parent_id FROM knowledge_items ki WHERE ki.source_type='pageindex-node' AND ki.source_id='pn-2'",
            [], |r| r.get(0),
        ).unwrap();
        let child_id: String = conn.query_row(
            "SELECT id FROM knowledge_items WHERE source_type='pageindex-node' AND source_id='pn-1'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(grandchild_parent, child_id);

        // 重复沉淀复用既有树，不制造重复内容
        let second = import_pageindex_inner(&mut conn, "f1").unwrap();
        assert!(second.reused);
        assert_eq!(second.knowledge_id, first.knowledge_id);
        assert_eq!(second.child_count, first.child_count);
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM knowledge_items WHERE source_type IN ('ocr-pageindex','pageindex-node')",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(total, 3);

        insert_note(&conn, "folder", "研究资料", "");
        conn.execute("UPDATE knowledge_items SET parent_id='folder' WHERE id=?1", [&first.knowledge_id]).unwrap();
        let moved = import_pageindex_inner(&mut conn, "f1").unwrap();
        assert!(moved.reused);
        assert_eq!(moved.knowledge_id, first.knowledge_id);
    }

    #[test]
    fn test_import_new_revision_preserves_previous_tree_and_edits() {
        let mut conn = test_conn();
        fixture_document(&conn);
        let old = import_pageindex_inner(&mut conn, "f1").unwrap();
        conn.execute("UPDATE knowledge_items SET content='人工编辑的结构笔记' WHERE source_id='pn-2'", []).unwrap();
        conn.execute_batch("INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,total_pages)
            VALUES('j2','f1','new-sha','completed',1);
            INSERT INTO document_pages(job_id,file_id,page_number,plain_text) VALUES('j2','f1',1,'修订正文');").unwrap();
        let new = import_pageindex_inner(&mut conn, "f1").unwrap();
        assert!(!new.reused);
        assert_ne!(new.knowledge_id, old.knowledge_id);
        let preserved: i64 = conn.query_row("SELECT count(*) FROM knowledge_items WHERE content='人工编辑的结构笔记'", [], |r| r.get(0)).unwrap();
        assert_eq!(preserved, 1);
        let total: i64 = conn.query_row("SELECT count(*) FROM knowledge_items", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 6);
    }

    #[test]
    fn test_import_pageindex_orphan_cleanup_after_root_deleted() {
        let mut conn = test_conn();
        fixture_document(&conn);
        let first = import_pageindex_inner(&mut conn, "f1").unwrap();

        // 用户直接删掉根笔记（绕过 delete_knowledge 的提升逻辑），子树残留
        conn.execute(
            "DELETE FROM knowledge_items WHERE id=?1",
            [&first.knowledge_id],
        )
        .unwrap();

        let second = import_pageindex_inner(&mut conn, "f1").unwrap();
        assert!(!second.reused, "根已删应重建而非复用");
        let orphans: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_items
             WHERE source_type='pageindex-node'
               AND (parent_id IS NULL OR parent_id NOT IN (SELECT id FROM knowledge_items))",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0, "重建后不应有孤儿子笔记");
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM knowledge_items WHERE source_type IN ('ocr-pageindex','pageindex-node')",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(total, 3, "旧子树已清理，恰好一棵新树");
    }

    #[test]
    fn test_export_knowledge_markdown_uses_saved_content_and_validates_extension() {
        let conn = test_conn();
        insert_note(
            &conn,
            "note-export",
            "导出测试",
            "# 导出测试\n\n中文正文 [[关联笔记]]",
        );
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("导出测试.md");

        let result =
            export_knowledge_markdown_inner(&conn, "note-export", output.to_str().unwrap())
                .unwrap();

        assert!(result.file_size > 0);
        let exported = std::fs::read_to_string(&output).unwrap();
        assert_eq!(exported, "# 导出测试\n\n中文正文 [[关联笔记]]\n");
        assert!(export_knowledge_markdown_inner(
            &conn,
            "note-export",
            temp.path().join("bad.txt").to_str().unwrap(),
        )
        .is_err());
    }

    #[test]
    fn test_export_knowledge_markdown_externalizes_ocr_images() {
        let conn = test_conn();
        let content = "# 原图\n<img src=\"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQKt8NAAITAVXMpdPnAAAAAElFTkSuQmCC\">";
        insert_note(&conn, "ocr-export", "原图", content);
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("原图.md");
        export_knowledge_markdown_inner(&conn, "ocr-export", output.to_str().unwrap()).unwrap();
        let md = std::fs::read_to_string(&output).unwrap();
        assert!(!md.contains("base64"));
        let relative = md.split("src=\"").nth(1).unwrap().split('"').next().unwrap();
        assert!(temp.path().join(relative).is_file());
        let saved: String = conn.query_row("SELECT content FROM knowledge_items WHERE id='ocr-export'", [], |r| r.get(0)).unwrap();
        assert_eq!(saved, content);
    }
}

fn line_diff(old: &str, new: &str) -> Vec<DiffLineDto> {
    fn emit(out:&mut Vec<DiffLineDto>,kind:&str,line:usize,text:&str){out.push(DiffLineDto{diff_type:kind.into(),line,text:text.into()});}
    fn compare(a:&[&str],b:&[&str],old_at:usize,new_at:usize,out:&mut Vec<DiffLineDto>){
        let prefix=a.iter().zip(b).take_while(|(x,y)|x==y).count();
        for (i,text) in a[..prefix].iter().enumerate(){emit(out,"equal",new_at+i,text);}
        let a=&a[prefix..];let b=&b[prefix..];let old_at=old_at+prefix;let new_at=new_at+prefix;
        let suffix=a.iter().rev().zip(b.iter().rev()).take_while(|(x,y)|x==y).count();
        let aa=&a[..a.len()-suffix];let bb=&b[..b.len()-suffix];
        if (aa.len()+1).saturating_mul(bb.len()+1)<=2_000_000 {
            let width=bb.len()+1;let mut lcs=vec![0u32;(aa.len()+1)*width];
            for i in (0..aa.len()).rev(){for j in (0..bb.len()).rev(){lcs[i*width+j]=if aa[i]==bb[j]{1+lcs[(i+1)*width+j+1]}else{lcs[(i+1)*width+j].max(lcs[i*width+j+1])};}}
            let(mut i,mut j)=(0,0);
            while i<aa.len() || j<bb.len(){
                if i<aa.len() && j<bb.len() && aa[i]==bb[j]{emit(out,"equal",new_at+j,aa[i]);i+=1;j+=1;}
                else if i<aa.len() && (j==bb.len() || lcs[(i+1)*width+j]>=lcs[i*width+j+1]){emit(out,"removed",old_at+i,aa[i]);i+=1;}
                else{emit(out,"added",new_at+j,bb[j]);j+=1;}
            }
        }else{
            // Large unrelated revisions: find a unique shared anchor without allocating a quadratic matrix.
            let mut positions=std::collections::HashMap::new();
            for (j,text) in bb.iter().enumerate(){positions.entry(*text).and_modify(|v:&mut Option<usize>|*v=None).or_insert(Some(j));}
            if let Some((i,j))=aa.iter().enumerate().filter_map(|(i,t)|positions.get(t).copied().flatten().map(|j|(i,j))).min_by_key(|(i,_)|i.abs_diff(aa.len()/2)){
                compare(&aa[..i],&bb[..j],old_at,new_at,out);emit(out,"equal",new_at+j,aa[i]);compare(&aa[i+1..],&bb[j+1..],old_at+i+1,new_at+j+1,out);
            }else{
                for(i,t)in aa.iter().enumerate(){emit(out,"removed",old_at+i,t);}
                for(j,t)in bb.iter().enumerate(){emit(out,"added",new_at+j,t);}
            }
        }
        for(i,t)in a[a.len()-suffix..].iter().enumerate(){emit(out,"equal",new_at+bb.len()+i,t);}
    }
    let mut out=Vec::new();compare(&old.lines().collect::<Vec<_>>(),&new.lines().collect::<Vec<_>>(),1,1,&mut out);out
}
#[cfg(test)] mod line_diff_tests {
    #[test] fn inserted_and_deleted_lines_do_not_shift_the_entire_document(){
        let diff=super::line_diff("甲\n乙\n丙","前言\n甲\n乙\n丙");
        assert_eq!(diff.iter().filter(|d|d.diff_type=="added").count(),1);
        assert_eq!(diff.iter().filter(|d|d.diff_type=="equal").count(),3);
        assert_eq!(super::line_diff("甲\n删去\n乙","甲\n乙").iter().filter(|d|d.diff_type=="removed").count(),1);
    }
}

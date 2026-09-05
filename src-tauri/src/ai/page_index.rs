//! PageIndex-inspired local tree over durable Page IR.
use log::{error, info};
use serde_json::json;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone)]
struct PageRecord {
    number: u32,
    plain_text: String,
    markdown: String,
}
#[derive(Debug, Clone)]
struct IndexNode {
    id: String,
    parent_id: Option<String>,
    level: usize,
    title: String,
    page_start: u32,
    page_end: u32,
    summary: String,
    content: String,
}

fn heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let count = trimmed.chars().take_while(|c| *c == '#').count();
    if count == 0 || count > 6 {
        return None;
    }
    let title = trimmed[count..].trim();
    (!title.is_empty()).then(|| (count, title.to_string()))
}

fn build_nodes(pages: &[PageRecord]) -> Vec<IndexNode> {
    let root_id = crate::db::new_id();
    let total = pages.last().map(|p| p.number).unwrap_or(1);
    let mut nodes = vec![IndexNode {
        id: root_id.clone(),
        parent_id: None,
        level: 0,
        title: "全文".into(),
        page_start: 1,
        page_end: total,
        summary: format!("共 {} 页", pages.len()),
        content: String::new(),
    }];
    let mut stack: Vec<(usize, String)> = vec![(0, root_id.clone())];
    let mut found_heading = false;
    for page in pages {
        for line in page.markdown.lines() {
            if let Some((level, title)) = heading(line) {
                found_heading = true;
                while stack.last().is_some_and(|(old, _)| *old >= level) {
                    stack.pop();
                }
                let parent = stack
                    .last()
                    .map(|(_, id)| id.clone())
                    .unwrap_or_else(|| root_id.clone());
                let id = crate::db::new_id();
                nodes.push(IndexNode {
                    id: id.clone(),
                    parent_id: Some(parent),
                    level,
                    title,
                    page_start: page.number,
                    page_end: page.number,
                    summary: page.plain_text.chars().take(240).collect(),
                    content: page.markdown.clone(),
                });
                stack.push((level, id));
            }
        }
    }
    if found_heading {
        for index in 1..nodes.len() {
            let start = nodes[index].page_start;
            let level = nodes[index].level;
            let next = nodes[index + 1..]
                .iter()
                .find(|node| node.level <= level)
                .map(|node| node.page_start.saturating_sub(1))
                .unwrap_or(total);
            nodes[index].page_end = next.max(start);
        }
    } else {
        for page in pages {
            nodes.push(IndexNode {
                id: crate::db::new_id(),
                parent_id: Some(root_id.clone()),
                level: 1,
                title: format!("第 {} 页", page.number),
                page_start: page.number,
                page_end: page.number,
                summary: page.plain_text.chars().take(240).collect(),
                content: if page.markdown.is_empty() {
                    page.plain_text.clone()
                } else {
                    page.markdown.clone()
                },
            });
        }
    }
    nodes
}

pub async fn build_page_index_tree(file_id: &str, _file_path: &str) -> Result<(), String> {
    let mut conn = crate::db::open_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let latest_job:String=tx.query_row("SELECT id FROM document_processing_jobs WHERE file_id=?1 AND status='completed' ORDER BY rowid DESC LIMIT 1",[file_id],|row|row.get(0)).map_err(|_|"文档尚未完成 OCR/Page IR 处理".to_string())?;
    let pages = {
        let mut stmt=tx.prepare("SELECT page_number,plain_text,markdown FROM document_pages WHERE job_id=?1 ORDER BY page_number").map_err(|e|e.to_string())?;
        let records = stmt
            .query_map([latest_job], |row| {
                Ok(PageRecord {
                    number: row.get(0)?,
                    plain_text: row.get(1)?,
                    markdown: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        records
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    if pages.is_empty() {
        return Err("Page IR 没有页面内容".into());
    }
    let nodes = build_nodes(&pages);
    tx.execute("DELETE FROM page_index_nodes WHERE file_id=?1", [file_id])
        .map_err(|e| e.to_string())?;
    for node in &nodes {
        tx.execute("INSERT INTO page_index_nodes(id,file_id,parent_id,title,summary,content,level,page_start,page_end)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",rusqlite::params![node.id,file_id,node.parent_id,node.title,node.summary,node.content,node.level,node.page_start,node.page_end]).map_err(|e|e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    info!(
        "PageIndex-inspired tree built: file={}, nodes={}",
        file_id,
        nodes.len()
    );
    Ok(())
}

fn read_node_pages(
    conn: &rusqlite::Connection,
    node_id: &str,
    requested_start: u32,
    requested_end: u32,
) -> Result<String, String> {
    let (file_id, node_start, node_end): (String, u32, u32) = conn
        .query_row(
            "SELECT file_id,page_start,page_end FROM page_index_nodes WHERE id=?1",
            [node_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "目标索引节点不存在".to_string())?;
    if requested_start < node_start || requested_end > node_end || requested_start > requested_end {
        return Err(format!(
            "请求页码 {}-{} 超出节点范围 {}-{}",
            requested_start, requested_end, node_start, node_end
        ));
    }
    if requested_end - requested_start + 1 > 20 {
        return Err("单次最多读取 20 页".into());
    }
    let latest_job:String=conn.query_row("SELECT id FROM document_processing_jobs WHERE file_id=?1 AND status='completed' ORDER BY rowid DESC LIMIT 1",[&file_id],|row|row.get(0)).map_err(|_|"找不到已完成的 Page IR".to_string())?;
    let mut stmt=conn.prepare("SELECT page_number,plain_text,markdown FROM document_pages WHERE job_id=?1 AND page_number BETWEEN ?2 AND ?3 ORDER BY page_number").map_err(|e|e.to_string())?;
    let rows = stmt
        .query_map(
            rusqlite::params![latest_job, requested_start, requested_end],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let mut chunks = Vec::new();
    for row in rows {
        let (page, plain, markdown) = row.map_err(|e| e.to_string())?;
        let content = if markdown.trim().is_empty() {
            plain
        } else {
            markdown
        };
        chunks.push(format!("[file {} p{}]\n{}", file_id, page, content));
    }
    if chunks.is_empty() {
        return Err("请求范围内没有页面内容".into());
    }
    Ok(chunks.join("\n\n"))
}

pub async fn navigate_and_reason_search(
    app: AppHandle,
    query: &str,
    scope_file_ids: Vec<String>,
) -> Result<String, String> {
    if scope_file_ids.is_empty() {
        return Err("请至少选择一份已处理文档".into());
    }
    let _=app.emit("reasoning_progress",json!({"status":"start","message":format!("正在查看 {} 份文档的结构树",scope_file_ids.len())}));
    let conn = crate::db::open_db().map_err(|e| e.to_string())?;
    let mut toc = Vec::new();
    for file_id in &scope_file_ids {
        let mut stmt=conn.prepare("SELECT id,title,summary,level,page_start,page_end FROM page_index_nodes WHERE file_id=?1 ORDER BY page_start,level").map_err(|e|e.to_string())?;
        for row in stmt.query_map([file_id],|row|Ok(json!({"id":row.get::<_,String>(0)?,"fileId":file_id,"title":row.get::<_,String>(1)?,"summary":row.get::<_,String>(2)?,"level":row.get::<_,u32>(3)?,"pageStart":row.get::<_,u32>(4)?,"pageEnd":row.get::<_,u32>(5)?}))).map_err(|e|e.to_string())?{toc.push(row.map_err(|e|e.to_string())?);}
    }
    if toc.is_empty() {
        return Err("所选文件还没有 PageIndex；请先生成可搜索 PDF".into());
    }
    let mut extracted = Vec::new();
    for _ in 0..4 {
        let system = r#"You are a legal document retrieval agent. Use only the supplied tree and extracted pages. If evidence is insufficient, return JSON {"action":"read_pages","target_node_id":"...","page_start":1,"page_end":1}. Read no more than 20 pages and stay inside the chosen node. If sufficient, return {"action":"answer","content":"...","citations":["file ... p..."]}. Never invent citations."#;
        let user = format!(
            "Query: {}\n\nTree:\n{}\n\nPages read:\n{}",
            query,
            serde_json::to_string_pretty(&toc).unwrap_or_default(),
            extracted.join("\n\n---\n\n")
        );
        let response = crate::ai::call_llm_json(system, &user)
            .await
            .map_err(|e| format!("Agent Reasoning Failed: {e}"))?;
        match response.get("action").and_then(|v| v.as_str()) {
            Some("read_pages") => {
                let node = response
                    .get("target_node_id")
                    .and_then(|v| v.as_str())
                    .ok_or("模型未返回目标节点")?;
                let start = response
                    .get("page_start")
                    .and_then(|v| v.as_u64())
                    .ok_or("模型未返回起始页")? as u32;
                let end = response
                    .get("page_end")
                    .and_then(|v| v.as_u64())
                    .ok_or("模型未返回结束页")? as u32;
                let _ = app.emit(
                    "reasoning_progress",
                    json!({"status":"reading","message":format!("读取第 {}-{} 页",start,end)}),
                );
                extracted.push(read_node_pages(&conn, node, start, end)?);
            }
            Some("answer") => {
                let content = response
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("未生成答案");
                let citations = response.get("citations").cloned().unwrap_or(json!([]));
                let _ = app.emit(
                    "reasoning_progress",
                    json!({"status":"success","message":"已根据真实页面完成检索"}),
                );
                return Ok(json!({"status":"success","data":[{"type":"reasoning_result","answer":content,"references":citations}]}).to_string());
            }
            other => {
                error!("unknown PageIndex action: {:?}", other);
                return Err("检索模型返回了无法识别的动作".into());
            }
        }
    }
    Err("读取预算已用尽，证据仍不足，未生成猜测性答案".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heading_tree_uses_real_page_ranges() {
        let pages = vec![
            PageRecord {
                number: 1,
                plain_text: "a".into(),
                markdown: "# 起诉状\n内容".into(),
            },
            PageRecord {
                number: 2,
                plain_text: "b".into(),
                markdown: "## 事实\n内容".into(),
            },
            PageRecord {
                number: 3,
                plain_text: "c".into(),
                markdown: "# 答辩状\n内容".into(),
            },
        ];
        let nodes = build_nodes(&pages);
        let first = nodes.iter().find(|n| n.title == "起诉状").unwrap();
        assert_eq!((first.page_start, first.page_end), (1, 2));
        let defense = nodes.iter().find(|n| n.title == "答辩状").unwrap();
        assert_eq!((defense.page_start, defense.page_end), (3, 3));
    }
}

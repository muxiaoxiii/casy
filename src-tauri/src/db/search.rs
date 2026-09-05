//! Keyword and versioned chunk-vector retrieval, combined with reciprocal rank fusion.
use crate::ai::{
    embeddings::{cosine, EmbeddingPlan},
    retrieval::query_terms,
};
use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use std::collections::HashMap;

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub category: String,
    pub content: String,
    pub tags: Option<String>,
    pub law_name: Option<String>,
    pub article_no: Option<String>,
    pub score: f64,
    pub source: String,
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub semantic_status: String,
    pub warning: Option<String>,
}

pub fn fts_search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<(String, f64)>> {
    let terms = query_terms(query);
    let mut phrases = vec![query.to_owned()];
    phrases.extend(terms.iter().cloned());
    let fts = phrases
        .iter()
        .filter(|s| s.chars().count() >= 3)
        .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ");
    let short: Vec<_> = phrases
        .iter()
        .filter(|s| s.chars().count() < 3)
        .map(|s| {
            format!(
                "%{}%",
                s.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        })
        .collect();
    let mut stmt = conn.prepare("WITH hits AS (
        SELECT rowid FROM knowledge_trigram WHERE knowledge_trigram MATCH ?1
        UNION SELECT rowid FROM knowledge_items WHERE EXISTS(SELECT 1 FROM json_each(?2) t
          WHERE title LIKE t.value ESCAPE '\\' OR content LIKE t.value ESCAPE '\\' OR tags LIKE t.value ESCAPE '\\')
        ) SELECT k.id,
          (CASE WHEN instr(lower(k.title),lower(?3))>0 THEN 100 ELSE 0 END
          + CASE WHEN instr(lower(k.content),lower(?3))>0 THEN 50 ELSE 0 END
          + (SELECT count(*) * 10 FROM json_each(?4) t WHERE instr(lower(k.title || k.content),lower(t.value))>0)) AS score
        FROM hits JOIN knowledge_items k ON k.rowid=hits.rowid
        WHERE COALESCE(k.status,'current')='current' ORDER BY score DESC,k.rowid DESC LIMIT ?5")?;
    let rows = stmt
        .query_map(
            params![
                if fts.is_empty() { "\"\"" } else { &fts },
                serde_json::to_string(&short)?,
                query,
                serde_json::to_string(&terms)?,
                limit.min(100)
            ],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn semantic_search(
    conn: &Connection,
    query: &[f32],
    fingerprint: &str,
    limit: usize,
) -> Result<Vec<(String, f64, String)>> {
    let mut stmt = conn.prepare("SELECT j.item_id,c.embedding,c.content FROM knowledge_index_chunks c
        JOIN knowledge_index_jobs j ON j.id=c.job_id JOIN knowledge_items k ON k.id=j.item_id
        WHERE j.status='completed' AND j.config_hash=?1 AND j.dimension=?2 AND COALESCE(k.status,'current')='current'")?;
    let mut rows = stmt.query(params![fingerprint, query.len()])?;
    let mut best: HashMap<String, (f64, String)> = HashMap::new();
    while let Some(row) = rows.next()? {
        let bytes: Vec<u8> = row.get(1)?;
        if bytes.len() != query.len() * 4 {
            continue;
        }
        let vector: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let score = cosine(query, &vector);
        if score <= 0.1 {
            continue;
        }
        let id: String = row.get(0)?;
        if best.get(&id).is_none_or(|(previous, _)| score > *previous) {
            best.insert(id, (score, row.get(2)?));
        }
    }
    let mut scored: Vec<_> = best
        .into_iter()
        .map(|(id, (score, content))| (id, score, content))
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored.truncate(limit.min(100));
    Ok(scored)
}

fn excerpt(text: &str, query: &str) -> String {
    let offset = text
        .find(query)
        .or_else(|| query_terms(query).iter().filter_map(|t| text.find(t)).min())
        .unwrap_or(0);
    let mut start = offset.saturating_sub(300);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].chars().take(1400).collect()
}

pub async fn search(query: &str, limit: usize, use_semantic: bool) -> Result<SearchResponse> {
    let query = query.trim();
    if query.chars().count() > 500 {
        bail!("检索问题不能超过 500 字");
    }
    if query.is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            semantic_status: "disabled".into(),
            warning: None,
        });
    }
    let limit = limit.clamp(1, 50);
    let mut warning = None;
    let mut status = "disabled";
    let mut embedded = None;
    if use_semantic {
        let prepared = tokio::task::spawn_blocking(|| -> Result<_> {
            let conn = super::open_db()?;
            let Some(plan) = EmbeddingPlan::load(&conn)? else { return Ok(None) };
            let ready: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_index_jobs j
                JOIN knowledge_items k ON k.id=j.item_id WHERE j.status='completed' AND j.config_hash=?1
                AND COALESCE(k.status,'current')='current')", [&plan.fingerprint], |r| r.get(0))?;
            let client = if ready { Some(plan.client(&conn)?) } else { None };
            Ok(Some((plan,client)))
        }).await?;
        match prepared {
            Ok(Some((plan, client))) => {
                if let Some(client) = client {
                    let result = async {
                        crate::ai::get_token_budget().consume().await?;
                        tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            client.embed(&[query.to_owned()]),
                        )
                        .await
                        .map_err(|_| anyhow::anyhow!("向量查询超时，本次返回关键词结果"))?
                    }
                    .await;
                    match result {
                        Ok(mut vectors) => {
                            embedded = Some((plan.fingerprint, vectors.remove(0)));
                            status = "ready";
                        }
                        Err(error) => {
                            warning = Some(error.to_string());
                            status = "unavailable";
                        }
                    }
                } else {
                    status = "not_indexed";
                }
            }
            Ok(None) => {
                status = "not_configured";
            }
            Err(error) => {
                warning = Some(error.to_string());
                status = "unavailable";
            }
        }
    }
    let query = query.to_owned();
    tokio::task::spawn_blocking(move || -> Result<SearchResponse> {
    let mut connection = super::open_db()?;
    // Rank and materialize one database snapshot so edits cannot mix source versions.
    let conn = connection.transaction()?;
    let query = query.as_str();
    let mut semantic = Vec::new();
    if let Some((fingerprint,vector)) = embedded {
        let result = (|| -> Result<_> {
            if EmbeddingPlan::require(&conn)?.fingerprint != fingerprint { bail!("向量配置已变化，本次返回关键词结果"); }
            let mismatched: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_index_jobs j
                JOIN knowledge_items k ON k.id=j.item_id WHERE j.status='completed' AND j.config_hash=?1
                AND j.dimension!=?2 AND COALESCE(k.status,'current')='current')", params![fingerprint,vector.len()], |r|r.get(0))?;
            if mismatched { bail!("向量维数与现有索引不一致，请重建索引；本次返回关键词结果"); }
            semantic_search(&conn,&vector,&fingerprint,limit * 2)
        })();
        match result {
            Ok(hits) => semantic = hits,
            Err(error) => { warning = Some(error.to_string()); status = "unavailable"; }
        }
    }
    let keywords = fts_search(&conn, query, limit * 2)?;
    let mut scores: HashMap<String, f64> = HashMap::new();
    for (rank, (id, _)) in keywords.iter().enumerate() {
        *scores.entry(id.clone()).or_default() += 1.0 / (61.0 + rank as f64);
    }
    for (rank, (id, _, _)) in semantic.iter().enumerate() {
        *scores.entry(id.clone()).or_default() += 1.0 / (61.0 + rank as f64);
    }
    let mut fused: Vec<_> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut results = Vec::new();
    for (id, score) in fused.into_iter().take(limit) {
        let keyword = keywords.iter().any(|(key, _)| key == &id);
        let passage = semantic.iter().find(|(key, _, _)| key == &id);
        let item = conn.query_row("SELECT id,title,category,content,tags,law_name,article_no FROM knowledge_items WHERE id=?1", [&id], |r| {
            let content: String = r.get(3)?;
            Ok(SearchResult {
                id:r.get(0)?,title:r.get(1)?,category:r.get(2)?,content:excerpt(passage.map(|(_,_,text)| text.as_str()).unwrap_or(&content),query),
                tags:r.get(4)?,law_name:r.get(5)?,article_no:r.get(6)?,score,
                source: match (keyword,passage.is_some()) { (true,true)=>"hybrid",(true,false)=>"fts",_=>"semantic" }.into(),
            })
        })?;
        results.push(item);
    }
    Ok(SearchResponse {
        results,
        semantic_status: status.into(),
        warning,
    })
    }).await?
}

#[tauri::command]
pub async fn hybrid_search_knowledge(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    search(&query, limit.unwrap_or(20), true)
        .await
        .map(|r| r.results)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_knowledge_index(
    query: String,
    use_semantic: bool,
) -> Result<SearchResponse, String> {
    search(&query, 30, use_semantic)
        .await
        .map_err(|e| e.to_string())
}

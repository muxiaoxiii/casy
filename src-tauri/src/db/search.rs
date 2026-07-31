//! 混合检索模块
//!
//! FTS5 关键词检索 + Ollama embedding 语义检索，使用 RRF 融合排序。

use anyhow::Result;
use rusqlite::Connection;

/// 检索结果条目
#[derive(Debug, serde::Serialize, serde::Deserialize)]
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
    pub source: String, // "fts" | "semantic" | "hybrid"
}

/// FTS5 关键词检索
fn fts_search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<(String, f64)>> {
    // 将用户查询转为 FTS5 查询（对每个词做前缀匹配）
    let fts_query: String = query
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|w| {
            // 转义特殊字符，添加前缀匹配
            let escaped = w.replace('"', "");
            format!("\"{}\"*", escaped)
        })
        .collect::<Vec<_>>()
        .join(" OR ");

    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT ki.id, rank FROM knowledge_fts f
         JOIN knowledge_items ki ON ki.rowid = f.rowid
         WHERE knowledge_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;

    let results: Vec<(String, f64)> = stmt
        .query_map(rusqlite::params![fts_query, limit as i64], |row| {
            let id: String = row.get(0)?;
            let rank: f64 = row.get(1)?;
            // FTS5 rank 是负数，越小越好，转换为正数分数
            Ok((id, -rank))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(results)
}

/// 从 Ollama 获取 embedding 向量
async fn get_embedding(text: &str) -> Result<Vec<f32>> {
    let config = crate::ai::load_ai_config();
    let base_url = config
        .api_url
        .as_deref()
        .unwrap_or("http://localhost:11434")
        .trim_end_matches('/');

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!("{}/api/embeddings", base_url);

    // 截断文本避免过长
    let truncated: String = text.chars().take(512).collect();

    let body = serde_json::json!({
        "model": "nomic-embed-text",
        "prompt": truncated,
    });

    let resp = client.post(&url).json(&body).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Ollama embedding API 错误: {}", resp.status());
    }

    let result: serde_json::Value = resp.json().await?;
    let embedding = result["embedding"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("embedding 响应格式错误"))?
        .iter()
        .map(|v| v.as_f64().unwrap_or(0.0) as f32)
        .collect();

    Ok(embedding)
}

/// 保存 embedding 到数据库
fn save_embedding(conn: &Connection, item_id: &str, embedding: &[f32]) -> Result<()> {
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    conn.execute(
        "INSERT OR REPLACE INTO knowledge_embeddings (item_id, embedding, model, dimension)
         VALUES (?1, ?2, 'nomic-embed-text', ?3)",
        rusqlite::params![item_id, bytes, embedding.len() as i64],
    )?;
    Ok(())
}

/// 加载所有 embedding
fn load_embeddings(conn: &Connection) -> Result<Vec<(String, Vec<f32>)>> {
    let mut stmt = conn.prepare(
        "SELECT ke.item_id, ke.embedding, ke.dimension
         FROM knowledge_embeddings ke
         JOIN knowledge_items ki ON ki.id = ke.item_id
         WHERE ki.status = 'current'",
    )?;

    let results: Vec<(String, Vec<f32>)> = stmt
        .query_map([], |row| {
            let item_id: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            let dimension: usize = row.get(2)?;

            let floats: Vec<f32> = bytes
                .chunks_exact(4)
                .take(dimension)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();

            Ok((item_id, floats))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(results)
}

/// 余弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// 语义向量检索
fn semantic_search(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<(String, f64)>> {
    let all_embeddings = load_embeddings(conn)?;

    let mut scored: Vec<(String, f64)> = all_embeddings
        .iter()
        .map(|(id, emb)| {
            let sim = cosine_similarity(query_embedding, emb);
            (id.clone(), sim as f64)
        })
        .filter(|(_, score)| *score > 0.1) // 过滤低相关度
        .collect();

    // 按相似度降序排序
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    Ok(scored)
}

/// Reciprocal Rank Fusion (RRF) 融合排序
///
/// RRF(k) = sum(1 / (k + rank_i))，k 通常取 60
fn rrf_fusion(
    fts_results: &[(String, f64)],
    semantic_results: &[(String, f64)],
    k: f64,
) -> Vec<(String, f64)> {
    use std::collections::HashMap;

    let mut scores: HashMap<String, f64> = HashMap::new();

    // FTS 结果按分数排序后分配 rank
    let mut fts_sorted = fts_results.to_vec();
    fts_sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (rank, (id, _)) in fts_sorted.iter().enumerate() {
        let rrf_score = 1.0 / (k + rank as f64 + 1.0);
        *scores.entry(id.clone()).or_insert(0.0) += rrf_score;
    }

    // 语义结果按分数排序后分配 rank
    let mut sem_sorted = semantic_results.to_vec();
    sem_sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (rank, (id, _)) in sem_sorted.iter().enumerate() {
        let rrf_score = 1.0 / (k + rank as f64 + 1.0);
        *scores.entry(id.clone()).or_insert(0.0) += rrf_score;
    }

    let mut fused: Vec<(String, f64)> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused
}

/// 为知识条目生成并保存 embedding
pub async fn embed_knowledge_item(item_id: &str, text: &str) -> Result<()> {
    let embedding = get_embedding(text).await?;
    let conn = crate::db::open_db()?;
    save_embedding(&conn, item_id, &embedding)?;
    Ok(())
}

/// 混合检索：FTS5 + 语义向量，RRF 融合排序
pub async fn hybrid_search(query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let conn = crate::db::open_db()?;

    // 1. FTS5 关键词检索
    let fts_results = fts_search(&conn, query, limit * 2).unwrap_or_default();

    // 2. 语义向量检索
    let semantic_results = match get_embedding(query).await {
        Ok(query_embedding) => {
            semantic_search(&conn, &query_embedding, limit * 2).unwrap_or_default()
        }
        Err(e) => {
            log::warn!("语义检索失败（Ollama 可能未运行），仅使用 FTS: {}", e);
            Vec::new()
        }
    };

    // 3. RRF 融合排序
    let fused = rrf_fusion(&fts_results, &semantic_results, 60.0);

    // 4. 获取完整条目数据
    let mut results = Vec::new();
    for (id, score) in fused.iter().take(limit) {
        if let Ok(item) = conn.query_row(
            "SELECT id, title, category, content, tags, law_name, article_no
             FROM knowledge_items WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(SearchResult {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    category: row.get(2)?,
                    content: row.get(3)?,
                    tags: row.get(4)?,
                    law_name: row.get(5)?,
                    article_no: row.get(6)?,
                    score: *score,
                    source: if fts_results.iter().any(|(fid, _)| fid == id)
                        && semantic_results.iter().any(|(sid, _)| sid == id)
                    {
                        "hybrid".to_string()
                    } else if fts_results.iter().any(|(fid, _)| fid == id) {
                        "fts".to_string()
                    } else {
                        "semantic".to_string()
                    },
                })
            },
        ) {
            results.push(item);
        }
    }

    Ok(results)
}

// ============================================================
// Tauri 命令
// ============================================================

/// 混合检索知识条目
#[tauri::command]
pub async fn hybrid_search_knowledge(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    hybrid_search(&query, limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())
}

/// 为单个知识条目生成 embedding（后台任务）
#[tauri::command]
pub async fn embed_knowledge(item_id: String) -> Result<String, String> {
    let conn = crate::db::open_db().map_err(|e| e.to_string())?;
    let content: String = conn
        .query_row(
            "SELECT content FROM knowledge_items WHERE id = ?1",
            rusqlite::params![item_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("知识条目不存在: {}", e))?;

    embed_knowledge_item(&item_id, &content)
        .await
        .map_err(|e| e.to_string())?;

    Ok("embedding 已生成".to_string())
}

/// 批量为所有知识条目生成 embeddings
#[tauri::command]
pub async fn embed_all_knowledge() -> Result<String, String> {
    let conn = crate::db::open_db().map_err(|e| e.to_string())?;

    // 获取没有 embedding 的条目
    let items: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT ki.id, ki.content FROM knowledge_items ki
             LEFT JOIN knowledge_embeddings ke ON ke.item_id = ki.id
             WHERE ke.item_id IS NULL AND ki.status = 'current'",
            )
            .map_err(|e| e.to_string())?;

        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };

    let count = items.len();
    for (id, content) in &items {
        if let Err(e) = embed_knowledge_item(id, content).await {
            log::warn!("生成 embedding 失败 ({}): {}", id, e);
        }
    }

    Ok(format!("已为 {} 条知识生成 embedding", count))
}

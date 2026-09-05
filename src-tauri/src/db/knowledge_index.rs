use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;

use crate::ai::embeddings::{digest, EmbeddingPlan};

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexJob {
    pub id: String,
    pub item_id: String,
    pub title: String,
    pub model: String,
    pub status: String,
    pub completed_chunks: usize,
    pub total_chunks: usize,
    pub error: Option<String>,
}

#[derive(Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueReport {
    pub queued: usize,
    pub up_to_date: usize,
    pub already_queued: usize,
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub configured: bool,
    pub total: usize,
    pub indexed: usize,
    pub queued: usize,
    pub running: usize,
    pub failed: usize,
    pub jobs: Vec<IndexJob>,
}

pub fn source_hash(title: &str, content: &str) -> String {
    digest(&serde_json::to_vec(&(title, content)).expect("text serialization"))
}

pub fn segments(text: &str, size: usize) -> Vec<&str> {
    assert!(size >= 128);
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = text[start..]
            .char_indices()
            .nth(size)
            .map(|(offset, _)| start + offset)
            .unwrap_or(text.len());
        chunks.push(&text[start..end]);
        if end == text.len() {
            break;
        }
        start += text[start..end]
            .char_indices()
            .rev()
            .nth((size / 8).min(128) - 1)
            .unwrap()
            .0;
    }
    chunks
}

fn source(conn: &Connection, id: &str) -> Result<(String, String)> {
    let (title, content): (String, String) = conn.query_row(
        "SELECT title,content FROM knowledge_items WHERE id=?1 AND COALESCE(status,'current')='current'",
        [id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).context("知识条目不存在或已归档")?;
    if content.trim().is_empty() {
        bail!("笔记正文为空");
    }
    if content.len() > 64 * 1024 * 1024 {
        bail!("笔记超过 64 MiB 索引上限");
    }
    Ok((title, content))
}

pub fn queue_inner(
    conn: &Connection,
    item_id: &str,
    plan: &EmbeddingPlan,
    report: &mut QueueReport,
) -> Result<String> {
    let (title, content) = source(conn, item_id)?;
    let hash = source_hash(&title, &content);
    if let Some((id, status, config_hash, source_hash)) = conn
        .query_row(
            "SELECT id,status,config_hash,source_hash FROM knowledge_index_jobs WHERE item_id=?1
         AND status IN ('queued','running','completed') ORDER BY rowid DESC LIMIT 1",
            [item_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
    {
        if status == "queued" || status == "running" {
            if config_hash == plan.fingerprint && source_hash == hash {
                report.already_queued += 1;
                return Ok(id);
            }
            conn.execute("UPDATE knowledge_index_jobs SET status='stale',error='配置或正文已变化' WHERE id=?1", [&id])?;
            conn.execute("DELETE FROM knowledge_index_chunks WHERE job_id=?1", [&id])?;
        } else if config_hash == plan.fingerprint && source_hash == hash {
            let complete: bool = conn.query_row(
                "SELECT total_chunks>0 AND completed_chunks=total_chunks
                AND total_chunks=(SELECT count(*) FROM knowledge_index_chunks WHERE job_id=j.id)
                FROM knowledge_index_jobs j WHERE id=?1",
                [&id],
                |r| r.get(0),
            )?;
            if complete {
                report.up_to_date += 1;
                return Ok(id);
            }
            conn.execute("UPDATE knowledge_index_jobs SET status='stale',error='索引分段缺失，请重建' WHERE id=?1", [&id])?;
            conn.execute("DELETE FROM knowledge_index_chunks WHERE job_id=?1", [&id])?;
        }
    }
    let id = super::new_id();
    conn.execute(
        "INSERT INTO knowledge_index_jobs(id,item_id,source_hash,config_hash,model,status)
        VALUES(?1,?2,?3,?4,?5,'queued')",
        params![id, item_id, hash, plan.fingerprint, plan.model],
    )?;
    report.queued += 1;
    Ok(id)
}

#[tauri::command]
pub async fn embed_knowledge(item_id: String, force: Option<bool>) -> Result<String, String> {
    crate::commands::run_blocking(move || {
        let mut conn = super::open_db()?;
        let plan = EmbeddingPlan::require(&conn)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if force.unwrap_or(false) {
            tx.execute("UPDATE knowledge_index_jobs SET status='stale',error='已请求重建' WHERE item_id=?1 AND status IN ('queued','running','completed')", [&item_id])?;
            tx.execute("DELETE FROM knowledge_index_chunks WHERE job_id IN (SELECT id FROM knowledge_index_jobs WHERE item_id=?1)", [&item_id])?;
        }
        let id = queue_inner(&tx, &item_id, &plan, &mut QueueReport::default())?;
        tx.commit()?;
        Ok(id)
    }).await
}

#[tauri::command]
pub async fn embed_all_knowledge() -> Result<QueueReport, String> {
    crate::commands::run_blocking(|| {
        let mut conn = super::open_db()?;
        let plan = EmbeddingPlan::require(&conn)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ids = {
            let mut stmt = tx.prepare("SELECT id FROM knowledge_items WHERE COALESCE(status,'current')='current' AND trim(content) != '' ORDER BY rowid")?;
            let rows = stmt.query_map([], |r| r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        let mut report = QueueReport::default();
        for id in ids { queue_inner(&tx, &id, &plan, &mut report)?; }
        tx.commit()?;
        Ok(report)
    }).await
}

#[tauri::command]
pub async fn cancel_knowledge_index_job(job_id: String) -> Result<(), String> {
    crate::commands::run_blocking(move || {
        let mut conn = super::open_db()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE knowledge_index_jobs SET status='cancelled',error=NULL,updated_at=datetime('now','localtime') WHERE id=?1 AND status IN ('queued','running')", [&job_id])? == 0 {
            bail!("任务已结束或不存在");
        }
        tx.execute("DELETE FROM knowledge_index_chunks WHERE job_id=?1", [&job_id])?;
        tx.commit()?;
        Ok(())
    }).await
}

pub fn status_inner(conn: &Connection) -> Result<IndexStatus> {
    let plan = EmbeddingPlan::load(conn)?;
    let fingerprint = plan.as_ref().map(|p| p.fingerprint.as_str()).unwrap_or("");
    let mut stmt = conn.prepare("SELECT j.id,j.item_id,k.title,j.model,
        CASE WHEN j.status='completed' AND j.config_hash!=?1 THEN 'stale' ELSE j.status END,
        j.completed_chunks,j.total_chunks,j.error
        FROM knowledge_index_jobs j JOIN knowledge_items k ON k.id=j.item_id
        WHERE j.rowid=(SELECT max(rowid) FROM knowledge_index_jobs WHERE item_id=j.item_id)
        ORDER BY CASE j.status WHEN 'running' THEN 0 WHEN 'queued' THEN 1 WHEN 'failed' THEN 2 ELSE 3 END,j.rowid DESC LIMIT 100")?;
    let jobs = stmt
        .query_map([fingerprint], |r| {
            Ok(IndexJob {
                id: r.get(0)?,
                item_id: r.get(1)?,
                title: r.get(2)?,
                model: r.get(3)?,
                status: r.get(4)?,
                completed_chunks: r.get(5)?,
                total_chunks: r.get(6)?,
                error: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let count = |sql: &str| -> Result<usize> { Ok(conn.query_row(sql, [], |r| r.get(0))?) };
    Ok(IndexStatus {
        configured: plan.is_some(),
        total: count("SELECT count(*) FROM knowledge_items WHERE COALESCE(status,'current')='current' AND trim(content) != ''")?,
        indexed: conn.query_row("SELECT count(DISTINCT j.item_id) FROM knowledge_index_jobs j JOIN knowledge_items k ON k.id=j.item_id WHERE j.status='completed' AND j.config_hash=?1 AND COALESCE(k.status,'current')='current'", [fingerprint], |r| r.get(0))?,
        queued: count("SELECT count(*) FROM knowledge_index_jobs WHERE status='queued'")?,
        running: count("SELECT count(*) FROM knowledge_index_jobs WHERE status='running'")?,
        failed: count("SELECT count(*) FROM knowledge_index_jobs j WHERE status='failed' AND rowid=(SELECT max(rowid) FROM knowledge_index_jobs WHERE item_id=j.item_id)")?,
        jobs,
    })
}

#[tauri::command]
pub async fn get_knowledge_index_status() -> Result<IndexStatus, String> {
    crate::commands::run_blocking(|| status_inner(&super::open_db()?)).await
}

struct ClaimedJob {
    id: String,
    item_id: String,
    source_hash: String,
    config_hash: String,
}

fn claim(conn: &mut Connection) -> Result<Option<ClaimedJob>> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let job = tx.query_row("SELECT id,item_id,source_hash,config_hash FROM knowledge_index_jobs WHERE status='queued' ORDER BY rowid LIMIT 1", [], |r| Ok(ClaimedJob {
        id:r.get(0)?, item_id:r.get(1)?, source_hash:r.get(2)?, config_hash:r.get(3)?,
    })).optional()?;
    if let Some(job) = &job {
        tx.execute("UPDATE knowledge_index_jobs SET status='running',updated_at=datetime('now','localtime') WHERE id=?1", [&job.id])?;
    }
    tx.commit()?;
    Ok(job)
}

fn guard(conn: &Connection, job: &ClaimedJob) -> Result<()> {
    let running: bool = conn
        .query_row(
            "SELECT status='running' FROM knowledge_index_jobs WHERE id=?1",
            [&job.id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(false);
    if !running {
        bail!("索引任务已取消、过期或删除");
    }
    let current: bool = conn
        .query_row(
            "SELECT COALESCE(status,'current')='current' FROM knowledge_items WHERE id=?1",
            [&job.item_id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(false);
    if !current {
        bail!("笔记已归档或删除");
    }
    if EmbeddingPlan::load(conn)?.is_none_or(|p| p.fingerprint != job.config_hash) {
        bail!("向量配置已变化，请重试");
    }
    Ok(())
}

async fn process(job: &ClaimedJob) -> Result<()> {
    let mut conn = super::open_db()?;
    guard(&conn, job)?;
    let plan = EmbeddingPlan::require(&conn)?;
    let client = plan.client(&conn)?;
    let (title, content) = source(&conn, &job.item_id)?;
    if source_hash(&title, &content) != job.source_hash {
        bail!("正文已变化，请重试");
    }
    let chunks = segments(&content, plan.chunk_chars);
    conn.execute(
        "UPDATE knowledge_index_jobs SET total_chunks=?1 WHERE id=?2",
        params![chunks.len(), job.id],
    )?;
    let mut dimension = None;
    for (batch_index, batch) in chunks.chunks(8).enumerate() {
        guard(&conn, job)?;
        let inputs: Vec<String> = batch
            .iter()
            .map(|chunk| {
                format!(
                    "{}\n\n{}",
                    title.chars().take(120).collect::<String>(),
                    chunk
                )
            })
            .collect();
        let vectors = client.embed(&inputs).await?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        guard(&tx, job)?;
        for (index, (chunk, vector)) in batch.iter().zip(&vectors).enumerate() {
            if dimension.is_some_and(|dim| dim != vector.len()) {
                bail!("向量模型在处理中改变了维数");
            }
            dimension = Some(vector.len());
            let bytes: Vec<u8> = vector.iter().flat_map(|n| n.to_le_bytes()).collect();
            tx.execute("INSERT INTO knowledge_index_chunks(job_id,chunk_index,content,embedding) VALUES(?1,?2,?3,?4)", params![job.id,batch_index * 8 + index,chunk,bytes])?;
        }
        tx.execute("UPDATE knowledge_index_jobs SET completed_chunks=?1,dimension=?2,updated_at=datetime('now','localtime') WHERE id=?3", params![batch_index * 8 + batch.len(),dimension,job.id])?;
        tx.commit()?;
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    guard(&tx, job)?;
    let actual: usize = tx.query_row(
        "SELECT count(*) FROM knowledge_index_chunks WHERE job_id=?1",
        [&job.id],
        |r| r.get(0),
    )?;
    if actual != chunks.len() {
        bail!("向量分段未完整写入");
    }
    tx.execute("DELETE FROM knowledge_index_chunks WHERE job_id IN (SELECT id FROM knowledge_index_jobs WHERE item_id=?1 AND id!=?2)", params![job.item_id,job.id])?;
    tx.execute("UPDATE knowledge_index_jobs SET status='stale' WHERE item_id=?1 AND id!=?2 AND status='completed'", params![job.item_id,job.id])?;
    tx.execute("UPDATE knowledge_index_jobs SET status='completed',error=NULL,updated_at=datetime('now','localtime') WHERE id=?1", [&job.id])?;
    tx.commit()?;
    Ok(())
}

pub async fn process_next() -> Result<bool> {
    let Some(job) = claim(&mut super::open_db()?)? else {
        return Ok(false);
    };
    if let Err(error) = process(&job).await {
        let mut conn = super::open_db()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE knowledge_index_jobs SET status='failed',error=?1,updated_at=datetime('now','localtime') WHERE id=?2 AND status='running'", params![error.to_string(),job.id])?;
        tx.execute("DELETE FROM knowledge_index_chunks WHERE job_id=?1 AND EXISTS(SELECT 1 FROM knowledge_index_jobs WHERE id=?1 AND status!='completed')", [&job.id])?;
        tx.commit()?;
    }
    Ok(true)
}

pub fn recover_interrupted(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE knowledge_index_jobs SET status='failed',error='上次退出时处理被中断，请重试' WHERE status='running'", [])?;
    conn.execute("DELETE FROM knowledge_index_chunks WHERE job_id IN (SELECT id FROM knowledge_index_jobs WHERE status IN ('failed','cancelled','stale'))", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segments_cover_full_cjk_text_with_overlap() {
        let text = "中文案情".repeat(4000) + "唯一文末赔偿金额";
        let chunks = segments(&text, 1000);
        assert!(chunks.len() > 16);
        assert!(chunks.last().unwrap().contains("唯一文末赔偿金额"));
        assert!(chunks.iter().all(|c| c.chars().count() <= 1000));
        let mut rebuilt = chunks[0].to_string();
        for chunk in &chunks[1..] {
            rebuilt.extend(chunk.chars().skip(125));
        }
        assert_eq!(rebuilt, text);
    }
}

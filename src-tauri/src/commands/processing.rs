use crate::{db, processing};
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingJob {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub stage: String,
    pub case_id: Option<String>,
    pub case_name: Option<String>,
    pub file_id: Option<String>,
    pub knowledge_id: Option<String>,
    pub output_path: Option<String>,
    pub error: Option<String>,
    pub current: i64,
    pub total: i64,
    pub progress: f64,
    pub elapsed_seconds: f64,
    pub remaining_seconds: Option<f64>,
    pub page_timing: Option<crate::document_pipeline::DocumentPageTiming>,
    pub created_at: String,
    pub updated_at: String,
    pub can_cancel: bool,
}
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingCenter {
    pub jobs: Vec<ProcessingJob>,
    pub services: Vec<ProcessingJob>,
    pub total: i64,
    pub active: i64,
    pub failed: i64,
}

// No case restriction and no implicit last-N truncation. Counts and page share one read transaction.
const UNION: &str = r#"
SELECT j.id,'document' kind,f.file_name title,
 CASE WHEN j.status='completed' AND j.index_status='running' THEN 'running'
 WHEN j.status='completed' AND j.index_status='failed' THEN 'failed' ELSE j.status END status,
 CASE WHEN j.status='completed' AND j.index_status IN ('running','failed') THEN 'indexing'
 ELSE j.phase END stage,
 f.case_id,c.case_name,f.id file_id,NULL knowledge_id,j.searchable_pdf_path output_path,
 COALESCE(j.index_error,j.error_message) error,j.current_page current,j.total_pages total,j.progress,
 j.elapsed_ms/1000.0 elapsed_seconds,j.remaining_ms/1000.0 remaining_seconds,j.timing_json page_timing,
 j.created_at,j.updated_at,j.status IN ('queued','running') can_cancel
 FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id LEFT JOIN cases c ON c.id=f.case_id
UNION ALL
SELECT j.id,'knowledge',k.title,CASE WHEN j.status='completed' AND j.config_hash!=:fingerprint THEN 'stale' ELSE j.status END,'embedding',NULL,NULL,NULL,k.id,NULL,j.error,j.completed_chunks,j.total_chunks,
 CASE WHEN j.total_chunks>0 THEN 1.0*j.completed_chunks/j.total_chunks ELSE 0 END,0,NULL,NULL,j.created_at,j.updated_at,j.status IN ('queued','running')
 FROM knowledge_index_jobs j JOIN knowledge_items k ON k.id=j.item_id
UNION ALL
SELECT id,kind,title,status,stage,NULL,NULL,NULL,NULL,output_path,error,current,total,
 CASE WHEN total>0 THEN 1.0*current/total ELSE 0 END,elapsed_ms/1000.0,remaining_ms/1000.0,timing_json,created_at,updated_at,kind='conversion' AND status IN ('queued','running') AND stage NOT IN ('cancelling','publishing') FROM processing_activities WHERE is_service=0
UNION ALL
SELECT id,'reminder',COALESCE(masked_content,'提醒 / 日历同步'),
 CASE status WHEN 'pending' THEN 'waiting' WHEN 'sync_failed' THEN 'failed' WHEN 'delivery_unknown' THEN 'failed'
 WHEN 'dead_lettered' THEN 'failed' WHEN 'cancelled' THEN 'cancelled' ELSE 'completed' END,
 channel || ' · ' || scheduled_at,CASE WHEN entity_type='case' THEN entity_id ELSE NULL END,NULL,NULL,NULL,NULL,last_error,
 0,0,0,0,NULL,NULL,created_at,updated_at,0 FROM reminder_jobs
UNION ALL
SELECT id,'ai_detail',purpose,CASE status WHEN 'pending' THEN 'waiting' ELSE status END,provider || ' / ' || model,
 NULL,NULL,NULL,NULL,NULL,error_message,0,0,0,0,NULL,NULL,created_at,COALESCE(completed_at,created_at),0 FROM ai_runs
"#;
fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProcessingJob> {
    Ok(ProcessingJob {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        stage: row.get(4)?,
        case_id: row.get(5)?,
        case_name: row.get(6)?,
        file_id: row.get(7)?,
        knowledge_id: row.get(8)?,
        output_path: row.get(9)?,
        error: row.get(10)?,
        current: row.get(11)?,
        total: row.get(12)?,
        progress: row.get(13)?,
        elapsed_seconds: row.get(14)?,
        remaining_seconds: row.get(15)?,
        page_timing: row.get::<_,Option<String>>(16)?.map(|value|serde_json::from_str(&value)).transpose().map_err(|error|rusqlite::Error::FromSqlConversionFailure(16,rusqlite::types::Type::Text,Box::new(error)))?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        can_cancel: row.get(19)?,
    })
}
pub(crate) fn list(
    conn: &mut Connection,
    filter: &str,
    offset: i64,
    limit: i64,
) -> anyhow::Result<ProcessingCenter> {
    anyhow::ensure!(
        matches!(
            filter,
            "all" | "active" | "failed" | "completed" | "waiting"
        ),
        "无效的任务筛选"
    );
    anyhow::ensure!(offset >= 0 && (1..=100).contains(&limit), "无效的分页参数");
    let tx = conn.transaction()?;
    let plan = crate::ai::embeddings::EmbeddingPlan::load(&tx)?;
    let fingerprint = plan.as_ref().map(|p| p.fingerprint.as_str()).unwrap_or("");
    let condition = match filter {
        "active" => "status IN ('queued','running')",
        "failed" => "status IN ('failed','stale')",
        "completed" => "status IN ('completed','cancelled')",
        "waiting" => "status='waiting'",
        _ => "1=1",
    };
    let (total,active,failed)=tx.query_row(&format!("WITH jobs AS ({UNION}) SELECT COALESCE(sum({condition}),0),COALESCE(sum(status IN ('queued','running')),0),COALESCE(sum(status IN ('failed','stale')),0) FROM jobs"),rusqlite::named_params![":fingerprint":fingerprint],|r|Ok((r.get(0)?,r.get::<_,i64>(1)?,r.get(2)?)))?;
    let jobs=tx.prepare(&format!("WITH jobs AS ({UNION}) SELECT * FROM jobs WHERE {condition} ORDER BY CASE status WHEN 'running' THEN 0 WHEN 'queued' THEN 1 ELSE 2 END,updated_at DESC,id DESC LIMIT :limit OFFSET :offset"))?.query_map(rusqlite::named_params![":limit":limit,":offset":offset,":fingerprint":fingerprint],map)?.collect::<rusqlite::Result<Vec<_>>>()?;
    let services=tx.prepare("SELECT id,kind,title,status,stage,NULL,NULL,NULL,NULL,NULL,error,current,total,0,elapsed_ms/1000.0,remaining_ms/1000.0,timing_json,created_at,updated_at,0 FROM processing_activities WHERE is_service=1 ORDER BY title")?.query_map([],map)?.collect::<rusqlite::Result<Vec<_>>>()?;
    let failed: i64 = failed;
    let failed = failed + services.iter().filter(|s| s.status == "failed").count() as i64;
    let active = active + services.iter().filter(|s| s.status == "running").count() as i64;
    tx.commit()?;
    Ok(ProcessingCenter {
        jobs,
        services,
        total,
        active,
        failed,
    })
}
#[tauri::command]
pub async fn get_processing_center(
    filter: String,
    offset: i64,
    limit: i64,
) -> Result<ProcessingCenter, String> {
    super::run_blocking(move || list(&mut *db::open_db()?, &filter, offset, limit)).await
}
#[tauri::command]
pub async fn register_conversion_batch(source_paths: Vec<String>) -> Result<Vec<String>, String> {
    super::run_blocking(move || {
        anyhow::ensure!(
            !source_paths.is_empty() && source_paths.len() <= 1000,
            "请选择 1 至 1000 个文件"
        );
        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;
        let mut ids = Vec::new();
        for source in source_paths {
            let title = std::path::Path::new(&source)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("文件转换");
            let id = db::new_id();
            processing::insert(&tx, &id, "conversion", title, Some(&source), true)?;
            ids.push(id);
        }
        tx.commit()?;
        Ok(ids)
    })
    .await
}
#[tauri::command]
pub async fn cancel_queued_conversions(job_ids: Vec<String>) -> Result<(), String> {
    super::run_blocking(move||{
        let conn=db::open_db()?;
        conn.execute("UPDATE processing_activities SET status='cancelled',updated_at=datetime('now','localtime') WHERE kind='conversion' AND status='queued' AND id IN (SELECT value FROM json_each(?1))",[serde_json::to_string(&job_ids)?])?;
        Ok(())
    }).await
}

#[tauri::command]
pub async fn cancel_conversion(job_id: String) -> Result<(), String> {
    super::run_blocking(move || {
        let conn = db::open_db()?;
        let queued = conn.execute("UPDATE processing_activities SET status='cancelled',updated_at=datetime('now','localtime') WHERE id=?1 AND kind='conversion' AND status='queued'", [&job_id])?;
        let running = conn.execute("UPDATE processing_activities SET stage='cancelling',updated_at=datetime('now','localtime') WHERE id=?1 AND kind='conversion' AND status='running' AND stage!='publishing'", [&job_id])?;
        anyhow::ensure!(queued + running > 0, "任务已结束或正在提交输出，不能取消");
        if running > 0 { processing::request_conversion_cancel(&job_id); }
        Ok(())
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version=1;").unwrap();
        db::schema::run_migrations(&conn, 1).unwrap();
        conn
    }
    fn document(conn: &Connection, case: &str, file: &str, job: &str, status: &str, index: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO cases(id,case_name,client_name) VALUES(?1,?1,'测试')",
            [case],
        )
        .unwrap();
        conn.execute("INSERT OR IGNORE INTO case_files(id,case_id,file_name,file_path,category) VALUES(?1,?2,?1,'/tmp/test.pdf','other')",params![file,case]).unwrap();
        conn.execute("INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,index_status) VALUES(?1,?2,'test',?3,?4)",params![job,file,status,index]).unwrap();
    }
    #[test]
    fn cross_case_aggregation_includes_post_ocr_indexing_and_all_history_pages() {
        let mut conn = database();
        document(&conn, "case-a", "file-a", "old", "failed", "none");
        document(&conn, "case-a", "file-a", "new", "completed", "running");
        document(&conn, "case-b", "file-b", "other", "queued", "none");
        conn.execute("UPDATE document_processing_jobs SET elapsed_ms=65000,remaining_ms=30000,timing_json='{\"totalMs\":4200}' WHERE id='new'",[]).unwrap();
        for i in 0..105 {
            processing::insert(
                &conn,
                &format!("conversion-{i:03}"),
                "conversion",
                "转换",
                None,
                false,
            )
            .unwrap();
            processing::finish(
                &conn,
                &format!("conversion-{i:03}"),
                None,
                Some("/tmp/result.md"),
            )
            .unwrap();
        }
        let first = list(&mut conn, "all", 0, 30).unwrap();
        assert_eq!(first.total, 108);
        assert_eq!(first.active, 2);
        assert_eq!(first.failed, 1);
        assert_eq!(first.jobs[0].id, "new");
        assert_eq!(first.jobs[0].stage, "indexing");
        assert_eq!(first.jobs[0].elapsed_seconds, 65.0);
        assert_eq!(first.jobs[0].remaining_seconds, Some(30.0));
        assert_eq!(first.jobs[0].page_timing.as_ref().unwrap().total_ms, 4200);
        assert!(!first.jobs[0].can_cancel);
        assert_eq!(first.jobs[1].case_id.as_deref(), Some("case-b"));
        let mut ids = std::collections::HashSet::new();
        for offset in [0, 30, 60, 90] {
            for job in list(&mut conn, "all", offset, 30).unwrap().jobs {
                assert!(ids.insert(job.id));
            }
        }
        assert_eq!(ids.len(), 108);
        conn.execute("UPDATE document_processing_jobs SET index_status='failed',index_error='索引失败' WHERE id='new'",[]).unwrap();
        let failed = list(&mut conn, "failed", 0, 30).unwrap();
        assert_eq!(failed.total, 2);
        assert_eq!(failed.active, 1);
        assert!(failed
            .jobs
            .iter()
            .any(|j| j.id == "new" && j.error.as_deref() == Some("索引失败")));
    }
    #[test]
    fn interruption_keeps_results_and_durable_document_queue_but_marks_unresumable_work() {
        let mut conn = database();
        document(&conn, "case", "file", "doc", "completed", "running");
        document(&conn, "case", "queued-file", "queued-doc", "queued", "none");
        processing::insert(&conn, "waiting", "conversion", "未开始", None, true).unwrap();
        processing::insert(&conn, "running", "conversion", "转换中", None, false).unwrap();
        processing::insert(&conn, "done", "conversion", "完成", None, false).unwrap();
        processing::finish(&conn, "done", None, Some("/tmp/output.md")).unwrap();
        processing::recover(&conn).unwrap();
        let state = list(&mut conn, "all", 0, 30).unwrap();
        assert_eq!(state.active, 1);
        assert_eq!(state.failed, 3);
        let done = state.jobs.iter().find(|j| j.id == "done").unwrap();
        assert_eq!(done.status, "completed");
        assert_eq!(done.output_path.as_deref(), Some("/tmp/output.md"));
        assert_eq!(
            state.jobs.iter().find(|j| j.id == "doc").unwrap().stage,
            "indexing"
        );
    }
    #[test]
    fn waiting_services_and_future_reminders_do_not_inflate_active_count() {
        let mut conn = database();
        conn.execute("INSERT INTO processing_activities(id,kind,title,status,is_service) VALUES('s','service','检查','waiting',1)",[]).unwrap();
        conn.execute("INSERT INTO reminder_jobs(id,entity_type,entity_id,channel,scheduled_at,status) VALUES('r','case','case','local','2099-01-01','pending')",[]).unwrap();
        let state = list(&mut conn, "all", 0, 30).unwrap();
        assert_eq!(state.active, 0);
        assert_eq!(state.total, 1);
        assert_eq!(state.services.len(), 1);
        conn.execute(
            "UPDATE processing_activities SET status='failed',error='检查失败' WHERE id='s'",
            [],
        )
        .unwrap();
        assert_eq!(list(&mut conn, "all", 0, 30).unwrap().failed, 1);
        assert!(list(&mut conn, "unknown", 0, 30).is_err());
        assert!(list(&mut conn, "all", -1, 30).is_err());
    }
    #[test]
    fn upgrading_legacy_database_backfills_only_latest_index_stage_and_is_repeatable() {
        let mut conn = database();
        document(&conn, "case", "file", "old", "completed", "none");
        document(&conn, "case", "file", "latest", "completed", "none");
        conn.execute_batch("UPDATE case_files SET index_status='failed',ocr_error='旧版索引失败'; DROP TRIGGER fact_history_insert; DROP TRIGGER fact_history_update; DROP TRIGGER fact_history_delete; ALTER TABLE fact_nodes DROP COLUMN knowledge_id; ALTER TABLE fact_nodes DROP COLUMN source_title; DROP TABLE processing_activities; ALTER TABLE document_pages DROP COLUMN layout_json; ALTER TABLE document_pages DROP COLUMN timing_json; ALTER TABLE document_processing_jobs DROP COLUMN phase; ALTER TABLE document_processing_jobs DROP COLUMN elapsed_ms; ALTER TABLE document_processing_jobs DROP COLUMN remaining_ms; ALTER TABLE document_processing_jobs DROP COLUMN timing_json; ALTER TABLE document_processing_jobs DROP COLUMN index_status; ALTER TABLE document_processing_jobs DROP COLUMN index_error; PRAGMA user_version=37;").unwrap();
        db::schema::run_migrations(&conn, 37).unwrap();
        db::schema::run_migrations(&conn, 37).unwrap();
        let state = list(&mut conn, "all", 0, 30).unwrap();
        assert_eq!(
            state.jobs.iter().find(|j| j.id == "old").unwrap().status,
            "completed"
        );
        let latest = state.jobs.iter().find(|j| j.id == "latest").unwrap();
        assert_eq!(latest.status, "failed");
        assert_eq!(latest.error.as_deref(), Some("旧版索引失败"));
    }
}

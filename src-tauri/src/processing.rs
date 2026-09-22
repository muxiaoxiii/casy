//! Shared activity reporting. Periodic services retain their latest cycle; user jobs retain history.
use anyhow::Result;
use rusqlite::{params, Connection};

pub fn insert(
    conn: &Connection,
    id: &str,
    kind: &str,
    title: &str,
    source: Option<&str>,
    queued: bool,
) -> Result<()> {
    conn.execute("INSERT INTO processing_activities(id,kind,title,status,source_path) VALUES(?1,?2,?3,?4,?5)",
        params![id,kind,title,if queued { "queued" } else { "running" },source])?;
    Ok(())
}

pub fn recover(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE ai_runs SET status='failed',error_message='应用退出时 AI 任务被中断',completed_at=datetime('now','localtime') WHERE status='running'", [])?;
    conn.execute("UPDATE processing_activities SET status='failed',error='应用退出时任务被中断，请重新发起',updated_at=datetime('now','localtime') WHERE status IN ('queued','running')", [])?;
    conn.execute("UPDATE document_processing_jobs SET index_status='failed',index_error='应用退出时文档索引被中断，请重新识别',updated_at=datetime('now','localtime') WHERE index_status='running'", [])?;
    conn.execute("UPDATE case_files SET index_status='failed',ocr_error='应用退出时文档索引被中断，请重新识别' WHERE index_status='processing'", [])?;
    Ok(())
}

fn write(f: impl FnOnce(&Connection) -> Result<()>) {
    // Headless unit helpers must never open a user profile implicitly.
    if crate::get_app_handle().is_none() {
        return;
    }
    if let Err(error) = crate::db::open_db().and_then(|conn| f(&conn)) {
        log::error!("处理中心状态写入失败: {error}");
    }
}

pub fn service(id: &str, title: &str, status: &str, stage: &str, error: Option<&str>) {
    write(|conn| {
        conn.execute("INSERT INTO processing_activities(id,kind,title,status,stage,error,is_service) VALUES(?1,'service',?2,?3,?4,?5,1)
        ON CONFLICT(id) DO UPDATE SET status=excluded.status,stage=excluded.stage,error=excluded.error,updated_at=datetime('now','localtime') WHERE processing_activities.status IS NOT excluded.status OR processing_activities.stage IS NOT excluded.stage OR processing_activities.error IS NOT excluded.error",
        params![format!("service:{id}"),title,status,stage,error])?;
        Ok(())
    });
}

pub fn progress(
    id: &str,
    stage: &str,
    current: u32,
    total: u32,
    elapsed_ms: u64,
    remaining_ms: Option<u64>,
    timing: Option<&serde_json::Value>,
) {
    write(|conn| {
        conn.execute("UPDATE processing_activities SET stage=CASE WHEN stage='cancelling' THEN stage ELSE ?2 END,current=?3,total=?4,elapsed_ms=?5,remaining_ms=?6,timing_json=COALESCE(?7,timing_json),updated_at=datetime('now','localtime') WHERE id=?1 AND status='running'",params![id,stage,current,total,elapsed_ms,remaining_ms,timing.map(serde_json::to_string).transpose()?])?;
        Ok(())
    });
}

pub fn finish(
    conn: &Connection,
    id: &str,
    error: Option<&str>,
    output: Option<&str>,
) -> Result<()> {
    conn.execute("UPDATE processing_activities SET status=?2,error=?3,output_path=?4,updated_at=datetime('now','localtime') WHERE id=?1 AND status='running'",
        params![id,if error.is_some_and(|e|e.contains("CONVERSION_CANCELLED")) { "cancelled" } else if error.is_some() { "failed" } else { "completed" },error,output])?;
    Ok(())
}

/// Guards also expose early returns/panics as interrupted instead of leaving an endless spinner.
pub struct Activity {
    id: String,
    finished: bool,
}
impl Activity {
    pub fn start(kind: &str, title: &str) -> Self {
        let id = crate::db::new_id();
        write(|conn| insert(conn, &id, kind, title, None, false));
        Self {
            id,
            finished: false,
        }
    }
    pub fn finish<T, E: std::fmt::Display>(mut self, result: &std::result::Result<T, E>) {
        let error = result.as_ref().err().map(ToString::to_string);
        write(|conn| finish(conn, &self.id, error.as_deref(), None));
        self.finished = true;
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        if !self.finished {
            write(|conn| finish(conn, &self.id, Some("处理被中断，未返回完成结果"), None));
        }
    }
}

pub async fn tracked<T, E: std::fmt::Display>(
    kind: &str,
    title: &str,
    future: impl std::future::Future<Output = std::result::Result<T, E>>,
) -> std::result::Result<T, E> {
    let activity = Activity::start(kind, title);
    let result = future.await;
    activity.finish(&result);
    result
}

// The persisted activity describes state; this token interrupts the live engine process.
static CONVERSION_CANCELLATIONS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<std::sync::atomic::AtomicBool>>>> = std::sync::LazyLock::new(Default::default);
pub struct ConversionCancellation(String);
impl ConversionCancellation {
    pub fn register(id: &str) -> Self {
        CONVERSION_CANCELLATIONS.lock().unwrap_or_else(|e|e.into_inner()).insert(id.into(), Default::default());
        Self(id.into())
    }
}
impl Drop for ConversionCancellation {
    fn drop(&mut self) { CONVERSION_CANCELLATIONS.lock().unwrap_or_else(|e|e.into_inner()).remove(&self.0); }
}
pub fn request_conversion_cancel(id: &str) {
    if let Some(token) = CONVERSION_CANCELLATIONS.lock().unwrap_or_else(|e|e.into_inner()).get(id) {
        token.store(true, std::sync::atomic::Ordering::Release);
    }
}
pub fn check_conversion_cancelled(id: &str) -> Result<()> {
    let cancelled = CONVERSION_CANCELLATIONS.lock().unwrap_or_else(|e|e.into_inner()).get(id)
        .is_some_and(|token|token.load(std::sync::atomic::Ordering::Acquire));
    anyhow::ensure!(!cancelled, "CONVERSION_CANCELLED: 转换已取消，未发布新文件");
    Ok(())
}

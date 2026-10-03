//! Opt-in reconciliation of real case files and their derived knowledge snapshots.
use crate::{
    commands::{files, knowledge},
    db, document_pipeline,
};
use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime},
};
use tauri::Emitter;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Options {
    pub register: bool,
    pub ocr: bool,
    pub knowledge: bool,
    pub embeddings: bool,
    pub name: bool,
    pub content_name: bool,
}

pub fn options(conn: &Connection) -> Result<Options> {
    Ok(db::get_setting(conn, "workspace_sync")?
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default())
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    checked_at: String,
    registered: usize,
    queued: usize,
    mirrored: usize,
    renamed: usize,
    missing: usize,
    errors: Vec<String>,
}
static STATUS: Mutex<Option<SyncStatus>> = Mutex::new(None);

#[tauri::command]
pub async fn get_workspace_sync_status() -> Result<serde_json::Value, String> {
    serde_json::to_value(
        STATUS
            .lock()
            .map_err(|_| "同步状态不可用")?
            .clone()
            .unwrap_or_default(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_workspace_sources(
    case_ids: Vec<String>,
) -> Result<Vec<serde_json::Value>, String> {
    crate::commands::run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare("SELECT f.id,f.case_id,f.file_name,f.file_path,j.id,j.status,j.total_pages,j.error_message,s.missing FROM case_files f
            LEFT JOIN document_processing_jobs j ON j.id=(SELECT id FROM document_processing_jobs WHERE file_id=f.id ORDER BY (status='completed') DESC,rowid DESC LIMIT 1)
            LEFT JOIN workspace_file_state s ON s.file_id=f.id
            WHERE f.deleted_at IS NULL AND f.case_id IN (SELECT value FROM json_each(?1)) ORDER BY f.file_path")?;
        let items = stmt.query_map([serde_json::to_string(&case_ids)?], |r| Ok(serde_json::json!({
            "fileId": r.get::<_,String>(0)?, "caseId":r.get::<_,String>(1)?, "fileName":r.get::<_,String>(2)?,
            "filePath":r.get::<_,String>(3)?, "jobId":r.get::<_,Option<String>>(4)?, "status":r.get::<_,Option<String>>(5)?,
            "totalPages":r.get::<_,Option<i64>>(6)?, "error":r.get::<_,Option<String>>(7)?, "missing":r.get::<_,Option<bool>>(8)?.unwrap_or(false)
        })))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }).await
}

#[tauri::command]
pub async fn get_workspace_document(file_id: String) -> Result<serde_json::Value, String> {
    crate::commands::run_blocking(move || {
        let conn = db::open_db()?;
        let (job,path,hash,markdown_path):(String,String,String,String)=conn.query_row("SELECT j.id,f.file_path,j.source_sha256,j.markdown_path FROM case_files f JOIN document_processing_jobs j ON j.id=(SELECT id FROM document_processing_jobs WHERE file_id=f.id ORDER BY (status='completed') DESC,rowid DESC LIMIT 1) WHERE f.id=?1 AND f.deleted_at IS NULL AND j.status='completed'",[&file_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        if document_pipeline::sha256_file(Path::new(&path))? != hash { bail!("原文件已变化，等待重新提取正文"); }
        if std::fs::metadata(&markdown_path)?.len() > 16 * 1024 * 1024 { bail!("正文超过 16 MiB，请使用分页对照查看"); }
        let markdown = std::fs::read_to_string(&markdown_path)?;
        let pages = {
            let mut stmt=conn.prepare("SELECT page_number,width,height,plain_text,regions_json FROM document_pages WHERE job_id=?1 ORDER BY page_number")?;
            let rows=stmt.query_map([&job],|r|Ok((r.get::<_,u32>(0)?,r.get::<_,Option<f32>>(1)?,r.get::<_,Option<f32>>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            rows.into_iter().map(|(number,width,height,text,regions)| Ok(crate::ai::document_match::PageText{number,width,height,text,regions:serde_json::from_str(&regions)?})).collect::<Result<Vec<_>>>()?
        };
        let continuations=crate::ai::continuations::detect(&pages);
        Ok(serde_json::json!({"jobId":job,"markdown":markdown,"filePath":path,"continuations":continuations}))
    }).await
}

#[derive(Default)]
pub struct Reconciler {
    observed: HashMap<PathBuf, String>,
}

fn stamp(path: &Path) -> Result<String> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        bail!("非普通文件");
    }
    let modified = metadata
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_nanos();
    Ok(format!("{}:{modified}", metadata.len()))
}

fn scan(root: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<()> {
    if depth > 20 || out.len() > 100_000 {
        bail!("卷宗目录过深或文件超过 100,000 个");
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || [".tmp", ".part", ".crdownload", ".download"]
                .iter()
                .any(|tail| name.ends_with(tail))
        {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            scan(&entry.path(), depth + 1, out)?;
        } else if kind.is_file() {
            out.push(entry.path());
        }
    }
    Ok(())
}

fn content_name(markdown: &str, original: &str) -> Option<String> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
    let prefix = markdown.lines().take(80).collect::<Vec<_>>().join("\n");
    let (mut code, mut quote, mut heading) = (false, 0u32, false);
    let mut title = String::new();
    let mut fallback = None;
    let mut text_count = 0;
    for event in Parser::new(&prefix) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code = true,
            Event::End(TagEnd::CodeBlock) => code = false,
            Event::Start(Tag::BlockQuote(_)) => quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote = quote.saturating_sub(1),
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1 | HeadingLevel::H2,
                ..
            }) if quote == 0 => heading = true,
            Event::End(TagEnd::Heading(_)) if heading => {
                if !title.trim().is_empty() {
                    break;
                }
                heading = false;
            }
            Event::Text(text) | Event::Code(text) if !code && quote == 0 => {
                if heading {
                    title.push_str(&text);
                } else {
                    text_count += 1;
                    let text = text.trim();
                    if text_count <= 12
                        && fallback.is_none()
                        && (4..=50).contains(&text.chars().count())
                        && !text.contains(['。', '！', '？'])
                        && [
                            "判决书",
                            "裁定书",
                            "起诉状",
                            "答辩状",
                            "代理意见",
                            "审查意见通知书",
                            "专利说明书",
                            "证据目录",
                        ]
                        .iter()
                        .any(|kind| text.ends_with(kind))
                    {
                        fallback = Some(text.to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    if title.trim().is_empty() {
        title = fallback?;
    }
    let title = crate::files::sanitize_filename(&title)
        .chars()
        .filter(|c| !c.is_control())
        .take(55)
        .collect::<String>();
    let title = title.trim_matches(['.', ' ', '_']);
    if title.chars().count() < 4 {
        return None;
    }
    let ext = Path::new(original).extension()?.to_str()?;
    Some(format!("{title}.{ext}"))
}



impl Reconciler {
    pub fn tick(&mut self) -> Result<SyncStatus> {
        let mut conn = db::open_db()?;
        let opts = options(&conn)?;
        let mut status = SyncStatus {
            checked_at: db::now_local(),
            ..Default::default()
        };
        if !(opts.register
            || opts.ocr
            || opts.knowledge
            || opts.embeddings
            || opts.name
            || opts.content_name)
        {
            self.observed.clear();
            return Ok(status);
        }
        let roots = {
            let mut stmt = conn.prepare("SELECT id,folder_path FROM cases WHERE folder_path IS NOT NULL AND folder_path!=''")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        let mut observed = HashMap::new();
        for (case_id, root) in roots {
            let root = match Path::new(&root).canonicalize() {
                Ok(root) => root,
                Err(_) => {
                    status.errors.push(format!("卷宗目录不可用：{root}"));
                    continue;
                }
            };
            let mut paths = vec![];
            if let Err(error) = scan(&root, 0, &mut paths) {
                status.errors.push(error.to_string());
                continue;
            }
            for path in paths {
                let current_stamp = match stamp(&path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                observed.insert(path.clone(), current_stamp.clone());
                if self.observed.get(&path) != Some(&current_stamp) {
                    continue;
                }
                if let Err(error) = self.sync_file(
                    &mut conn,
                    &opts,
                    &case_id,
                    &path,
                    &current_stamp,
                    &mut status,
                ) {
                    if status.errors.len() < 50 {
                        status.errors.push(format!("{}：{error}", path.display()));
                    }
                }
            }
        }
        self.observed = observed;
        let tracked = {
            let mut stmt = conn.prepare("SELECT f.id,f.file_path,s.missing FROM case_files f JOIN workspace_file_state s ON s.file_id=f.id WHERE f.deleted_at IS NULL")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?,r.get::<_,bool>(2)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for (id, path, was_missing) in tracked {
            let missing = !Path::new(&path).is_file();
            if missing != was_missing { conn.execute(
                "UPDATE workspace_file_state SET missing=?2 WHERE file_id=?1 AND missing IS NOT ?2",
                params![id, missing],
            )?; }
            status.missing += usize::from(missing);
        }
        if opts.embeddings {
            if let Some(plan) = crate::ai::embeddings::EmbeddingPlan::load(&conn)? {
                let ids = {
                    let mut stmt = conn.prepare("SELECT id FROM knowledge_items WHERE parent_id IS NULL OR parent_id='' ORDER BY updated_at DESC")?;
                    let rows = stmt
                        .query_map([], |r| r.get::<_, String>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()?;
                    rows
                };
                let mut report = crate::db::knowledge_index::QueueReport::default();
                for id in ids {
                    let failed: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_index_jobs WHERE item_id=?1 AND config_hash=?2 AND status IN ('failed','cancelled') AND source_hash=(SELECT ?3))", params![id,plan.fingerprint, {
                        let (title, content):(String,String)=conn.query_row("SELECT title,content FROM knowledge_items WHERE id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?)))?;
                        crate::db::knowledge_index::source_hash(&title,&content)
                    }], |r|r.get(0))?;
                    if !failed {
                        crate::db::knowledge_index::queue_inner(&conn, &id, &plan, &mut report)?;
                    }
                }
            } else {
                status.errors.push("自动向量索引尚未配置接口".into());
            }
        }
        Ok(status)
    }

    fn sync_file(
        &self,
        conn: &mut Connection,
        opts: &Options,
        case_id: &str,
        path: &Path,
        current_stamp: &str,
        status: &mut SyncStatus,
    ) -> Result<()> {
        let path_text = path.to_string_lossy();
        let known: Option<(String, bool)> = conn.query_row("SELECT id,deleted_at IS NOT NULL FROM case_files WHERE case_id=?1 AND file_path=?2",params![case_id,path_text],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if known.as_ref().is_some_and(|(_, deleted)| *deleted) {
            return Ok(());
        }
        if known.is_none() && !opts.register {
            return Ok(());
        }
        let cached: Option<(String, String)> = if let Some((id, _)) = &known {
            conn.query_row(
                "SELECT stamp,sha256 FROM workspace_file_state WHERE file_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
        } else {
            None
        };
        let hash = match cached {
            Some((old_stamp, hash)) if old_stamp == current_stamp => hash,
            _ => document_pipeline::sha256_file(path)?,
        };
        if stamp(path)? != current_stamp {
            return Ok(());
        }
        let id = if let Some((id, _)) = known {
            id
        } else {
            // An unambiguous moved file keeps its ID and all source citations.
            let missing = {
                let mut stmt = conn.prepare("SELECT f.id,f.file_path FROM case_files f JOIN workspace_file_state s ON s.file_id=f.id WHERE f.case_id=?1 AND f.deleted_at IS NULL AND s.sha256=?2")?;
                let rows = stmt
                    .query_map(params![case_id, hash], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?
                    .into_iter()
                    .filter(|(_, p)| !Path::new(p).exists())
                    .collect::<Vec<_>>();
                rows
            };
            if missing.len() == 1 {
                let id = &missing[0].0;
                let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                    log::warn!("跳过缺少文件名的路径: {}", path.display());
                    return Ok(());
                };
                let tx =
                    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                tx.execute(
                    "UPDATE case_files SET file_path=?2,file_name=?3 WHERE id=?1",
                    params![id, path_text, file_name],
                )?;
                files::relocate_knowledge_references(&tx, Path::new(&missing[0].1), path)?;
                tx.commit()?;
                status.renamed += 1;
                id.clone()
            } else {
                let tx =
                    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let file = files::insert_file(&tx, case_id, path, "other", None)?;
                tx.commit()?;
                status.registered += 1;
                file.id
            }
        };
        conn.execute("INSERT INTO workspace_file_state(file_id,stamp,sha256) VALUES(?1,?2,?3) ON CONFLICT(file_id) DO UPDATE SET stamp=excluded.stamp,sha256=excluded.sha256,missing=0",params![id,current_stamp,hash])?;
        conn.execute(
            "UPDATE case_files SET file_size=?2 WHERE id=?1",
            params![id, std::fs::metadata(path)?.len() as i64],
        )?;
        let (mut name,named,content_named):(String,bool,Option<String>)=conn.query_row("SELECT f.file_name,s.named,s.content_named_job FROM case_files f JOIN workspace_file_state s ON s.file_id=f.id WHERE f.id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if opts.name && !named {
            let case = db::cases::get_case(conn, case_id)?;
            let code = case
                .case_no
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&case.id[..8.min(case.id.len())]);
            let prefix = crate::files::sanitize_filename(code);
            let new_name = if name.starts_with(&format!("{prefix}_")) {
                name.clone()
            } else {
                format!("{prefix}_{name}")
            };
            let result = files::relocate_files(
                case_id,
                &[files::RenameItem {
                    id: id.clone(),
                    new_name,
                }],
                None,
            )?;
            name = result[0].new_name.clone();
            status.renamed += usize::from(result[0].old_name != name);
            conn.execute(
                "UPDATE workspace_file_state SET named=1 WHERE file_id=?1",
                [&id],
            )?;
        }
        let job:Option<(String,String,String)> = conn.query_row("SELECT id,status,source_sha256 FROM document_processing_jobs WHERE file_id=?1 ORDER BY rowid DESC LIMIT 1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if opts.ocr
            && document_pipeline::supports_path(path)
            && job.as_ref().is_none_or(|(_, state, sha)| {
                sha != &hash && !["running", "queued"].contains(&state.as_str())
            })
        {
            crate::commands::document_intelligence::queue_file(conn, &id)?;
            status.queued += 1;
            return Ok(());
        }
        if let Some((job_id, state, sha)) = job {
            if state != "completed" || sha != hash {
                return Ok(());
            }
            if opts.content_name && content_named.as_deref() != Some(job_id.as_str()) {
                let markdown: String = conn.query_row("SELECT COALESCE(group_concat(markdown,char(10)),'') FROM (SELECT markdown FROM document_pages WHERE job_id=?1 ORDER BY page_number LIMIT 2)",[&job_id],|r|r.get(0))?;
                if let Some(new_name) = content_name(&markdown, &name) {
                    let new_name = if opts.name {
                        let case = db::cases::get_case(conn, case_id)?;
                        let code = case
                            .case_no
                            .as_deref()
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| case.id.get(..8).unwrap_or(&case.id));
                        format!("{}_{}", crate::files::sanitize_filename(code), new_name)
                    } else {
                        new_name
                    };
                    let result = files::relocate_files(
                        case_id,
                        &[files::RenameItem {
                            id: id.clone(),
                            new_name,
                        }],
                        None,
                    )?;
                    status.renamed += usize::from(result[0].old_name != result[0].new_name);
                }
                conn.execute(
                    "UPDATE workspace_file_state SET content_named_job=?2 WHERE file_id=?1",
                    params![id, job_id],
                )?;
            }
            let indexed: bool = conn.query_row(
                "SELECT index_status='completed' FROM case_files WHERE id=?1",
                [&id],
                |r| r.get(0),
            )?;
            if opts.knowledge && indexed {
                let result = knowledge::import_pageindex_inner(conn, &id)?;
                status.mirrored += usize::from(!result.reused);
            }
        }
        Ok(())
    }
}

pub fn start() {
    tauri::async_runtime::spawn(async {
        let mut reconciler = Reconciler::default();
        loop {
            let result = tauri::async_runtime::spawn_blocking(move || {
                let enabled=db::open_db().and_then(|conn|options(&conn)).map(|o|o.register||o.ocr||o.knowledge||o.embeddings||o.name||o.content_name);
                if matches!(enabled,Ok(false)) {
                    crate::processing::service("workspace","案件目录自动同步","disabled","尚未在设置中启用",None);
                } else {
                    crate::processing::service("workspace","案件目录自动同步","running","检查目录与文件变化",None);
                }
                let result = reconciler.tick();
                if !matches!(enabled,Ok(false)) {
                    let error=match &result {Ok(s) if !s.errors.is_empty()=>Some(s.errors.join("；")),Err(e)=>Some(e.to_string()),_=>None};
                    let summary=match &result {Ok(s)=>format!("最近检查：登记 {}，排队 {}，知识同步 {}，更名 {}；每 10 秒检查",s.registered,s.queued,s.mirrored,s.renamed),Err(_)=>"目录检查失败".into()};
                    crate::processing::service("workspace","案件目录自动同步",if error.is_some(){"failed"}else{"waiting"},&summary,error.as_deref());
                }
                (reconciler, result)
            })
            .await;
            match result {
                Ok((state, result)) => {
                    reconciler = state;
                    match result {
                        Ok(status) => {
                            if status.registered + status.queued + status.mirrored + status.renamed
                                > 0
                            {
                                if let Some(app) = crate::get_app_handle() {
                                    let _ = app.emit("workspace:updated", &status);
                                }
                            }
                            if let Ok(mut value) = STATUS.lock() {
                                *value = Some(status);
                            }
                        }
                        Err(error) => log::warn!("Workspace reconciliation: {error}"),
                    }
                }
                Err(error) => {
                    log::error!("Workspace worker stopped: {error}");
                    return;
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn naming_ignores_quoted_or_code_titles_and_preserves_extensions() {
        assert_eq!(
            content_name(
                "```\n# Wrong Heading\n```\n\n> # Other Source\n\n# Actual **Patent** Title",
                "scan.pdf"
            ),
            Some("Actual Patent Title.pdf".into())
        );
        assert_eq!(content_name("> 北京市某法院民事判决书", "scan.pdf"), None);
        assert_eq!(
            content_name("北京市某法院民事判决书", "scan.pdf"),
            Some("北京市某法院民事判决书.pdf".into())
        );
        assert_eq!(content_name("正文没有可靠标题", "scan.pdf"), None);
    }
}

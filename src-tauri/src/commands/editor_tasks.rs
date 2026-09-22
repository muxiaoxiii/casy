//! Explicit links between document checklists and tasks. Ordinary checklists stay local.
use super::run_blocking;
use crate::db;
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorTask {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub missing: bool,
}
fn verify_source(conn: &Connection, kind: &str, id: &str) -> Result<()> {
    let table = match kind {
        "knowledge" => "knowledge_items",
        "doc" => "drafts",
        _ => anyhow::bail!("无效的文档类型"),
    };
    ensure!(
        conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),
            [id],
            |r| r.get::<_, bool>(0)
        )?,
        "来源文档不存在"
    );
    Ok(())
}
#[tauri::command]
pub async fn get_editor_tasks(
    source_type: String,
    source_id: String,
) -> Result<Vec<EditorTask>, String> {
    run_blocking(move||{let conn=db::open_db()?;verify_source(&conn,&source_type,&source_id)?;
 let rows=conn.prepare("SELECT DISTINCT l.target_id,COALESCE(t.task_name,l.label,''),COALESCE(t.completed,0),t.id IS NULL OR t.deleted_at IS NOT NULL FROM links l LEFT JOIN tasks t ON t.id=l.target_id WHERE l.source_type=?1 AND l.source_id=?2 AND l.target_type='task'")?.query_map(params![source_type,source_id],|r|Ok(EditorTask{id:r.get(0)?,title:r.get(1)?,completed:r.get(2)?,missing:r.get(3)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;Ok(rows)}).await
}
#[tauri::command]
pub async fn bind_editor_task(
    source_type: String,
    source_id: String,
    binding_id: String,
    title: String,
    task_id: Option<String>,
    case_id: Option<String>,
) -> Result<String, String> {
    run_blocking(move||{let mut conn=db::open_db()?;let tx=conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;verify_source(&tx,&source_type,&source_id)?;
 ensure!(!binding_id.is_empty() && binding_id.len()<=80 && !title.trim().is_empty(),"请填写待办内容");
 let existing:Option<String>=tx.query_row("SELECT target_id FROM links WHERE source_type=?1 AND source_id=?2 AND anchor=?3 AND target_type='task'",params![source_type,source_id,format!("checklist:{binding_id}")],|r|r.get(0)).optional()?;
 if let Some(id)=existing{return Ok(id)}
 let id=if let Some(id)=task_id {ensure!(tx.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND deleted_at IS NULL)",[&id],|r|r.get::<_,bool>(0))?,"任务已移除");id}else {
  let value=super::tasks::create_task_in_transaction(&tx,serde_json::json!({"taskName":title,"caseId":case_id,"knowledgeId":if source_type=="knowledge"{Some(&source_id)}else{None},"notes":"由文档待办关联创建","startBucket":"inbox"}))?;value["id"].as_str().ok_or_else(|| anyhow::anyhow!("创建任务未返回有效 ID，关联已回滚"))?.to_owned()
 };
 tx.execute("INSERT INTO links(id,source_type,source_id,target_type,target_id,anchor,label) VALUES(?1,?2,?3,'task',?4,?5,?6)",params![db::new_id(),source_type,source_id,id,format!("checklist:{binding_id}"),title])?;tx.commit()?;Ok(id)}).await
}
#[tauri::command]
pub async fn set_editor_task_completed(
    source_type: String,
    source_id: String,
    task_id: String,
    completed: bool,
    expected_completed: bool,
) -> Result<(), String> {
    let cancelled=run_blocking(move||{let mut conn=db::open_db()?;let tx=conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;verify_source(&tx,&source_type,&source_id)?;
 ensure!(tx.query_row("SELECT EXISTS(SELECT 1 FROM links WHERE source_type=?1 AND source_id=?2 AND target_type='task' AND target_id=?3)",params![source_type,source_id,task_id],|r|r.get::<_,bool>(0))?,"此任务尚未与当前文档关联");
 let current:bool=tx.query_row("SELECT completed FROM tasks WHERE id=?1 AND deleted_at IS NULL",[&task_id],|r|r.get(0)).map_err(|_|anyhow::anyhow!("关联任务已移除"))?;
 if current==completed{return Ok(vec![])}
 ensure!(current==expected_completed,"任务状态已变化，请刷新后核对");
 tx.execute("UPDATE tasks SET completed=?2 WHERE id=?1",params![task_id,completed])?;
 tx.execute("INSERT INTO task_events(id,task_id,event_type,occurred_at,payload,actor) VALUES(?1,?2,?3,?4,?5,'user')",params![db::new_id(),task_id,if completed{"completed"}else{"restored"},db::now_local(),serde_json::json!({"sourceType":source_type,"sourceId":source_id}).to_string()])?;
 let cancelled=super::task_lifecycle::completion_effects(&tx,&task_id,if completed{1}else{0})?;tx.commit()?;Ok(cancelled)}).await?;
    for id in cancelled {
        if let Err(error) = super::caldav::cancel_jobs_for_entity("task", &id).await {
            log::warn!("取消任务提醒失败: {error}")
        }
    }
    Ok(())
}

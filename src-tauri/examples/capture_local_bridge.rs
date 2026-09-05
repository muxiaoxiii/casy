//! Synthetic capture QA only. Does not start sync, document workers, or external AI.
use anyhow::{bail, Result};
use casy_lib::{commands::{inbox, cases, tasks, calendar_events, knowledge}, db};
use serde_json::{json, Value};
use std::io::Read;

fn state() -> Result<Value> {
    let conn = db::open_db()?;
    let items: Vec<Value> = conn.prepare("SELECT id,status,source_type,source_path,content_text,linked_case_id FROM inbox_items")?
        .query_map([], |r| Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"sourceType":r.get::<_,String>(2)?,"sourcePath":r.get::<_,Option<String>>(3)?,"content":r.get::<_,Option<String>>(4)?,"caseId":r.get::<_,Option<String>>(5)?})))?.collect::<rusqlite::Result<_>>()?;
    let tasks: Vec<Value> = conn.prepare("SELECT id,task_name,due_date,due_time,inbox_source_id,case_id,description FROM tasks")?
        .query_map([], |r| Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"date":r.get::<_,Option<String>>(2)?,"time":r.get::<_,Option<String>>(3)?,"source":r.get::<_,Option<String>>(4)?,"caseId":r.get::<_,Option<String>>(5)?,"description":r.get::<_,Option<String>>(6)?})))?.collect::<rusqlite::Result<_>>()?;
    let files: Vec<Value> = conn.prepare("SELECT id,case_id,file_path FROM case_files")?
        .query_map([], |r| Ok(json!({"id":r.get::<_,String>(0)?,"caseId":r.get::<_,String>(1)?,"path":r.get::<_,String>(2)?})))?.collect::<rusqlite::Result<_>>()?;
    Ok(json!({"items":items,"tasks":tasks,"files":files,"foreignKeyErrors":conn.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0))?,"integrity":conn.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0))?}))
}

#[tokio::main]
async fn main() -> Result<()> {
    if !cfg!(debug_assertions) || std::env::var_os("CASY_TEST_DATA_DIR").is_none() { bail!("Capture QA requires an isolated debug profile"); }
    let mut raw = String::new(); std::io::stdin().read_to_string(&mut raw)?;
    let request: Value = serde_json::from_str(&raw)?;
    let conn = db::open_db()?; db::init_db(&conn)?;
    if casy_lib::ai::load_ai_config().mode != "noop" { bail!("External AI is disabled for capture QA"); }
    let p = &request["args"];
    let text = |key: &str| p[key].as_str().map(str::to_owned);
    let result: Result<Value,String> = match request["command"].as_str().unwrap_or("") {
        "qa_state" => state().map_err(|e|e.to_string()),
        "qa_seed" => {
            conn.execute("INSERT INTO cases(id,case_name,client_name) VALUES('capture-qa-case','捕获验收案件','合成客户')",[])?;
            Ok(json!(true))
        }
        "add_inbox_item" => inbox::add_inbox_item(text("sourceType").unwrap_or_default(),text("title"),text("contentText"),text("sourcePath")).await.map(|v|json!(v)),
        "list_inbox_items" => inbox::list_inbox_items(text("status")).await.map(|v|json!(v)),
        "quick_judge_inbox_item" => inbox::quick_judge_inbox_item(text("id").unwrap_or_default()).await.map(|v|json!(v)),
        "confirm_inbox_action" => inbox::confirm_inbox_action(text("inboxItemId").unwrap_or_default(),text("action").unwrap_or_default(),text("targetCaseId"),text("targetCategory"),p.get("intent").cloned()).await,
        "list_cases" => cases::list_cases(serde_json::from_value(p["filter"].clone()).unwrap_or_default()).await.map(|v|json!(v)),
        "get_case" => cases::get_case(text("id").unwrap_or_default()).await.map(|v|json!(v)),
        "list_tasks" => tasks::list_tasks(None).await.map(|v|json!(v)),
        "list_calendar_events" => calendar_events::list_calendar_events(text("startDate").unwrap_or_default(),text("endDate").unwrap_or_default()).await.map(|v|json!(v)),
        "list_knowledge" => knowledge::list_knowledge(serde_json::from_value(p["filter"].clone()).unwrap_or_default()).await.map(|v|json!(v)),
        _ => Err("Command not permitted in capture QA".into()),
    };
    println!("{}",match result { Ok(data)=>json!({"ok":true,"data":data}),Err(error)=>json!({"ok":false,"error":error}) });
    Ok(())
}

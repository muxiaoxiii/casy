//! Synthetic dashboard QA. Isolated debug database, no sync or external AI workers.
use anyhow::{bail, Result};
use casy_lib::{commands::{self, calendar, cases, dashboard, tasks, timeline}, db};
use chrono::{Datelike, Duration, Months};
use rusqlite::params;
use serde_json::{json, Value};
use std::io::Read;

fn seed() -> Result<Value> {
    let mut conn = db::open_db()?;
    let tx = conn.transaction()?;
    let today = chrono::Local::now().date_naive();
    let previous = today.with_day(1).unwrap().checked_sub_months(Months::new(1)).unwrap();
    for (id, name, track, status) in [
        ("qa-civil", "合成民事案件甲", "civil_tort", "在办"),
        ("qa-closed", "合成民事案件乙", "civil_tort", "已完结"),
        ("qa-admin", "合成行政案件", "admin_litigation", "在办"),
    ] {
        tx.execute("INSERT INTO cases(id,case_name,client_name,track,case_status) VALUES(?1,?2,'合成客户',?3,?4)", params![id,name,track,status])?;
    }
    for (id, name, completed) in [("qa-due","今天到期任务",0),("qa-deadline","今天截止任务",0),("qa-waiting","等待法院回执",0),("qa-person","等待第三人材料",0),("qa-done","已完成任务",1),("qa-old","上月已完成任务",1)] {
        tx.execute("INSERT INTO tasks(id,task_name,case_id,start_bucket,created_date,completed) VALUES(?1,?2,'qa-civil','anytime',?3,?4)", params![id,name,if id=="qa-old"{previous.to_string()}else{today.to_string()},completed])?;
    }
    tx.execute("UPDATE tasks SET due_date=?1 WHERE id='qa-due'",[today.to_string()])?;
    tx.execute("UPDATE tasks SET deadline=?1 WHERE id='qa-deadline'",[today.to_string()])?;
    tx.execute("UPDATE tasks SET task_type='waiting',follow_up_date=?1,next_review_date=?2 WHERE id='qa-waiting'",params![(today-Duration::days(1)).to_string(),today.to_string()])?;
    tx.execute("UPDATE tasks SET waiting_for='第三人',follow_up_date=?1 WHERE id='qa-person'",[(today-Duration::days(1)).to_string()])?;
    for (id, task, date) in [("qa-completed","qa-done",today),("qa-completed-old","qa-old",previous)] {
        tx.execute("INSERT INTO task_events(id,task_id,event_type,occurred_at,actor) VALUES(?1,?2,'completed',?3,'user')",params![id,task,date.to_string()])?;
    }
    for (id,name,date) in [("qa-hearing","第三人参加庭审",format!("{today} 09:37:00")),("qa-allday","未定时庭审",today.to_string()),("qa-last","第三十天庭审",format!("{} 15:10:00",today+Duration::days(30)))] {
        tx.execute("INSERT INTO hearings(id,case_id,hearing_record,hearing_name,hearing_date) VALUES(?1,'qa-civil',?2,?2,?3)",params![id,name,date])?;
    }
    tx.execute("INSERT INTO calendar_events(id,title,event_date,start_time,end_time,all_day,case_id,created_at,updated_at) VALUES('qa-event','早间材料核对',?1,'07:15','07:45',0,'qa-civil',datetime('now'),datetime('now'))",[today.to_string()])?;
    tx.commit()?;
    Ok(json!({"today":today.to_string()}))
}

#[tokio::main]
async fn main() -> Result<()> {
    if !cfg!(debug_assertions) || std::env::var_os("CASY_TEST_DATA_DIR").is_none() { bail!("Dashboard QA requires an isolated debug profile"); }
    let mut raw = String::new(); std::io::stdin().read_to_string(&mut raw)?;
    let request: Value = serde_json::from_str(&raw)?;
    let conn = db::open_db()?; db::init_db(&conn)?;
    let p = &request["args"];
    let id = || p["id"].as_str().unwrap_or_default().to_string();
    let result: Result<Value,String> = match request["command"].as_str().unwrap_or_default() {
        "qa_seed" => seed().map_err(|error|error.to_string()),
        "qa_state" => Ok(json!({"integrity":conn.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0))?,"foreignKeyErrors":conn.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0))?})),
        "get_monthly_task_trend" => dashboard::get_monthly_task_trend(p["months"].as_i64().map(|v|v as i32)).await.map(|v|json!(v)),
        "get_track_distribution" => dashboard::get_track_distribution().await.map(|v|json!(v)),
        "get_upcoming_hearings" => dashboard::get_upcoming_hearings(p["days"].as_i64().map(|v|v as i32)).await.map(|v|json!(v)),
        "get_today_kpis" => dashboard::get_today_kpis().await.map(|v|json!(v)),
        "case_stats" => cases::case_stats().await.map(|v|json!(v)),
        "list_cases" => cases::list_cases(serde_json::from_value(p["filter"].clone()).unwrap_or_default()).await.map(|v|json!(v)),
        "get_case" => cases::get_case(id()).await.map(|v|json!(v)),
        "list_case_hearings" => cases::list_case_hearings(p["caseId"].as_str().unwrap_or_default().into()).await.map(|v|json!(v)),
        "get_case_timeline" => timeline::get_case_timeline(p["caseId"].as_str().unwrap_or_default().into()).await.map(|v|json!(v)),
        "get_relations" => commands::relations::get_relations(p["caseId"].as_str().unwrap_or_default().into()).await.map(|v|json!(v)),
        "list_case_persons" => commands::persons::list_case_persons(p["caseId"].as_str().unwrap_or_default().into()).await.map(|v|json!(v)),
        "list_tasks" => tasks::list_tasks(serde_json::from_value(p["filter"].clone()).ok()).await.map(|v|json!(v)),
        "create_task" => tasks::create_task(p["data"].clone()).await.map(|v|json!(v)),
        "update_task" => tasks::update_task(p["data"].clone()).await.map(|v|json!(v)),
        "toggle_task" => tasks::toggle_task(id(),None,None,None).await.map(|v|json!(v)),
        "restore_task" => tasks::restore_task(p["snapshot"].clone()).await.map(|v|json!(v)),
        "get_calendar_events" => calendar::get_calendar_events(p["year"].as_i64().unwrap_or(2026) as i32,p["month"].as_u64().unwrap_or(9) as u32, p["monthCount"].as_u64().map(|v|v as u32)).await.map(|v|json!(v)),
        "get_deadline_warnings" => commands::get_deadline_warnings().await.map(|v|json!(v)),
        "get_holiday_calendar" => commands::settings::get_holiday_calendar(p["year"].as_i64().unwrap_or(2026) as i32).await,
        _ => Err("Command outside dashboard QA allowlist".into()),
    };
    println!("{}",match result{Ok(data)=>json!({"ok":true,"data":data}),Err(error)=>json!({"ok":false,"error":error})});
    Ok(())
}

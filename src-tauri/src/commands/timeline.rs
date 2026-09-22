use super::run_blocking;
use crate::db;

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub id: String,
    pub source_table: String,
    pub source_id: String,
    pub event_date: String,
    pub event_type: String,
    pub title: String,
    pub detail: Option<String>,
    pub icon: String,
    pub color: String,
}

#[tauri::command]
pub async fn get_case_timeline(case_id: String) -> Result<Vec<TimelineEvent>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut events = Vec::new();

        // 办案日志
        let mut stmt = conn.prepare(
            "SELECT id, event_date, event_type, event_summary, content
             FROM case_logs WHERE case_id = ?1 OR EXISTS (SELECT 1 FROM case_log_links cll WHERE cll.log_id=case_logs.id AND cll.case_id=?1)",
        )?;
        for row in stmt.query_map(rusqlite::params![case_id], |r| {
            let event_type: String = r.get(2)?;
            Ok(TimelineEvent {
                id: r.get(0)?,
                source_table: "case_logs".into(),
                source_id: r.get::<_, String>(0)?,
                event_date: r.get(1)?,
                icon: match_event_icon(&event_type),
                color: match_event_color(&event_type),
                event_type,
                title: r.get(3)?,
                detail: r.get(4)?,
            })
        })? {
            events.push(row?);
        }

        // 庭审
        let mut stmt = conn.prepare(
            "SELECT id, hearing_date, hearing_name, venue, lifecycle_status, change_reason
             FROM hearings WHERE case_id = ?1 OR EXISTS (SELECT 1 FROM case_hearing_links chl WHERE chl.hearing_id=hearings.id AND chl.case_id=?1)",
        )?;
        for row in stmt.query_map(rusqlite::params![case_id], |r| {
            Ok(TimelineEvent {
                id: r.get(0)?,
                source_table: "hearings".into(),
                source_id: r.get::<_, String>(0)?,
                event_date: r.get(1)?,
                icon: "📅".into(),
                color: "#3b82f6".into(),
                event_type: "hearing".into(),
                title: r
                    .get::<_, Option<String>>(2)?
                    .unwrap_or_else(|| "开庭".into()),
                detail: Some(format!("{} · {} · {}",r.get::<_,Option<String>>(3)?.unwrap_or_default(),match r.get::<_,String>(4)?.as_str(){"held"=>"已开庭","postponed"=>"延期待定","cancelled"=>"已取消",_=>"已排期"},r.get::<_,String>(5)?)),
            })
        })? {
            events.push(row?);
        }

        // 任务
        let mut stmt = conn.prepare(
            "SELECT id, created_date, task_name, description, completed
             FROM tasks WHERE (case_id = ?1 OR EXISTS (SELECT 1 FROM case_task_links ctl WHERE ctl.task_id=tasks.id AND ctl.case_id=?1)) AND deleted_at IS NULL",
        )?;
        for row in stmt.query_map(rusqlite::params![case_id], |r| {
            let completed: i32 = r.get(4)?;
            Ok(TimelineEvent {
                id: r.get(0)?,
                source_table: "tasks".into(),
                source_id: r.get::<_, String>(0)?,
                event_date: r.get(1)?,
                icon: if completed == 1 { "✅" } else { "📌" }.into(),
                color: "#8b5cf6".into(),
                event_type: "task".into(),
                title: r.get(2)?,
                detail: r.get(3)?,
            })
        })? {
            events.push(row?);
        }

        // Each receipt/forwarding/summons remains an independent event, including retractions.
        let mut stmt=conn.prepare("SELECT id,created_at,payload FROM procedure_events WHERE case_id=?1")?;
        for row in stmt.query_map([&case_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))? {
            let (id, created, payload) = row?;
            let event:crate::deadline::procedure::ProcedureEvent=match serde_json::from_str(&payload) {
                Ok(event) => event,
                Err(error) => {
                    log::warn!("Invalid procedure event {id}: {error}");
                    events.push(TimelineEvent { id:format!("procedure:{id}"),source_table:"procedure_events".into(),source_id:id,event_date:created,event_type:"procedure".into(),title:"程序事件数据损坏，待核对".into(),detail:Some("原始记录仍保留，请从备份核对并修复此事件。".into()),icon:"⚠️".into(),color:"#b45309".into() });
                    continue;
                }
            };
            events.push(TimelineEvent{id:format!("procedure:{}",event.id),source_table:"procedure_events".into(),source_id:event.id,event_date:event.occurred_on,event_type:"procedure".into(),title:format!("{}{}",event.title,if event.retracted{"（已撤销）"}else{""}),detail:Some(format!("责任方：{}；转文：{}；有效起算／送达：{}；{}",event.actor_role,event.forwarded_on.as_deref().unwrap_or("未登记"),event.start_on.as_deref().unwrap_or("未核实"),event.source_note)),icon:"📥".into(),color:"#0f766e".into()});
        }
        // 按日期倒序
        events.sort_by(|a, b| b.event_date.cmp(&a.event_date));
        Ok(events)
    })
    .await
}

#[tauri::command]
pub async fn add_case_log(
    case_id: String,
    event_summary: String,
    event_type: String,
    event_date: String,
    content: Option<String>,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        conn.execute(
            "INSERT INTO case_logs (id, case_id, event_summary, event_type, event_date, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id, case_id, event_summary, event_type, event_date,
                content.unwrap_or_default(), db::now_local(),
            ],
        )?;
        Ok(id)
    })
    .await
}

#[tauri::command]
pub async fn delete_case_log(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM case_logs WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
    .await
}

fn match_event_icon(event_type: &str) -> String {
    match event_type {
        "submitted" => "📤",
        "received" => "📥",
        "record" => "📝",
        "task" => "📌",
        _ => "📄",
    }
    .into()
}

fn match_event_color(event_type: &str) -> String {
    match event_type {
        "submitted" => "#22c55e",
        "received" => "#3b82f6",
        "record" => "#6b7280",
        "task" => "#8b5cf6",
        _ => "#9ca3af",
    }
    .into()
}

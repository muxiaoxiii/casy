//! Rest-day planning notices are local-only and never alter legal deadlines.
use std::collections::{BTreeMap, HashMap, HashSet};
use anyhow::Result;
use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection};
use crate::{db, deadline::holidays::HolidayCalendar};

pub(super) fn refresh(conn: &Connection, today: NaiveDate) -> Result<()> {
    let official = match db::get_setting(conn, "holidays_json")? {
        Some(raw) => HolidayCalendar::from_json_str(&raw).map_err(anyhow::Error::msg)?,
        None => HolidayCalendar::builtin(),
    };
    let mut personal = HashMap::new();
    if let Some(raw) = db::get_setting(conn, "personal_calendar_days")? {
        let rows: Vec<serde_json::Value> = serde_json::from_str(&raw)?;
        for row in rows {
            if let (Some(date), Some(kind)) = (row["date"].as_str(), row["kind"].as_str()) {
                personal.insert(date.to_owned(), kind.to_owned());
            }
        }
    }
    let is_rest = |date: NaiveDate| personal.get(&date.to_string())
        .map(|kind| kind == "holiday").unwrap_or_else(|| !official.is_workday(date));
    let today_rest = is_rest(today);
    let tomorrow_rest = is_rest(today + Duration::days(1));
    let mut desired = HashSet::new();
    if today_rest || tomorrow_rest {
        // On the eve of a break include its entire continuous span, so work
        // later in a long holiday is visible before the user leaves.
        let mut through = today;
        if !today_rest {
            for offset in 1..=366 {
                let date = today + Duration::days(offset);
                if !is_rest(date) { break; }
                through = date;
            }
        }
        let start = today.to_string(); let end = through.to_string();
        // Identity is shared by a task and its calendar time block: one notice.
        let mut work: BTreeMap<String, (String, String)> = BTreeMap::new();
        let mut stmt = conn.prepare("SELECT t.id,t.task_name,CASE WHEN p.task_id IS NOT NULL THEN p.start_date ELSE COALESCE(NULLIF(t.start_date,''),NULLIF(t.due_date,''),NULLIF(t.deadline,'')) END,CASE WHEN p.task_id IS NOT NULL THEN p.end_date ELSE COALESCE(NULLIF(t.due_date,''),NULLIF(t.deadline,''),NULLIF(t.start_date,'')) END FROM tasks t LEFT JOIN task_plans p ON p.task_id=t.id WHERE t.completed=0 AND t.deleted_at IS NULL UNION ALL SELECT id,task_name,COALESCE(NULLIF(due_date,''),NULLIF(deadline,'')),COALESCE(NULLIF(due_date,''),NULLIF(deadline,'')) FROM tasks WHERE completed=0 AND deleted_at IS NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?)))?;
        for row in rows {
            let (id,title,from,to) = row?;
            if let (Some(from),Some(to)) = (from,to) {
                if from <= end && to >= start {
                    work.insert(format!("task:{id}"),(title,from.max(start.clone())));
                }
            }
        }
        let mut stmt = conn.prepare("SELECT 'event:'||e.id,e.title,e.event_date,e.task_id FROM calendar_events e LEFT JOIN tasks t ON t.id=e.task_id WHERE e.event_date BETWEEN ?1 AND ?2 AND (e.task_id IS NULL OR (t.id IS NOT NULL AND t.completed=0 AND t.deleted_at IS NULL))
            UNION ALL SELECT 'deadline:'||id,deadline_name,due_date,NULL FROM case_deadlines WHERE completed=0 AND due_date BETWEEN ?1 AND ?2
            UNION ALL SELECT 'hearing:'||id,COALESCE(hearing_name,hearing_record),substr(hearing_date,1,10),NULL FROM hearings WHERE lifecycle_status='scheduled' AND COALESCE(actual_status,'未开')!='已开' AND substr(hearing_date,1,10) BETWEEN ?1 AND ?2")?;
        let rows = stmt.query_map(params![start,end],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?)))?;
        for row in rows {
            let (id,title,date,task_id) = row?;
            let key = task_id.map(|id|format!("task:{id}")).unwrap_or(id);
            work.entry(key).or_insert((title,date));
        }
        for (entity,(title,date)) in work {
            let id = format!("rest-work:{today}:{entity}");
            let heading = if today_rest { "休息日仍有工作" } else { "休息前请安排工作" };
            let body = if today_rest { format!("今天是休息日，仍有未完成或已排期事项：{title}。请提前处理或调整安排。") }
                else { format!("明天开始休息，今天至 {end} 仍有事项：{title}（{date}）。请在休息前确认安排。") };
            let payload = serde_json::json!({"calendarDate":date,"entity":entity,"restReminderDay":start}).to_string();
            conn.execute("INSERT INTO notifications(id,type,title,body,payload_json) VALUES(?1,'rest_day_work',?2,?3,?4) ON CONFLICT(id) DO UPDATE SET title=excluded.title,body=excluded.body,payload_json=excluded.payload_json",params![id,heading,body,payload])?;
            // Preserve read/dismiss state: refreshes must not resurrect handled notices.
            desired.insert(id);
        }
    }
    let ids = conn.prepare("SELECT id FROM notifications WHERE type='rest_day_work' AND dismissed_at IS NULL")?
        .query_map([],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
    for id in ids {
        if !desired.contains(&id) {
            conn.execute("UPDATE notifications SET dismissed_at=datetime('now','localtime') WHERE id=?1",[id])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn covers_entire_official_break_and_real_work_only() {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(db::schema::SCHEMA_SQL).unwrap(); db::schema::run_migrations(&conn,1).unwrap();
        conn.execute_batch("INSERT INTO cases(id,case_name,track,client_name) VALUES('c','提醒验证','other','test');
            INSERT INTO tasks(id,task_name,created_date,due_date,completed) VALUES('t','长假最后一天工作','2026-09-01','2026-09-27',0),('done','已完成','2026-09-01','2026-09-25',1),('deleted','已删除','2026-09-01','2026-09-25',0); UPDATE tasks SET deleted_at='2026-09-01' WHERE id='deleted';
            INSERT INTO calendar_events(id,title,event_date,task_id,created_at,updated_at) VALUES('linked','任务时间块','2026-09-27','t','now','now'),('done','已完成任务的时间块','2026-09-25','done','now','now'),('meeting','会议','2026-09-26',NULL,'now','now'),('deleted','已删除任务时间块','2026-09-25','deleted','now','now');
            INSERT INTO case_deadlines(id,case_id,deadline_name,due_date) VALUES('d','c','举证截止','2026-09-25');
            INSERT INTO hearings(id,case_id,hearing_record,hearing_date,lifecycle_status) VALUES('h','c','待开庭','2026-09-26 09:00:00','scheduled'),('old','c','已开庭','2026-09-26','held'),('cancel','c','已取消','2026-09-26','cancelled');").unwrap();
        refresh(&conn,NaiveDate::from_ymd_opt(2026,9,24).unwrap()).unwrap();
        let count:i64=conn.query_row("SELECT COUNT(*) FROM notifications WHERE dismissed_at IS NULL",[],|r|r.get(0)).unwrap();
        assert_eq!(count,4,"task and time block must not duplicate; completed/held/cancelled work is excluded");
        let body:String=conn.query_row("SELECT body FROM notifications WHERE id='rest-work:2026-09-24:task:t'",[],|r|r.get(0)).unwrap();
        assert!(body.contains("2026-09-27"));
        refresh(&conn,NaiveDate::from_ymd_opt(2026,9,20).unwrap()).unwrap();
        let count:i64=conn.query_row("SELECT COUNT(*) FROM notifications WHERE dismissed_at IS NULL",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0,"official make-up working Sunday must not be treated as rest");
    }
    #[test]
    fn warns_before_and_during_rest_deduplicates_and_clears_resolved_work() {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(db::schema::SCHEMA_SQL).unwrap(); db::schema::run_migrations(&conn,1).unwrap();
        conn.execute_batch("INSERT INTO tasks(id,task_name,created_date,due_date,completed) VALUES('a','休息前交稿','2027-01-01','2027-03-10',0),('b','休息日工作','2027-01-01','2027-03-11',0),('c','已完成','2027-01-01','2027-03-11',1);").unwrap();
        db::set_setting(&conn,"personal_calendar_days",r#"[{"date":"2027-03-11","kind":"holiday","name":"休息"}]"#).unwrap();
        let before=NaiveDate::from_ymd_opt(2027,3,10).unwrap();
        let count=||conn.query_row("SELECT COUNT(*) FROM notifications WHERE type='rest_day_work' AND dismissed_at IS NULL",[],|r|r.get::<_,i64>(0)).unwrap();
        refresh(&conn,before).unwrap(); assert_eq!(count(),2);
        refresh(&conn,before).unwrap(); assert_eq!(count(),2);
        conn.execute("UPDATE tasks SET completed=1 WHERE id='a'",[]).unwrap();
        refresh(&conn,before).unwrap(); assert_eq!(count(),1);
        conn.execute("UPDATE notifications SET dismissed_at='handled'",[]).unwrap();
        refresh(&conn,before).unwrap(); assert_eq!(count(),0);
        refresh(&conn,before+Duration::days(1)).unwrap(); assert_eq!(count(),1);
        db::set_setting(&conn,"personal_calendar_days",r#"[{"date":"2027-03-11","kind":"workday","name":"上班"}]"#).unwrap();
        refresh(&conn,before+Duration::days(1)).unwrap(); assert_eq!(count(),0);
        db::set_setting(&conn,"personal_calendar_days","[]").unwrap();
        conn.execute("UPDATE tasks SET due_date='2027-03-13' WHERE id='b'",[]).unwrap();
        refresh(&conn,NaiveDate::from_ymd_opt(2027,3,12).unwrap()).unwrap(); assert_eq!(count(),1);
        conn.execute("UPDATE tasks SET due_date='2027-03-15' WHERE id='b'",[]).unwrap();
        refresh(&conn,NaiveDate::from_ymd_opt(2027,3,12).unwrap()).unwrap(); assert_eq!(count(),0);
    }
    #[test]
    fn independent_plans_warn_on_rest_without_moving_task_deadline() {
        let conn=Connection::open_in_memory().unwrap(); db::init_db(&conn).unwrap();
        conn.execute_batch("INSERT INTO tasks(id,task_name,created_date,due_date,completed) VALUES('p','计划工作','2027-01-01','2027-04-01',0); INSERT INTO task_plans(task_id,start_date,end_date,revision) VALUES('p','2027-03-11','2027-03-12',1);").unwrap();
        db::set_setting(&conn,"personal_calendar_days",r#"[{"date":"2027-03-11","kind":"holiday","name":"休息"}]"#).unwrap();
        refresh(&conn,NaiveDate::from_ymd_opt(2027,3,10).unwrap()).unwrap();
        let count=||conn.query_row("SELECT count(*) FROM notifications WHERE type='rest_day_work' AND dismissed_at IS NULL",[],|r|r.get::<_,i64>(0)).unwrap();
        assert_eq!(count(),1);
        conn.execute("UPDATE task_plans SET start_date=NULL,end_date=NULL,revision=2",[]).unwrap();
        refresh(&conn,NaiveDate::from_ymd_opt(2027,3,10).unwrap()).unwrap(); assert_eq!(count(),0);
        conn.execute("UPDATE tasks SET due_date='2027-03-11' WHERE id='p'",[]).unwrap();
        refresh(&conn,NaiveDate::from_ymd_opt(2027,3,11).unwrap()).unwrap(); assert_eq!(count(),1);
    }

}

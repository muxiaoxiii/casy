use casy_lib::{commands::{calendar, dashboard}, db};
use chrono::{Datelike, Duration, Months};
use rusqlite::params;

#[tokio::test]
async fn dashboard_matches_live_records_and_preserves_hearing_boundaries() {
    db::enable_test_mode();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    let today = chrono::Local::now().date_naive();
    let yesterday = (today - Duration::days(1)).to_string();
    let previous_month = today.with_day(1).unwrap().checked_sub_months(Months::new(1)).unwrap();
    conn.execute("INSERT INTO cases(id,case_name,client_name,track) VALUES('dashboard-case','合成看板案件','客户甲','civil_tort')", []).unwrap();
    conn.execute("UPDATE case_legal_details SET track='admin_litigation' WHERE project_id='dashboard-case'", []).unwrap();
    let tracks = dashboard::get_track_distribution().await.unwrap();
    assert_eq!((tracks[0].label.as_str(), tracks[0].value), ("civil_tort", 1));

    for (id, completed) in [("due",0),("deadline",0),("waiting",0),("waiting-for",0),("blank",0),("done",1),("reopened",0),("deleted",1),("older",1)] {
        conn.execute("INSERT INTO tasks(id,task_name,created_date,completed) VALUES(?1,?1,?2,?3)", params![id, if id == "older" { previous_month.to_string() } else { today.to_string() }, completed]).unwrap();
    }
    conn.execute("UPDATE tasks SET due_date=?1 WHERE id='due'", [today.to_string()]).unwrap();
    conn.execute("UPDATE tasks SET deadline=?1 WHERE id='deadline'", [today.to_string()]).unwrap();
    conn.execute("UPDATE tasks SET task_type='waiting',follow_up_date=?1,next_review_date=?2 WHERE id='waiting'", params![yesterday,today.to_string()]).unwrap();
    conn.execute("UPDATE tasks SET waiting_for='客户回执',follow_up_date=?1 WHERE id='waiting-for'", [&yesterday]).unwrap();
    conn.execute("UPDATE tasks SET task_type='waiting',follow_up_date='',next_review_date='' WHERE id='blank'", []).unwrap();
    conn.execute("UPDATE tasks SET deleted_at=?1,due_date=?1,next_review_date=?1 WHERE id='deleted'", [today.to_string()]).unwrap();
    for (event, task, date) in [("done-1","done",today),("done-2","done",today),("reopened-1","reopened",today),("deleted-1","deleted",today),("older-1","older",previous_month)] {
        conn.execute("INSERT INTO task_events(id,task_id,event_type,occurred_at,actor) VALUES(?1,?2,'completed',?3,'user')", params![event,task,date.to_string()]).unwrap();
    }
    let trend = dashboard::get_monthly_task_trend(Some(2)).await.unwrap();
    assert_eq!((trend[0].created, trend[0].completed), (1,1));
    assert_eq!((trend[1].created, trend[1].completed), (7,1));
    assert_eq!(dashboard::get_monthly_task_trend(Some(12)).await.unwrap().len(),12);

    for index in 0..51 {
        conn.execute("INSERT INTO hearings(id,case_id,hearing_record,hearing_name,hearing_date) VALUES(?1,'dashboard-case',?1,?1,?2)", params![format!("upcoming-{index}"),(today + Duration::days(1)).to_string()]).unwrap();
    }
    for (id, offset, status) in [("today",0,"未开"),("last-day",30,"未开"),("beyond",31,"未开"),("already-held",1,"已开")] {
        conn.execute("INSERT INTO hearings(id,case_id,hearing_record,hearing_date,actual_status) VALUES(?1,'dashboard-case',?1,?2,?3)", params![id,format!("{} 15:37:00",today + Duration::days(offset)),status]).unwrap();
    }
    let hearings = dashboard::get_upcoming_hearings(Some(30)).await.unwrap();
    assert_eq!(hearings.len(),53);
    assert_eq!(hearings.first().unwrap().days_left,0);
    assert_eq!(hearings.last().unwrap().days_left,30);
    assert_eq!(hearings.last().unwrap().id,"last-day");
    let kpis = dashboard::get_today_kpis().await.unwrap();
    assert_eq!((kpis.today_events,kpis.due_today,kpis.waiting_overdue,kpis.review_due),(1,2,2,1));
    for (id, date) in [("month-end", "2026-12-31 15:37:00"), ("next-month", "2027-01-01T09:15:00"), ("all-day", "2026-12-01")] {
        conn.execute("INSERT INTO hearings(id,case_id,hearing_record,hearing_date) VALUES(?1,'dashboard-case',?1,?2)", params![id,date]).unwrap();
    }
    let events = calendar::get_calendar_events(2026, 12, None).await.unwrap();
    let hearing = events.iter().find(|event| event.id == "month-end").unwrap();
    assert_eq!(hearing.date,"2026-12-31");
    assert_eq!(hearing.start_time.as_deref(),Some("15:37"));
    assert_eq!(hearing.all_day,Some(false));
    assert_eq!(hearing.case_name,"合成看板案件");
    assert!(!events.iter().any(|event| event.id == "next-month"));
    let all_day = events.iter().find(|event| event.id == "all-day").unwrap();
    assert_eq!((all_day.start_time.as_deref(),all_day.all_day),(None,Some(true)));
    let january = calendar::get_calendar_events(2027, 1, None).await.unwrap();
    assert_eq!(january.iter().find(|event| event.id == "next-month").unwrap().start_time.as_deref(),Some("09:15"));
    assert!(calendar::get_calendar_events(2026, 13, None).await.is_err());
    assert_eq!(conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| row.get::<_,i64>(0)).unwrap(),0);
}

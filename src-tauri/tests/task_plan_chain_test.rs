use casy_lib::{
    commands::{calendar, settings, task_plans, tasks},
    db,
};
use serde_json::json;

#[tokio::test]
async fn plans_project_through_task_commands_and_deadlines_remain_one_value() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    drop(conn);
    let today = chrono::Local::now().date_naive();
    let start = today.to_string();
    let end = (today + chrono::Duration::days(2)).to_string();
    let due = (today + chrono::Duration::days(10)).to_string();
    let id=tasks::create_task(json!({"taskName":"chain-test","startDate":"2099-01-01","dueDate":due,"startBucket":"anytime"})).await.unwrap()["id"].as_str().unwrap().to_string();
    task_plans::save_task_plan(task_plans::TaskPlanInput {
        task_id: id.clone(),
        start_date: Some(start.clone()),
        end_date: Some(end.clone()),
        expected_revision: 0,
    })
    .await
    .unwrap();
    let all = tasks::list_tasks(None).await.unwrap();
    let task = all.iter().find(|t| t.id == id).unwrap();
    assert!(task.plan_defined);
    assert_eq!(task.planned_start_date.as_deref(), Some(start.as_str()));
    assert_eq!(task.planned_end_date.as_deref(), Some(end.as_str()));
    assert_eq!(task.due_date, task.deadline);
    assert_eq!(task.start_date.as_deref(), Some("2099-01-01"));
    let today_tasks = tasks::list_tasks(Some(tasks::TaskFilter {
        completed: None,
        case_id: None,
        area_id: None,
        task_type: None,
        start_bucket: Some("today".into()),
    }))
    .await
    .unwrap();
    assert!(
        today_tasks.iter().any(|t| t.id == id),
        "home/native today must use independent plan"
    );
    assert!(
        tasks::update_task(json!({"id":id,"startDate":"2098-01-01"}))
            .await
            .unwrap_err()
            .contains("独立计划")
    );
    assert!(
        tasks::snooze_task(id.clone(), Some("tomorrow".into()), None)
            .await
            .unwrap_err()
            .contains("独立计划")
    );
    tasks::update_task(json!({"id":id,"dueDate":"2027-03-11","dueTime":"14:00"}))
        .await
        .unwrap();
    let conn = db::open_db().unwrap();
    let pair: (String, String) = conn
        .query_row(
            "SELECT due_date,deadline FROM tasks WHERE id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(pair, ("2027-03-11".into(), "2027-03-11".into()));
    drop(conn);
    let events = calendar::get_calendar_events(2027, 3, Some(1))
        .await
        .unwrap();
    let event = events.iter().find(|e| e.id == id).unwrap();
    assert_eq!(event.date, "2027-03-11");
    assert_eq!(event.start_time.as_deref(), Some("14:00"));
    tasks::update_task(json!({"id":id,"dueDate":null}))
        .await
        .unwrap();
    let all = tasks::list_tasks(None).await.unwrap();
    let task = all.iter().find(|t| t.id == id).unwrap();
    assert!(task.due_date.is_none() && task.deadline.is_none());
    assert_eq!(task.plan_revision, Some(1));
    assert!(!calendar::get_calendar_events(2027, 3, Some(1))
        .await
        .unwrap()
        .iter()
        .any(|e| e.id == id));
    tasks::update_task(json!({"id":id,"deadline":"2027-03-12"}))
        .await
        .unwrap();
    let all = tasks::list_tasks(None).await.unwrap();
    let task = all.iter().find(|t| t.id == id).unwrap();
    assert_eq!(task.due_date.as_deref(), Some("2027-03-12"));
    let snapshot = serde_json::to_value(task).unwrap();
    tasks::delete_task(id.clone(), None, None).await.unwrap();
    assert!(!task_plans::list_task_plans()
        .await
        .unwrap()
        .iter()
        .any(|p| p.task_id == id));
    let restored = tasks::restore_task(snapshot).await.unwrap();
    assert_eq!(restored.planned_start_date.as_deref(), Some(start.as_str()));
    assert_eq!(restored.plan_revision, Some(1));
    task_plans::save_task_plan(task_plans::TaskPlanInput {
        task_id: id.clone(),
        start_date: None,
        end_date: None,
        expected_revision: 1,
    })
    .await
    .unwrap();
    let all = tasks::list_tasks(None).await.unwrap();
    let task = all.iter().find(|t| t.id == id).unwrap();
    assert!(task.plan_defined);
    assert!(task.planned_start_date.is_none());
    assert_eq!(task.plan_revision, Some(2));
    let mut values = std::collections::HashMap::new();
    values.insert("personal_calendar_days".into(),json!([{"date":"2027-03-11","kind":"holiday","name":"请假","startTime":"13:00","endTime":"18:00"}]));
    settings::save_settings(values).await.unwrap();
    let holidays = settings::get_holiday_calendar(2027).await.unwrap();
    let entry = holidays["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["source"] == "personal" && e["date"] == "2027-03-11")
        .unwrap();
    assert_eq!(entry["startTime"], "13:00");
    assert_eq!(entry["endTime"], "18:00");
    // Recurrence advances the independent interval; editing its successor survives undo.
    let repeat = tasks::create_task(
        json!({"taskName":"recurring-plan","startDate":"2099-01-01","recurrenceRule":"daily"}),
    )
    .await
    .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    task_plans::save_task_plan(task_plans::TaskPlanInput {
        task_id: repeat.clone(),
        start_date: Some("2027-03-11".into()),
        end_date: Some("2027-03-12".into()),
        expected_revision: 0,
    })
    .await
    .unwrap();
    tasks::toggle_task(repeat.clone(), None, None, None)
        .await
        .unwrap();
    let conn = db::open_db().unwrap();
    let successor: String = conn
        .query_row(
            "SELECT successor_task_id FROM task_recurrence_instances WHERE source_task_id=?1",
            [&repeat],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);
    let all = tasks::list_tasks(None).await.unwrap();
    let next = all.iter().find(|t| t.id == successor).unwrap();
    assert_eq!(next.planned_start_date.as_deref(), Some("2027-03-12"));
    assert_eq!(next.planned_end_date.as_deref(), Some("2027-03-13"));
    assert!(next.due_date.is_none());
    task_plans::save_task_plan(task_plans::TaskPlanInput {
        task_id: successor.clone(),
        start_date: Some("2027-03-15".into()),
        end_date: Some("2027-03-16".into()),
        expected_revision: 1,
    })
    .await
    .unwrap();
    tasks::toggle_task(repeat, None, None, None).await.unwrap();
    assert!(tasks::list_tasks(None)
        .await
        .unwrap()
        .iter()
        .any(|t| t.id == successor));
}

use casy_lib::{commands::tasks, db};
use rusqlite::params;
use serde_json::json;

#[tokio::test]
async fn recurring_tasks_complete_undo_and_reschedule_without_losing_relationships() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute("INSERT INTO cases(id,case_name,client_name) VALUES('c1','合成案件一','客户'),('c2','合成案件二','客户')",[]).unwrap();
    let source = tasks::create_task(json!({"taskName":"每月核对案件期限","caseId":"c1","startDate":"2026-11-25","dueDate":"2026-11-30",
        "dueTime":"09:30","timeBlock":"morning","recurrenceRule":"monthly:31","followUpDate":"2026-11-29",
        "nextReviewDate":"2026-11-28","deferUntil":"2026-11-26","startBucket":"today","estimatedMinutes":0})).await.unwrap()["id"].as_str().unwrap().to_string();
    conn.execute(
        "INSERT INTO case_task_links(case_id,task_id) VALUES('c2',?1)",
        [&source],
    )
    .unwrap();
    let next_task = tasks::create_task(json!({"taskName":"后续步骤","caseId":"c1"}))
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        conn.query_row("SELECT blocked FROM tasks WHERE id=?1", [&next_task], |r| r
            .get::<_, i32>(0))
            .unwrap(),
        1
    );
    tasks::update_task(json!({"id":source,"completed":1}))
        .await
        .unwrap();
    let successor: String = conn
        .query_row(
            "SELECT successor_task_id FROM task_recurrence_instances WHERE source_task_id=?1",
            [&source],
            |r| r.get(0),
        )
        .unwrap();
    let first = tasks::list_tasks(None)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.id == successor)
        .unwrap();
    assert_eq!(first.due_date.as_deref(), Some("2026-12-31"));
    assert_eq!(first.start_date.as_deref(), Some("2026-12-26"));
    assert_eq!(first.due_time.as_deref(), Some("09:30"));
    assert_eq!(first.time_block.as_deref(), Some("morning"));
    assert_eq!(first.start_bucket, "anytime");
    assert_eq!(first.follow_up_date.as_deref(), Some("2026-12-30"));
    assert_eq!(first.next_review_date.as_deref(), Some("2026-12-29"));
    assert_eq!(first.defer_until.as_deref(), Some("2026-12-27"));
    assert_eq!(first.estimated_minutes, Some(0));
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM case_task_links WHERE case_id='c2' AND task_id=?1",
            [&successor],
            |r| r.get::<_, i32>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT blocked FROM tasks WHERE id=?1", [&next_task], |r| r
            .get::<_, i32>(0))
            .unwrap(),
        0
    );
    tasks::update_task(json!({"id":source,"completed":1}))
        .await
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM tasks WHERE recurrence_rule='monthly:31' AND deleted_at IS NULL",
            [],
            |r| r.get::<_, i32>(0)
        )
        .unwrap(),
        2
    );
    tasks::update_task(json!({"id":source,"completed":0}))
        .await
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM tasks WHERE id=?1 AND deleted_at IS NULL",
            [&successor],
            |r| r.get::<_, i32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT blocked FROM tasks WHERE id=?1", [&next_task], |r| r
            .get::<_, i32>(0))
            .unwrap(),
        1
    );
    tasks::toggle_task(source.clone(), Some(20), None, None)
        .await
        .unwrap();
    let successor: String = conn
        .query_row(
            "SELECT successor_task_id FROM task_recurrence_instances WHERE source_task_id=?1",
            [&source],
            |r| r.get(0),
        )
        .unwrap();
    tasks::update_task(json!({"id":successor,"description":"已补充工作成果"}))
        .await
        .unwrap();
    tasks::toggle_task(source.clone(), None, None, None)
        .await
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT description FROM tasks WHERE id=?1 AND deleted_at IS NULL",
            [&successor],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "已补充工作成果"
    );
    tasks::toggle_task(source.clone(), None, None, None)
        .await
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM tasks WHERE recurrence_rule='monthly:31' AND deleted_at IS NULL",
            [],
            |r| r.get::<_, i32>(0)
        )
        .unwrap(),
        2
    );

    let plain = tasks::create_task(json!({"taskName":"保留法定截止日","caseId":"","areaId":"","startDate":"","dueDate":"2026-12-15","dueTime":"16:20","recurrenceRule":"daily"})).await.unwrap()["id"].as_str().unwrap().to_owned();
    conn.execute_batch("CREATE TRIGGER reject_next_instance BEFORE INSERT ON tasks WHEN NEW.recurrence_rule='daily' BEGIN SELECT RAISE(ABORT,'forced successor failure'); END").unwrap();
    assert!(tasks::update_task(json!({"id":plain,"completed":1}))
        .await
        .is_err());
    assert_eq!(
        conn.query_row("SELECT completed FROM tasks WHERE id=?1", [&plain], |r| r
            .get::<_, i32>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM task_events WHERE task_id=?1 AND event_type='completed'",
            [&plain],
            |r| r.get::<_, i32>(0)
        )
        .unwrap(),
        0
    );
    conn.execute_batch("DROP TRIGGER reject_next_instance")
        .unwrap();
    tasks::snooze_task(
        plain.clone(),
        Some("custom".into()),
        Some("2026-12-10".into()),
    )
    .await
    .unwrap();
    let dates: (String, String, String, String) = conn
        .query_row(
            "SELECT start_date,due_date,deadline,due_time FROM tasks WHERE id=?1",
            [&plain],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        dates,
        (
            "2026-12-10".into(),
            "2026-12-15".into(),
            "2026-12-15".into(),
            "16:20".into()
        )
    );
    use chrono::Datelike;
    tasks::snooze_task(plain.clone(), Some("weekend".into()), None)
        .await
        .unwrap();
    let weekend: String = conn
        .query_row("SELECT start_date FROM tasks WHERE id=?1", [&plain], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        chrono::NaiveDate::parse_from_str(&weekend, "%Y-%m-%d")
            .unwrap()
            .weekday(),
        chrono::Weekday::Sat
    );
    conn.execute_batch("CREATE TRIGGER reject_snooze BEFORE INSERT ON task_events WHEN NEW.event_type='snoozed' BEGIN SELECT RAISE(ABORT,'forced event failure'); END").unwrap();
    assert!(tasks::snooze_task(
        plain.clone(),
        Some("custom".into()),
        Some("2027-02-01".into())
    )
    .await
    .is_err());
    assert_eq!(
        conn.query_row("SELECT start_date FROM tasks WHERE id=?1", [&plain], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        weekend
    );
    conn.execute_batch("DROP TRIGGER reject_snooze").unwrap();
    let saved = tasks::list_tasks(None)
        .await
        .unwrap()
        .into_iter()
        .find(|task| task.id == source)
        .unwrap();
    tasks::delete_task(source.clone(), None, None)
        .await
        .unwrap();
    let restored = tasks::restore_task(serde_json::to_value(saved).unwrap())
        .await
        .unwrap();
    assert_eq!(restored.time_block.as_deref(), Some("morning"));
    assert_eq!(restored.due_time.as_deref(), Some("09:30"));
    let parent = tasks::create_task(json!({"taskName":"独立子步骤组","caseId":"c1"}))
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let child =
        tasks::create_task(json!({"taskName":"第一个子步骤","caseId":"c1","parentTaskId":parent}))
            .await
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
    let child_task = tasks::list_tasks(None)
        .await
        .unwrap()
        .into_iter()
        .find(|task| task.id == child)
        .unwrap();
    assert_eq!(child_task.parent_task_id.as_deref(), Some(parent.as_str()));
    assert_eq!(child_task.blocked, 0);
    let sibling =
        tasks::create_task(json!({"taskName":"第二个子步骤","caseId":"c1","parentId":parent}))
            .await
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
    assert_eq!(
        conn.query_row("SELECT blocked FROM tasks WHERE id=?1", [&sibling], |r| r
            .get::<_, i32>(
            0
        ))
        .unwrap(),
        1
    );
    tasks::delete_task(child.clone(), None, None).await.unwrap();
    assert_eq!(
        conn.query_row("SELECT blocked FROM tasks WHERE id=?1", [&sibling], |r| r
            .get::<_, i32>(
            0
        ))
        .unwrap(),
        0
    );
    tasks::restore_task(serde_json::to_value(child_task).unwrap())
        .await
        .unwrap();
    assert_eq!(
        conn.query_row("SELECT blocked FROM tasks WHERE id=?1", [&sibling], |r| r
            .get::<_, i32>(
            0
        ))
        .unwrap(),
        1
    );
    for invalid in [
        json!({"taskName":""}),
        json!({"taskName":"错误日期","dueDate":"2026-02-30"}),
        json!({"taskName":"错误规则","recurrenceRule":"monthly:99"}),
    ] {
        assert!(tasks::create_task(invalid).await.is_err());
    }
    let planned = tasks::create_task(
        json!({"taskName":"每日整理","startDate":"2026-09-01","recurrenceRule":"daily"}),
    )
    .await
    .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    tasks::toggle_task(planned.clone(), None, None, None)
        .await
        .unwrap();
    let schedule: (String,Option<String>,Option<String>) = conn.query_row("SELECT t.start_date,t.due_date,t.deadline FROM tasks t JOIN task_recurrence_instances r ON r.successor_task_id=t.id WHERE r.source_task_id=?1",[planned],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(schedule, ("2026-09-02".into(), None, None));
    for (anchor, rule, expected) in [
        ("2028-01-31", "monthly:31", "2028-02-29"),
        ("2026-12-31", "monthly:31", "2027-01-31"),
        ("2026-09-04", "weekdays", "2026-09-07"),
    ] {
        let id = tasks::create_task(
            json!({"taskName":"边界日期","dueDate":anchor,"recurrenceRule":rule}),
        )
        .await
        .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        tasks::toggle_task(id.clone(), None, None, None)
            .await
            .unwrap();
        let date: String = conn.query_row("SELECT t.due_date FROM tasks t JOIN task_recurrence_instances r ON r.successor_task_id=t.id WHERE r.source_task_id=?1",[id],|r|r.get(0)).unwrap();
        assert_eq!(date, expected);
    }
    assert_eq!(
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i32>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(
        conn.query_row(
            "SELECT count(*) FROM task_events WHERE task_id=?1 AND event_type='restored'",
            params![source],
            |r| r.get::<_, i32>(0)
        )
        .unwrap()
            > 0
    );
}

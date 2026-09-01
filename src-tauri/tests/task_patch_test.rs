use casy_lib::types::PatchField;
use rusqlite::Connection;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE tasks (
            id TEXT PRIMARY KEY,
            case_id TEXT,
            task_name TEXT NOT NULL,
            description TEXT,
            created_date TEXT,
            deadline TEXT,
            priority TEXT DEFAULT 'normal',
            completed INTEGER DEFAULT 0,
            assignee TEXT,
            finish_note TEXT,
            task_type TEXT DEFAULT 'action',
            start_date TEXT,
            due_date TEXT,
            due_time TEXT,
            waiting_for TEXT,
            follow_up_date TEXT,
            context TEXT,
            flagged INTEGER DEFAULT 0,
            sequential INTEGER DEFAULT 0,
            blocked INTEGER DEFAULT 0,
            blocked_reason TEXT,
            sequence_order INTEGER DEFAULT 0,
            start_bucket TEXT DEFAULT 'anytime' CHECK(start_bucket IN ('inbox','anytime','someday','today')),
            today_index INTEGER DEFAULT 0,
            estimated_minutes INTEGER,
            actual_minutes INTEGER,
            area_id TEXT,
            next_review_date TEXT,
            created_at TEXT DEFAULT (datetime('now','localtime')),
            updated_at TEXT DEFAULT (datetime('now','localtime')),
            parent_task_id TEXT,
            recurrence_rule TEXT,
            is_focus INTEGER DEFAULT 0
        );
        CREATE TABLE task_events (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            occurred_at TEXT NOT NULL,
            payload TEXT,
            actor TEXT NOT NULL DEFAULT 'user'
        );
        CREATE TABLE ai_proposals (
            id TEXT PRIMARY KEY,
            tool_name TEXT NOT NULL,
            target_entity_type TEXT NOT NULL,
            target_entity_id TEXT,
            pre_state_hash TEXT,
            payload_json TEXT NOT NULL,
            auth_token TEXT NOT NULL UNIQUE,
            expires_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL,
            executed_at TEXT
        );",
    )
    .unwrap();

    // 插入初始测试任务
    conn.execute(
        "INSERT INTO tasks (
            id, task_name, description, due_date, priority, completed,
            start_bucket, estimated_minutes, flagged, is_focus
        ) VALUES (
            'task-100', '起草上诉状', '详细阐述一审判决证据认定缺陷', '2026-09-01',
            'urgent_important', 0, 'today', 120, 1, 1
        )",
        [],
    )
    .unwrap();

    conn
}

#[test]
fn test_partial_patch_preserves_other_fields() {
    let mut conn = setup_test_db();
    let now = "2026-08-29 18:00:00";

    // 模拟仅更新 completed 字段
    let json_patch = serde_json::json!({
        "id": "task-100",
        "completed": 1
    });

    let patch: casy_lib::commands::tasks::UpdateTaskPatch =
        serde_json::from_value(json_patch).unwrap();
    patch.validate().unwrap();

    let tx = conn.transaction().unwrap();

    // 验证旧值存在
    let old_info: (Option<String>, i32, String) = tx
        .query_row(
            "SELECT due_date, completed, start_bucket FROM tasks WHERE id = ?1",
            rusqlite::params![patch.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(old_info.1, 0);

    // 动态更新语句构建
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let PatchField::Value(c) = patch.completed {
        sets.push("completed = ?".to_string());
        params.push(Box::new(c));
    }
    sets.push("updated_at = ?".to_string());
    params.push(Box::new(now));

    let sql = format!("UPDATE tasks SET {} WHERE id = ?", sets.join(", "));
    params.push(Box::new(patch.id.clone()));

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let rows_affected = tx
        .execute(&sql, rusqlite::params_from_iter(params_refs))
        .unwrap();
    assert_eq!(rows_affected, 1);

    tx.commit().unwrap();

    // 查询全量字段，断言除 completed 外所有字段完整保留（绝未被置为 NULL）
    let (name, desc, due, priority, completed, bucket, est, flagged, focus): (
        String,
        String,
        String,
        String,
        i32,
        String,
        i32,
        i32,
        i32,
    ) = conn
        .query_row(
            "SELECT task_name, description, due_date, priority, completed, start_bucket, estimated_minutes, flagged, is_focus FROM tasks WHERE id = 'task-100'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)),
        )
        .unwrap();

    assert_eq!(completed, 1);
    assert_eq!(name, "起草上诉状");
    assert_eq!(desc, "详细阐述一审判决证据认定缺陷");
    assert_eq!(due, "2026-09-01");
    assert_eq!(priority, "urgent_important");
    assert_eq!(bucket, "today");
    assert_eq!(est, 120);
    assert_eq!(flagged, 1);
    assert_eq!(focus, 1);
}

#[test]
fn test_explicit_null_clears_field() {
    let mut conn = setup_test_db();
    let now = "2026-08-29 18:00:00";

    // 显式将 due_date 设为 null
    let json_patch = serde_json::json!({
        "id": "task-100",
        "dueDate": null
    });

    let patch: casy_lib::commands::tasks::UpdateTaskPatch =
        serde_json::from_value(json_patch).unwrap();
    patch.validate().unwrap();

    assert!(patch.due_date.is_null());
    assert!(patch.task_name.is_unset());

    let tx = conn.transaction().unwrap();
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    match patch.due_date {
        PatchField::Unset => {}
        PatchField::Null => sets.push("due_date = NULL".to_string()),
        PatchField::Value(v) => {
            sets.push("due_date = ?".to_string());
            params.push(Box::new(v));
        }
    }
    sets.push("updated_at = ?".to_string());
    params.push(Box::new(now));

    let sql = format!("UPDATE tasks SET {} WHERE id = ?", sets.join(", "));
    params.push(Box::new(patch.id.clone()));

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    tx.execute(&sql, rusqlite::params_from_iter(params_refs))
        .unwrap();
    tx.commit().unwrap();

    // 验证 due_date 变为了 NULL，而 task_name 仍完好
    let (name, due): (String, Option<String>) = conn
        .query_row(
            "SELECT task_name, due_date FROM tasks WHERE id = 'task-100'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert_eq!(name, "起草上诉状");
    assert!(due.is_none());
}

#[test]
fn test_validation_rejects_invalid_inputs() {
    // 1. 无效 start_bucket
    let invalid_bucket = serde_json::json!({
        "id": "task-100",
        "startBucket": "invalid_bucket_name"
    });
    let patch: casy_lib::commands::tasks::UpdateTaskPatch =
        serde_json::from_value(invalid_bucket).unwrap();
    assert!(patch.validate().is_err());

    // 2. 无效日期格式
    let invalid_date = serde_json::json!({
        "id": "task-100",
        "dueDate": "2026/09/01"
    });
    let patch: casy_lib::commands::tasks::UpdateTaskPatch =
        serde_json::from_value(invalid_date).unwrap();
    assert!(patch.validate().is_err());

    // 3. 负数耗时
    let invalid_mins = serde_json::json!({
        "id": "task-100",
        "estimatedMinutes": -10
    });
    let patch: casy_lib::commands::tasks::UpdateTaskPatch =
        serde_json::from_value(invalid_mins).unwrap();
    assert!(patch.validate().is_err());
}

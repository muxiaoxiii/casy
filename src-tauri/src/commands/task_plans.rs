//! Independent task planning intervals. Never rewrite task or statutory deadlines.
use super::run_blocking;
use crate::db;
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlan {
    pub task_id: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub revision: i32,
}

#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPlanInput {
    pub task_id: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub expected_revision: i32,
}

fn read_plan(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskPlan> {
    Ok(TaskPlan {
        task_id: row.get(0)?,
        start_date: row.get(1)?,
        end_date: row.get(2)?,
        revision: row.get(3)?,
    })
}

fn list_inner(conn: &Connection) -> Result<Vec<TaskPlan>> {
    Ok(conn.prepare("SELECT p.task_id,p.start_date,p.end_date,p.revision FROM task_plans p JOIN tasks t ON t.id=p.task_id WHERE t.deleted_at IS NULL ORDER BY p.task_id")?
        .query_map([], read_plan)?.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn save_inner(conn: &mut Connection, data: TaskPlanInput) -> Result<TaskPlan> {
    ensure!(data.expected_revision >= 0, "计划版本无效");
    match (&data.start_date, &data.end_date) {
        (None, None) => {}
        (Some(start), Some(end)) => {
            let parse = |value: &str| -> Result<chrono::NaiveDate> {
                let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
                ensure!(
                    value.len() == 10
                        && date.to_string() == value
                        && ("1900-01-01"..="9999-12-31").contains(&value),
                    "计划日期无效"
                );
                Ok(date)
            };
            let days = (parse(end)? - parse(start)?).num_days();
            ensure!(
                (0..=3659).contains(&days),
                "计划结束不能早于开始，且跨度不能超过 3660 天"
            );
        }
        _ => anyhow::bail!("请同时填写计划开始和结束日期"),
    }
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let completed: Option<i32> = tx
        .query_row(
            "SELECT completed FROM tasks WHERE id=?1 AND deleted_at IS NULL",
            [&data.task_id],
            |r| r.get(0),
        )
        .optional()?;
    ensure!(
        completed == Some(0),
        "任务不存在、已删除或已完成，请刷新后重试"
    );
    let previous = tx
        .query_row(
            "SELECT task_id,start_date,end_date,revision FROM task_plans WHERE task_id=?1",
            [&data.task_id],
            read_plan,
        )
        .optional()?;
    ensure!(
        previous.as_ref().map_or(0, |p| p.revision) == data.expected_revision,
        "PLAN_CONFLICT: 计划已被其他操作修改，请刷新后重新排期"
    );
    let revision = data
        .expected_revision
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("计划版本已达到上限"))?;
    let plan = TaskPlan {
        task_id: data.task_id,
        start_date: data.start_date,
        end_date: data.end_date,
        revision,
    };
    // Keep a cleared row's revision to prevent stale writes after clear/re-create (ABA).
    tx.execute("INSERT INTO task_plans(task_id,start_date,end_date,revision) VALUES(?1,?2,?3,?4) ON CONFLICT(task_id) DO UPDATE SET start_date=excluded.start_date,end_date=excluded.end_date,revision=excluded.revision",
        params![plan.task_id, plan.start_date, plan.end_date, plan.revision])?;
    let payload =
        serde_json::json!({ "planning": { "before": previous, "after": plan } }).to_string();
    tx.execute("INSERT INTO task_events(id,task_id,event_type,payload,actor) VALUES(?1,?2,'edited',?3,'user')", params![uuid::Uuid::new_v4().to_string(),plan.task_id,payload])?;
    tx.commit()?;
    Ok(plan)
}

#[tauri::command]
pub async fn list_task_plans() -> Result<Vec<TaskPlan>, String> {
    run_blocking(|| db::with_conn(list_inner)).await
}

#[tauri::command]
pub async fn save_task_plan(data: TaskPlanInput) -> Result<TaskPlan, String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        save_inner(&mut conn, data)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_db(&conn).unwrap();
        conn.execute("INSERT INTO tasks(id,task_name,created_date,completed,start_date,due_date,deadline) VALUES('t','task','2026-01-01',0,'2026-09-01','2026-09-30','2026-09-30')", []).unwrap();
        conn
    }
    fn input(start: Option<&str>, end: Option<&str>, revision: i32) -> TaskPlanInput {
        TaskPlanInput {
            task_id: "t".into(),
            start_date: start.map(Into::into),
            end_date: end.map(Into::into),
            expected_revision: revision,
        }
    }
    #[test]
    fn saves_plans_without_rewriting_deadlines_and_keeps_audit() {
        let mut conn = setup();
        let plan = save_inner(&mut conn, input(Some("2026-12-30"), Some("2027-01-04"), 0)).unwrap();
        assert_eq!(list_inner(&conn).unwrap(), vec![plan]);
        let dates: (String, String, String) = conn
            .query_row(
                "SELECT start_date,due_date,deadline FROM tasks WHERE id='t'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            dates,
            (
                "2026-09-01".into(),
                "2026-09-30".into(),
                "2026-09-30".into()
            )
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM task_events WHERE task_id='t' AND event_type='edited'",
                [],
                |r| r.get::<_, i32>(0)
            )
            .unwrap(),
            1
        );
        // Migration can be rerun on a current DB without losing plans.
        crate::db::init_db(&conn).unwrap();
        assert_eq!(list_inner(&conn).unwrap().len(), 1);
    }
    #[test]
    fn rejects_invalid_dates_ranges_deleted_and_completed_tasks() {
        let mut conn = setup();
        for (s, e) in [
            (Some("2026-02-30"), Some("2026-03-01")),
            (Some("2026-03-02"), Some("2026-03-01")),
            (Some("2026-01-01"), None),
            (Some("2026-01-01"), Some("2050-01-01")),
        ] {
            assert!(save_inner(&mut conn, input(s, e, 0)).is_err());
        }
        conn.execute("UPDATE tasks SET completed=1 WHERE id='t'", [])
            .unwrap();
        assert!(save_inner(&mut conn, input(Some("2026-01-01"), Some("2026-01-01"), 0)).is_err());
        conn.execute(
            "UPDATE tasks SET completed=0,deleted_at='2026-01-01' WHERE id='t'",
            [],
        )
        .unwrap();
        assert!(save_inner(&mut conn, input(None, None, 0)).is_err());
    }
    #[test]
    fn rejects_stale_writes_including_clear_recreate_and_rolls_back_audit_failure() {
        let mut conn = setup();
        save_inner(&mut conn, input(Some("2026-01-01"), Some("2026-01-02"), 0)).unwrap();
        assert!(save_inner(&mut conn, input(None, None, 0))
            .unwrap_err()
            .to_string()
            .contains("PLAN_CONFLICT"));
        save_inner(&mut conn, input(None, None, 1)).unwrap();
        assert_eq!(list_inner(&conn).unwrap()[0].revision, 2);
        assert!(save_inner(&mut conn, input(Some("2026-02-01"), Some("2026-02-02"), 0)).is_err());
        conn.execute_batch("CREATE TRIGGER fail_plan_audit BEFORE INSERT ON task_events BEGIN SELECT RAISE(ABORT,'audit failed'); END;").unwrap();
        assert!(save_inner(&mut conn, input(Some("2026-02-01"), Some("2026-02-02"), 2)).is_err());
        let plan = &list_inner(&conn).unwrap()[0];
        assert_eq!(plan.revision, 2);
        assert!(plan.start_date.is_none());
    }
    #[test]
    fn hides_soft_deleted_plans_and_restores_them_on_undelete() {
        let mut conn = setup();
        save_inner(&mut conn, input(Some("2026-01-01"), Some("2026-01-01"), 0)).unwrap();
        conn.execute("UPDATE tasks SET deleted_at='2026-01-01' WHERE id='t'", [])
            .unwrap();
        assert!(list_inner(&conn).unwrap().is_empty());
        conn.execute("UPDATE tasks SET deleted_at=NULL WHERE id='t'", [])
            .unwrap();
        assert_eq!(list_inner(&conn).unwrap().len(), 1);
    }
    #[test]
    fn upgrades_v41_database_without_changing_existing_task_dates() {
        let conn = setup();
        conn.execute_batch("DROP TABLE task_plans; PRAGMA user_version=41;")
            .unwrap();
        crate::db::schema::run_migrations(&conn, 41).unwrap();
        assert!(list_inner(&conn).unwrap().is_empty());
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            crate::db::schema::CURRENT_SCHEMA_VERSION as i32
        );
        assert_eq!(
            conn.query_row("SELECT due_date FROM tasks WHERE id='t'", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "2026-09-30"
        );
    }
}

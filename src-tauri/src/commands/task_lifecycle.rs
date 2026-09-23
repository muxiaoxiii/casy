use crate::db;
use anyhow::{bail, Result};
use chrono::{Datelike, Duration, Months, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

pub(super) fn next_occurrence(rule: &str, from: &str) -> Option<String> {
    let date = NaiveDate::parse_from_str(from, "%Y-%m-%d").ok()?;
    let next = match rule {
        "daily" => date.checked_add_signed(Duration::days(1))?,
        "weekdays" => {
            let days = match date.weekday().number_from_monday() {
                5 => 3,
                6 => 2,
                _ => 1,
            };
            date.checked_add_signed(Duration::days(days))?
        }
        r if r.starts_with("weekly:") => {
            let target = r.strip_prefix("weekly:")?.parse::<u32>().ok()?;
            if !(1..=7).contains(&target) {
                return None;
            }
            let delta = (target + 7 - date.weekday().number_from_monday()) % 7;
            date.checked_add_signed(Duration::days(if delta == 0 { 7 } else { delta } as i64))?
        }
        r if r.starts_with("monthly:") => {
            let day = r.strip_prefix("monthly:")?.parse::<u32>().ok()?;
            if !(1..=31).contains(&day) {
                return None;
            }
            let first = date.with_day(1)?.checked_add_months(Months::new(1))?;
            let last = first.checked_add_months(Months::new(1))?.pred_opt()?.day();
            first.with_day(day.min(last))?
        }
        _ => return None,
    };
    Some(next.to_string())
}

pub(super) fn validate_recurrence(rule: &str) -> Result<()> {
    if !rule.is_empty() && next_occurrence(rule, "2026-01-01").is_none() {
        bail!("无效重复规则");
    }
    Ok(())
}

fn snapshot_hash(conn: &Connection, id: &str) -> Result<String> {
    let task = crate::ai::gateway::compute_current_entity_hash(conn, "task", id)?;
    let queries = [
        "SELECT case_id FROM case_task_links WHERE task_id=?1 ORDER BY case_id",
        "SELECT id FROM task_events WHERE task_id=?1 ORDER BY id",
        "SELECT id FROM tasks WHERE parent_task_id=?1 ORDER BY id",
        "SELECT id FROM links WHERE (source_type='task' AND source_id=?1) OR (target_type='task' AND target_id=?1) ORDER BY id",
    ];
    let mut references = Vec::new();
    for sql in queries {
        references.push(
            conn.prepare(sql)?
                .query_map([id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        );
    }
    Ok(crate::ai::gateway::compute_entity_hash(
        &serde_json::json!({"task":task,"references":references}),
    ))
}

fn shifted(value: Option<String>, days: i64) -> Option<String> {
    value
        .and_then(|v| NaiveDate::parse_from_str(&v, "%Y-%m-%d").ok())
        .and_then(|d| d.checked_add_signed(Duration::days(days)))
        .map(|d| d.to_string())
}

// Only tasks belonging to the same case and parent form a sequential group.
pub(super) fn refresh_sequence(conn: &Connection, id: &str) -> Result<()> {
    let (case_id, parent): (Option<String>, Option<String>) = conn.query_row(
        "SELECT case_id,parent_task_id FROM tasks WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if case_id.is_none() && parent.is_none() {
        return Ok(());
    }
    let ids = conn.prepare("SELECT id FROM tasks WHERE case_id IS ?1 AND parent_task_id IS ?2 AND sequential=1 AND completed=0 AND deleted_at IS NULL ORDER BY sequence_order,created_at,id")?
        .query_map(params![case_id,parent], |r| r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    for (position, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE tasks SET blocked=?2 WHERE id=?1 AND blocked IS NOT ?2",
            params![id, if position == 0 { 0 } else { 1 }],
        )?;
    }
    Ok(())
}

/// Called by both completion APIs inside their write transaction.
pub(super) fn completion_effects(
    conn: &Connection,
    id: &str,
    completed: i32,
) -> Result<Vec<String>> {
    let now = db::now_local();
    let mut cancelled = Vec::new();
    let existing: Option<(String,String)> = conn.query_row(
        "SELECT successor_task_id,generated_hash FROM task_recurrence_instances WHERE source_task_id=?1",
        [id], |r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let mut generated = None;
    if completed == 1 {
        cancelled.push(id.to_owned());
        let rule: Option<String> = conn.query_row(
            "SELECT NULLIF(recurrence_rule,'') FROM tasks WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        if let Some(rule) = rule.filter(|_| existing.is_none()) {
            validate_recurrence(&rule)?;
            let plan: Option<(Option<String>,Option<String>)> = conn.query_row(
                "SELECT start_date,end_date FROM task_plans WHERE task_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let (anchor,start,follow,review,defer,has_due): (String,Option<String>,Option<String>,Option<String>,Option<String>,bool) = conn.query_row(
                "SELECT COALESCE(NULLIF(due_date,''),NULLIF(deadline,''),CASE WHEN EXISTS(SELECT 1 FROM task_plans WHERE task_id=tasks.id) THEN (SELECT start_date FROM task_plans WHERE task_id=tasks.id) ELSE NULLIF(start_date,'') END,date('now','localtime')),start_date,follow_up_date,next_review_date,defer_until,COALESCE(NULLIF(due_date,''),NULLIF(deadline,'')) IS NOT NULL FROM tasks WHERE id=?1",
                [id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
            let next = next_occurrence(&rule, &anchor)
                .ok_or_else(|| anyhow::anyhow!("无法计算下一次重复日期"))?;
            let shift = (NaiveDate::parse_from_str(&next, "%Y-%m-%d")?
                - NaiveDate::parse_from_str(&anchor, "%Y-%m-%d")?)
            .num_days();
            let successor = db::new_id();
            conn.execute("INSERT INTO tasks(id,case_id,task_name,description,created_date,deadline,priority,assignee,
                task_type,start_date,due_date,due_time,waiting_for,follow_up_date,context,flagged,sequential,blocked,sequence_order,
                start_bucket,estimated_minutes,area_id,parent_task_id,recurrence_rule,created_at,knowledge_id,next_review_date,defer_until,time_block)
                SELECT ?1,case_id,task_name,description,?2,?3,priority,assignee,task_type,?4,?3,due_time,waiting_for,?5,context,flagged,
                sequential,0,COALESCE((SELECT max(t.sequence_order)+1 FROM tasks t WHERE t.case_id IS tasks.case_id AND t.parent_task_id IS tasks.parent_task_id AND t.deleted_at IS NULL),0),
                'anytime',estimated_minutes,area_id,parent_task_id,recurrence_rule,?2,knowledge_id,?6,?7,time_block FROM tasks WHERE id=?8",
                params![successor,now,has_due.then_some(&next),shifted(start,shift).or_else(|| (!has_due).then(||next.clone())),shifted(follow,shift),shifted(review,shift),shifted(defer,shift),id])?;
            if let Some((start,end))=plan {
                let move_date=|value:Option<String>| -> Result<Option<String>> {
                    value.map(|value| {
                        let next=shifted(Some(value),shift).ok_or_else(||anyhow::anyhow!("重复计划日期超出范围"))?;
                        anyhow::ensure!(next.len()==10 && next.as_str() <= "9999-12-31", "重复计划日期超出范围");
                        Ok(next)
                    }).transpose()
                };
                conn.execute("INSERT INTO task_plans(task_id,start_date,end_date,revision) VALUES(?1,?2,?3,1)",params![successor,move_date(start)?,move_date(end)?])?;
            }
            conn.execute("INSERT INTO case_task_links(case_id,task_id) SELECT case_id,?2 FROM case_task_links WHERE task_id=?1",params![id,successor])?;
            conn.execute("INSERT INTO task_events(id,task_id,event_type,occurred_at,payload,actor) VALUES(?1,?2,'created',?3,?4,'system')",
                params![db::new_id(),successor,now,serde_json::json!({"recurrenceSource":id}).to_string()])?;
            generated = Some(successor);
        }
    } else if let Some((successor, expected)) = existing {
        // An edited or referenced next occurrence is user work and must remain intact.
        if snapshot_hash(conn, &successor)? == expected {
            conn.execute(
                "UPDATE tasks SET deleted_at=?2 WHERE id=?1 AND completed=0 AND deleted_at IS NULL",
                params![successor, now],
            )?;
            conn.execute("INSERT INTO task_events(id,task_id,event_type,payload,actor) VALUES(?1,?2,'deleted',?3,'system')",
                params![db::new_id(),successor,serde_json::json!({"reason":"completion_reverted","sourceTaskId":id}).to_string()])?;
            conn.execute(
                "DELETE FROM task_recurrence_instances WHERE source_task_id=?1",
                [id],
            )?;
            cancelled.push(successor);
        }
    }
    refresh_sequence(conn, id)?;
    if let Some(successor) = generated {
        conn.execute("INSERT INTO task_recurrence_instances(source_task_id,successor_task_id,generated_hash) VALUES(?1,?2,?3)",
            params![id,successor,snapshot_hash(conn,&successor)?])?;
    }
    Ok(cancelled)
}

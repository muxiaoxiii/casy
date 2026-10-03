use crate::db;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};

pub(super) fn supports(action: &str) -> bool {
    matches!(action, "create_task" | "create_deadline" | "set_reminder" | "create_event" | "save_knowledge" | "create_case" | "create_project" | "update_holidays")
}

pub(super) fn confirm(
    conn: &mut rusqlite::Connection,
    item_id: &str,
    action: &str,
    case_id: Option<&str>,
    intent: Option<&Value>,
) -> anyhow::Result<Value> {
    // The receipt and business record commit together; a lost response can be retried.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let prior: Option<(String, String)> = tx.query_row(
        "SELECT action,result_json FROM inbox_action_results WHERE inbox_item_id=?1",
        [item_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?;
    if let Some((previous_action, result)) = prior {
        anyhow::ensure!(previous_action == action, "此收件项已处理，请打开已创建的记录继续编辑");
        let result = serde_json::from_str(&result)?;
        tx.execute("UPDATE inbox_items SET status='filed' WHERE id=?1", [item_id])?;
        tx.commit()?;
        return Ok(result);
    }
    let (content, status): (String, String) = tx.query_row(
        "SELECT COALESCE(NULLIF(content_text,''),title,''),status FROM inbox_items WHERE id=?1",
        [item_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).map_err(|_| anyhow::anyhow!("收件箱项不存在"))?;
    anyhow::ensure!(!matches!(status.as_str(), "filed" | "ignored" | "dismissed"), "此收件项已经处理");
    let case_id = case_id.map(str::trim).filter(|value| !value.is_empty());
    if let Some(id) = case_id {
        db::cases::get_case(&tx, id)?;
    }
    let field = |name: &str| intent.and_then(|value| value.get(name)).and_then(Value::as_str)
        .map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned);
    let mut payload = intent.filter(|value| value.is_object()).cloned().unwrap_or_else(|| json!({}));
    // These actions have already been reviewed by the user, and keep their original source.
    payload.as_object_mut().unwrap().remove("origin");
    payload.as_object_mut().unwrap().remove("proposalToken");
    payload["caseId"] = json!(case_id);
    let result = match action {
        "create_task" | "create_deadline" | "set_reminder" => {
            let due = field("dueDate").or_else(|| field("remindAt"));
            payload["taskName"] = json!(field("taskName").or_else(|| field("name")).unwrap_or_else(|| content.clone()));
            payload["description"] = json!(field("description").unwrap_or_else(|| content.clone()));
            payload["taskType"] = json!(field("taskType").unwrap_or_else(|| if action == "create_deadline" { "deadline" } else { "action" }.into()));
            payload["startBucket"] = json!(field("startBucket").unwrap_or_else(|| if due.is_some() { "anytime" } else { "inbox" }.into()));
            payload["dueDate"] = json!(due);
            payload["startDate"] = json!(field("startDate").or_else(|| due.clone()));
            payload["inboxSourceId"] = json!(item_id);
            let task = super::tasks::create_task_in_transaction(&tx, payload)?;
            json!({"success":true,"action":"task_created","task":task})
        }
        "create_event" => {
            let date = field("eventDate").or_else(|| field("dueDate"))
                .ok_or_else(|| anyhow::anyhow!("请选择日程日期"))?;
            payload["title"] = json!(field("title").or_else(|| field("name")).unwrap_or_else(|| content.clone()));
            payload["eventDate"] = json!(date);
            payload["notes"] = json!(content);
            payload["allDay"] = json!(field("startTime").is_none());
            let event = super::calendar_events::create_event_in_transaction(&tx, payload)?;
            json!({"success":true,"action":"event_created","event":event})
        }
        "save_knowledge" => {
            let input = super::knowledge::CreateKnowledgeInput {
                title: field("title").unwrap_or_else(|| content.chars().take(60).collect()),
                content: intent.and_then(|value| value.get("content")).and_then(Value::as_str).unwrap_or(&content).to_owned(),
                category: field("category").unwrap_or_else(|| "reference".into()),
                source_type: Some("inbox".into()), source_id: Some(item_id.into()),
                linked_case_id: case_id.map(str::to_owned), ..Default::default()
            };
            let id = super::knowledge::create_knowledge_in_transaction(&tx, input)?;
            json!({"success":true,"action":"knowledge_saved","knowledgeId":id})
        }
        "create_case" => {
            payload["caseName"] = json!(field("caseName").or_else(|| field("name")).unwrap_or_else(|| content.clone()));
            payload["track"] = json!(field("track").unwrap_or_else(|| "patent_invalidation".into()));
            payload["clientName"] = json!(field("clientName").unwrap_or_default());
            payload["opponentName"] = json!(field("opponentName").unwrap_or_default());
            if payload["notes"].is_null() { payload["notes"] = json!(content); }
            let case = super::cases::create_case_in_transaction(&tx, payload)?;
            json!({"success":true,"action":"case_created","case":case})
        }
        "create_project" => {
            payload["name"] = json!(field("name").or_else(|| field("title")).unwrap_or_else(|| content.clone()));
            payload["description"] = json!(field("description").unwrap_or_else(|| content.clone()));
            let project = super::projects::create_project_in_transaction(&tx, payload)?;
            json!({"success":true,"action":"project_created","project":project})
        }
        "update_holidays" => {
            let notice = if let Some(value) = intent.filter(|value| value.get("holidays").is_some() || value.get("workdays").is_some()) {
                let notice = serde_json::from_value(value.clone())?;
                super::inbox::validate_holiday_notice(notice)
            } else { super::inbox::parse_holiday_dates(&content) }.map_err(anyhow::Error::msg)?;
            let mut calendar = db::get_setting(&tx, "holidays_json")?.and_then(|value|
                crate::deadline::holidays::HolidayCalendar::from_json_str(&value).ok())
                .unwrap_or_else(crate::deadline::holidays::HolidayCalendar::builtin);
            let current: std::collections::HashMap<_, _> = notice.holidays.iter().chain(&notice.workdays)
                .filter_map(|date| date.get(..4)?.parse::<i32>().ok()).collect::<std::collections::BTreeSet<_>>()
                .into_iter().flat_map(|year| calendar.entries_for_year(year)).map(|entry| (entry.date, entry.kind)).collect();
            let changed = notice.holidays.iter().filter(|date| current.get(*date).map(String::as_str) != Some("holiday")).count()
                + notice.workdays.iter().filter(|date| current.get(*date).map(String::as_str) != Some("workday")).count();
            calendar.merge_dates(&notice.holidays, &notice.workdays).map_err(anyhow::Error::msg)?;
            tx.execute("INSERT INTO settings(key,value) VALUES('holidays_json',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[calendar.to_json()])?;
            json!({"success":true,"action":"holidays_updated","year":notice.year,"changedDates":changed,"holidaysCount":notice.holidays.len(),"workdaysCount":notice.workdays.len(),"holidays":notice.holidays,"workdays":notice.workdays})
        }
        _ => anyhow::bail!("未知收件箱动作: {action}"),
    };
    let linked_case = result.get("case").and_then(|value| value["id"].as_str()).or(case_id);
    let now = db::now_local();
    tx.execute("UPDATE inbox_items SET status='filed',linked_case_id=?2,processed_at=?3 WHERE id=?1", params![item_id,linked_case,now])?;
    tx.execute("INSERT INTO inbox_feedback(id,inbox_item_id,action,intent_json,accepted,rejected_at) VALUES(?1,?2,?3,?4,1,?5)",
        params![db::new_id(),item_id,action,intent.map(Value::to_string),now])?;
    tx.execute("INSERT INTO inbox_action_results(inbox_item_id,action,result_json,created_at) VALUES(?1,?2,?3,?4)",params![item_id,action,result.to_string(),now])?;
    tx.commit()?;
    if action == "create_case" { crate::sync::feishu::get_auto_push_manager().notify_change(); }
    Ok(result)
}

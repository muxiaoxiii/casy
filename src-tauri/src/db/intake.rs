use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use serde_json::Value;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("").trim()
}

fn required<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str> {
    let result = text(value, key);
    if result.is_empty() {
        bail!("{label}不能为空");
    }
    Ok(result)
}

pub fn validate_case(data: &Value) -> Result<()> {
    required(data, "caseName", "案件名称")?;
    validate_patch(data)
}

pub fn validate_patch(data: &Value) -> Result<()> {
    if data.get("caseName").is_some() {
        required(data, "caseName", "案件名称")?;
    }
    let parties = match &data["thirdParties"] {
        Value::Null => vec![],
        Value::String(raw) if raw.trim().is_empty() => vec![],
        Value::String(raw) => serde_json::from_str::<Vec<Value>>(raw)?,
        Value::Array(parties) => parties.clone(),
        _ => bail!("第三人必须是列表"),
    };
    for party in &parties {
        required(party, "name", "第三人名称")?;
    }
    for key in ["caseAmount", "legalFees"] {
        let amount = match &data[key] {
            Value::Null => continue,
            Value::String(raw) if raw.trim().is_empty() => continue,
            Value::String(raw) => raw
                .trim()
                .parse::<f64>()
                .map_err(|_| anyhow::anyhow!("金额格式不正确"))?,
            Value::Number(raw) => raw
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("金额格式不正确"))?,
            _ => bail!("金额格式不正确"),
        };
        if !amount.is_finite() || amount < 0.0 {
            bail!("金额不能为负数");
        }
    }
    Ok(())
}

/// Called within the same transaction as the case, so a rejected child rolls back everything.
pub fn insert_children(conn: &Connection, case_id: &str, data: &Value) -> Result<()> {
    for (key, label) in [
        ("relatedCases", "关联案件"),
        ("hearings", "庭审"),
        ("logs", "日志"),
        ("tasks", "任务"),
        ("officials", "联系人"),
    ] {
        if let Some(value) = data.get(key) {
            if !value.is_array() {
                bail!("{label}必须是列表");
            }
        }
    }
    for item in data["relatedCases"].as_array().into_iter().flatten() {
        let target = required(item, "caseId", "关联案件")?;
        if target == case_id {
            bail!("不能关联案件自身");
        }
        let kind = required(item, "relationType", "关联类型")?;
        conn.execute("INSERT INTO case_relations (id,source_case_id,target_case_id,relation_type,label) VALUES (?1,?2,?3,?4,?5)",
            params![super::new_id(),case_id,target,kind,text(item,"label")])?;
    }
    for item in data["hearings"].as_array().into_iter().flatten() {
        let name = required(item, "hearingName", "庭审名称")?;
        let date = required(item, "hearingDate", "庭审时间")?;
        conn.execute("INSERT INTO hearings (id,case_id,hearing_record,hearing_name,hearing_date,venue,attendees,judges,court,case_level,contact_info,actual_status) VALUES (?1,?2,?3,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![super::new_id(),case_id,name,date,text(item,"venue"),text(item,"attendees"),text(item,"judges"),text(item,"court"),text(item,"caseLevel"),text(item,"contactInfo"),item["actualStatus"].as_str().filter(|s| !s.is_empty())])?;
    }
    for item in data["logs"].as_array().into_iter().flatten() {
        let summary = required(item, "eventSummary", "日志概述")?;
        let date = required(item, "eventDate", "发生时间")?;
        let kind = required(item, "eventType", "事件类型")?;
        conn.execute("INSERT INTO case_logs (id,case_id,event_summary,event_name,event_date,event_type,content) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![super::new_id(),case_id,summary,text(item,"eventName"),date,kind,text(item,"content")])?;
    }
    for item in data["tasks"].as_array().into_iter().flatten() {
        let name = required(item, "taskName", "任务名称")?;
        let date = text(item, "deadline");
        let deadline = (!date.is_empty()).then_some(date);
        conn.execute("INSERT INTO tasks (id,case_id,task_name,description,created_date,deadline,due_date,priority,completed,assignee,finish_note) VALUES (?1,?2,?3,?4,?5,?6,?6,?7,?8,?9,?10)",
            params![super::new_id(),case_id,name,text(item,"description"),super::now_local(),deadline,
                item["priority"].as_str().unwrap_or("normal"),item["completed"].as_bool().unwrap_or(false) as i32,text(item,"assignee"),text(item,"finishNote")])?;
    }
    for item in data["officials"].as_array().into_iter().flatten() {
        let name = required(item, "name", "联系人姓名")?;
        let id = super::new_id();
        conn.execute("INSERT INTO officials (id,name,role,court,contact_detail,contact_text,contact_record) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![id,name,required(item,"role","身份")?,text(item,"court"),text(item,"contactDetail"),text(item,"contactText"),text(item,"contactRecord")])?;
        conn.execute(
            "INSERT INTO case_officials (case_id,official_id) VALUES (?1,?2)",
            params![case_id, id],
        )?;
        conn.execute(
            "INSERT INTO persons (id,kind,name,org,phone,notes) VALUES (?1,'judge',?2,?3,?4,?5)",
            params![
                id,
                name,
                text(item, "court"),
                text(item, "contactDetail"),
                text(item, "contactRecord")
            ],
        )?;
        conn.execute(
            "INSERT INTO case_persons (id,case_id,person_id,role) VALUES (?1,?2,?3,?4)",
            params![super::new_id(), case_id, id, text(item, "role")],
        )?;
    }
    Ok(())
}

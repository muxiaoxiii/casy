use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

use crate::db::{self, now_local};

#[derive(Debug, Deserialize)]
struct FeishuDump {
    tables: Option<FeishuTables>,
}

#[derive(Debug, Deserialize)]
struct FeishuTables {
    cases: Option<FeishuTable>,
    case_logs: Option<FeishuTable>,
    hearings: Option<FeishuTable>,
    tasks: Option<FeishuTable>,
    officials: Option<FeishuTable>,
}

#[derive(Debug, Deserialize)]
struct FeishuTable {
    records: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub cases: usize,
    pub logs: usize,
    pub hearings: usize,
    pub tasks: usize,
    pub officials: usize,
    pub errors: Vec<String>,
}

pub fn import_feishu_dump(conn: &mut rusqlite::Connection, json_path: &Path) -> Result<ImportReport> {
    let data = std::fs::read_to_string(json_path)?;
    let dump: FeishuDump = serde_json::from_str(&data)?;

    let mut report = ImportReport {
        cases: 0,
        logs: 0,
        hearings: 0,
        tasks: 0,
        officials: 0,
        errors: Vec::new(),
    };

    let tables = match dump.tables {
        Some(t) => t,
        None => return Ok(report),
    };

    // 整个导入包在事务中，失败时全部回滚
    let tx = conn.transaction()?;

    // 导入案件
    if let Some(table) = tables.cases {
        for record in &table.records {
            match import_case(&tx, record) {
                Ok(_) => report.cases += 1,
                Err(e) => report.errors.push(format!("案件: {}", e)),
            }
        }
    }

    // 导入日志
    if let Some(table) = tables.case_logs {
        for record in &table.records {
            match import_log(&tx, record) {
                Ok(_) => report.logs += 1,
                Err(e) => report.errors.push(format!("日志: {}", e)),
            }
        }
    }

    // 导入庭审
    if let Some(table) = tables.hearings {
        for record in &table.records {
            match import_hearing(&tx, record) {
                Ok(_) => report.hearings += 1,
                Err(e) => report.errors.push(format!("庭审: {}", e)),
            }
        }
    }

    // 导入任务
    if let Some(table) = tables.tasks {
        for record in &table.records {
            match import_task(&tx, record) {
                Ok(_) => report.tasks += 1,
                Err(e) => report.errors.push(format!("任务: {}", e)),
            }
        }
    }

    // 导入官方人员
    if let Some(table) = tables.officials {
        for record in &table.records {
            match import_official(&tx, record) {
                Ok(_) => report.officials += 1,
                Err(e) => report.errors.push(format!("人员: {}", e)),
            }
        }
    }

    tx.commit()?;
    Ok(report)
}

fn import_case(conn: &rusqlite::Connection, record: &serde_json::Value) -> Result<()> {
    let feishu_id = record["record_id"].as_str().unwrap_or_default();
    let fields = &record["fields"];

    let case_name = fields["案件信息"].as_str().unwrap_or("").trim();
    if case_name.is_empty() {
        anyhow::bail!("案件名称为空，跳过");
    }

    let track = match extract_single_select(&fields["案由"]).as_deref() {
        Some("专利无效") => "patent_invalidation",
        Some("专利侵权" | "技术秘密" | "著作权权属" | "专利权属" | "外观侵权" | "恶意诉讼不正当竞争") => "civil_tort",
        Some("专利行政" | "商标行政") => "admin_litigation",
        _ => "other",
    };

    let case = db::cases::Case {
        id: feishu_id.to_string(),
        track: track.to_string(),
        case_name: case_name.to_string(),
        case_no: fields["案号"].as_str().map(|s| s.to_string()),
        internal_no: fields["内部卷号"].as_str().map(|s| s.to_string()),
        cause_action: extract_single_select(&fields["案由"]),
        client_name: fields["客户名称"].as_str().unwrap_or("").to_string(),
        our_role: fields["我方诉讼地位"].as_str().map(|s| s.to_string()),
        opponent_name: fields["对方名称"].as_str().unwrap_or("").to_string(),
        opponent_role: extract_single_select(&fields["诉讼地位"]),
        opponent_firm: fields["对方代理律所"].as_str().map(|s| s.to_string()),
        opponent_agent: fields["对方代理人"].as_str().map(|s| s.to_string()),
        court: extract_single_select(&fields["审理机关"]),
        judge_panel: fields["合议庭"].as_str().map(|s| s.to_string()),
        clerk: fields["书记员"].as_str().map(|s| s.to_string()),
        attorneys: extract_multi_select_json_option(&fields["代理人"]),
        case_level: extract_single_select(&fields["审级"]),
        case_progress: extract_single_select(&fields["案件进展"]),
        case_result: extract_single_select(&fields["案件结果"]),
        patent_name: fields["专利名称"].as_str().map(|s| s.to_string()),
        patent_app_no: fields["专利申请号"].as_str().map(|s| s.to_string()),
        procedure_type: extract_single_select(&fields["诉讼程序"]),
        filing_date: extract_datetime(&fields["立案"]),
        complaint_received_date: extract_datetime(&fields["收到起诉状时间"]),
        trial_date: extract_datetime(&fields["开庭|口审"]),
        trial2_date: extract_datetime(&fields["二次开庭|口审"]),
        trial3_date: extract_datetime(&fields["三次开庭丨口审"]),
        verdict_type: extract_single_select(&fields["收到判决/裁定/决定类型"]),
        verdict_date: extract_datetime(&fields["收到判决/裁定/决定时间"]),
        // 专利无效专属期限字段
        petitioner_first_invalid: extract_datetime(&fields["请求人首次无效宣告理由"]),
        petitioner_supp_deadline: extract_datetime(&fields["请求人补正期限"]),
        petitioner_submit_date: extract_datetime(&fields["请求人提交日期"]),
        petitioner_received_date: extract_datetime(&fields["请求人收到日期"]),
        petitioner_reply_deadline: extract_datetime(&fields["请求人答复期限"]),
        patentee_received_date: extract_datetime(&fields["专利权人收到日期"]),
        patentee_statement_deadline: extract_datetime(&fields["专利权人陈述期限"]),
        patentee_received_supp_date: extract_datetime(&fields["专利权人收到补充日期"]),
        patentee_supp_deadline: extract_datetime(&fields["专利权人补正期限"]),
        patentee_submit_supp_date: extract_datetime(&fields["专利权人提交补充日期"]),
        notes: fields["备注"].as_str().map(|s| s.to_string()),
        ..Default::default()
    };

    conn.execute(
        "INSERT OR REPLACE INTO cases (id, track, case_name, case_no, internal_no, cause_action,
         client_name, our_role, opponent_name, opponent_role, opponent_firm, opponent_agent,
         court, judge_panel, clerk, attorneys, case_level, case_progress, case_result,
         patent_name, patent_app_no, procedure_type,
         filing_date, complaint_received_date, trial_date, trial2_date, trial3_date,
         verdict_type, verdict_date,
         petitioner_first_invalid, petitioner_supp_deadline, petitioner_submit_date,
         petitioner_received_date, petitioner_reply_deadline,
         patentee_received_date, patentee_statement_deadline, patentee_received_supp_date,
         patentee_supp_deadline, patentee_submit_supp_date,
         notes, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,
                 ?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,
                 ?38,?39,?40,?41,?42)",
        rusqlite::params![
            case.id, case.track, case.case_name, case.case_no, case.internal_no, case.cause_action,
            case.client_name, case.our_role, case.opponent_name, case.opponent_role,
            case.opponent_firm, case.opponent_agent, case.court, case.judge_panel, case.clerk,
            case.attorneys, case.case_level, case.case_progress, case.case_result,
            case.patent_name, case.patent_app_no, case.procedure_type,
            case.filing_date, case.complaint_received_date,
            case.trial_date, case.trial2_date, case.trial3_date, case.verdict_type,
            case.verdict_date,
            case.petitioner_first_invalid, case.petitioner_supp_deadline,
            case.petitioner_submit_date, case.petitioner_received_date,
            case.petitioner_reply_deadline,
            case.patentee_received_date, case.patentee_statement_deadline,
            case.patentee_received_supp_date, case.patentee_supp_deadline,
            case.patentee_submit_supp_date,
            case.notes, now_local(), now_local(),
        ],
    )?;
    Ok(())
}

fn import_log(conn: &rusqlite::Connection, record: &serde_json::Value) -> Result<()> {
    let feishu_id = record["record_id"].as_str().unwrap_or_default();
    let fields = &record["fields"];

    let summary = fields["事件概述"].as_str().unwrap_or("").trim();
    if summary.is_empty() {
        anyhow::bail!("事件概述为空，跳过");
    }

    // 从关联字段提取 case_id
    let case_id = extract_first_link_id(&fields["案件名称"]);

    let event_type = match extract_single_select(&fields["类型"]).as_deref() {
        Some("任务") => "task",
        Some("交文") => "submitted",
        Some("收文") => "received",
        _ => "record",
    };

    conn.execute(
        "INSERT OR REPLACE INTO case_logs (id, case_id, event_summary, event_name, event_type, event_date, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            feishu_id,
            case_id.unwrap_or_default(),
            summary,
            fields["事件名称"].as_str().unwrap_or(""),
            event_type,
            extract_datetime(&fields["发生时间"]).unwrap_or_else(|| now_local()),
            fields["操作内容"].as_str().unwrap_or(""),
            now_local(),
        ],
    )?;
    Ok(())
}

fn import_hearing(conn: &rusqlite::Connection, record: &serde_json::Value) -> Result<()> {
    let feishu_id = record["record_id"].as_str().unwrap_or_default();
    let fields = &record["fields"];

    let hearing_record = fields["开庭记录"].as_str().unwrap_or("").trim();
    if hearing_record.is_empty() {
        anyhow::bail!("开庭记录为空，跳过");
    }

    let case_id = extract_first_link_id(&fields["案件信息"]);

    // 从关联案件查询 court 和 case_level，避免硬编码空字符串
    let (court, case_level) = if let Some(ref cid) = case_id {
        lookup_case_court_level(conn, cid)
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    conn.execute(
        "INSERT OR REPLACE INTO hearings (id, case_id, hearing_record, hearing_name, hearing_date,
         venue, attendees, judges, court, case_level, actual_status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            feishu_id,
            case_id.unwrap_or_default(),
            hearing_record,
            fields["开庭名称"].as_str().unwrap_or(""),
            extract_datetime(&fields["开庭时间"]).unwrap_or_default(),
            fields["开庭地点"].as_str().unwrap_or(""),
            fields["出庭人员"].as_str().unwrap_or(""),
            extract_multi_select_json(&fields["审判人员"]),
            court.unwrap_or_default(),
            case_level.unwrap_or_default(),
            extract_single_select(&fields["实际开庭情况"]).unwrap_or_default(),
            now_local(),
        ],
    )?;
    Ok(())
}

/// 从 cases 表查询 court 和 case_level
fn lookup_case_court_level(conn: &rusqlite::Connection, case_id: &str) -> Result<(Option<String>, Option<String>)> {
    let mut stmt = conn.prepare("SELECT court, case_level FROM cases WHERE id = ?1")?;
    let result = stmt.query_row(rusqlite::params![case_id], |row| {
        Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    Ok(result)
}

fn import_task(conn: &rusqlite::Connection, record: &serde_json::Value) -> Result<()> {
    let feishu_id = record["record_id"].as_str().unwrap_or_default();
    let fields = &record["fields"];

    let task_name = fields["任务名称"].as_str().unwrap_or("").trim();
    if task_name.is_empty() {
        anyhow::bail!("任务名称为空，跳过");
    }

    let case_id = extract_first_link_id(&fields["关联项目"]);
    let completed = fields["完成状态"].as_bool().unwrap_or(false) as i32;

    conn.execute(
        "INSERT OR REPLACE INTO tasks (id, case_id, task_name, description, created_date,
         deadline, priority, completed, assignee, finish_note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            feishu_id,
            case_id,
            task_name,
            fields["任务详细描述"].as_str().unwrap_or(""),
            extract_datetime(&fields["创建日期"]).unwrap_or_else(|| now_local()),
            extract_datetime(&fields["截止日期"]),
            extract_single_select(&fields["优先级"]),
            completed,
            "", // assignee
            fields["完结记录"].as_str().unwrap_or(""),
            now_local(),
        ],
    )?;
    Ok(())
}

fn import_official(conn: &rusqlite::Connection, record: &serde_json::Value) -> Result<()> {
    let feishu_id = record["record_id"].as_str().unwrap_or_default();
    let fields = &record["fields"];

    let name = fields["姓名"].as_str().unwrap_or("").trim();
    if name.is_empty() {
        anyhow::bail!("姓名为空，跳过");
    }

    conn.execute(
        "INSERT OR REPLACE INTO officials (id, name, role, court, contact_detail, contact_text, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            feishu_id,
            name,
            extract_single_select(&fields["身份"]).unwrap_or_default(),
            extract_single_select(&fields["所属机关"]).unwrap_or_default(),
            fields["具体联系方式"].as_str().unwrap_or(""),
            fields["联系方式"].as_str().unwrap_or(""),
            now_local(),
        ],
    )?;
    Ok(())
}

fn extract_single_select(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => arr.first().and_then(|v| v.as_str()).map(|s| s.to_string()),
        serde_json::Value::Object(obj) => obj.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
        _ => None,
    }
}

fn extract_datetime(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Number(n) => {
            let ms = n.as_i64()?;
            if ms == 0 {
                return None;
            }
            let dt = chrono::DateTime::from_timestamp_millis(ms)?.naive_utc();
            Some(dt.format("%Y-%m-%d").to_string())
        }
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() { None } else { Some(s.to_string()) }
        }
        _ => None,
    }
}

fn extract_first_link_id(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(arr) => {
            arr.first().and_then(|v| v.as_str()).map(|s| s.to_string())
        }
        serde_json::Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn extract_multi_select_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
        }
        serde_json::Value::String(s) => {
            if s.is_empty() {
                "[]".to_string()
            } else {
                serde_json::to_string(&[s]).unwrap_or_else(|_| "[]".to_string())
            }
        }
        _ => "[]".to_string(),
    }
}

/// 提取多选字段为 JSON 数组字符串，返回 Option（空数组返回 None）
fn extract_multi_select_json_option(value: &serde_json::Value) -> Option<String> {
    let s = extract_multi_select_json(value);
    if s == "[]" { None } else { Some(s) }
}

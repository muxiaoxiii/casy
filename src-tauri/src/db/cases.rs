use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::{row_get_string, row_get_string_or};

/// 案件数据结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub id: String,
    pub track: String,
    pub case_name: String,
    pub case_no: Option<String>,
    pub internal_no: Option<String>,
    pub cause_action: Option<String>,
    pub client_name: String,
    pub our_role: Option<String>,
    pub opponent_name: String,
    pub opponent_role: Option<String>,
    pub opponent_firm: Option<String>,
    pub opponent_agent: Option<String>,
    pub court: Option<String>,
    pub judge_panel: Option<String>,
    pub clerk: Option<String>,
    pub attorneys: Option<String>,
    pub case_level: Option<String>,
    pub case_status: Option<String>,
    pub case_progress: Option<String>,
    pub case_result: Option<String>,
    pub patent_name: Option<String>,
    pub patent_app_no: Option<String>,
    pub procedure_type: Option<String>,
    pub filing_date: Option<String>,
    pub complaint_received_date: Option<String>,
    pub trial_date: Option<String>,
    pub trial2_date: Option<String>,
    pub trial3_date: Option<String>,
    pub verdict_type: Option<String>,
    pub verdict_date: Option<String>,
    pub stay_date: Option<String>,
    pub relief_deadline: Option<String>,
    pub petitioner_first_invalid: Option<String>,
    pub petitioner_supp_deadline: Option<String>,
    pub petitioner_submit_date: Option<String>,
    pub petitioner_received_date: Option<String>,
    pub petitioner_reply_deadline: Option<String>,
    pub patentee_received_date: Option<String>,
    pub patentee_statement_deadline: Option<String>,
    pub patentee_received_supp_date: Option<String>,
    pub patentee_supp_deadline: Option<String>,
    pub patentee_submit_supp_date: Option<String>,
    // 双轨状态机
    pub case_route: Option<String>,
    pub civil_status: Option<String>,
    pub invalidation_status: Option<String>,
    pub admin_status: Option<String>,
    // 无效程序新增日期
    pub invalidation_decision_date: Option<String>,
    pub invalidation_decision_type: Option<String>,
    // 行政诉讼新增日期
    pub admin_filing_date: Option<String>,
    pub admin_verdict_date: Option<String>,
    pub admin_trial2_date: Option<String>,
    pub folder_path: Option<String>,
    pub folder_template_id: Option<String>,
    pub last_doc_path: Option<String>,
    pub last_doc_at: Option<String>,
    pub completed_text: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// 期限紧急度（仅列表查询时填充）：red=3天内, yellow=14天内, green=其他
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline_urgency: Option<String>,
}

/// 列表查询过滤条件
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CaseFilter {
    pub track: Option<String>,
    pub client: Option<String>,
    pub court: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub sort_by: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    // 新状态机筛选
    pub case_route: Option<String>,
    pub civil_status: Option<String>,
    pub invalidation_status: Option<String>,
    pub admin_status: Option<String>,
}

/// 列表查询结果
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseListResult {
    pub items: Vec<Case>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// 列表查询
pub fn list_cases(conn: &Connection, filter: &CaseFilter) -> Result<CaseListResult> {
    let mut sql = String::from("SELECT * FROM cases WHERE 1=1");
    let mut count_sql = String::from("SELECT COUNT(*) FROM cases WHERE 1=1");
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut param_idx = 1;

    if let Some(track) = &filter.track {
        if !track.is_empty() {
            sql.push_str(&format!(" AND track = ?{}", param_idx));
            count_sql.push_str(&format!(" AND track = ?{}", param_idx));
            params_vec.push(Box::new(track.clone()));
            param_idx += 1;
        }
    }
    if let Some(client) = &filter.client {
        if !client.is_empty() {
            sql.push_str(&format!(" AND client_name = ?{}", param_idx));
            count_sql.push_str(&format!(" AND client_name = ?{}", param_idx));
            params_vec.push(Box::new(client.clone()));
            param_idx += 1;
        }
    }
    if let Some(court) = &filter.court {
        if !court.is_empty() {
            sql.push_str(&format!(" AND court = ?{}", param_idx));
            count_sql.push_str(&format!(" AND court = ?{}", param_idx));
            params_vec.push(Box::new(court.clone()));
            param_idx += 1;
        }
    }
    if let Some(status) = &filter.status {
        if !status.is_empty() {
            sql.push_str(&format!(" AND case_status = ?{}", param_idx));
            count_sql.push_str(&format!(" AND case_status = ?{}", param_idx));
            params_vec.push(Box::new(status.clone()));
            param_idx += 1;
        }
    }
    if let Some(search) = &filter.search {
        if !search.is_empty() {
            let like = format!("%{}%", search);
            // SQLite LIKE with same param for all columns
            let cond = format!(
                " AND (case_name LIKE ?{0} ESCAPE '\\' OR case_no LIKE ?{0} ESCAPE '\\' OR client_name LIKE ?{0} ESCAPE '\\' OR opponent_name LIKE ?{0} ESCAPE '\\')",
                param_idx
            );
            sql.push_str(&cond);
            count_sql.push_str(&cond);
            params_vec.push(Box::new(like));
            param_idx += 1;
        }
    }

    // 新状态机筛选
    if let Some(case_route) = &filter.case_route {
        if !case_route.is_empty() {
            sql.push_str(&format!(" AND case_route = ?{}", param_idx));
            count_sql.push_str(&format!(" AND case_route = ?{}", param_idx));
            params_vec.push(Box::new(case_route.clone()));
            param_idx += 1;
        }
    }
    if let Some(civil_status) = &filter.civil_status {
        if !civil_status.is_empty() {
            sql.push_str(&format!(" AND civil_status = ?{}", param_idx));
            count_sql.push_str(&format!(" AND civil_status = ?{}", param_idx));
            params_vec.push(Box::new(civil_status.clone()));
            param_idx += 1;
        }
    }
    if let Some(invalidation_status) = &filter.invalidation_status {
        if !invalidation_status.is_empty() {
            sql.push_str(&format!(" AND invalidation_status = ?{}", param_idx));
            count_sql.push_str(&format!(" AND invalidation_status = ?{}", param_idx));
            params_vec.push(Box::new(invalidation_status.clone()));
            param_idx += 1;
        }
    }
    if let Some(admin_status) = &filter.admin_status {
        if !admin_status.is_empty() {
            sql.push_str(&format!(" AND admin_status = ?{}", param_idx));
            count_sql.push_str(&format!(" AND admin_status = ?{}", param_idx));
            params_vec.push(Box::new(admin_status.clone()));
            param_idx += 1;
        }
    }

    // 日期范围筛选（基于 filing_date）
    if let Some(date_from) = &filter.date_from {
        if !date_from.is_empty() {
            let cond = format!(" AND filing_date >= ?{}", param_idx);
            sql.push_str(&cond);
            count_sql.push_str(&cond);
            params_vec.push(Box::new(date_from.clone()));
            param_idx += 1;
        }
    }
    if let Some(date_to) = &filter.date_to {
        if !date_to.is_empty() {
            let cond = format!(" AND filing_date <= ?{}", param_idx);
            sql.push_str(&cond);
            count_sql.push_str(&cond);
            params_vec.push(Box::new(date_to.clone()));
            // param_idx not needed after last filter
        }
    }

    // 排序
    let order = match filter.sort_by.as_deref().unwrap_or("filing_date") {
        "filing_date" => "filing_date DESC NULLS LAST",
        "case_name" => "case_name ASC",
        "client_name" => "client_name ASC",
        "updated_at" => "updated_at DESC",
        _ => "filing_date DESC NULLS LAST",
    };
    sql.push_str(&format!(" ORDER BY {}", order));

    // 分页
    let page = filter.page.unwrap_or(1).max(1);
    let per_page = filter.per_page.unwrap_or(50).min(200);
    sql.push_str(&format!(" LIMIT {} OFFSET {}", per_page, (page - 1) * per_page));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    // 查询总数
    let total: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |r| r.get(0))?;

    // 查询数据
    let mut stmt = conn.prepare(&sql)?;
    let cases = stmt
        .query_map(param_refs.as_slice(), |row| row_to_case(row))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // 计算每个案件的期限紧急度
    let case_ids: Vec<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    let urgency_map = compute_deadline_urgency(conn, &case_ids);

    let mut items = cases;
    for case in &mut items {
        case.deadline_urgency = urgency_map.get(case.id.as_str()).cloned();
    }

    Ok(CaseListResult {
        items,
        total,
        page,
        per_page,
    })
}

/// 获取单个案件
pub fn get_case(conn: &Connection, id: &str) -> Result<Case> {
    let mut stmt = conn.prepare("SELECT * FROM cases WHERE id = ?1")?;
    let case = stmt.query_row(params![id], |row| row_to_case(row))?;
    Ok(case)
}

/// 创建案件
pub fn insert_case(conn: &Connection, case: &Case) -> Result<()> {
    conn.execute(
        "INSERT INTO cases (id, track, case_name, case_no, internal_no, cause_action,
         client_name, our_role, opponent_name, opponent_role, opponent_firm, opponent_agent,
         court, judge_panel, clerk, attorneys, case_level, case_progress, case_result,
         patent_name, patent_app_no, procedure_type,
         filing_date, complaint_received_date, trial_date, trial2_date, trial3_date,
         verdict_type, verdict_date, stay_date, relief_deadline,
         petitioner_first_invalid, petitioner_supp_deadline, petitioner_submit_date,
         petitioner_received_date, petitioner_reply_deadline,
         patentee_received_date, patentee_statement_deadline, patentee_received_supp_date,
         patentee_supp_deadline, patentee_submit_supp_date,
         case_route, civil_status, invalidation_status, admin_status,
         invalidation_decision_date, invalidation_decision_type,
         admin_filing_date, admin_verdict_date, admin_trial2_date,
         folder_path, notes, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,
                 ?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,
                 ?36,?37,?38,?39,?40,?41,?42,?43,?44,?45,?46,?47,?48,?49,?50,?51,?52,?53,?54)",
        params![
            case.id, case.track, case.case_name, case.case_no, case.internal_no, case.cause_action,
            case.client_name, case.our_role, case.opponent_name, case.opponent_role,
            case.opponent_firm, case.opponent_agent, case.court, case.judge_panel, case.clerk,
            case.attorneys, case.case_level, case.case_progress, case.case_result,
            case.patent_name, case.patent_app_no, case.procedure_type,
            case.filing_date, case.complaint_received_date, case.trial_date, case.trial2_date,
            case.trial3_date, case.verdict_type, case.verdict_date, case.stay_date, case.relief_deadline,
            case.petitioner_first_invalid, case.petitioner_supp_deadline, case.petitioner_submit_date,
            case.petitioner_received_date, case.petitioner_reply_deadline,
            case.patentee_received_date, case.patentee_statement_deadline, case.patentee_received_supp_date,
            case.patentee_supp_deadline, case.patentee_submit_supp_date,
            case.case_route, case.civil_status, case.invalidation_status, case.admin_status,
            case.invalidation_decision_date, case.invalidation_decision_type,
            case.admin_filing_date, case.admin_verdict_date, case.admin_trial2_date,
            case.folder_path, case.notes, case.created_at, case.updated_at,
        ],
    )?;
    Ok(())
}

/// 更新案件（PATCH 语义）
pub fn update_case(conn: &Connection, id: &str, data: &serde_json::Value) -> Result<Case> {
    let mut sql = String::from("UPDATE cases SET updated_at = datetime('now','localtime')");
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    let fields = [
        ("caseName", "case_name"), ("caseNo", "case_no"), ("internalNo", "internal_no"),
        ("causeAction", "cause_action"), ("clientName", "client_name"), ("ourRole", "our_role"),
        ("opponentName", "opponent_name"), ("opponentRole", "opponent_role"),
        ("opponentFirm", "opponent_firm"), ("opponentAgent", "opponent_agent"),
        ("court", "court"), ("judgePanel", "judge_panel"), ("clerk", "clerk"),
        ("attorneys", "attorneys"), ("caseLevel", "case_level"), ("caseProgress", "case_progress"),
        ("caseResult", "case_result"), ("patentName", "patent_name"), ("patentAppNo", "patent_app_no"),
        ("procedureType", "procedure_type"), ("filingDate", "filing_date"),
        ("complaintReceivedDate", "complaint_received_date"), ("trialDate", "trial_date"),
        ("trial2Date", "trial2_date"), ("trial3Date", "trial3_date"),
        ("verdictType", "verdict_type"), ("verdictDate", "verdict_date"),
        ("stayDate", "stay_date"), ("reliefDeadline", "relief_deadline"),
        ("notes", "notes"), ("track", "track"),
        ("completedText", "completed_text"),
        ("petitionerFirstInvalid", "petitioner_first_invalid"),
        ("petitionerSuppDeadline", "petitioner_supp_deadline"),
        ("petitionerSubmitDate", "petitioner_submit_date"),
        ("petitionerReceivedDate", "petitioner_received_date"),
        ("petitionerReplyDeadline", "petitioner_reply_deadline"),
        ("patenteeReceivedDate", "patentee_received_date"),
        ("patenteeStatementDeadline", "patentee_statement_deadline"),
        ("patenteeReceivedSuppDate", "patentee_received_supp_date"),
        ("patenteeSuppDeadline", "patentee_supp_deadline"),
        ("patenteeSubmitSuppDate", "patentee_submit_supp_date"),
        ("folderTemplateId", "folder_template_id"),
        // 双轨状态机
        ("caseRoute", "case_route"),
        ("civilStatus", "civil_status"),
        ("invalidationStatus", "invalidation_status"),
        ("adminStatus", "admin_status"),
        ("invalidationDecisionDate", "invalidation_decision_date"),
        ("invalidationDecisionType", "invalidation_decision_type"),
        ("adminFilingDate", "admin_filing_date"),
        ("adminVerdictDate", "admin_verdict_date"),
        ("adminTrial2Date", "admin_trial2_date"),
    ];

    let mut param_idx = 1;
    for (json_key, db_col) in &fields {
        if let Some(val) = data.get(*json_key) {
            sql.push_str(&format!(", {} = ?{}", db_col, param_idx));
            match val {
                serde_json::Value::String(s) => params_vec.push(Box::new(s.clone())),
                serde_json::Value::Null => params_vec.push(Box::new(rusqlite::types::Null)),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        params_vec.push(Box::new(i));
                    } else {
                        params_vec.push(Box::new(n.to_string()));
                    }
                }
                serde_json::Value::Bool(b) => params_vec.push(Box::new(*b as i32)),
                _ => params_vec.push(Box::new(val.to_string())),
            }
            param_idx += 1;
        }
    }

    sql.push_str(&format!(" WHERE id = ?{}", param_idx));
    params_vec.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;

    get_case(conn, id)
}

/// 删除案件
pub fn delete_case(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM cases WHERE id = ?1", params![id])?;
    Ok(())
}

/// 全文搜索
pub fn search_cases(conn: &Connection, query: &str) -> Result<Vec<Case>> {
    let mut stmt = conn.prepare(
        "SELECT c.* FROM cases_fts f JOIN cases c ON c.rowid = f.rowid
         WHERE cases_fts MATCH ?1 ORDER BY rank LIMIT 50"
    )?;
    let cases = stmt
        .query_map(params![query], |row| row_to_case(row))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(cases)
}

/// 活跃案件（未完结）
pub fn active_cases(conn: &Connection) -> Result<Vec<Case>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cases WHERE case_status IS NULL OR case_status != '已完结' ORDER BY filing_date DESC"
    )?;
    let cases = stmt
        .query_map([], |row| row_to_case(row))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(cases)
}

/// 按客户分组统计
pub fn case_counts_by_client(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT client_name, COUNT(*) FROM cases GROUP BY client_name ORDER BY COUNT(*) DESC"
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 按轨道分组统计
pub fn case_counts_by_track(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT track, COUNT(*) FROM cases GROUP BY track ORDER BY COUNT(*) DESC"
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 计算案件期限紧急度：red=3天内到期, yellow=14天内到期
fn compute_deadline_urgency(conn: &Connection, case_ids: &[&str]) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if case_ids.is_empty() {
        return map;
    }

    let today = chrono::Local::now().naive_local().date().format("%Y-%m-%d").to_string();

    // 查询每个案件最近的未完成期限
    let placeholders: String = case_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT case_id, MIN(due_date) as nearest_due
         FROM case_deadlines
         WHERE case_id IN ({}) AND completed = 0 AND due_date >= ?{}
         GROUP BY case_id",
        placeholders,
        case_ids.len() + 1
    );

    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for id in case_ids {
        params_vec.push(Box::new(id.to_string()));
    }
    params_vec.push(Box::new(today.clone()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(param_refs.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                let (case_id, nearest_due) = row;
                if let (Ok(today_d), Ok(due_d)) = (
                    chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d"),
                    chrono::NaiveDate::parse_from_str(&nearest_due, "%Y-%m-%d"),
                ) {
                    let days_left = (due_d - today_d).num_days();
                    let urgency = if days_left <= 3 {
                        "red".to_string()
                    } else if days_left <= 14 {
                        "yellow".to_string()
                    } else {
                        "green".to_string()
                    };
                    map.insert(case_id, urgency);
                }
            }
        }
    }

    map
}

/// 行转结构体
fn row_to_case(row: &rusqlite::Row) -> rusqlite::Result<Case> {
    Ok(Case {
        id: row_get_string_or(row, "id")?,
        track: row_get_string_or(row, "track")?,
        case_name: row_get_string_or(row, "case_name")?,
        case_no: row_get_string(row, "case_no")?,
        internal_no: row_get_string(row, "internal_no")?,
        cause_action: row_get_string(row, "cause_action")?,
        client_name: row_get_string_or(row, "client_name")?,
        our_role: row_get_string(row, "our_role")?,
        opponent_name: row_get_string_or(row, "opponent_name")?,
        opponent_role: row_get_string(row, "opponent_role")?,
        opponent_firm: row_get_string(row, "opponent_firm")?,
        opponent_agent: row_get_string(row, "opponent_agent")?,
        court: row_get_string(row, "court")?,
        judge_panel: row_get_string(row, "judge_panel")?,
        clerk: row_get_string(row, "clerk")?,
        attorneys: row_get_string(row, "attorneys")?,
        case_level: row_get_string(row, "case_level")?,
        case_status: row_get_string(row, "case_status")?,
        case_progress: row_get_string(row, "case_progress")?,
        case_result: row_get_string(row, "case_result")?,
        patent_name: row_get_string(row, "patent_name")?,
        patent_app_no: row_get_string(row, "patent_app_no")?,
        procedure_type: row_get_string(row, "procedure_type")?,
        filing_date: row_get_string(row, "filing_date")?,
        complaint_received_date: row_get_string(row, "complaint_received_date")?,
        trial_date: row_get_string(row, "trial_date")?,
        trial2_date: row_get_string(row, "trial2_date")?,
        trial3_date: row_get_string(row, "trial3_date")?,
        verdict_type: row_get_string(row, "verdict_type")?,
        verdict_date: row_get_string(row, "verdict_date")?,
        stay_date: row_get_string(row, "stay_date")?,
        relief_deadline: row_get_string(row, "relief_deadline")?,
        petitioner_first_invalid: row_get_string(row, "petitioner_first_invalid")?,
        petitioner_supp_deadline: row_get_string(row, "petitioner_supp_deadline")?,
        petitioner_submit_date: row_get_string(row, "petitioner_submit_date")?,
        petitioner_received_date: row_get_string(row, "petitioner_received_date")?,
        petitioner_reply_deadline: row_get_string(row, "petitioner_reply_deadline")?,
        patentee_received_date: row_get_string(row, "patentee_received_date")?,
        patentee_statement_deadline: row_get_string(row, "patentee_statement_deadline")?,
        patentee_received_supp_date: row_get_string(row, "patentee_received_supp_date")?,
        patentee_supp_deadline: row_get_string(row, "patentee_supp_deadline")?,
        patentee_submit_supp_date: row_get_string(row, "patentee_submit_supp_date")?,
        case_route: row_get_string(row, "case_route")?,
        civil_status: row_get_string(row, "civil_status")?,
        invalidation_status: row_get_string(row, "invalidation_status")?,
        admin_status: row_get_string(row, "admin_status")?,
        invalidation_decision_date: row_get_string(row, "invalidation_decision_date")?,
        invalidation_decision_type: row_get_string(row, "invalidation_decision_type")?,
        admin_filing_date: row_get_string(row, "admin_filing_date")?,
        admin_verdict_date: row_get_string(row, "admin_verdict_date")?,
        admin_trial2_date: row_get_string(row, "admin_trial2_date")?,
        folder_path: row_get_string(row, "folder_path")?,
        folder_template_id: row_get_string(row, "folder_template_id")?,
        last_doc_path: row_get_string(row, "last_doc_path")?,
        last_doc_at: row_get_string(row, "last_doc_at")?,
        completed_text: row_get_string(row, "completed_text")?,
        notes: row_get_string(row, "notes")?,
        created_at: row_get_string(row, "created_at")?,
        updated_at: row_get_string(row, "updated_at")?,
        deadline_urgency: None,
    })
}

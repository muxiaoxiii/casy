use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::{row_get_string, row_get_string_or};

/// 案件数据结构
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type)]
#[serde(rename_all = "camelCase", default)]
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
    pub third_parties: Option<String>,
    pub case_amount: Option<String>,
    pub legal_fees: Option<String>,
    pub fee_payment: Option<String>,
    pub claims: Option<String>,
    pub jurisdiction_objection: Option<String>,
    pub external_case_no: Option<String>,
    pub defense_deadline: Option<String>,
    pub estimated_trial_end: Option<String>,
    pub court: Option<String>,
    pub judge_panel: Option<String>,
    pub clerk: Option<String>,
    pub attorneys: Option<String>,
    pub case_level: Option<String>,
    pub case_status: Option<String>,
    pub case_progress: Option<String>,
    pub case_result: Option<String>,
    pub case_goal: Option<String>,
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
#[derive(Debug, Deserialize, Default, specta::Type)]
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
    pub deadline_from: Option<String>,
    pub deadline_to: Option<String>,
    pub hearing_from: Option<String>,
    pub hearing_to: Option<String>,
    pub operator: Option<String>,
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
#[derive(Debug, Serialize, specta::Type)]
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
            param_idx += 1;
        }
    }

    for (value, column, comparison) in [
        (&filter.deadline_from, "next_deadline", ">="),
        (&filter.deadline_to, "next_deadline", "<="),
        (&filter.hearing_from, "next_hearing", ">="),
        (&filter.hearing_to, "next_hearing", "<="),
        (&filter.operator, "operator", "LIKE"),
    ] {
        if let Some(value) = value.as_ref().filter(|value| !value.is_empty()) {
            let expression = if column == "operator" { column.to_owned() } else { format!("substr({column},1,10)") };
            let condition = format!(" AND id IN (SELECT id FROM v_case_unified WHERE {expression} {comparison} ?{param_idx})");
            sql.push_str(&condition);
            count_sql.push_str(&condition);
            params_vec.push(Box::new(if column == "operator" { format!("%{value}%") } else { value.clone() }));
            param_idx += 1;
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
    let per_page = filter.per_page.unwrap_or(50).clamp(1, 200);
    sql.push_str(&format!(
        " LIMIT {} OFFSET {}",
        per_page,
        (page - 1) * per_page
    ));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();

    // 查询总数
    let total: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |r| r.get(0))?;

    // 查询数据
    let mut stmt = conn.prepare(&sql)?;
    let cases = stmt
        .query_map(param_refs.as_slice(), row_to_case)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // 计算每个案件的期限紧急度
    let urgency_map = compute_deadline_urgency(conn, &cases)?;

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
    let case = stmt.query_row(params![id], row_to_case)?;
    Ok(case)
}

/// 创建案件
pub fn insert_case(conn: &Connection, case: &Case) -> Result<()> {
    conn.execute(
        "INSERT INTO cases (id, track, case_name, case_no, internal_no, cause_action,
         client_name, our_role, opponent_name, opponent_role, opponent_firm, opponent_agent,
         court, judge_panel, clerk, attorneys, case_level, case_progress, case_result, case_goal,
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
                 ?36,?37,?38,?39,?40,?41,?42,?43,?44,?45,?46,?47,?48,?49,?50,?51,?52,?53,?54,?55)",
        params![
            case.id,
            case.track,
            case.case_name,
            case.case_no,
            case.internal_no,
            case.cause_action,
            case.client_name,
            case.our_role,
            case.opponent_name,
            case.opponent_role,
            case.opponent_firm,
            case.opponent_agent,
            case.court,
            case.judge_panel,
            case.clerk,
            case.attorneys,
            case.case_level,
            case.case_progress,
            case.case_result,
            case.case_goal,
            case.patent_name,
            case.patent_app_no,
            case.procedure_type,
            case.filing_date,
            case.complaint_received_date,
            case.trial_date,
            case.trial2_date,
            case.trial3_date,
            case.verdict_type,
            case.verdict_date,
            case.stay_date,
            case.relief_deadline,
            case.petitioner_first_invalid,
            case.petitioner_supp_deadline,
            case.petitioner_submit_date,
            case.petitioner_received_date,
            case.petitioner_reply_deadline,
            case.patentee_received_date,
            case.patentee_statement_deadline,
            case.patentee_received_supp_date,
            case.patentee_supp_deadline,
            case.patentee_submit_supp_date,
            case.case_route
                .as_deref()
                .unwrap_or(match case.track.as_str() {
                    "patent_invalidation" => "专利无效",
                    "admin_litigation" => "行政诉讼",
                    "other" => "其他",
                    _ => "民事诉讼",
                }),
            case.civil_status,
            case.invalidation_status,
            case.admin_status,
            case.invalidation_decision_date,
            case.invalidation_decision_type,
            case.admin_filing_date,
            case.admin_verdict_date,
            case.admin_trial2_date,
            case.folder_path,
            case.notes,
            case.created_at.clone().unwrap_or_else(super::now_local),
            case.updated_at.clone().unwrap_or_else(super::now_local),
        ],
    )?;
    update_case(
        conn,
        &case.id,
        &serde_json::json!({
            "thirdParties": case.third_parties, "caseAmount": case.case_amount,
            "legalFees": case.legal_fees, "feePayment": case.fee_payment,
            "claims": case.claims, "jurisdictionObjection": case.jurisdiction_objection,
            "externalCaseNo": case.external_case_no, "defenseDeadline": case.defense_deadline,
            "estimatedTrialEnd": case.estimated_trial_end, "completedText": case.completed_text,
            "folderTemplateId": case.folder_template_id
        }),
    )?;
    Ok(())
}

/// 更新案件（PATCH 语义）
pub fn update_case(conn: &Connection, id: &str, data: &serde_json::Value) -> Result<Case> {
    super::intake::validate_patch(data)?;
    let mut sql = String::from("UPDATE cases SET updated_at = datetime('now','localtime')");
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    let fields = [
        ("caseName", "case_name"),
        ("caseNo", "case_no"),
        ("internalNo", "internal_no"),
        ("causeAction", "cause_action"),
        ("clientName", "client_name"),
        ("ourRole", "our_role"),
        ("opponentName", "opponent_name"),
        ("opponentRole", "opponent_role"),
        ("opponentFirm", "opponent_firm"),
        ("opponentAgent", "opponent_agent"),
        ("thirdParties", "third_parties"),
        ("caseAmount", "case_amount"),
        ("legalFees", "legal_fees"),
        ("feePayment", "fee_payment"),
        ("claims", "claims"),
        ("jurisdictionObjection", "jurisdiction_objection"),
        ("externalCaseNo", "external_case_no"),
        ("defenseDeadline", "defense_deadline"),
        ("estimatedTrialEnd", "estimated_trial_end"),
        ("court", "court"),
        ("judgePanel", "judge_panel"),
        ("clerk", "clerk"),
        ("attorneys", "attorneys"),
        ("caseLevel", "case_level"),
        ("caseProgress", "case_progress"),
        ("caseResult", "case_result"),
        ("caseGoal", "case_goal"),
        ("patentName", "patent_name"),
        ("patentAppNo", "patent_app_no"),
        ("procedureType", "procedure_type"),
        ("filingDate", "filing_date"),
        ("complaintReceivedDate", "complaint_received_date"),
        ("trialDate", "trial_date"),
        ("trial2Date", "trial2_date"),
        ("trial3Date", "trial3_date"),
        ("verdictType", "verdict_type"),
        ("verdictDate", "verdict_date"),
        ("stayDate", "stay_date"),
        ("reliefDeadline", "relief_deadline"),
        ("notes", "notes"),
        ("track", "track"),
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

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;

    get_case(conn, id)
}

/// 程序投影表按 `item_id` 归档，既无外键也无 case_id（audit F1/F2）。
///
/// item_id 的构成本身带案件线索，删除案件时按这些前缀一并清理：
/// `{event_id}:{suffix}`（procedure_events，随案件 CASCADE）、
/// `legacy:{case_id}:{key}:{suffix}`（案件旧字段合成事件）、
/// `manual:{deadline_id}:manual`（case_deadlines，随案件 CASCADE）、
/// `task|hearing:{record_id}`（共享任务/庭审记录，只取本案自己的记录——
/// 被他案链接共享的记录仍归他案所有，不能一起清）。
fn delete_procedure_item_rows(conn: &Connection, table: &str, id: &str) -> Result<()> {
    // 表名来自本文件内的两个固定字面量，不做外部拼接。
    let sql = format!(
        "DELETE FROM {table} WHERE
           substr(item_id, 1, length(?1) + 8) = 'legacy:' || ?1 || ':'
           OR item_id IN (SELECT 'manual:' || d.id || ':manual' FROM case_deadlines d WHERE d.case_id = ?1)
           OR EXISTS (SELECT 1 FROM procedure_events e WHERE e.case_id = ?1
                      AND substr(item_id, 1, length(e.id) + 1) = e.id || ':')
           OR item_id IN (SELECT 'task:' || t.id FROM tasks t WHERE t.case_id = ?1)
           OR item_id IN (SELECT 'hearing:' || h.id FROM hearings h WHERE h.case_id = ?1)"
    );
    conn.execute(&sql, params![id])?;
    Ok(())
}

/// 删除案件
pub fn delete_case(conn: &Connection, id: &str) -> Result<()> {
    // links 无外键：先清双向孤儿行，避免案件删除后留下无法查阅的关联。
    conn.execute(
        "DELETE FROM links WHERE (source_type='case' AND source_id=?1)
         OR (target_type='case' AND target_id=?1)",
        params![id],
    )?;
    // CASCADE 会硬删该案 tasks/case_files；links 同样无 FK，需先清这些实体的双向引用。
    conn.execute(
        "DELETE FROM links WHERE
         (source_type='task' AND source_id IN (SELECT id FROM tasks WHERE case_id=?1))
         OR (target_type='task' AND target_id IN (SELECT id FROM tasks WHERE case_id=?1))
         OR (source_type='file' AND source_id IN (SELECT id FROM case_files WHERE case_id=?1))
         OR (target_type='file' AND target_id IN (SELECT id FROM case_files WHERE case_id=?1))",
        params![id],
    )?;
    // 子任务自引用无 ON DELETE SET NULL：先断开父子链，避免 CASCADE 顺序撞 FK。
    // 跨案父子链同样要断开：他案任务引用本案任务时，案件删除会被 FK 永久挡住。
    conn.execute(
        "UPDATE tasks SET parent_task_id = NULL
         WHERE parent_task_id IN (SELECT id FROM tasks WHERE case_id = ?1)",
        params![id],
    )?;
    // F1/F2：程序投影的处理状态、提醒回执、程序审计与提醒日志无级联，随案件一并清理，
    // 否则删除案件后会留下永久的孤儿行（提醒回执还会影响以后复用的 item_id）。
    delete_procedure_item_rows(conn, "procedure_item_states", id)?;
    delete_procedure_item_rows(conn, "procedure_reminder_receipts", id)?;
    conn.execute("DELETE FROM procedure_audit WHERE case_id = ?1", params![id])?;
    conn.execute("DELETE FROM reminder_log WHERE case_id = ?1", params![id])?;
    conn.execute("DELETE FROM cases WHERE id = ?1", params![id])?;
    Ok(())
}

/// 全文搜索
pub fn search_cases(conn: &Connection, query: &str) -> Result<Vec<Case>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }
    let pattern = format!(
        "%{}%",
        query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let mut stmt = conn.prepare(
        "SELECT * FROM cases WHERE case_name LIKE ?1 ESCAPE '\\' OR case_no LIKE ?1 ESCAPE '\\'
         OR client_name LIKE ?1 ESCAPE '\\' OR opponent_name LIKE ?1 ESCAPE '\\'
         OR patent_app_no LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT 50",
    )?;
    let cases = stmt
        .query_map(params![pattern], row_to_case)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(cases)
}

/// 活跃案件（未完结）
pub fn active_cases(conn: &Connection) -> Result<Vec<Case>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cases WHERE case_status IS NULL OR case_status != '已完结' ORDER BY filing_date DESC"
    )?;
    let cases = stmt
        .query_map([], row_to_case)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(cases)
}

/// 按客户分组统计
pub fn case_counts_by_client(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT client_name, COUNT(*) FROM cases GROUP BY client_name ORDER BY COUNT(*) DESC",
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 按轨道分组统计
pub fn case_counts_by_track(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt =
        conn.prepare("SELECT track, COUNT(*) FROM cases GROUP BY track ORDER BY COUNT(*) DESC")?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 计算案件期限紧急度：red=3天内到期, yellow=14天内到期
fn compute_deadline_urgency(
    conn: &Connection,
    cases: &[Case],
) -> Result<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    let case_ids: Vec<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    if case_ids.is_empty() {
        return Ok(map);
    }

    let today = chrono::Local::now()
        .naive_local()
        .date()
        .format("%Y-%m-%d")
        .to_string();

    // 查询每个案件最近的未完成期限
    let placeholders: String = case_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT case_id, MIN(due_date) as nearest_due
         FROM case_deadlines
         WHERE case_id IN ({}) AND completed = 0 AND due_date >= ?{}
         GROUP BY case_id",
        placeholders,
        case_ids.len() + 1
    );

    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for id in &case_ids {
        params_vec.push(Box::new(id.to_string()));
    }
    params_vec.push(Box::new(today.clone()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();

    {
        let mut stmt = conn.prepare(&sql)?;
        { let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
            for row in rows {
                let (case_id, nearest_due) = row?;
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

    { let context = crate::deadline::procedure::ProjectionContext::load(conn,cases)?;
        for case in cases {
            { let (_,items)=crate::deadline::procedure::case_items_with_context(case,&context)?;
                if let Some(days)=items.iter().filter(|i|i.status=="open").filter_map(|i|i.days_left).min() {
                    let urgency=if days<=3 {"red"}else if days<=14 {"yellow"}else{"green"};
                    let rank=|s:&str|match s {"red"=>0,"yellow"=>1,_=>2};
                    if map.get(&case.id).is_none_or(|old|rank(urgency)<rank(old)) {map.insert(case.id.clone(),urgency.to_string());}
                }
            }
        }
    }

    Ok(map)
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
        third_parties: row_get_string(row, "third_parties")?,
        case_amount: row_get_string(row, "case_amount")?,
        legal_fees: row_get_string(row, "legal_fees")?,
        fee_payment: row_get_string(row, "fee_payment")?,
        claims: row_get_string(row, "claims")?,
        jurisdiction_objection: row_get_string(row, "jurisdiction_objection")?,
        external_case_no: row_get_string(row, "external_case_no")?,
        defense_deadline: row_get_string(row, "defense_deadline")?,
        estimated_trial_end: row_get_string(row, "estimated_trial_end")?,
        court: row_get_string(row, "court")?,
        judge_panel: row_get_string(row, "judge_panel")?,
        clerk: row_get_string(row, "clerk")?,
        attorneys: row_get_string(row, "attorneys")?,
        case_level: row_get_string(row, "case_level")?,
        case_status: row_get_string(row, "case_status")?,
        case_progress: row_get_string(row, "case_progress")?,
        case_result: row_get_string(row, "case_result")?,
        case_goal: row_get_string(row, "case_goal")?,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(super::super::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        super::super::schema::run_migrations(&conn, 1).unwrap();
        // 打开外键：跨案父子链不断开时，删除案件会被 tasks.parent_task_id 的 FK 挡住（F2）。
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn
    }

    /// F1/F2：删除案件必须同时清掉四张无级联的表，并断开跨案父子链。
    #[test]
    fn delete_case_clears_procedure_orphans_and_cross_case_parent_chain() {
        let conn = test_conn();
        conn.execute_batch(
            "INSERT INTO cases (id, case_name, client_name, track) VALUES
               ('c1', '本案', '委托人', 'civil_tort'), ('c2', '他案', '委托人', 'civil_tort');
             INSERT INTO tasks (id, case_id, task_name, created_date) VALUES
               ('t-mine', 'c1', '本案任务', '2026-10-01'),
               ('t-other', 'c2', '他案任务', '2026-10-01'),
               ('t-child', 'c2', '他案子任务', '2026-10-01');
             UPDATE tasks SET parent_task_id = 't-mine' WHERE id = 't-child';
             INSERT INTO hearings (id, case_id, hearing_record, hearing_date) VALUES
               ('h1', 'c1', '本案庭审', '2026-11-01'), ('h2', 'c2', '他案庭审', '2026-11-02');
             INSERT INTO procedure_events (id, case_id, payload, created_at, updated_at)
               VALUES ('ev-1', 'c1', '{}', '2026-10-01', '2026-10-01');
             INSERT INTO case_deadlines (id, case_id, deadline_name, due_date)
               VALUES ('dl-1', 'c1', '答辩期', '2026-10-20');
             INSERT INTO procedure_audit (id, case_id, event_id, action, after_json, reason, created_at)
               VALUES ('pa-1', 'c1', 'ev-1', 'create', '{}', '测试', '2026-10-01');
             INSERT INTO procedure_item_states (item_id, fingerprint, status, note, updated_at) VALUES
               ('ev-1:answer', 'fp', 'open', 'note', '2026-10-01'),
               ('legacy:c1:filing_date:answer', 'fp', 'open', 'note', '2026-10-01'),
               ('manual:dl-1:manual', 'fp', 'open', 'note', '2026-10-01'),
               ('task:t-mine', 'fp', 'open', 'note', '2026-10-01'),
               ('hearing:h1', 'fp', 'open', 'note', '2026-10-01'),
               ('ev-9:answer', 'fp', 'open', 'note', '2026-10-01'),
               ('task:t-other', 'fp', 'open', 'note', '2026-10-01'),
               ('hearing:h2', 'fp', 'open', 'note', '2026-10-01');
             INSERT INTO procedure_reminder_receipts (item_id, fingerprint, rule_id, sent_on) VALUES
               ('ev-1:answer', 'fp', 'rule-1', '2026-10-01'),
               ('legacy:c1:filing_date:answer', 'fp', 'rule-1', '2026-10-01'),
               ('ev-9:answer', 'fp', 'rule-1', '2026-10-01');
             INSERT INTO reminder_log (id, rule_id, case_id, channel, message, level, status)
               VALUES ('rl-1', 'rule-1', 'c1', 'local', 'm', 'R1', 'sent');",
        )
        .unwrap();

        delete_case(&conn, "c1").unwrap();

        // 本案程序事件/期限/庭审随 CASCADE 消失，其处理状态与回执不得留孤儿
        assert_eq!(count(&conn, "procedure_audit"), 0);
        assert_eq!(count(&conn, "procedure_reminder_receipts"), 1);
        assert_eq!(count(&conn, "reminder_log"), 0);
        // 只剩与他案/未知事项相关的状态行
        let left: Vec<String> = conn
            .prepare("SELECT item_id FROM procedure_item_states ORDER BY item_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            left,
            vec![
                "ev-9:answer".to_string(),
                "hearing:h2".to_string(),
                "task:t-other".to_string(),
            ]
        );
        // 跨案父子链被断开，他案任务本身保留
        let parent: Option<String> = conn
            .query_row(
                "SELECT parent_task_id FROM tasks WHERE id = 't-child'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(parent, None, "指向已删案件任务的父子链必须断开");
        assert_eq!(count(&conn, "tasks"), 2);
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
}

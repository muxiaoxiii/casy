use crate::commands::run_blocking;
use crate::docsy_engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 模板列表响应
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateListResponse {
    pub templates: Vec<docsy_engine::DocsyTemplate>,
    pub total: usize,
}

/// 渲染结果响应
#[derive(Debug, Serialize, Deserialize)]
pub struct RenderResponse {
    pub html: String,
    pub text: String,
    pub used_fields: HashMap<String, String>,
    pub missing_fields: Vec<String>,
}

/// 导出结果响应
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportResponse {
    pub output_path: String,
    pub file_size: u64,
    pub exported_at: String,
}

/// 列出所有可用的 Docsy 模板
#[tauri::command]
pub async fn list_docsy_templates() -> Result<TemplateListResponse, String> {
    run_blocking(move || {
        let templates = docsy_engine::list_templates()?;
        let total = templates.len();
        Ok(TemplateListResponse { templates, total })
    })
    .await
}

/// 渲染模板，用案件数据填充占位符
#[tauri::command]
pub async fn render_docsy_template(
    template_id: String,
    case_id: String,
) -> Result<RenderResponse, String> {
    run_blocking(move || {
        // 1. 加载模板
        let template = docsy_engine::load_template(&template_id)?;

        // 2. 加载案件数据
        let conn = crate::db::open_db()?;
        let case_data = load_case_data(&conn, &case_id)?;

        // 3. 加载设置
        let settings = load_settings(&conn).unwrap_or_default();

        // 4. 映射字段
        let values = map_case_to_template_values(&case_data, &settings);

        // 5. 渲染模板
        let result = docsy_engine::render_template(&template.path, &values)?;

        Ok(RenderResponse {
            html: result.html,
            text: result.text,
            used_fields: result.used_fields,
            missing_fields: result.missing_fields,
        })
    })
    .await
}

/// 导出 DOCX 文件
#[tauri::command]
pub async fn export_docx(
    template_id: String,
    case_id: String,
    output_path: Option<String>,
) -> Result<ExportResponse, String> {
    run_blocking(move || {
        // 1. 加载模板
        let template = docsy_engine::load_template(&template_id)?;

        // 2. 加载案件数据
        let conn = crate::db::open_db()?;
        let case_data = load_case_data(&conn, &case_id)?;

        // 3. 加载设置
        let settings = load_settings(&conn).unwrap_or_default();

        // 4. 映射字段
        let values = map_case_to_template_values(&case_data, &settings);

        // 5. 导出 DOCX
        let result = docsy_engine::export_docx(
            &template.path,
            &values,
            output_path.as_deref(),
        )?;

        Ok(ExportResponse {
            output_path: result.output_path,
            file_size: result.file_size,
            exported_at: result.exported_at,
        })
    })
    .await
}

/// 案件数据结构
#[derive(Debug, Default)]
#[allow(dead_code)]
struct CaseData {
    id: String,
    case_name: String,
    case_no: Option<String>,
    internal_no: Option<String>,
    cause_action: Option<String>,
    client_name: String,
    our_role: Option<String>,
    opponent_name: Option<String>,
    opponent_role: Option<String>,
    opponent_firm: Option<String>,
    opponent_agent: Option<String>,
    court: Option<String>,
    judge_panel: Option<String>,
    clerk: Option<String>,
    attorneys: Option<String>,
    case_level: Option<String>,
    case_progress: Option<String>,
    case_result: Option<String>,
    patent_name: Option<String>,
    patent_app_no: Option<String>,
    procedure_type: Option<String>,
    filing_date: Option<String>,
    complaint_received_date: Option<String>,
    trial_date: Option<String>,
    trial2_date: Option<String>,
    trial3_date: Option<String>,
    verdict_type: Option<String>,
    verdict_date: Option<String>,
    stay_date: Option<String>,
    relief_deadline: Option<String>,
    petitioner_first_invalid: Option<String>,
    petitioner_supp_deadline: Option<String>,
    petitioner_submit_date: Option<String>,
    petitioner_received_date: Option<String>,
    petitioner_reply_deadline: Option<String>,
    patentee_received_date: Option<String>,
    patentee_statement_deadline: Option<String>,
    patentee_received_supp_date: Option<String>,
    patentee_supp_deadline: Option<String>,
    patentee_submit_supp_date: Option<String>,
    notes: Option<String>,
}

/// 设置数据
#[derive(Debug, Default)]
struct Settings {
    firm_name: String,
}

/// 从数据库加载案件数据
fn load_case_data(conn: &rusqlite::Connection, case_id: &str) -> anyhow::Result<CaseData> {
    let mut stmt = conn.prepare(
        "SELECT id, case_name, case_no, internal_no, cause_action,
                client_name, our_role, opponent_name, opponent_role, opponent_firm, opponent_agent,
                court, judge_panel, clerk, attorneys, case_level, case_progress, case_result,
                patent_name, patent_app_no, procedure_type,
                filing_date, complaint_received_date, trial_date, trial2_date, trial3_date,
                verdict_type, verdict_date, stay_date, relief_deadline,
                petitioner_first_invalid, petitioner_supp_deadline,
                petitioner_submit_date, petitioner_received_date, petitioner_reply_deadline,
                patentee_received_date, patentee_statement_deadline,
                patentee_received_supp_date, patentee_supp_deadline, patentee_submit_supp_date,
                notes
         FROM cases WHERE id = ?1",
    )?;

    let case = stmt.query_row(rusqlite::params![case_id], |row| {
        Ok(CaseData {
            id: row.get(0)?,
            case_name: row.get(1)?,
            case_no: row.get(2)?,
            internal_no: row.get(3)?,
            cause_action: row.get(4)?,
            client_name: row.get(5)?,
            our_role: row.get(6)?,
            opponent_name: row.get(7)?,
            opponent_role: row.get(8)?,
            opponent_firm: row.get(9)?,
            opponent_agent: row.get(10)?,
            court: row.get(11)?,
            judge_panel: row.get(12)?,
            clerk: row.get(13)?,
            attorneys: row.get(14)?,
            case_level: row.get(15)?,
            case_progress: row.get(16)?,
            case_result: row.get(17)?,
            patent_name: row.get(18)?,
            patent_app_no: row.get(19)?,
            procedure_type: row.get(20)?,
            filing_date: row.get(21)?,
            complaint_received_date: row.get(22)?,
            trial_date: row.get(23)?,
            trial2_date: row.get(24)?,
            trial3_date: row.get(25)?,
            verdict_type: row.get(26)?,
            verdict_date: row.get(27)?,
            stay_date: row.get(28)?,
            relief_deadline: row.get(29)?,
            petitioner_first_invalid: row.get(30)?,
            petitioner_supp_deadline: row.get(31)?,
            petitioner_submit_date: row.get(32)?,
            petitioner_received_date: row.get(33)?,
            petitioner_reply_deadline: row.get(34)?,
            patentee_received_date: row.get(35)?,
            patentee_statement_deadline: row.get(36)?,
            patentee_received_supp_date: row.get(37)?,
            patentee_supp_deadline: row.get(38)?,
            patentee_submit_supp_date: row.get(39)?,
            notes: row.get(40)?,
        })
    })?;

    Ok(case)
}

/// 从数据库加载设置
fn load_settings(conn: &rusqlite::Connection) -> anyhow::Result<Settings> {
    // 尝试从 settings 表读取，如果表不存在则返回默认值
    let result = conn.query_row(
        "SELECT value FROM settings WHERE key = 'firm_name'",
        [],
        |row| row.get::<_, String>(0),
    );

    match result {
        Ok(name) => Ok(Settings { firm_name: name }),
        Err(_) => Ok(Settings::default()),
    }
}

/// 将案件数据映射为模板值（40+ 字段）
fn map_case_to_template_values(
    case: &CaseData,
    settings: &Settings,
) -> HashMap<String, serde_json::Value> {
    let mut values = HashMap::new();

    // ---- 文本字段 ----
    values.insert("法院".into(), json_str(&case.court));
    values.insert("案号".into(), json_str(&case.case_no));
    values.insert("案件名称".into(), json_str(&Some(case.case_name.clone())));
    values.insert("案由".into(), json_str(&case.cause_action));
    values.insert("内部卷号".into(), json_str(&case.internal_no));
    values.insert("专利名称".into(), json_str(&case.patent_name));
    values.insert("专利申请号".into(), json_str(&case.patent_app_no));
    values.insert("诉讼阶段".into(), json_str(&case.case_level));
    values.insert("案件进展".into(), json_str(&case.case_progress));
    values.insert("案件结果".into(), json_str(&case.case_result));
    values.insert("备注".into(), json_str(&case.notes));
    values.insert("律所名称".into(), json_str(&Some(settings.firm_name.clone())));
    values.insert("律师".into(), json_str(&case.attorneys));

    // ---- 日期字段 ----
    values.insert("立案日期".into(), json_str(&case.filing_date));
    values.insert("收到起诉状日期".into(), json_str(&case.complaint_received_date));
    values.insert("开庭日期".into(), json_str(&case.trial_date));
    values.insert("二审日期".into(), json_str(&case.trial2_date));
    values.insert("三审日期".into(), json_str(&case.trial3_date));
    values.insert("判决日期".into(), json_str(&case.verdict_date));
    values.insert("中止日期".into(), json_str(&case.stay_date));
    values.insert("救济期限".into(), json_str(&case.relief_deadline));
    values.insert("请求人首次无效日期".into(), json_str(&case.petitioner_first_invalid));
    values.insert("请求人补充意见期限".into(), json_str(&case.petitioner_supp_deadline));
    values.insert("请求人提交日期".into(), json_str(&case.petitioner_submit_date));
    values.insert("请求人收到日期".into(), json_str(&case.petitioner_received_date));
    values.insert("请求人答复期限".into(), json_str(&case.petitioner_reply_deadline));
    values.insert("专利权人收到日期".into(), json_str(&case.patentee_received_date));
    values.insert("专利权人陈述期限".into(), json_str(&case.patentee_statement_deadline));
    values.insert("专利权人收到补充日期".into(), json_str(&case.patentee_received_supp_date));
    values.insert("专利权人补充期限".into(), json_str(&case.patentee_supp_deadline));
    values.insert("专利权人提交补充日期".into(), json_str(&case.patentee_submit_supp_date));

    // 今日日期
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    values.insert("日期".into(), serde_json::Value::String(today.clone()));
    values.insert("今日日期".into(), serde_json::Value::String(today));

    // ---- party_list 字段 ----
    let mut our_parties = Vec::new();
    if !case.client_name.is_empty() {
        our_parties.push(serde_json::json!({
            "name": case.client_name,
            "suffix": case.our_role.as_deref().unwrap_or("请求人")
        }));
    }
    values.insert("我方当事人".into(), serde_json::Value::Array(our_parties.clone()));

    let mut opponent_parties = Vec::new();
    if let Some(ref name) = case.opponent_name {
        if !name.is_empty() {
            opponent_parties.push(serde_json::json!({
                "name": name,
                "suffix": case.opponent_role.as_deref().unwrap_or("被请求人")
            }));
        }
    }
    if let Some(ref agent) = case.opponent_agent {
        if !agent.is_empty() {
            opponent_parties.push(serde_json::json!({
                "name": agent,
                "suffix": "代理人"
            }));
        }
    }
    values.insert("对方当事人".into(), serde_json::Value::Array(opponent_parties.clone()));

    // 合并当事人
    our_parties.extend(opponent_parties);
    values.insert("当事人".into(), serde_json::Value::Array(our_parties));

    // ---- reference 字段 ----
    values.insert("审理机关".into(), json_str(&case.court));
    values.insert("审级".into(), json_str(&case.case_level));
    values.insert("对方代理律所".into(), json_str(&case.opponent_firm));

    // ---- checkbox/radio 字段 ----
    values.insert(
        "普通程序".into(),
        serde_json::Value::Bool(case.procedure_type.as_deref() == Some("普通")),
    );
    values.insert(
        "简易程序".into(),
        serde_json::Value::Bool(case.procedure_type.as_deref() == Some("简易")),
    );
    values.insert("判决类型".into(), json_str(&case.verdict_type));
    values.insert(
        "胜诉".into(),
        serde_json::Value::Bool(case.case_result.as_deref() == Some("胜诉")),
    );
    values.insert(
        "败诉".into(),
        serde_json::Value::Bool(case.case_result.as_deref() == Some("败诉")),
    );
    values.insert(
        "部分胜诉".into(),
        serde_json::Value::Bool(case.case_result.as_deref() == Some("部分胜诉")),
    );

    // 清理空值
    values.retain(|_, v| !v.is_null());

    values
}

/// 辅助函数：将 Option<String> 转为 JSON Value
fn json_str(opt: &Option<String>) -> serde_json::Value {
    match opt {
        Some(s) => serde_json::Value::String(s.clone()),
        None => serde_json::Value::String(String::new()),
    }
}

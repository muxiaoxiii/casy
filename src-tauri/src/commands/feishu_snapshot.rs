use super::import_excel::{
    clean_admin_status, clean_civil_status, clean_date_str, clean_invalidation_status,
    infer_track_and_route, match_field_confidence,
};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, FixedOffset};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotReport {
    pub assets: usize,
    pub files: usize,
    pub cases: usize,
    pub logs: usize,
    pub hearings: usize,
    pub tasks: usize,
    pub officials: usize,
    pub relations: usize,
    pub source_records: usize,
    pub source_links: usize,
    pub skipped: usize,
    pub warnings: Vec<String>,
}

fn array(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn s<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
fn id(source: &str, table: &str, record: &str) -> String {
    format!("feishu:{source}:{table}:{record}")
}

fn collect_options(value: &Value, options: &mut HashMap<String, String>) {
    match value {
        Value::Object(obj) => {
            if let (Some(key), Some(name)) = (
                obj.get("id").and_then(Value::as_str),
                obj.get("name").and_then(Value::as_str),
            ) {
                if key.starts_with("opt") {
                    options.insert(key.into(), name.into());
                }
            }
            for child in obj.values() {
                collect_options(child, options);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_options(child, options);
            }
        }
        _ => {}
    }
}

fn decode(value: &Value, options: &HashMap<String, String>) -> Value {
    match value {
        Value::String(raw) => Value::String(options.get(raw).unwrap_or(raw).clone()),
        Value::Array(items) => Value::Array(items.iter().map(|v| decode(v, options)).collect()),
        Value::Object(obj) => Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), decode(v, options)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(v) => v.trim().to_string(),
        Value::Array(items) => {
            // Rich text fragments are contiguous; selections and people are separate values.
            let separator = if items
                .iter()
                .all(|v| v.get("text").is_some() && v.get("record_ids").is_none())
            {
                ""
            } else {
                "、"
            };
            items
                .iter()
                .map(display)
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>()
                .join(separator)
        }
        Value::Object(_) => ["text", "name", "link", "value"]
            .iter()
            .find_map(|k| value.get(k).map(display))
            .unwrap_or_default(),
        _ => value.to_string(),
    }
}

pub fn field_display(value: &Value, field: &Value, options: &HashMap<String, String>) -> String {
    let date = field["type"] == 5 || field["property"]["type"]["data_type"] == 5;
    if date {
        let raw = value
            .as_i64()
            .or_else(|| array(value).first().and_then(Value::as_i64));
        if let Some(dt) = raw.and_then(DateTime::from_timestamp_millis) {
            let dt = dt.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());
            let fmt = field["property"]["date_formatter"]
                .as_str()
                .or(field["property"]["type"]["ui_property"]["date_formatter"].as_str())
                .or(field["property"]["formatter"].as_str())
                .unwrap_or("");
            return dt
                .format(if fmt.contains("HH") {
                    "%Y-%m-%d %H:%M:%S"
                } else {
                    "%Y-%m-%d"
                })
                .to_string();
        }
    }
    display(&decode(value, options))
}

fn refs(value: &Value, default_table: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Array(items) => {
            for item in items {
                refs(item, default_table, out);
            }
        }
        Value::Object(_) => {
            let table = value["table_id"].as_str().unwrap_or(default_table);
            for record in array(&value["record_ids"]) {
                if let Some(record) = record.as_str() {
                    out.push((table.into(), record.into()));
                }
            }
            if let Some(record) = value["record_id"].as_str() {
                out.push((table.into(), record.into()));
            }
        }
        Value::String(record) if record.starts_with("rec") => {
            out.push((default_table.into(), record.clone()))
        }
        _ => {}
    }
}

const CASE_FIELDS: &[(&str, &str)] = &[
    ("案号", "caseNo"),
    ("案由", "causeAction"),
    ("内部卷号", "internalNo"),
    ("客户名称", "clientName"),
    ("案件进展", "caseProgress"),
    ("专利名称", "patentName"),
    ("专利申请号", "patentAppNo"),
    ("办案人", "attorneys"),
    ("对方名称", "opponentName"),
    ("对方代理律所", "opponentFirm"),
    ("对方代理人", "opponentAgent"),
    ("审理机关", "court"),
    ("合议庭", "judgePanel"),
    ("审级", "caseLevel"),
    ("案件结果", "caseResult"),
    ("已完成", "completedText"),
    ("备注", "notes"),
    ("诉讼程序", "procedureType"),
    ("救济期限", "reliefDeadline"),
    ("案件信息", "caseName"),
    ("案件状态", "caseStatus"),
    ("我方诉讼地位", "ourRole"),
    ("诉讼地位", "opponentRole"),
    ("书记员|助理", "clerk"),
    ("金助理案号", "externalCaseNo"),
    ("管辖异议", "jurisdictionObjection"),
    ("开庭|口审", "trialDate"),
    ("二次开庭|口审", "trial2Date"),
    ("三次开庭丨口审", "trial3Date"),
    ("收到判决/裁定/决定类型", "verdictType"),
    ("收到判决/裁定/决定时间", "verdictDate"),
    ("收到起诉状时间", "complaintReceivedDate"),
    ("提交答辩状期间", "defenseDeadline"),
    ("预估审限", "estimatedTrialEnd"),
    ("裁定中止日", "stayDate"),
    ("立案", "filingDate"),
    ("请求人首次无效时间", "petitionerFirstInvalid"),
    ("请求人补充意见期限", "petitionerSuppDeadline"),
    ("请求人提交补充意见时间", "petitionerSubmitDate"),
    ("请求人收到专利权人意见时间", "petitionerReceivedDate"),
    ("请求人答复意见期限", "petitionerReplyDeadline"),
    ("专利权人收到受通时间", "patenteeReceivedDate"),
    ("专利权人陈述意见期限", "patenteeStatementDeadline"),
    ("专利权人收到补充意见时间", "patenteeReceivedSuppDate"),
    ("专利权人补充意见时间", "patenteeSuppDeadline"),
    ("专利权人提交补充意见时间", "patenteeSubmitSuppDate"),
    ("诉讼请求", "claims"),
    ("标的额", "caseAmount"),
    ("律师费用", "legalFees"),
    ("律师费到账情况", "feePayment"),
];

fn values(table: &Value, record: &Value, options: &HashMap<String, String>) -> Value {
    Value::Object(
        array(&table["fields"])
            .iter()
            .map(|f| {
                let name = s(f, "field_name");
                (
                    name.to_string(),
                    json!(field_display(&record["fields"][name], f, options)),
                )
            })
            .collect(),
    )
}

fn case_data(fields: &Value, case_id: &str) -> Result<crate::db::cases::Case> {
    let mut data = json!({"id":case_id,"clientName":"","opponentName":""});
    for (header, value) in fields.as_object().context("案件字段格式错误")? {
        if [
            "事件记录",
            "未来开庭",
            "最近已开庭",
            "关联案件",
            "开庭记录",
            "官方人员联系方式",
        ]
        .contains(&header.as_str())
        {
            continue;
        }
        let key = CASE_FIELDS
            .iter()
            .find(|(name, _)| *name == header)
            .map(|(_, key)| key.to_string())
            .or_else(|| match_field_confidence(header).0);
        if let Some(key) = key {
            if !display(value).is_empty() {
                data[&key] = value.clone();
            }
        }
    }
    if s(&data, "caseName").is_empty() {
        bail!("案件名称为空");
    }
    let (track, _, route) = infer_track_and_route(
        None,
        None,
        None,
        s(&data, "caseName"),
        data["causeAction"].as_str(),
    );
    data["track"] = json!(track);
    data["caseRoute"] = json!(route);
    let progress = match s(&data, "caseProgress") {
        "待立案" => "intake",
        "待提无效" => "preparing",
        "待开庭" => "pre_hearing",
        "待口审" => "pre_oral",
        "待判决" => "awaiting_verdict",
        "待无效决定" => "awaiting_decision",
        "中止" => "suspended",
        "结案" | "胜诉" | "败诉" | "对方撤案" => {
            if track == "patent_invalidation" {
                "decision_issued"
            } else {
                "closed"
            }
        }
        other => other,
    }
    .to_string();
    match track.as_str() {
        "civil_tort" => data["civilStatus"] = json!(clean_civil_status(Some(&progress))),
        "patent_invalidation" => {
            data["invalidationStatus"] = json!(clean_invalidation_status(Some(&progress)))
        }
        "admin_litigation" => data["adminStatus"] = json!(clean_admin_status(Some(&progress))),
        _ => {}
    }
    if !["一审", "二审", "再审", "结案"].contains(&s(&data, "caseLevel")) {
        data["caseLevel"] = Value::Null;
    }
    if !["普通", "简易"].contains(&s(&data, "procedureType")) {
        data["procedureType"] = Value::Null;
    }
    if let Some(third) = fields["第三人"].as_str().filter(|v| !v.is_empty()) {
        data["thirdParties"] = json!(
            json!([{"name":third,"role":"第三人","agent":"","firm":"","contact":""}]).to_string()
        );
    }
    Ok(serde_json::from_value(data)?)
}

/// Offline-only projection. Source rows and edges retain their original IDs and full JSON.
/// Re-import skips existing source rows, preserving subsequent local edits.
pub fn import_snapshot(
    conn: &mut Connection,
    snapshot: &Value,
    case_table_id: &str,
    selected: &[String],
) -> Result<SnapshotReport> {
    let source = s(snapshot, "appToken");
    if source.is_empty() || array(&snapshot["tables"]).is_empty() {
        bail!("无效的飞书快照");
    }
    let tables = array(&snapshot["tables"]);
    let primary = tables
        .iter()
        .find(|t| s(t, "table_id") == case_table_id)
        .context("未找到案件主表")?;
    let mut options = HashMap::new();
    for table in tables {
        collect_options(&table["fields"], &mut options);
    }
    let mut report = SnapshotReport {
        files: 0,
        assets: 0,
        cases: 0,
        logs: 0,
        hearings: 0,
        tasks: 0,
        officials: 0,
        relations: 0,
        source_records: 0,
        source_links: 0,
        skipped: 0,
        warnings: vec![],
    };
    let mut chosen: HashSet<String> = if selected.is_empty() {
        array(&primary["records"])
            .iter()
            .map(|r| s(r, "record_id").into())
            .collect()
    } else {
        selected.iter().cloned().collect()
    };
    let available: HashSet<String> = array(&primary["records"])
        .iter()
        .map(|r| s(r, "record_id").into())
        .collect();
    if !chosen.is_subset(&available) {
        bail!("选中的案件不在快照中");
    }
    // Expand only explicit case-to-case references, never infer relationships from similar names.
    loop {
        let before = chosen.len();
        for record in array(&primary["records"]) {
            for field in array(&primary["fields"]) {
                if [18, 21].contains(&field["type"].as_i64().unwrap_or(0))
                    && s(&field["property"], "table_id") == case_table_id
                {
                    let mut links = vec![];
                    refs(
                        &record["fields"][s(field, "field_name")],
                        case_table_id,
                        &mut links,
                    );
                    for (_, target) in links {
                        if available.contains(&target)
                            && (chosen.contains(s(record, "record_id")) || chosen.contains(&target))
                        {
                            chosen.insert(s(record, "record_id").to_string());
                            chosen.insert(target);
                        }
                    }
                }
            }
        }
        if chosen.len() == before {
            break;
        }
    }
    let tx = conn.transaction()?;
    let mut all_links = Vec::new();
    for table in tables {
        let table_id = s(table, "table_id");
        tx.execute("INSERT INTO imported_tables (source,table_id,name,fields_json) VALUES (?1,?2,?3,?4) ON CONFLICT(source,table_id) DO UPDATE SET name=excluded.name,fields_json=excluded.fields_json",
            params![source,table_id,s(table,"name"),table["fields"].to_string()])?;
        for record in array(&table["records"]) {
            let record_id = s(record, "record_id");
            if record_id.is_empty() {
                bail!("快照记录缺少 ID");
            }
            for field in array(&table["fields"]) {
                if ![18, 21].contains(&field["type"].as_i64().unwrap_or(0)) {
                    continue;
                }
                let name = s(field, "field_name");
                let mut links = vec![];
                refs(
                    &record["fields"][name],
                    s(&field["property"], "table_id"),
                    &mut links,
                );
                for (target_table, target_record) in links {
                    all_links.push((
                        table_id.to_string(),
                        record_id.to_string(),
                        name.to_string(),
                        target_table,
                        target_record,
                    ));
                }
            }
        }
    }
    // Retain the complete snapshot even for a selected-case import; projection remains scoped.
    for table in tables {
        let table_id = s(table, "table_id");
        for record in array(&table["records"]) {
            report.source_records += tx.execute("INSERT OR IGNORE INTO imported_records (source,table_id,record_id,fields_json,display_json) VALUES (?1,?2,?3,?4,?5)",
                params![source,table_id,s(record,"record_id"),record["fields"].to_string(),values(table,record,&options).to_string()])?;
        }
    }
    for (table, record, field, target_table, target) in &all_links {
        report.source_links += tx.execute("INSERT OR IGNORE INTO imported_links (source,table_id,record_id,field_name,target_table_id,target_record_id) VALUES (?1,?2,?3,?4,?5,?6)",
            params![source,table,record,field,target_table,target])?;
        let found = tables
            .iter()
            .find(|t| s(t, "table_id") == target_table)
            .is_some_and(|t| {
                array(&t["records"])
                    .iter()
                    .any(|r| s(r, "record_id") == target)
            });
        if !found {
            report.warnings.push(format!(
                "{table}/{record}/{field}: 引用目标 {target_table}/{target} 不在快照中"
            ));
        }
    }
    for record in array(&primary["records"]) {
        let record_id = s(record, "record_id");
        if !chosen.contains(record_id) {
            continue;
        }
        let case_id = id(source, case_table_id, record_id);
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM cases WHERE id=?1)",
            params![case_id],
            |r| r.get(0),
        )?;
        if exists {
            report.skipped += 1;
        } else {
            let case = case_data(&values(primary, record, &options), &case_id)?;
            crate::db::cases::insert_case(&tx, &case)?;
            if let Some(status) = case.case_status {
                tx.execute(
                    "UPDATE cases SET case_status=?1 WHERE id=?2",
                    params![status, case_id],
                )?;
            }
            if !case.client_name.is_empty() {
                tx.execute(
                    "INSERT OR IGNORE INTO clients (id,name) VALUES (?1,?2)",
                    params![crate::db::new_id(), case.client_name],
                )?;
            }
            report.cases += 1;
        }
        tx.execute("INSERT OR IGNORE INTO imported_case_records (case_id,source,table_id,record_id) VALUES (?1,?2,?3,?4)",params![case_id,source,case_table_id,record_id])?;
    }
    for (table, record, _, target_table, target) in &all_links {
        if table == case_table_id
            && target_table == case_table_id
            && record != target
            && chosen.contains(record)
            && chosen.contains(target)
        {
            let a = id(source, table, record);
            let b = id(source, target_table, target);
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            report.relations += tx.execute("INSERT OR IGNORE INTO case_relations (id,source_case_id,target_case_id,relation_type,label) VALUES (?1,?2,?3,'cross_reference','飞书关联案件')",
                params![format!("{a}:{b}"),a,b])?;
        }
    }
    for table in tables.iter().filter(|t| s(t, "table_id") != case_table_id) {
        let table_id = s(table, "table_id");
        for record in array(&table["records"]) {
            let record_id = s(record, "record_id");
            let mut cases = HashSet::new();
            for (from_table, from_record, _, to_table, to_record) in &all_links {
                if from_table == table_id
                    && from_record == record_id
                    && to_table == case_table_id
                    && chosen.contains(to_record)
                {
                    cases.insert(id(source, to_table, to_record));
                }
                if to_table == table_id
                    && to_record == record_id
                    && from_table == case_table_id
                    && chosen.contains(from_record)
                {
                    cases.insert(id(source, from_table, from_record));
                }
            }
            let fields = values(table, record, &options);
            let entity = s(table, "name");
            if cases.is_empty() && !selected.is_empty() {
                continue;
            }
            if cases.is_empty() && entity != "任务管理" && entity != "官方人员联系方式"
            {
                report.warnings.push(format!(
                    "{table_id}/{record_id}: 未关联案件，已保留在本地原始记录"
                ));
                continue;
            }
            let mut targets: Vec<Option<String>> = if cases.is_empty() {
                vec![None]
            } else {
                cases.into_iter().map(Some).collect()
            };
            targets.sort();
            for case_id in targets {
                let base_id = id(source, table_id, record_id);
                let local_id = base_id.clone();
                let count = project_child(
                    &tx,
                    entity,
                    &local_id,
                    case_id.as_deref(),
                    &fields,
                    &record["fields"],
                )?;
                match entity {
                    "办案日志" => report.logs += count,
                    "庭审信息" => report.hearings += count,
                    "任务管理" => report.tasks += count,
                    "官方人员联系方式" => report.officials += count,
                    _ => report
                        .warnings
                        .push(format!("{table_id}: 未识别子表，已保留原始记录")),
                }
                if let Some(case_id) = &case_id {
                    let link = match entity {
                        "办案日志" => Some(("case_log_links", "log_id")),
                        "庭审信息" => Some(("case_hearing_links", "hearing_id")),
                        "任务管理" => Some(("case_task_links", "task_id")),
                        _ => None,
                    };
                    if let Some((table, column)) = link {
                        tx.execute(
                            &format!(
                                "INSERT OR IGNORE INTO {table} (case_id,{column}) VALUES (?1,?2)"
                            ),
                            params![case_id, local_id],
                        )?;
                    }
                    tx.execute("INSERT OR IGNORE INTO imported_case_records (case_id,source,table_id,record_id) VALUES (?1,?2,?3,?4)",params![case_id,source,table_id,record_id])?;
                }
            }
        }
    }
    use base64::Engine;
    for asset in array(&snapshot["assets"]) {
        if let Some(encoded) = asset["contentBase64"].as_str() {
            let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
            if let Some(size) = asset["size"].as_u64() {
                if size != bytes.len() as u64 {
                    bail!("附件字节数不一致: {}", s(asset, "fileToken"));
                }
            }
            report.assets += tx.execute("INSERT OR IGNORE INTO imported_assets (source,file_token,name,mime_type,content) VALUES (?1,?2,?3,?4,?5)",
                params![source,s(asset,"fileToken"),s(asset,"name"),s(asset,"mimeType"),bytes])?;
        } else {
            report.warnings.push(format!(
                "附件 {} 尚未下载: {}",
                s(asset, "name"),
                s(asset, "error")
            ));
        }
    }
    let mut missing_assets = HashSet::new();
    for table in tables {
        for record in array(&table["records"]) {
            for field in array(&table["fields"]).iter().filter(|f| f["type"] == 17) {
                for asset in array(&record["fields"][s(field, "field_name")]) {
                    let token = s(asset, "file_token");
                    let found:bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM imported_assets WHERE source=?1 AND file_token=?2)",params![source,token],|r|r.get(0))?;
                    if !found && missing_assets.insert(token.to_string()) {
                        report.warnings.push(format!(
                            "附件 {} 只有元数据，本地尚无文件内容",
                            s(asset, "name")
                        ));
                    }
                }
            }
        }
    }
    tx.commit()?;
    Ok(report)
}

fn project_child(
    conn: &Connection,
    entity: &str,
    id: &str,
    case: Option<&str>,
    f: &Value,
    raw: &Value,
) -> Result<usize> {
    let v = |key| s(f, key);
    let date = |key| {
        let value = v(key);
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    };
    Ok(match entity {
        "办案日志" => {
            let kind = match v("类型") {
                "任务" => "task",
                "交文" => "submitted",
                "收文" => "received",
                _ => "record",
            };
            conn.execute("INSERT INTO case_logs (id,case_id,event_summary,event_name,event_type,event_date,content,files_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO NOTHING",
                params![id,case,v("事件概述"),v("事件名称"),kind,date("发生时间").context("日志缺少发生时间")?,v("操作内容"),raw["附件"].to_string()])?
        }
        "庭审信息" => {
            let status = match v("实际开庭情况") {
                "已开" => Some("已开"),
                "未开" => Some("未开"),
                _ => None,
            };
            conn.execute("INSERT INTO hearings (id,case_id,hearing_record,hearing_name,hearing_date,venue,attendees,judges,court,case_level,contact_info,actual_status,files_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(id) DO NOTHING",
                params![id,case,v("开庭记录"),v("开庭名称"),date("开庭时间").context("庭审缺少开庭时间")?,v("开庭地点"),v("出庭人员"),json!(v("审判人员").split('、').filter(|s| !s.is_empty()).collect::<Vec<_>>()).to_string(),v("审理机关"),v("审级"),v("联系方式"),status,raw["附件"].to_string()])?
        }
        "任务管理" => {
            let priority = match v("优先级") {
                "重要紧急" | "重要且紧急" | "高" => "urgent_important",
                "重要不紧急" | "重要" => "important",
                "紧急不重要" | "紧急" => "urgent",
                _ => "normal",
            };
            let due = date("截止日期").and_then(clean_date_str);
            conn.execute("INSERT INTO tasks (id,case_id,task_name,description,created_date,deadline,due_date,priority,completed,assignee,finish_note) VALUES (?1,?2,?3,?4,?5,?6,?6,?7,?8,?9,?10) ON CONFLICT(id) DO NOTHING",
                params![id,case,v("任务名称"),v("任务详细描述"),date("创建日期").context("任务缺少创建日期")?,due,priority,raw["完成状态"].as_bool().unwrap_or(false) as i32,v("任务执行人"),v("完结记录")])?
        }
        "官方人员联系方式" => {
            let role = match v("身份") {
                "法官" => "法官",
                "书记员" => "书记员",
                "法院" => "法院",
                _ => "法官助理",
            };
            let count = conn.execute("INSERT INTO officials (id,name,role,court,contact_detail,contact_text,contact_record) VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO NOTHING",
                params![id,v("姓名"),role,v("所属机关"),v("具体联系方式"),v("联系方式"),v("联系记录")])?;
            conn.execute("INSERT OR IGNORE INTO persons (id,kind,name,org,phone,notes) VALUES (?1,'judge',?2,?3,?4,?5)",params![id,v("姓名"),v("所属机关"),v("具体联系方式"),v("联系记录")])?;
            if let Some(case) = case {
                conn.execute(
                    "INSERT OR IGNORE INTO case_officials (case_id,official_id) VALUES (?1,?2)",
                    params![case, id],
                )?;
                conn.execute("INSERT OR IGNORE INTO case_persons (id,case_id,person_id,role) VALUES (?1,?2,?3,?4)",params![format!("{id}:{case}"),case,id,role])?;
            }
            count
        }
        _ => 0,
    })
}

#[tauri::command]
pub async fn feishu_download_snapshot(url_or_token: String) -> Result<Value, String> {
    let source = super::import_feishu::extract_feishu_tokens(&url_or_token)
        .0
        .ok_or("无法识别多维表格链接")?;
    let (client, token) = super::import_feishu::get_feishu_client_and_token()
        .await
        .map_err(|e| e.to_string())?;
    async fn pages(
        client: &reqwest::Client,
        token: &str,
        source: &str,
        path: &str,
    ) -> Result<Vec<Value>> {
        let mut items = vec![];
        let mut page = String::new();
        let mut seen = HashSet::new();
        loop {
            let mut url = reqwest::Url::parse(&format!(
                "https://open.feishu.cn/open-apis/bitable/v1/apps/{source}/{path}"
            ))?;
            url.query_pairs_mut().append_pair("page_size", "100");
            if !page.is_empty() {
                url.query_pairs_mut().append_pair("page_token", &page);
            }
            let body: Value = client
                .get(url)
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if body["code"] != 0 {
                bail!("飞书读取失败 [{}]: {}", body["code"], s(&body, "msg"));
            }
            items.extend(array(&body["data"]["items"]).iter().cloned());
            if body["data"]["has_more"] != true {
                break;
            }
            page = s(&body["data"], "page_token").into();
            if page.is_empty() || !seen.insert(page.clone()) {
                bail!("飞书分页标记无效");
            }
        }
        Ok(items)
    }
    let mut tables = pages(&client, &token, &source, "tables")
        .await
        .map_err(|e| e.to_string())?;
    for table in &mut tables {
        let table_id = s(table, "table_id").to_string();
        table["fields"] = json!(pages(
            &client,
            &token,
            &source,
            &format!("tables/{table_id}/fields")
        )
        .await
        .map_err(|e| e.to_string())?);
        table["records"] = json!(pages(
            &client,
            &token,
            &source,
            &format!("tables/{table_id}/records")
        )
        .await
        .map_err(|e| e.to_string())?);
    }
    let mut assets = vec![];
    let mut seen_files = HashSet::new();
    use base64::Engine;
    for table in &tables {
        for record in array(&table["records"]) {
            for field in array(&table["fields"]).iter().filter(|f| f["type"] == 17) {
                for file in array(&record["fields"][s(field, "field_name")]) {
                    let file_token = s(file, "file_token");
                    if file_token.is_empty() || !seen_files.insert(file_token.to_string()) {
                        continue;
                    }
                    let mut asset = json!({"fileToken":file_token,"name":file["name"],"size":file["size"],"mimeType":file["type"]});
                    let downloaded: Result<Vec<u8>> = async {
                        let mut url = reqwest::Url::parse(
                            "https://open.feishu.cn/open-apis/drive/v1/medias/",
                        )?;
                        url.path_segments_mut()
                            .unwrap()
                            .pop_if_empty()
                            .push(file_token)
                            .push("download");
                        let response = client
                            .get(url)
                            .bearer_auth(&token)
                            .send()
                            .await?
                            .error_for_status()?;
                        let bytes = response.bytes().await?;
                        if file["size"]
                            .as_u64()
                            .is_some_and(|size| size != bytes.len() as u64)
                        {
                            bail!("附件字节数不一致");
                        }
                        Ok(bytes.to_vec())
                    }
                    .await;
                    match downloaded {
                        Ok(bytes) => {
                            asset["contentBase64"] =
                                json!(base64::engine::general_purpose::STANDARD.encode(bytes))
                        }
                        Err(error) => asset["error"] = json!(error.to_string()),
                    }
                    assets.push(asset);
                }
            }
        }
    }
    Ok(
        json!({"version":1,"appToken":source,"fetchedAt":chrono::Utc::now().to_rfc3339(),"tables":tables,"assets":assets}),
    )
}

#[tauri::command]
pub async fn feishu_import_snapshot(
    snapshot: Value,
    case_table_id: String,
    selected_record_ids: Vec<String>,
) -> Result<SnapshotReport, String> {
    super::run_blocking(move || {
        let mut conn = crate::db::open_db()?;
        let mut report =
            import_snapshot(&mut conn, &snapshot, &case_table_id, &selected_record_ids)?;
        materialize_assets(&conn, s(&snapshot, "appToken"), &mut report)?;
        Ok(report)
    })
    .await
}

fn materialize_assets(conn: &Connection, source: &str, report: &mut SnapshotReport) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let rows: Vec<(String,String,String,String)> = conn.prepare(
        "SELECT c.case_id,r.table_id,r.record_id,r.fields_json FROM imported_case_records c JOIN imported_records r USING(source,table_id,record_id) WHERE r.source=?1")?
        .query_map(params![source],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (case_id, table_id, record_id, raw) in rows {
        let fields: Value = serde_json::from_str(&raw)?;
        let mut attached = vec![];
        for file in fields
            .as_object()
            .into_iter()
            .flat_map(|o| o.values())
            .flat_map(array)
            .filter(|v| v.get("file_token").is_some())
        {
            let token = s(file, "file_token");
            let imported_id = format!("feishu-file:{case_id}:{token}");
            let result: Result<(String, usize)> = (|| {
                let existing: Option<String> = conn
                    .query_row(
                        "SELECT file_path FROM case_files WHERE id=?1",
                        params![imported_id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(path) = existing {
                    return Ok((path, 0));
                }
                let bytes: Vec<u8> = conn.query_row(
                    "SELECT content FROM imported_assets WHERE source=?1 AND file_token=?2",
                    params![source, token],
                    |r| r.get(0),
                )?;
                let case = crate::db::cases::get_case(conn, &case_id)?;
                let root = crate::files::ensure_case_folder(&case)?.canonicalize()?;
                let base = crate::files::case_folder_base().canonicalize()?;
                if !root.starts_with(base) {
                    bail!("案件目录不在本地卷宗根目录内");
                }
                let folder = root.join("飞书附件");
                std::fs::create_dir_all(&folder)?;
                let folder = folder.canonicalize()?;
                if !folder.starts_with(&root) {
                    bail!("附件目录不在案件内");
                }
                let extension = std::path::Path::new(s(file, "name"))
                    .extension()
                    .and_then(|v| v.to_str())
                    .unwrap_or("bin");
                let extension = if extension.chars().all(|c| c.is_ascii_alphanumeric()) {
                    extension
                } else {
                    "bin"
                };
                let path = folder.join(format!(
                    "{}.{}",
                    hex::encode(Sha256::digest(token.as_bytes())),
                    extension
                ));
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                {
                    Ok(mut output) => output.write_all(&bytes)?,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if std::fs::symlink_metadata(&path)?.file_type().is_symlink()
                            || std::fs::read(&path)? != bytes
                        {
                            bail!("附件路径已被不同内容占用");
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
                let path = path.to_string_lossy().to_string();
                let count = conn.execute("INSERT INTO case_files (id,case_id,file_name,file_path,file_size,file_type,category,source_type) VALUES (?1,?2,?3,?4,?5,?6,'other','imported') ON CONFLICT(id) DO NOTHING",
                    params![imported_id,case_id,s(file,"name"),path,bytes.len() as i64,extension])?;
                conn.execute(
                    "UPDATE cases SET folder_path=?1 WHERE id=?2 AND folder_path IS NULL",
                    params![root.to_string_lossy(), case_id],
                )?;
                Ok((path, count))
            })();
            match result {
                Ok((path, count)) => {
                    report.files += count;
                    attached.push(json!({"name":file["name"],"path":path,"size":file["size"]}));
                }
                Err(error) => report.warnings.push(format!(
                    "附件 {} 卷宗登记失败（原始记录已保留）: {error}",
                    s(file, "name")
                )),
            }
        }
        if !attached.is_empty() {
            for table in ["case_logs", "hearings"] {
                conn.execute(
                    &format!("UPDATE {table} SET files_json=?1 WHERE id=?2"),
                    params![
                        json!(attached).to_string(),
                        id(source, &table_id, &record_id)
                    ],
                )?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_case_source_records(case_id: String) -> Result<Value, String> {
    super::run_blocking(move || {
        let conn = crate::db::open_db()?;
        let mut stmt = conn.prepare("SELECT t.name,r.record_id,r.display_json,r.fields_json,t.fields_json,r.source FROM imported_case_records c JOIN imported_records r USING(source,table_id,record_id) JOIN imported_tables t USING(source,table_id) WHERE c.case_id=?1 ORDER BY t.name,r.record_id")?;
        let rows = stmt.query_map(params![case_id],|r| Ok(json!({"tableName":r.get::<_,String>(0)?,"recordId":r.get::<_,String>(1)?,"fields":serde_json::from_str::<Value>(&r.get::<_,String>(2)?).unwrap_or(Value::Null),"raw":serde_json::from_str::<Value>(&r.get::<_,String>(3)?).unwrap_or(Value::Null),"schema":serde_json::from_str::<Value>(&r.get::<_,String>(4)?).unwrap_or(Value::Null),"source":r.get::<_,String>(5)?})))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(json!(rows))
    }).await
}

#[tauri::command]
pub async fn get_imported_asset(source: String, file_token: String) -> Result<Value, String> {
    super::run_blocking(move || {
        use base64::Engine;
        let conn = crate::db::open_db()?;
        let (name,mime,bytes):(String,String,Vec<u8>) = conn.query_row("SELECT name,mime_type,content FROM imported_assets WHERE source=?1 AND file_token=?2",
            params![source,file_token],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        Ok(json!({"name":name,"mimeType":mime,"contentBase64":base64::engine::general_purpose::STANDARD.encode(bytes)}))
    }).await
}

#[tauri::command]
pub async fn export_imported_asset(
    source: String,
    file_token: String,
    output_path: String,
) -> Result<(), String> {
    super::run_blocking(move || {
        let conn = crate::db::open_db()?;
        let bytes: Vec<u8> = conn.query_row(
            "SELECT content FROM imported_assets WHERE source=?1 AND file_token=?2",
            params![source, file_token],
            |r| r.get(0),
        )?;
        let path = crate::docsy_engine::output_path::resolve_explicit_output_path(
            &output_path,
            &[
                "pdf", "png", "jpg", "jpeg", "webp", "gif", "doc", "docx", "xls", "xlsx", "ppt",
                "pptx", "txt", "zip",
            ],
        )?;
        std::fs::write(path, bytes)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn save_feishu_snapshot(snapshot: Value, output_path: String) -> Result<(), String> {
    super::run_blocking(move || {
        let path = crate::docsy_engine::output_path::resolve_explicit_output_path(
            &output_path,
            &["json"],
        )?;
        std::fs::write(path, serde_json::to_vec_pretty(&snapshot)?)?;
        Ok(())
    })
    .await
}

use anyhow::Result;
use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;

use super::holidays::HolidayCalendar;
use crate::db;

#[derive(Debug, Serialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineResult {
    pub rule_id: Option<String>,
    pub rule_name: String,
    pub due_date: String,
    pub days_left: i64,
    pub urgency: String,
    pub deadline_source: String,
    pub legal_basis: Option<String>,
    pub case_id: String,
    pub case_name: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineRule {
    pub id: String,
    pub track: String,
    pub rule_name: String,
    pub legal_basis: String,
    pub trigger_field: String,
    pub offset_value: i64,
    pub offset_unit: String,
    pub calc_method: String,
    pub procedure_types: Option<String>,
    pub deadline_source: String,
    pub auto_calculate: bool,
    pub priority: i32,
}

pub struct DeadlineEngine {
    rules: Vec<DeadlineRule>,
    calendar: HolidayCalendar,
}

impl DeadlineEngine {
    pub fn new(conn: &Connection) -> Result<Self> {
        let mut stmt = conn.prepare(
            "SELECT id, track, rule_name, legal_basis, trigger_field, offset_value,
             offset_unit, calc_method, procedure_types, deadline_source, auto_calculate, priority
             FROM deadline_rules ORDER BY priority DESC",
        )?;
        let rules = stmt
            .query_map([], |row| {
                Ok(DeadlineRule {
                    id: row.get(0)?,
                    track: row.get(1)?,
                    rule_name: row.get(2)?,
                    legal_basis: row.get(3)?,
                    trigger_field: row.get(4)?,
                    offset_value: row.get(5)?,
                    offset_unit: row.get(6)?,
                    calc_method: row.get(7)?,
                    procedure_types: row.get(8)?,
                    deadline_source: row.get(9)?,
                    auto_calculate: row.get::<_, i32>(10)? != 0,
                    priority: row.get(11)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let calendar = match db::get_setting(conn, "holidays_json")? {
            Some(value) => HolidayCalendar::from_json_str(&value).map_err(anyhow::Error::msg)?,
            None => HolidayCalendar::builtin(),
        };
        Ok(Self { rules, calendar })
    }

    /// 计算单个案件的所有期限
    pub fn evaluate_case(&self, conn: &Connection, case: &db::cases::Case) -> Vec<DeadlineResult> {
        let today = Local::now().naive_local().date();
        let mut results = Vec::new();

        // 1. 法定期限自动计算
        for rule in &self.rules {
            if rule.track != case.track {
                continue;
            }
            if rule.deadline_source=="recommended" && case.stay_date.is_some(){continue;}
            if !rule.auto_calculate {
                continue;
            }

            // These legacy seeds cannot express service, party or notice conditions.
            // Their fields remain visible as unconfirmed projections in the procedure board.
            if matches!(rule.id.as_str(), "rule-pi-001"|"rule-pi-002"|"rule-pi-003"|"rule-pi-004"|"rule-al-001"|"rule-al-004"|"rule-al-005"|"rule-ct-001"|"rule-ct-004"|"rule-ct-005") { continue; }
            if rule.offset_value < 1 || rule.offset_value > 3650 || (rule.offset_unit == "calendar_month" && rule.offset_value > 120) { continue; }
            // 检查适用程序
            if let Some(proc_types) = &rule.procedure_types {
                match serde_json::from_str::<serde_json::Value>(proc_types) {
                    Ok(serde_json::Value::Array(types)) => {
                        if !case.procedure_type.as_ref().is_some_and(|p| types.iter().any(|t|t.as_str()==Some(p.as_str()))) { continue; }
                    }
                    Ok(serde_json::Value::Object(condition)) => {
                        if condition.len()!=1 || !condition.get("verdict_type").is_some_and(|t| t.as_str()==case.verdict_type.as_deref() && t.is_string()) {continue;}
                    }
                    _ => continue,
                }
            }

            // 获取触发日期
            let Some(trigger_str) = get_case_date_field(case, &rule.trigger_field) else {
                continue;
            };
            let Ok(trigger) = NaiveDate::parse_from_str(&trigger_str, "%Y-%m-%d") else {
                continue;
            };

            // 根据 calc_method 选择算法
            let due = match rule.calc_method.as_str() {
                "patent" => match rule.offset_unit.as_str() {
                    "calendar_month" => self
                        .calendar
                        .add_months_patent(trigger, rule.offset_value as u32),
                    "day" => self.calendar.add_days_patent(trigger, rule.offset_value),
                    _ => continue,
                },
                // "civil" 与其余轨道共用默认偏移规则（行为保留）
                _ => match rule.offset_unit.as_str() {
                    "calendar_month" => self
                        .calendar
                        .add_months_civil(trigger, rule.offset_value as u32),
                    "day" => self.calendar.add_days_civil(trigger, rule.offset_value),
                    _ => continue,
                },
            };

            let days_left = (due - today).num_days();
            results.push(DeadlineResult {
                rule_id: Some(rule.id.clone()),
                rule_name: rule.rule_name.clone(),
                due_date: due.format("%Y-%m-%d").to_string(),
                days_left,
                urgency: classify_urgency(days_left),
                deadline_source: rule.deadline_source.clone(),
                legal_basis: Some(rule.legal_basis.clone()),
                case_id: case.id.clone(),
                case_name: case.case_name.clone(),
            });
        }

        // 2. 手动录入的期限
        if let Ok(manual) = query_case_deadlines(conn, &case.id) {
            for dl in manual {
                if dl.completed {
                    continue;
                }
                if let Ok(due) = NaiveDate::parse_from_str(&dl.due_date, "%Y-%m-%d") {
                    let days_left = (due - today).num_days();
                    results.push(DeadlineResult {
                        rule_id: dl.rule_id,
                        rule_name: dl.deadline_name,
                        due_date: due.format("%Y-%m-%d").to_string(),
                        days_left,
                        urgency: classify_urgency(days_left),
                        deadline_source: dl.deadline_source,
                        legal_basis: dl.legal_basis,
                        case_id: case.id.clone(),
                        case_name: case.case_name.clone(),
                    });
                }
            }
        }

        match super::procedure::case_items(conn, case) {
            Ok((_,items)) => for i in items.into_iter().filter(|i| i.status=="open") {
                if let Some(due_date)=i.due_on {
                    let days_left=i.days_left.unwrap_or(0);
                    let prefix=if i.needs_review { "[待核对] " } else if i.owner=="opponent" { "[对方] " } else if i.source=="internal" { "[内部] " } else { "" };
                    results.push(DeadlineResult{rule_id:Some(i.id),rule_name:format!("{}{} · {}",prefix,i.actor_role,i.title),due_date,days_left,urgency:classify_urgency(days_left),deadline_source:i.source,legal_basis:Some(format!("{}；{}",i.legal_basis,i.explanation)),case_id:case.id.clone(),case_name:case.case_name.clone()});
                }
            },
            Err(err)=>results.push(DeadlineResult{rule_id:None,rule_name:"程序期限读取失败，请检查案件程序面板".into(),due_date:today.to_string(),days_left:0,urgency:"red".into(),deadline_source:"unconfirmed".into(),legal_basis:Some(err.to_string()),case_id:case.id.clone(),case_name:case.case_name.clone()}),
        }
        results.sort_by(|a, b| a.due_date.cmp(&b.due_date));
        results
    }

    /// 计算所有活跃案件的期限预警
    pub fn generate_all_warnings(&self, conn: &Connection) -> Result<Vec<DeadlineResult>> {
        let cases = db::cases::active_cases(conn)?;
        let mut all = Vec::new();
        for case in cases {
            all.extend(self.evaluate_case(conn, &case));
        }
        all.sort_by_key(|r| r.days_left);
        Ok(all)
    }
}

fn classify_urgency(days_left: i64) -> String {
    if days_left <= 3 {
        "red".to_string()
    } else if days_left <= 14 {
        "yellow".to_string()
    } else {
        "green".to_string()
    }
}

fn get_case_date_field(case: &db::cases::Case, field: &str) -> Option<String> {
    match field {
        "filing_date" => case.filing_date.clone(),
        "complaint_received_date" => case.complaint_received_date.clone(),
        "trial_date" => case.trial_date.clone(),
        "trial2_date" => case.trial2_date.clone(),
        "trial3_date" => case.trial3_date.clone(),
        "verdict_date" => case.verdict_date.clone(),
        "stay_date" => case.stay_date.clone(),
        "relief_deadline" => case.relief_deadline.clone(),
        "petitioner_first_invalid" => case.petitioner_first_invalid.clone(),
        "petitioner_submit_date" => case.petitioner_submit_date.clone(),
        "petitioner_received_date" => case.petitioner_received_date.clone(),
        "patentee_received_date" => case.patentee_received_date.clone(),
        "patentee_received_supp_date" => case.patentee_received_supp_date.clone(),
        _ => None,
    }
}

struct CaseDeadline {
    rule_id: Option<String>,
    deadline_name: String,
    due_date: String,
    deadline_source: String,
    legal_basis: Option<String>,
    completed: bool,
}

fn query_case_deadlines(conn: &Connection, case_id: &str) -> Result<Vec<CaseDeadline>> {
    let mut stmt = conn.prepare(
        "SELECT rule_id, deadline_name, due_date, deadline_source, legal_basis, completed
         FROM case_deadlines WHERE case_id = ?1",
    )?;
    let rows = stmt
        .query_map([case_id], |row| {
            Ok(CaseDeadline {
                rule_id: row.get(0)?,
                deadline_name: row.get(1)?,
                due_date: row.get(2)?,
                deadline_source: row.get(3)?,
                legal_basis: row.get(4)?,
                completed: row.get::<_, i32>(5)? != 0,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

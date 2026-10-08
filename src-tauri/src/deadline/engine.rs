use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};
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
    /// D4/P1-24：旧种子规则名单（数据驱动，用户编辑过的规则不再跳过）
    legacy_seed_rule_ids: std::collections::HashSet<String>,
    /// D4/P1-24：用户编辑过的规则 id（deadline_rule_audit action='update'）
    edited_rule_ids: std::collections::HashSet<String>,
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
        Ok(Self {
            rules,
            calendar,
            legacy_seed_rule_ids: legacy_seed_rule_ids(conn)?,
            edited_rule_ids: edited_rule_ids(conn)?,
        })
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
            // D4/P1-24：跳过改为数据驱动——名单内的规则一旦被用户编辑过（deadline_rule_audit
            // action='update'）就参与计算，否则 deadline_rules 的编辑"成功但不生效"。
            if self.legacy_seed_rule_ids.contains(&rule.id)
                && !self.edited_rule_ids.contains(&rule.id)
            {
                continue;
            }
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
            // C4/P0-6b：期限落在日历未覆盖的年份时，顺延实际只会跳周末，结果不可信。
            // 与 procedure.rs 的降级路径保持一致：标记 deadline_source 并加 [待核对] 前缀，
            // 不让 2027 年后的期限静默按正常紧急度弹出。
            let unconfirmed_calendar = !self.calendar.covers_year(due.year());
            results.push(DeadlineResult {
                rule_id: Some(rule.id.clone()),
                rule_name: if unconfirmed_calendar {
                    format!("[待核对] {}", rule.rule_name)
                } else {
                    rule.rule_name.clone()
                },
                due_date: due.format("%Y-%m-%d").to_string(),
                days_left,
                urgency: classify_urgency(days_left),
                deadline_source: if unconfirmed_calendar {
                    "unconfirmed_calendar".to_string()
                } else {
                    rule.deadline_source.clone()
                },
                legal_basis: Some(if unconfirmed_calendar {
                    format!(
                        "[待核对] {}（该年份节假日未覆盖，请导入官方日历后核对）",
                        rule.legal_basis
                    )
                } else {
                    rule.legal_basis.clone()
                }),
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

/// 旧种子规则名单的设置键（D4：跳过名单数据驱动化；不改 schema.rs 就是不改迁移）
const LEGACY_SEED_SETTING_KEY: &str = "deadline_legacy_seed_rule_ids";

/// 无法表达送达／当事人／通知条件的旧版种子规则（schema.rs 的 statutory 种子）。
const LEGACY_SEED_RULE_IDS: &[&str] = &[
    "rule-pi-001",
    "rule-pi-002",
    "rule-pi-003",
    "rule-pi-004",
    "rule-al-001",
    "rule-al-004",
    "rule-al-005",
    "rule-ct-001",
    "rule-ct-004",
    "rule-ct-005",
];

/// 读取旧种子规则名单；设置缺失时按内置常量初始化后落库（首次使用即补齐）。
///
/// 设置损坏时退回内置名单并告警：名单丢只会让这些规则重新参与计算（偏保守），
/// 不会让期限消失。
fn legacy_seed_rule_ids(conn: &Connection) -> Result<std::collections::HashSet<String>> {
    let builtin = || {
        LEGACY_SEED_RULE_IDS
            .iter()
            .map(|id| id.to_string())
            .collect::<std::collections::HashSet<String>>()
    };
    let Some(raw) = db::get_setting(conn, LEGACY_SEED_SETTING_KEY)? else {
        let ids = builtin();
        // 落库失败不影响本次计算（内置常量仍是兜底），只告警
        if let Err(error) =
            db::set_setting(conn, LEGACY_SEED_SETTING_KEY, &serde_json::to_string(&ids)?)
        {
            log::warn!("写入 {LEGACY_SEED_SETTING_KEY} 失败: {error}");
        }
        return Ok(ids);
    };
    match serde_json::from_str::<Vec<String>>(&raw) {
        Ok(ids) => Ok(ids.into_iter().collect()),
        Err(error) => {
            log::warn!("设置 {LEGACY_SEED_SETTING_KEY} 损坏，改用内置旧种子名单: {error}");
            Ok(builtin())
        }
    }
}

/// 用户编辑过的规则 id（upsert_deadline_rule 对 update 动作写 deadline_rule_audit）。
fn edited_rule_ids(conn: &Connection) -> Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT rule_id FROM deadline_rule_audit WHERE action = 'update'",
    )?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ids.into_iter().collect())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::{seed_deadline_rules, SCHEMA_SQL};
    use rusqlite::params;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        seed_deadline_rules(&conn).unwrap();
        conn
    }

    fn insert_case(conn: &Connection, id: &str, track: &str, filing: &str, procedure: &str) {
        conn.execute(
            "INSERT INTO cases (id, track, case_name, client_name, opponent_name, case_status,
              filing_date, procedure_type) VALUES (?1, ?2, '测试案件', '委托人', '对方', '进行中', ?3, ?4)",
            params![id, track, filing, procedure],
        )
        .unwrap();
    }

    /// C4/P0-6b：期限落在日历未覆盖年份时必须标记为待核对，而不是按正常紧急度呈现。
    #[test]
    fn uncovered_calendar_year_is_marked_unconfirmed() {
        let conn = test_conn();
        insert_case(&conn, "c-2027", "civil_tort", "2027-01-15", "简易");

        let engine = DeadlineEngine::new(&conn).unwrap();
        let case = db::cases::get_case(&conn, "c-2027").unwrap();
        let results = engine.evaluate_case(&conn, &case);
        let result = results
            .iter()
            .find(|r| r.rule_id.as_deref() == Some("rule-ct-002"))
            .expect("简易程序预估审限应被计算");

        assert_eq!(result.deadline_source, "unconfirmed_calendar");
        assert!(result.rule_name.starts_with("[待核对]"), "{}", result.rule_name);
        assert!(result
            .legal_basis
            .as_deref()
            .is_some_and(|basis| basis.starts_with("[待核对]")));
        assert_eq!(result.due_date, "2027-04-15");
    }

    /// 已覆盖年份不加标记（2026 在内置日历内）。
    #[test]
    fn covered_calendar_year_keeps_rule_source() {
        let conn = test_conn();
        insert_case(&conn, "c-2026", "civil_tort", "2026-01-15", "简易");

        let engine = DeadlineEngine::new(&conn).unwrap();
        let case = db::cases::get_case(&conn, "c-2026").unwrap();
        let result = engine
            .evaluate_case(&conn, &case)
            .into_iter()
            .find(|r| r.rule_id.as_deref() == Some("rule-ct-002"))
            .expect("简易程序预估审限应被计算");

        assert_eq!(result.deadline_source, "recommended");
        assert!(!result.rule_name.contains("待核对"));
    }

    /// D4/P1-24：旧种子规则默认跳过；用户编辑过（audit action='update'）后必须生效。
    #[test]
    fn legacy_seed_skip_is_data_driven() {
        let conn = test_conn();
        insert_case(&conn, "c-al", "admin_litigation", "2026-01-10", "普通");
        conn.execute(
            "UPDATE cases SET complaint_received_date = '2026-02-01' WHERE id = 'c-al'",
            [],
        )
        .unwrap();

        let engine = DeadlineEngine::new(&conn).unwrap();
        let case = db::cases::get_case(&conn, "c-al").unwrap();
        assert!(
            engine
                .evaluate_case(&conn, &case)
                .iter()
                .all(|r| r.rule_id.as_deref() != Some("rule-al-001")),
            "未被编辑的旧种子规则仍应跳过"
        );

        // 名单已按内置常量落库，供用户查看/调整
        let stored = db::get_setting(&conn, LEGACY_SEED_SETTING_KEY)
            .unwrap()
            .expect("旧种子名单应在首次使用时落库");
        let ids: Vec<String> = serde_json::from_str(&stored).unwrap();
        assert_eq!(ids.len(), LEGACY_SEED_RULE_IDS.len());

        // 用户通过 deadline_rules 编辑该规则 → 写 audit → 规则参与计算
        conn.execute(
            "INSERT INTO deadline_rule_audit (id, rule_id, action, after_json, created_at)
             VALUES ('a-1', 'rule-al-001', 'update', '{}', '2026-02-02')",
            [],
        )
        .unwrap();
        let engine = DeadlineEngine::new(&conn).unwrap();
        let result = engine
            .evaluate_case(&conn, &case)
            .into_iter()
            .find(|r| r.rule_id.as_deref() == Some("rule-al-001"))
            .expect("编辑过的旧种子规则必须参与计算");
        assert_eq!(result.deadline_source, "statutory");
        // 2026-02-01 + 15 天 = 2026-02-16（春节假期内）→ 顺延到节后第一个工作日
        assert_eq!(result.due_date, "2026-02-24");
    }
}

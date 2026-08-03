pub mod ast;
pub mod dependency;
pub mod engine;
pub mod eval;
pub mod holidays;
pub mod parser;

use anyhow::Result;
use ast::Value;
use chrono::Local;
use dependency::build_casy_dependency_graph;
use eval::{FormulaEvaluator, RecordContext};
use rusqlite::Connection;

/// Parse a Feishu formula string into an AST.
pub fn parse_formula(input: &str) -> Result<ast::Expr, String> {
    parser::parse_formula(input)
}

/// Evaluate a parsed formula AST against a record context.
pub fn evaluate_formula(expr: &ast::Expr, ctx: &dyn RecordContext) -> Result<Value> {
    let evaluator = FormulaEvaluator::new();
    evaluator.evaluate(expr, ctx)
}

/// Recalculate all formula cache columns for a given case.
///
/// Reads the case data from SQLite, evaluates each formula, and writes
/// results back to the formula_ cache columns.
pub fn recalculate_case_formulas(conn: &Connection, case_id: &str) -> Result<usize> {
    let _graph = build_casy_dependency_graph();
    let evaluator = FormulaEvaluator::new();
    let _today = Local::now().naive_local().date();

    // Read current case data
    let mut stmt = conn.prepare(
        "SELECT case_progress, cause_action, complaint_received_date, filing_date,
                procedure_type, stay_date, petitioner_first_invalid,
                petitioner_received_date, patentee_received_date,
                patentee_received_supp_date
         FROM cases WHERE id = ?1",
    )?;

    let row = stmt.query_row(rusqlite::params![case_id], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
        ))
    })?;

    let (
        case_progress,
        cause_action,
        complaint_received_date,
        filing_date,
        procedure_type,
        stay_date,
        petitioner_first_invalid,
        petitioner_received_date,
        patentee_received_date,
        patentee_received_supp_date,
    ) = row;

    // Build context from case data
    let mut ctx = eval::SimpleRecordContext::new();

    if let Some(ref v) = case_progress {
        ctx.set("case_progress", Value::String(v.clone()));
    }
    if let Some(ref v) = cause_action {
        ctx.set("cause_action", Value::String(v.clone()));
    }
    if let Some(ref v) = complaint_received_date {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
            ctx.set("complaint_received_date", Value::Date(d));
        }
    }
    if let Some(ref v) = filing_date {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
            ctx.set("filing_date", Value::Date(d));
        }
    }
    if let Some(ref v) = procedure_type {
        ctx.set("procedure_type", Value::String(v.clone()));
    }
    if let Some(ref v) = stay_date {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
            ctx.set("stay_date", Value::Date(d));
        }
    }
    if let Some(ref v) = petitioner_first_invalid {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
            ctx.set("petitioner_first_invalid", Value::Date(d));
        }
    }
    if let Some(ref v) = petitioner_received_date {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
            ctx.set("petitioner_received_date", Value::Date(d));
        }
    }
    if let Some(ref v) = patentee_received_date {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
            ctx.set("patentee_received_date", Value::Date(d));
        }
    }
    if let Some(ref v) = patentee_received_supp_date {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
            ctx.set("patentee_received_supp_date", Value::Date(d));
        }
    }

    // Evaluate each formula and collect results
    let mut updates: Vec<(&str, Option<String>)> = Vec::new();

    // formula_case_status: IF(OR(progress="结案", progress="胜诉", progress="败诉", progress="对方撤案"), "已完结", IF(ISBLANK(progress), "未知", "进行中"))
    let status_val = evaluate_simple_formula(
        &evaluator,
        &ctx,
        "case_progress",
        &["结案", "胜诉", "败诉", "对方撤案"],
    );
    updates.push(("formula_case_status", status_val));

    // formula_petitioner_first: IF(cause_action="专利无效", filing_date, "")
    if cause_action.as_deref() == Some("专利无效") {
        updates.push(("formula_petitioner_first", filing_date.clone()));
    } else {
        updates.push(("formula_petitioner_first", None));
    }

    // formula_defense_deadline: skip if patent_invalidation or no complaint_received_date
    if cause_action.as_deref() != Some("专利无效") && complaint_received_date.is_some() {
        if let Some(ref d) = complaint_received_date {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                let deadline = date + chrono::Duration::days(15);
                let deadline = evaluator
                    .evaluate(
                        &parse_formula(&format!(
                            "WORKDAY(\"{}\", 1)",
                            deadline.format("%Y-%m-%d")
                        ))
                        .unwrap_or(ast::Expr::Literal(Value::Null)),
                        &ctx,
                    )
                    .unwrap_or(Value::Null);
                updates.push((
                    "formula_defense_deadline",
                    Some(deadline.to_display_string()),
                ));
            }
        }
    } else {
        updates.push(("formula_defense_deadline", None));
    }

    // Estimated trial limit
    if filing_date.is_some() {
        if stay_date.is_some() {
            updates.push(("formula_estimated_trial_limit", None));
        } else if cause_action.as_deref() == Some("专利无效") {
            if let Some(ref d) = filing_date {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                    let est = date + chrono::Duration::days((5.7 * 30.0) as i64);
                    updates.push((
                        "formula_estimated_trial_limit",
                        Some(est.format("%Y-%m-%d").to_string()),
                    ));
                }
            }
        } else {
            let months = if procedure_type.as_deref() == Some("简易") {
                3
            } else {
                6
            };
            if let Some(ref d) = filing_date {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                    let est = eval::FormulaEvaluator::new()
                        .evaluate(
                            &ast::Expr::Call {
                                name: "EDATE".to_string(),
                                args: vec![
                                    ast::Expr::Literal(Value::Date(date)),
                                    ast::Expr::Literal(Value::Number(months as f64)),
                                ],
                            },
                            &ctx,
                        )
                        .unwrap_or(Value::Null);
                    updates.push((
                        "formula_estimated_trial_limit",
                        Some(est.to_display_string()),
                    ));
                }
            }
        }
    } else {
        updates.push(("formula_estimated_trial_limit", None));
    }

    // Deadline formulas (petitioner_supp, petitioner_reply, patentee_statement, patentee_supp)
    // All follow the pattern: IF(AND(cause_action="专利无效", NOT(ISBLANK(source_date))),
    //   EDATE(source_date, 1) adjusted to workday, "")
    let deadline_formulas: Vec<(&str, &str)> = vec![
        ("formula_petitioner_supp", "petitioner_first_invalid"),
        ("formula_petitioner_reply", "petitioner_received_date"),
        ("formula_patentee_statement", "patentee_received_date"),
        ("formula_patentee_supp", "patentee_received_supp_date"),
    ];

    for (formula_col, source_col) in deadline_formulas {
        if cause_action.as_deref() == Some("专利无效") {
            let source_val = match source_col {
                "petitioner_first_invalid" => petitioner_first_invalid.clone(),
                "petitioner_received_date" => petitioner_received_date.clone(),
                "patentee_received_date" => patentee_received_date.clone(),
                "patentee_received_supp_date" => patentee_received_supp_date.clone(),
                _ => None,
            };
            if let Some(ref d) = source_val {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                    // EDATE(date, 1) with workday adjustment
                    let edate = eval::FormulaEvaluator::new()
                        .evaluate(
                            &ast::Expr::Call {
                                name: "EDATE".to_string(),
                                args: vec![
                                    ast::Expr::Literal(Value::Date(date)),
                                    ast::Expr::Literal(Value::Number(1.0)),
                                ],
                            },
                            &ctx,
                        )
                        .unwrap_or(Value::Null);
                    if let Value::Date(ed) = edate {
                        // Check if ed is a workday, if so use it; otherwise use WORKDAY(ed, -1)
                        let cal = holidays::HolidayCalendar::builtin();
                        let result = if cal.is_workday(ed) {
                            ed
                        } else {
                            // WORKDAY(ed, -1)
                            let mut d = ed - chrono::Duration::days(1);
                            while !cal.is_workday(d) {
                                d = d - chrono::Duration::days(1);
                            }
                            d
                        };
                        updates.push((
                            formula_col,
                            Some(result.format("%Y-%m-%d").to_string()),
                        ));
                    } else {
                        updates.push((formula_col, None));
                    }
                } else {
                    updates.push((formula_col, None));
                }
            } else {
                updates.push((formula_col, None));
            }
        } else {
            updates.push((formula_col, None));
        }
    }

    // Write all updates
    let mut updated_count = 0;
    for (col, val) in updates {
        let sql = format!("UPDATE cases SET {} = ?1 WHERE id = ?2", col);
        conn.execute(&sql, rusqlite::params![val, case_id])?;
        updated_count += 1;
    }

    Ok(updated_count)
}

/// Helper: evaluate a simple OR-based status formula
fn evaluate_simple_formula(
    _evaluator: &FormulaEvaluator,
    ctx: &eval::SimpleRecordContext,
    field: &str,
    completed_values: &[&str],
) -> Option<String> {
    let val = ctx.get_field(field);
    match val {
        Value::String(ref s) => {
            if completed_values.contains(&s.as_str()) {
                Some("已完结".to_string())
            } else if s.is_empty() {
                Some("未知".to_string())
            } else {
                Some("进行中".to_string())
            }
        }
        Value::Null => Some("未知".to_string()),
        _ => Some("进行中".to_string()),
    }
}

/// Recalculate formula cache columns for all cases in the database.
pub fn recalculate_all_formulas(conn: &Connection) -> Result<usize> {
    let mut stmt = conn.prepare("SELECT id FROM cases")?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut total = 0;
    for id in &ids {
        if let Err(e) = recalculate_case_formulas(conn, id) {
            log::warn!("Formula recalc failed for case {}: {}", id, e);
        } else {
            total += 1;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::Value;
    use eval::{FormulaEvaluator, SimpleRecordContext};

    #[test]
    fn parse_and_evaluate_case_status_completed() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set("progress", Value::String("结案".to_string()));

        let expr =
            parse_formula(r#"IF(OR(progress="结案", progress="胜诉"), "已完结", "进行中")"#)
                .unwrap();
        let result = evaluate_formula(&expr, &ctx).unwrap();
        assert_eq!(result, Value::String("已完结".to_string()));
    }

    #[test]
    fn parse_and_evaluate_case_status_ongoing() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set("progress", Value::String("等待开庭".to_string()));

        let expr =
            parse_formula(r#"IF(OR(progress="结案", progress="胜诉"), "已完结", "进行中")"#)
                .unwrap();
        let result = evaluate_formula(&expr, &ctx).unwrap();
        assert_eq!(result, Value::String("进行中".to_string()));
    }

    #[test]
    fn parse_and_evaluate_hearing_status() {
        let mut ctx = SimpleRecordContext::new();
        // Past date
        ctx.set(
            "hearing_date",
            Value::Date(chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
        );

        let expr = parse_formula("IF(hearing_date<TODAY(),\"已开\",\"待开\")").unwrap();
        let result = evaluate_formula(&expr, &ctx).unwrap();
        assert_eq!(result, Value::String("已开".to_string()));
    }

    #[test]
    fn parse_and_evaluate_edate() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set(
            "start",
            Value::Date(chrono::NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()),
        );

        let expr = parse_formula("EDATE(start, 2)").unwrap();
        let result = evaluate_formula(&expr, &ctx).unwrap();
        assert_eq!(
            result,
            Value::Date(chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
        );
    }

    #[test]
    fn parse_and_evaluate_workday() {
        let mut ctx = SimpleRecordContext::new();
        // 2026-08-07 is Friday
        ctx.set(
            "d",
            Value::Date(chrono::NaiveDate::from_ymd_opt(2026, 8, 7).unwrap()),
        );

        let expr = parse_formula("WORKDAY(d, 1)").unwrap();
        let result = evaluate_formula(&expr, &ctx).unwrap();
        // +1 workday from Friday = Monday
        assert_eq!(
            result,
            Value::Date(chrono::NaiveDate::from_ymd_opt(2026, 8, 10).unwrap())
        );
    }

    #[test]
    fn dependency_graph_cascade() {
        let g = build_casy_dependency_graph();
        // When filing_date changes, multiple formulas should recalculate
        let order = g.get_recalc_order("filing_date").unwrap();
        assert!(order.len() >= 2);
        assert!(order.contains(&"formula_estimated_trial_limit".to_string()));
        assert!(order.contains(&"formula_petitioner_first".to_string()));
    }
}

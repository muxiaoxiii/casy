//! Formula evaluator: evaluates AST expressions against a record context.
//!
//! Provides a function registry (IF, AND, OR, NOT, ISBLANK, TODAY, EDATE,
//! WORKDAY, LISTCOMBINE, FILTER) and cross-table resolution.

use anyhow::Result;
use chrono::{Local, NaiveDate};

use super::ast::{CmpOp, Expr, LogicOp, Value};
use super::holidays::HolidayCalendar;

/// Record context: provides field values by field_id (or local column name).
pub trait RecordContext {
    /// Get a field value by its local column name or Feishu field_id.
    fn get_field(&self, field_id: &str) -> Value;

    /// Get a cross-table field value (for bitable::$table[xxx].$field[yyy]).
    /// Returns None if the cross-table ref cannot be resolved.
    fn get_cross_table_field(&self, table_id: &str, field_id: &str) -> Option<Value>;

    /// Execute a lookup: query a foreign table with a filter, extract a column, return array.
    /// Used for FILTER + LISTCOMBINE patterns.
    fn execute_lookup(
        &self,
        table_id: &str,
        filter_field: &str,
        filter_value: &Value,
        extract_field: &str,
    ) -> Vec<Value>;
}

/// Simple in-memory record context for testing / formula evaluation.
pub struct SimpleRecordContext {
    pub fields: std::collections::HashMap<String, Value>,
}

impl SimpleRecordContext {
    pub fn new() -> Self {
        Self {
            fields: std::collections::HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.fields.insert(key.to_string(), value);
    }
}

impl RecordContext for SimpleRecordContext {
    fn get_field(&self, field_id: &str) -> Value {
        self.fields
            .get(field_id)
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn get_cross_table_field(&self, _table_id: &str, _field_id: &str) -> Option<Value> {
        None
    }

    fn execute_lookup(
        &self,
        _table_id: &str,
        _filter_field: &str,
        _filter_value: &Value,
        _extract_field: &str,
    ) -> Vec<Value> {
        Vec::new()
    }
}

/// Formula evaluator with holiday calendar support.
pub struct FormulaEvaluator {
    calendar: HolidayCalendar,
}

impl FormulaEvaluator {
    pub fn new() -> Self {
        Self {
            calendar: HolidayCalendar::builtin(),
        }
    }

    /// Evaluate an AST expression against a record context.
    pub fn evaluate(&self, expr: &Expr, ctx: &dyn RecordContext) -> Result<Value> {
        match expr {
            Expr::Literal(v) => Ok(v.clone()),

            Expr::FieldRef { field_id } => Ok(ctx.get_field(field_id)),

            Expr::CrossTableRef { table_id, field_id } => {
                Ok(ctx.get_cross_table_field(table_id, field_id).unwrap_or(Value::Null))
            }

            Expr::Call { name, args } => self.eval_call(name, args, ctx),

            Expr::Compare { op, left, right } => {
                let lv = self.evaluate(left, ctx)?;
                let rv = self.evaluate(right, ctx)?;
                Ok(Value::Bool(eval_compare(op, &lv, &rv)))
            }

            Expr::Logic { op, left, right } => {
                let lv = self.evaluate(left, ctx)?;
                let lb = lv.as_bool().unwrap_or(false);
                match op {
                    LogicOp::And => {
                        if !lb {
                            return Ok(Value::Bool(false));
                        }
                        let rv = self.evaluate(right, ctx)?;
                        Ok(Value::Bool(rv.as_bool().unwrap_or(false)))
                    }
                    LogicOp::Or => {
                        if lb {
                            return Ok(Value::Bool(true));
                        }
                        let rv = self.evaluate(right, ctx)?;
                        Ok(Value::Bool(rv.as_bool().unwrap_or(false)))
                    }
                }
            }

            Expr::Concat(parts) => {
                let mut result = String::new();
                for part in parts {
                    let v = self.evaluate(part, ctx)?;
                    result.push_str(&v.to_display_string());
                }
                Ok(Value::String(result))
            }

            Expr::Lookup {
                table_id,
                filter,
                column_field_id,
            } => {
                // Evaluate the filter expression to extract filter criteria
                // The filter is typically: CurrentValue.$column[fldX]=bitable::$table[tblY].$field[fldZ]
                // We extract the comparison and use ctx.execute_lookup
                let filter_val = self.evaluate(filter, ctx)?;
                // For now, return the filter result as an array if it's an array,
                // or wrap in array
                match filter_val {
                    Value::Array(_) => Ok(filter_val),
                    _ => Ok(Value::Array(vec![filter_val])),
                }
            }
        }
    }

    fn eval_call(&self, name: &str, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        match name.to_uppercase().as_str() {
            "IF" => self.eval_if(args, ctx),
            "AND" => self.eval_and(args, ctx),
            "OR" => self.eval_or(args, ctx),
            "NOT" => self.eval_not(args, ctx),
            "ISBLANK" => self.eval_isblank(args, ctx),
            "TODAY" => Ok(Value::Date(Local::now().naive_local().date())),
            "EDATE" => self.eval_edate(args, ctx),
            "WORKDAY" => self.eval_workday(args, ctx),
            "LISTCOMBINE" => self.eval_listcombine(args, ctx),
            "FILTER" => self.eval_filter(args, ctx),
            "ADD" => self.eval_add(args, ctx),
            "SUB" => self.eval_sub(args, ctx),
            _ => Err(anyhow::anyhow!("Unknown function: {}", name)),
        }
    }

    fn eval_if(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        if args.len() != 3 {
            return Err(anyhow::anyhow!("IF requires exactly 3 arguments"));
        }
        let cond = self.evaluate(&args[0], ctx)?;
        if cond.as_bool().unwrap_or(false) {
            self.evaluate(&args[1], ctx)
        } else {
            self.evaluate(&args[2], ctx)
        }
    }

    fn eval_and(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        for arg in args {
            let v = self.evaluate(arg, ctx)?;
            if !v.as_bool().unwrap_or(false) {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    fn eval_or(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        for arg in args {
            let v = self.evaluate(arg, ctx)?;
            if v.as_bool().unwrap_or(false) {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    fn eval_not(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        if args.len() != 1 {
            return Err(anyhow::anyhow!("NOT requires exactly 1 argument"));
        }
        let v = self.evaluate(&args[0], ctx)?;
        Ok(Value::Bool(!v.as_bool().unwrap_or(false)))
    }

    fn eval_isblank(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        if args.len() != 1 {
            return Err(anyhow::anyhow!("ISBLANK requires exactly 1 argument"));
        }
        let v = self.evaluate(&args[0], ctx)?;
        Ok(Value::Bool(v.is_blank()))
    }

    fn eval_edate(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        if args.len() != 2 {
            return Err(anyhow::anyhow!("EDATE requires exactly 2 arguments"));
        }
        let date_val = self.evaluate(&args[0], ctx)?;
        let months_val = self.evaluate(&args[1], ctx)?;

        let Some(date) = date_val.as_date() else {
            return Ok(Value::Null);
        };
        let Some(months) = months_val.as_number() else {
            return Ok(Value::Null);
        };

        let result = add_months_clamp(date, months as u32);
        Ok(Value::Date(result))
    }

    fn eval_workday(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        if args.len() != 2 {
            return Err(anyhow::anyhow!("WORKDAY requires exactly 2 arguments"));
        }
        let date_val = self.evaluate(&args[0], ctx)?;
        let n_val = self.evaluate(&args[1], ctx)?;

        let Some(date) = date_val.as_date() else {
            return Ok(Value::Null);
        };
        let Some(n) = n_val.as_number() else {
            return Ok(Value::Null);
        };

        let n = n as i64;
        if n == 0 {
            return Ok(Value::Date(self.calendar.extend_to_workday(date)));
        }

        let direction = if n > 0 { 1i64 } else { -1i64 };
        let mut remaining = n.abs();
        let mut current = date;

        while remaining > 0 {
            current = current + chrono::Duration::days(direction);
            if self.calendar.is_workday(current) {
                remaining -= 1;
            }
        }

        Ok(Value::Date(current))
    }

    fn eval_listcombine(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        let mut result = Vec::new();
        for arg in args {
            let v = self.evaluate(arg, ctx)?;
            match v {
                Value::Array(arr) => result.extend(arr),
                other => result.push(other),
            }
        }
        // Deduplicate by display string
        let mut seen = std::collections::HashSet::new();
        result.retain(|v| seen.insert(v.to_display_string()));
        Ok(Value::Array(result))
    }

    fn eval_filter(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        // FILTER is typically used in lookup chains; as a standalone function,
        // it filters an array by a predicate
        if args.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        // The first arg is the array, rest are predicates
        let arr = self.evaluate(&args[0], ctx)?;
        match arr {
            Value::Array(items) => {
                let mut result = Vec::new();
                for item in items {
                    // Evaluate predicate against item (simplified)
                    result.push(item);
                }
                Ok(Value::Array(result))
            }
            _ => Ok(Value::Array(vec![arr])),
        }
    }

    fn eval_add(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        if args.len() != 2 {
            return Err(anyhow::anyhow!("ADD requires exactly 2 arguments"));
        }
        let left = self.evaluate(&args[0], ctx)?;
        let right = self.evaluate(&args[1], ctx)?;

        // Date + number = date arithmetic
        if let (Some(date), Some(days)) = (left.as_date(), right.as_number()) {
            return Ok(Value::Date(date + chrono::Duration::days(days as i64)));
        }
        if let (Some(days), Some(date)) = (left.as_number(), right.as_date()) {
            return Ok(Value::Date(date + chrono::Duration::days(days as i64)));
        }

        // Number + number
        if let (Some(a), Some(b)) = (left.as_number(), right.as_number()) {
            return Ok(Value::Number(a + b));
        }

        // String concatenation fallback
        Ok(Value::String(format!(
            "{}{}",
            left.to_display_string(),
            right.to_display_string()
        )))
    }

    fn eval_sub(&self, args: &[Expr], ctx: &dyn RecordContext) -> Result<Value> {
        if args.len() != 2 {
            return Err(anyhow::anyhow!("SUB requires exactly 2 arguments"));
        }
        let left = self.evaluate(&args[0], ctx)?;
        let right = self.evaluate(&args[1], ctx)?;

        // Date - number = date arithmetic
        if let (Some(date), Some(days)) = (left.as_date(), right.as_number()) {
            return Ok(Value::Date(date - chrono::Duration::days(days as i64)));
        }

        // Date - date = days between
        if let (Some(a), Some(b)) = (left.as_date(), right.as_date()) {
            return Ok(Value::Number((a - b).num_days() as f64));
        }

        // Number - number
        if let (Some(a), Some(b)) = (left.as_number(), right.as_number()) {
            return Ok(Value::Number(a - b));
        }

        Ok(Value::Null)
    }
}

impl Default for FormulaEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Comparison helpers ────────────────────────────────────────

fn eval_compare(op: &CmpOp, left: &Value, right: &Value) -> bool {
    match op {
        CmpOp::Eq => values_equal(left, right),
        CmpOp::Ne => !values_equal(left, right),
        CmpOp::Lt => values_less(left, right),
        CmpOp::Gt => values_less(right, left),
        CmpOp::Le => values_equal(left, right) || values_less(left, right),
        CmpOp::Ge => values_equal(left, right) || values_less(right, left),
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    left == right
}

fn values_less(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => a < b,
        (Value::String(a), Value::String(b)) => a < b,
        (Value::Date(a), Value::Date(b)) => a < b,
        (Value::DateTime(a), Value::DateTime(b)) => a < b,
        (Value::Bool(a), Value::Bool(b)) => !a && *b,
        // Date comparison with cross-type
        (Value::Date(a), Value::DateTime(b)) => *a < b.date(),
        (Value::DateTime(a), Value::Date(b)) => a.date() < *b,
        // Number vs string
        (Value::Number(a), Value::String(b)) => {
            if let Ok(bn) = b.parse::<f64>() {
                a < &bn
            } else {
                false
            }
        }
        _ => false,
    }
}

// ── Date helpers ──────────────────────────────────────────────

fn add_months_clamp(date: NaiveDate, months: u32) -> NaiveDate {
    use chrono::Datelike;
    let total = date.month() + months;
    let year = date.year() + ((total - 1) / 12) as i32;
    let month = ((total - 1) % 12) + 1;
    let max_day = days_in_month(year, month);
    NaiveDate::from_ymd_opt(year, month, date.day().min(max_day)).unwrap_or(date)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    if month == 12 {
        31
    } else {
        let next = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
        let curr = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        (next - curr).num_days() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval_str(formula: &str, ctx: &dyn RecordContext) -> Value {
        let ast = super::super::parser::parse_formula(formula).unwrap();
        let evaluator = FormulaEvaluator::new();
        evaluator.evaluate(&ast, ctx).unwrap()
    }

    #[test]
    fn eval_if_true() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str(r#"IF(true, "yes", "no")"#, &ctx);
        assert_eq!(v, Value::String("yes".to_string()));
    }

    #[test]
    fn eval_if_false() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str(r#"IF(false, "yes", "no")"#, &ctx);
        assert_eq!(v, Value::String("no".to_string()));
    }

    #[test]
    fn eval_and_true() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str("AND(true, true)", &ctx);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn eval_and_short_circuit() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str("AND(false, true)", &ctx);
        assert_eq!(v, Value::Bool(false));
    }

    #[test]
    fn eval_or_short_circuit() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str("OR(true, false)", &ctx);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn eval_not() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str("NOT(false)", &ctx);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn eval_isblank_null() {
        let ctx = SimpleRecordContext::new();
        // Null field should be blank
        let v = eval_str("ISBLANK(missing_field)", &ctx);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn eval_isblank_empty_string() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set("empty", Value::String("".to_string()));
        let v = eval_str("ISBLANK(empty)", &ctx);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn eval_today() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str("TODAY()", &ctx);
        match v {
            Value::Date(d) => assert_eq!(d, Local::now().naive_local().date()),
            _ => panic!("Expected Date"),
        }
    }

    #[test]
    fn eval_edate() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set(
            "start",
            Value::Date(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()),
        );
        let v = eval_str("EDATE(start, 1)", &ctx);
        assert_eq!(
            v,
            Value::Date(NaiveDate::from_ymd_opt(2026, 2, 15).unwrap())
        );
    }

    #[test]
    fn eval_edate_clamp() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set(
            "start",
            Value::Date(NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()),
        );
        let v = eval_str("EDATE(start, 1)", &ctx);
        // Jan 31 + 1 month = Feb 28 (clamp)
        assert_eq!(
            v,
            Value::Date(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap())
        );
    }

    #[test]
    fn eval_workday_forward() {
        let mut ctx = SimpleRecordContext::new();
        // 2026-08-07 is Friday
        ctx.set(
            "start",
            Value::Date(NaiveDate::from_ymd_opt(2026, 8, 7).unwrap()),
        );
        let v = eval_str("WORKDAY(start, 1)", &ctx);
        // +1 workday from Friday = Monday 2026-08-10
        assert_eq!(
            v,
            Value::Date(NaiveDate::from_ymd_opt(2026, 8, 10).unwrap())
        );
    }

    #[test]
    fn eval_workday_backward() {
        let mut ctx = SimpleRecordContext::new();
        // 2026-08-10 is Monday
        ctx.set(
            "start",
            Value::Date(NaiveDate::from_ymd_opt(2026, 8, 10).unwrap()),
        );
        let v = eval_str("WORKDAY(start, -1)", &ctx);
        // -1 workday from Monday = Friday 2026-08-07
        assert_eq!(
            v,
            Value::Date(NaiveDate::from_ymd_opt(2026, 8, 7).unwrap())
        );
    }

    #[test]
    fn eval_concat() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str(r#""hello"&" world""#, &ctx);
        assert_eq!(v, Value::String("hello world".to_string()));
    }

    #[test]
    fn eval_comparison_eq() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str(r#""test"="test""#, &ctx);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn eval_comparison_ne() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str(r#""test"!="other""#, &ctx);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn eval_nested_if_or() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set("progress", Value::String("结案".to_string()));
        let v = eval_str(
            r#"IF(OR(progress="结案", progress="胜诉"), "已完结", "进行中")"#,
            &ctx,
        );
        assert_eq!(v, Value::String("已完结".to_string()));
    }

    #[test]
    fn eval_date_comparison() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set(
            "hearing_date",
            Value::Date(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
        );
        let v = eval_str("hearing_date<TODAY()", &ctx);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn eval_add_days_to_date() {
        let mut ctx = SimpleRecordContext::new();
        ctx.set(
            "start",
            Value::Date(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );
        let v = eval_str("start+15", &ctx);
        assert_eq!(
            v,
            Value::Date(NaiveDate::from_ymd_opt(2026, 1, 16).unwrap())
        );
    }

    #[test]
    fn eval_subtract_numbers() {
        let ctx = SimpleRecordContext::new();
        let v = eval_str("100-42", &ctx);
        assert_eq!(v, Value::Number(58.0));
    }
}

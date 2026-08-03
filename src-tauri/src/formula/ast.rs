//! Formula AST types for Feishu bitable formula parsing.
//!
//! Mirrors the Feishu formula language: IF, AND, OR, NOT, ISBLANK, TODAY,
//! EDATE, WORKDAY, FILTER, LISTCOMBINE, comparison/logic operators, string
//! concatenation, and cross-table references via `bitable::$table[xxx].$field[yyy]`.

use chrono::{NaiveDate, NaiveDateTime};

// ── Comparison operators ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CmpOp {
    Eq,  // ==
    Ne,  // !=
    Lt,  // <
    Gt,  // >
    Le,  // <=
    Ge,  // >=
}

// ── Logic operators (binary chaining) ─────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LogicOp {
    And, // &&
    Or,  // ||
}

// ── AST expression nodes ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Literal value
    Literal(Value),

    /// Reference to a field in the current record (by local column name or field_id)
    FieldRef {
        field_id: String,
    },

    /// Cross-table field reference: bitable::$table[xxx].$field[yyy]
    CrossTableRef {
        table_id: String,
        field_id: String,
    },

    /// Function call: IF, AND, OR, NOT, ISBLANK, TODAY, EDATE, WORKDAY, etc.
    Call {
        name: String,
        args: Vec<Expr>,
    },

    /// Binary comparison
    Compare {
        op: CmpOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Binary logic (AND / OR as infix operators)
    Logic {
        op: LogicOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// String concatenation with & operator
    Concat(Vec<Expr>),

    /// Lookup chain: table_ref.FILTER(cond).$column[fld].LISTCOMBINE()
    Lookup {
        table_id: String,
        filter: Box<Expr>,
        column_field_id: String,
    },
}

// ── Runtime values ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Array(Vec<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => (a - b).abs() < f64::EPSILON,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Date(a), Value::Date(b)) => a == b,
            (Value::DateTime(a), Value::DateTime(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            // Cross-type: Number == String (numeric string comparison)
            (Value::Number(a), Value::String(b)) => {
                if let Ok(bn) = b.parse::<f64>() {
                    (a - bn).abs() < f64::EPSILON
                } else {
                    false
                }
            }
            (Value::String(a), Value::Number(b)) => {
                if let Ok(an) = a.parse::<f64>() {
                    (an - b).abs() < f64::EPSILON
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl Value {
    /// Returns true if the value is considered "blank" (Feishu ISBLANK semantics)
    pub fn is_blank(&self) -> bool {
        match self {
            Value::Null => true,
            Value::String(s) if s.is_empty() => true,
            _ => false,
        }
    }

    /// Try to interpret as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Number(n) => Some(*n != 0.0),
            Value::Null => Some(false),
            Value::String(s) => Some(!s.is_empty()),
            _ => None,
        }
    }

    /// Try to interpret as f64
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Bool(true) => Some(1.0),
            Value::Bool(false) => Some(0.0),
            Value::String(s) => s.parse::<f64>().ok(),
            _ => None,
        }
    }

    /// Try to interpret as a date
    pub fn as_date(&self) -> Option<NaiveDate> {
        match self {
            Value::Date(d) => Some(*d),
            Value::DateTime(dt) => Some(dt.date()),
            Value::String(s) => {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .or_else(|_| NaiveDate::parse_from_str(s, "%Y/%m/%d"))
                    .ok()
            }
            _ => None,
        }
    }

    /// Display as string (for formula output)
    pub fn to_display_string(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => {
                if (*n - n.round()).abs() < f64::EPSILON {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            Value::String(s) => s.clone(),
            Value::Date(d) => d.format("%Y-%m-%d").to_string(),
            Value::DateTime(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
            Value::Array(arr) => {
                let strs: Vec<String> = arr.iter().map(|v| v.to_display_string()).collect();
                strs.join(", ")
            }
        }
    }

    /// Date arithmetic: add days
    pub fn add_days(&self, days: i64) -> Value {
        use chrono::Duration;
        match self {
            Value::Date(d) => Value::Date(*d + Duration::days(days)),
            Value::DateTime(dt) => Value::DateTime(*dt + Duration::days(days)),
            _ => Value::Null,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

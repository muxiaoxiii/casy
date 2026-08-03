//! Feishu formula parser using nom.
//!
//! Parses Feishu bitable formula syntax into an AST (Expr).
//!
//! Supported constructs:
//! - Literals: strings, numbers, booleans, null
//! - Field references: local fields
//! - Cross-table references: bitable::$table[xxx].$field[yyy]
//! - Functions: IF, AND, OR, NOT, ISBLANK, TODAY, EDATE, WORKDAY, LISTCOMBINE, FILTER
//! - Operators: ==, !=, <, >, <=, >=, &&, ||, & (concat), +, -
//! - Lookup chains: table.FILTER(cond).$column[fld].LISTCOMBINE()

use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1},
    character::complete::{char, multispace0},
    combinator::{opt, recognize, value},
    multi::separated_list0,
    number::complete::double,
    sequence::{delimited, pair, preceded},
    IResult,
};

use super::ast::{CmpOp, Expr, LogicOp, Value};

// ── Helpers ───────────────────────────────────────────────────

fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        take_while1(is_ident_start),
        take_while(is_ident_char),
    ))(input)
}

// ── Literals ──────────────────────────────────────────────────

fn parse_string_literal(input: &str) -> IResult<&str, Expr> {
    let (input, s) = delimited(
        char('"'),
        take_while(|c: char| c != '"'),
        char('"'),
    )(input)?;
    Ok((input, Expr::Literal(Value::String(s.to_string()))))
}

fn parse_number_literal(input: &str) -> IResult<&str, Expr> {
    let (input, n) = double(input)?;
    Ok((input, Expr::Literal(Value::Number(n))))
}

fn parse_bool_literal(input: &str) -> IResult<&str, Expr> {
    alt((
        value(Expr::Literal(Value::Bool(true)), tag_no_case("true")),
        value(Expr::Literal(Value::Bool(false)), tag_no_case("false")),
    ))(input)
}

fn parse_null_literal(input: &str) -> IResult<&str, Expr> {
    value(Expr::Literal(Value::Null), tag_no_case("null"))(input)
}

fn parse_literal(input: &str) -> IResult<&str, Expr> {
    alt((
        parse_string_literal,
        parse_number_literal,
        parse_bool_literal,
        parse_null_literal,
    ))(input)
}

// ── Cross-table reference ────────────────────────────────────
// bitable::$table[tblXXX].$field[fldYYY]

fn parse_cross_table_ref(input: &str) -> IResult<&str, Expr> {
    let (input, _) = tag("bitable::")(input)?;
    let (input, _) = tag("$table[")(input)?;
    let (input, table_id) = take_while(|c: char| c != ']')(input)?;
    let (input, _) = char(']')(input)?;
    let (input, _) = char('.')(input)?;
    let (input, _) = tag("$field[")(input)?;
    let (input, field_id) = take_while(|c: char| c != ']')(input)?;
    let (input, _) = char(']')(input)?;
    Ok((
        input,
        Expr::CrossTableRef {
            table_id: table_id.to_string(),
            field_id: field_id.to_string(),
        },
    ))
}

// ── Lookup chain ─────────────────────────────────────────────
// bitable::$table[xxx].FILTER(cond).$column[yyy].LISTCOMBINE()

fn parse_lookup_chain(input: &str) -> IResult<&str, Expr> {
    let (input, _) = tag("bitable::")(input)?;
    let (input, _) = tag("$table[")(input)?;
    let (input, table_id) = take_while(|c: char| c != ']')(input)?;
    let (input, _) = char(']')(input)?;

    // .FILTER(...)
    let (input, _) = ws(char('.'))(input)?;
    let (input, _) = tag("FILTER")(input)?;
    let (input, filter_expr) = delimited(ws(char('(')), parse_expr, ws(char(')')))(input)?;

    // .COLUMN[yyy] or .$column[yyy]
    let (input, _) = ws(char('.'))(input)?;
    let (input, _) = opt(char('$'))(input)?;
    let (input, _) = alt((tag("column"), tag("COLUMN")))(input)?;
    let (input, _) = char('[')(input)?;
    let (input, column_field_id) = take_while(|c: char| c != ']')(input)?;
    let (input, _) = char(']')(input)?;

    // optional .LISTCOMBINE()
    let (input, _) = opt(preceded(
        ws(char('.')),
        pair(tag("LISTCOMBINE"), ws(delimited(char('('), take_while(|c: char| c != ')'), char(')')))),
    ))(input)?;

    // optional .CONTAIN(...) — treat as part of filter expression, already parsed
    // For now, we just skip if present
    let (input, _) = opt(preceded(
        ws(char('.')),
        pair(tag("CONTAIN"), ws(delimited(char('('), take_while(|c: char| c != ')'), char(')')))),
    ))(input)?;

    Ok((
        input,
        Expr::Lookup {
            table_id: table_id.to_string(),
            filter: Box::new(filter_expr),
            column_field_id: column_field_id.to_string(),
        },
    ))
}

// ── Function calls ────────────────────────────────────────────

fn parse_function_call(input: &str) -> IResult<&str, Expr> {
    let (input, name) = identifier(input)?;
    let (input, args) = delimited(
        ws(char('(')),
        separated_list0(ws(char(',')), parse_expr),
        ws(char(')')),
    )(input)?;
    Ok((
        input,
        Expr::Call {
            name: name.to_string(),
            args,
        },
    ))
}

// ── Field reference (identifiers that aren't function calls) ──

fn parse_field_ref(input: &str) -> IResult<&str, Expr> {
    let (input, name) = identifier(input)?;
    Ok((
        input,
        Expr::FieldRef {
            field_id: name.to_string(),
        },
    ))
}

// ── Primary (atom) expressions ────────────────────────────────

fn parse_primary(input: &str) -> IResult<&str, Expr> {
    let input = input.trim_start();
    alt((
        delimited(ws(char('(')), parse_expr, ws(char(')'))),
        parse_lookup_chain,
        parse_cross_table_ref,
        parse_function_call,
        parse_literal,
        parse_field_ref,
    ))(input)
}

// ── Unary NOT ─────────────────────────────────────────────────

fn parse_unary(input: &str) -> IResult<&str, Expr> {
    let input = input.trim_start();
    if let Ok((input, _)) = ws::<_, _>(tag_no_case("NOT"))(input) {
        let (input, _) = ws(char('('))(input)?;
        let (input, expr) = parse_expr(input)?;
        let (input, _) = ws(char(')'))(input)?;
        Ok((
            input,
            Expr::Call {
                name: "NOT".to_string(),
                args: vec![expr],
            },
        ))
    } else {
        parse_primary(input)
    }
}

// ── Multiplicative / date arithmetic (+ -) ────────────────────

fn parse_additive(input: &str) -> IResult<&str, Expr> {
    let (mut input, mut left) = parse_unary(input)?;
    loop {
        let trimmed = input.trim_start();
        if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('+')(trimmed) {
            let (rest, right) = parse_unary(rest)?;
            left = Expr::Call {
                name: "ADD".to_string(),
                args: vec![left, right],
            };
            input = rest;
        } else if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('-')(trimmed) {
            let (rest, right) = parse_unary(rest)?;
            left = Expr::Call {
                name: "SUB".to_string(),
                args: vec![left, right],
            };
            input = rest;
        } else {
            break;
        }
    }
    Ok((input, left))
}

// ── Concatenation (&) ────────────────────────────────────────

fn parse_concat(input: &str) -> IResult<&str, Expr> {
    let (mut input, mut parts) = {
        let (input, first) = parse_additive(input)?;
        (input, vec![first])
    };
    loop {
        let trimmed = input.trim_start();
        if let Ok((rest, _)) = ws::<_, _>(char::<&str, nom::error::Error<&str>>('&'))(trimmed) {
            let (rest, next) = parse_additive(rest)?;
            parts.push(next);
            input = rest;
        } else {
            break;
        }
    }
    if parts.len() == 1 {
        Ok((input, parts.into_iter().next().unwrap()))
    } else {
        Ok((input, Expr::Concat(parts)))
    }
}

// ── Comparison operators ──────────────────────────────────────

fn parse_comparison(input: &str) -> IResult<&str, Expr> {
    let (input, left) = parse_concat(input)?;
    let trimmed = input.trim_start();

    let ops: &[(&str, CmpOp)] = &[
        ("==", CmpOp::Eq),
        ("!=", CmpOp::Ne),
        ("<=", CmpOp::Le),
        (">=", CmpOp::Ge),
        ("<", CmpOp::Lt),
        (">", CmpOp::Gt),
        // Feishu uses = for equality too
        ("=", CmpOp::Eq),
    ];

    for (sym, op) in ops {
        if let Ok((rest, _)) = ws::<_, _>(tag::<&str, &str, nom::error::Error<&str>>(*sym))(trimmed)
        {
            let (rest, right) = parse_concat(rest)?;
            return Ok((
                rest,
                Expr::Compare {
                    op: op.clone(),
                    left: Box::new(left),
                    right: Box::new(right),
                },
            ));
        }
    }

    Ok((input, left))
}

// ── Logic operators (&& / ||) ─────────────────────────────────

fn parse_logic(input: &str) -> IResult<&str, Expr> {
    let (mut input, mut left) = parse_comparison(input)?;
    loop {
        let trimmed = input.trim_start();
        if let Ok((rest, _)) = ws::<_, _>(tag::<&str, &str, nom::error::Error<&str>>("&&"))(trimmed)
        {
            let (rest, right) = parse_comparison(rest)?;
            left = Expr::Logic {
                op: LogicOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
            input = rest;
        } else if let Ok((rest, _)) =
            ws::<_, _>(tag::<&str, &str, nom::error::Error<&str>>("||"))(trimmed)
        {
            let (rest, right) = parse_comparison(rest)?;
            left = Expr::Logic {
                op: LogicOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
            input = rest;
        } else {
            break;
        }
    }
    Ok((input, left))
}

// ── Top-level expression ──────────────────────────────────────

fn parse_expr(input: &str) -> IResult<&str, Expr> {
    parse_logic(input)
}

// ── Public API ────────────────────────────────────────────────

/// Parse a Feishu formula string into an AST.
///
/// Returns `Err` if the formula cannot be parsed.
pub fn parse_formula(input: &str) -> Result<Expr, String> {
    let input = input.trim();
    match parse_expr(input) {
        Ok(("", expr)) => Ok(expr),
        Ok((remaining, _)) => Err(format!(
            "Unexpected trailing input: '{}'",
            &remaining[..remaining.len().min(40)]
        )),
        Err(e) => Err(format!("Parse error: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_if() {
        let expr = parse_formula(r#"IF(true, "yes", "no")"#).unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "IF");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("Expected Call"),
        }
    }

    #[test]
    fn parse_cross_table_ref() {
        let expr =
            parse_formula("bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]").unwrap();
        match expr {
            Expr::CrossTableRef { table_id, field_id } => {
                assert_eq!(table_id, "tbl4fMNw2UJfXBgy");
                assert_eq!(field_id, "fld6KMKs7x");
            }
            _ => panic!("Expected CrossTableRef"),
        }
    }

    #[test]
    fn parse_comparison_eq() {
        let expr = parse_formula(r#"bitable::$table[tblX].$field[fldY]="结案""#).unwrap();
        match expr {
            Expr::Compare { op, .. } => assert_eq!(op, CmpOp::Eq),
            _ => panic!("Expected Compare, got {:?}", expr),
        }
    }

    #[test]
    fn parse_nested_if_or() {
        let formula = r#"IF(OR(
            bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]="结案",
            bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]="胜诉"
        ), "已完结", "进行中")"#;
        let expr = parse_formula(formula).unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "IF");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("Expected IF Call, got {:?}", expr),
        }
    }

    #[test]
    fn parse_today_function() {
        let expr = parse_formula("TODAY()").unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "TODAY");
                assert!(args.is_empty());
            }
            _ => panic!("Expected TODAY Call"),
        }
    }

    #[test]
    fn parse_comparison_lt() {
        let expr = parse_formula(
            "bitable::$table[tblXrb7Y6c9i2o8D].$field[fldXapQ4bm]<TODAY()",
        )
        .unwrap();
        match expr {
            Expr::Compare { op, .. } => assert_eq!(op, CmpOp::Lt),
            _ => panic!("Expected Compare, got {:?}", expr),
        }
    }

    #[test]
    fn parse_and_function() {
        let expr = parse_formula(r#"AND(true, false)"#).unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "AND");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected AND Call"),
        }
    }

    #[test]
    fn parse_not_isblank() {
        let expr = parse_formula("NOT(ISBLANK(\"test\"))").unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "NOT");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected NOT Call"),
        }
    }

    #[test]
    fn parse_concat() {
        let expr = parse_formula(r#""hello"&" world""#).unwrap();
        match expr {
            Expr::Concat(parts) => assert_eq!(parts.len(), 2),
            _ => panic!("Expected Concat, got {:?}", expr),
        }
    }

    #[test]
    fn parse_edate() {
        let expr = parse_formula("EDATE(date_field, 1)").unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "EDATE");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected EDATE Call"),
        }
    }

    #[test]
    fn parse_workday() {
        let expr = parse_formula("WORKDAY(date_field, 5)").unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "WORKDAY");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected WORKDAY Call"),
        }
    }

    #[test]
    fn parse_hearing_status_formula() {
        let formula = "IF(bitable::$table[tblXrb7Y6c9i2o8D].$field[fldXapQ4bm]<TODAY(),\"已开\",\"待开\")";
        let expr = parse_formula(formula).unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "IF");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("Expected IF Call, got {:?}", expr),
        }
    }

    #[test]
    fn parse_task_deadline_formula() {
        let formula = r#"IF(AND(
            bitable::$table[tblVzAWjugfmRqhR].$field[fldoAS6Hfr]=0,
            ISBLANK(bitable::$table[tblVzAWjugfmRqhR].$field[fld6Gxc8Mj])=false
        ), "has_deadline", "")"#;
        let expr = parse_formula(formula).unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "IF");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("Expected IF Call, got {:?}", expr),
        }
    }
}

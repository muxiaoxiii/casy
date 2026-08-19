//! L3 递归确认（设计哲学 §11.5）
//!
//! 关键决策经用户确认后，由独立 AI 任务核对决策与数据链的一致性
//! （遗漏 / 矛盾 / 依据可查）。核对范围有界：决策关联实体展开 1-2 层，
//! 条目上限 MAX_CONTEXT_ITEMS，不把全库塞进上下文。
//!
//! 降级：AI 不可用 / 调用失败时退化为纯规则校验（日期格式、必填字段、
//! 引用记录存在性，全部本地 SQL 完成），不阻塞主流程。

use anyhow::Result;
use rusqlite::{params, Connection};

/// 核对上下文条目上限（有界范围）
const MAX_CONTEXT_ITEMS: usize = 50;

/// 决策快照（核对对象）
struct DecisionSnapshot {
    id: String,
    entity_type: String,
    entity_id: String,
    decision_type: String,
    decision: String,
    basis: Option<String>,
    source_ref: Option<String>,
    review_due: Option<String>,
}

/// 对一条决策执行递归确认核对。
///
/// 流程：有界上下文采集（同步 SQL）→ 规则校验（始终执行）→ AI 一致性核对
/// （可用时，过 ai_runs 审计）→ 结果写回 decisions.recursive_checked，
/// 发现的缺口写入 task_events（event_type='recursion_gap'）。
///
/// `conn` 按值传入：owned Connection 是 Send，可跨 await 持有（&Connection 不是 Send）。
pub async fn recursive_check_decision(
    conn: Connection,
    decision_id: &str,
) -> Result<serde_json::Value> {
    // ── 阶段 1：采集有界核对范围（同步 SQL）──
    let (snapshot, context_items) = collect_bounded_context(&conn, decision_id)?;
    let rule_gaps = rule_validate(&conn, &snapshot);

    // ── 阶段 2：AI 一致性核对（独立提示词；不可用则跳过，走规则降级）──
    let config = crate::ai::load_ai_config();
    let mut ai_used = false;
    let mut ai_gaps: Vec<String> = Vec::new();

    if config.mode != "noop" {
        let user_prompt = build_check_prompt(&snapshot, &context_items);
        let provider = config.mode.clone();
        let model = config.model.clone().unwrap_or_default();

        use sha2::Digest;
        let input_hash = hex::encode(sha2::Sha256::digest(user_prompt.as_bytes()));
        let backend = crate::ai::create_backend(&config);

        let system_prompt = "你是独立复核员。核对以下决策与其关联数据是否一致，找出遗漏、矛盾、依据不可查之处。\
            只返回 JSON：{\"consistent\": true 或 false, \"gaps\": [\"问题1\", \"问题2\"]}。\
            若一切一致，gaps 为空数组。不要输出其他文字。";

        match backend.chat_completion(system_prompt, &user_prompt).await {
            Ok(text) => {
                ai_used = true;
                let output_hash = hex::encode(sha2::Sha256::digest(text.as_bytes()));
                if let Err(e) = crate::commands::ai_routes::log_ai_run(
                    &provider,
                    &model,
                    "recursive_check",
                    Some("v1"),
                    &input_hash,
                    Some(&output_hash),
                    "completed",
                    None,
                ) {
                    log::warn!("AI 审计日志写入失败: {}", e);
                }
                match parse_check_response(&text) {
                    Ok(gaps) => ai_gaps = gaps,
                    Err(_) => ai_gaps.push("AI 复核返回无法解析，需人工确认".to_string()),
                }
            }
            Err(e) => {
                // 降级：AI 失败只记审计 + 日志，规则校验结果仍然有效
                log::warn!("递归确认 AI 核对失败，降级为规则校验: {}", e);
                let _ = crate::commands::ai_routes::log_ai_run(
                    &provider,
                    &model,
                    "recursive_check",
                    Some("v1"),
                    &input_hash,
                    None,
                    "failed",
                    Some(&e.to_string()),
                );
            }
        }
    }

    // ── 阶段 3：汇总结果并写回 ──
    let mut gaps = rule_gaps;
    gaps.extend(ai_gaps);
    let consistent = gaps.is_empty();

    if consistent {
        conn.execute(
            "UPDATE decisions SET recursive_checked = 1 WHERE id = ?1",
            params![decision_id],
        )?;
    } else {
        // 失败原因写 task_events（recursion_gap）；决策关联任务时挂到该任务
        let task_id = if snapshot.entity_type == "task" {
            Some(snapshot.entity_id.clone())
        } else {
            None
        };
        let payload = serde_json::json!({
            "decisionId": decision_id,
            "entityType": snapshot.entity_type,
            "entityId": snapshot.entity_id,
            "gaps": gaps,
            "source": if ai_used { "ai" } else { "rule" },
        });
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, payload, actor)
             VALUES (?1, ?2, 'recursion_gap', ?3, ?4)",
            params![
                crate::db::new_id(),
                task_id,
                payload.to_string(),
                if ai_used { "ai" } else { "system" },
            ],
        )?;
    }

    Ok(serde_json::json!({
        "decisionId": decision_id,
        "consistent": consistent,
        "gaps": gaps,
        "aiUsed": ai_used,
        "contextItems": context_items.len(),
    }))
}

/// 采集有界核对上下文：按决策关联实体展开 1-2 层，条目上限 MAX_CONTEXT_ITEMS
fn collect_bounded_context(
    conn: &Connection,
    decision_id: &str,
) -> Result<(DecisionSnapshot, Vec<String>)> {
    let snapshot = conn.query_row(
        "SELECT id, entity_type, entity_id, decision_type, decision, basis, source_ref, review_due
         FROM decisions WHERE id = ?1",
        params![decision_id],
        |row| {
            Ok(DecisionSnapshot {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                decision_type: row.get(3)?,
                decision: row.get(4)?,
                basis: row.get(5)?,
                source_ref: row.get(6)?,
                review_due: row.get(7)?,
            })
        },
    )?;

    let mut items: Vec<String> = Vec::new();
    items.push(format!(
        "决策[{}] type={} decision={} basis={}",
        snapshot.id,
        snapshot.decision_type,
        snapshot.decision,
        snapshot.basis.as_deref().unwrap_or("（无）")
    ));

    // 深度 1：关联实体本身
    let mut case_id: Option<String> = None;
    match snapshot.entity_type.as_str() {
        "case" => {
            case_id = Some(snapshot.entity_id.clone());
            if let Ok((name, no, status)) = conn.query_row(
                "SELECT case_name, COALESCE(case_no, ''), COALESCE(case_status, '') FROM cases WHERE id = ?1",
                params![snapshot.entity_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
            ) {
                items.push(format!("案件[{}] {} 案号={} 状态={}", snapshot.entity_id, name, no, status));
            }
        }
        "task" => {
            if let Ok((name, cid, deadline, completed)) = conn.query_row(
                "SELECT task_name, case_id, COALESCE(COALESCE(due_date, deadline), ''), completed
                 FROM tasks WHERE id = ?1",
                params![snapshot.entity_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?)),
            ) {
                case_id = cid;
                items.push(format!(
                    "任务[{}] {} 截止={} 已完成={}",
                    snapshot.entity_id, name, deadline, completed
                ));
            }
        }
        "knowledge" => {
            if let Ok((title, cid)) = conn.query_row(
                "SELECT title, linked_case_id FROM knowledge_items WHERE id = ?1",
                params![snapshot.entity_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
            ) {
                case_id = cid;
                items.push(format!("知识[{}] {}", snapshot.entity_id, title));
            }
        }
        "client" => {
            if let Ok(name) = conn.query_row(
                "SELECT name FROM clients WHERE id = ?1",
                params![snapshot.entity_id],
                |r| r.get::<_, String>(0),
            ) {
                items.push(format!("客户[{}] {}", snapshot.entity_id, name));
            }
            // 深度 2：客户名下案件
            let mut stmt = conn.prepare(
                "SELECT id, case_name FROM cases WHERE client_id = ?1 LIMIT 10",
            )?;
            let rows = stmt
                .query_map(params![snapshot.entity_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok());
            for (cid, name) in rows {
                items.push(format!("客户案件[{}] {}", cid, name));
            }
        }
        _ => {}
    }

    // 深度 2：案件的任务 / 期限 / 开庭 / 最近决策
    if let Some(cid) = &case_id {
        if items.len() < MAX_CONTEXT_ITEMS {
            let mut stmt = conn.prepare(
                "SELECT id, task_name, COALESCE(COALESCE(due_date, deadline), ''), completed
                 FROM tasks WHERE case_id = ?1 ORDER BY created_at DESC LIMIT 10",
            )?;
            let rows = stmt
                .query_map(params![cid], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))
                })?
                .filter_map(|r| r.ok());
            for (id, name, due, completed) in rows {
                items.push(format!("案件任务[{}] {} 截止={} 已完成={}", id, name, due, completed));
            }
        }

        if items.len() < MAX_CONTEXT_ITEMS {
            let mut stmt = conn.prepare(
                "SELECT id, deadline_name, due_date, completed FROM case_deadlines
                 WHERE case_id = ?1 ORDER BY due_date LIMIT 10",
            )?;
            let rows = stmt
                .query_map(params![cid], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))
                })?
                .filter_map(|r| r.ok());
            for (id, name, due, completed) in rows {
                items.push(format!("案件期限[{}] {} 到期={} 已完成={}", id, name, due, completed));
            }
        }

        if items.len() < MAX_CONTEXT_ITEMS {
            let mut stmt = conn.prepare(
                "SELECT id, COALESCE(hearing_name, '庭审'), hearing_date FROM hearings
                 WHERE case_id = ?1 ORDER BY hearing_date DESC LIMIT 5",
            )?;
            let rows = stmt
                .query_map(params![cid], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
                })?
                .filter_map(|r| r.ok());
            for (id, name, date) in rows {
                items.push(format!("开庭[{}] {} 日期={}", id, name, date));
            }
        }

        if items.len() < MAX_CONTEXT_ITEMS {
            let mut stmt = conn.prepare(
                "SELECT id, decision_type, decision, status FROM decisions
                 WHERE entity_type = 'case' AND entity_id = ?1 AND id != ?2
                 ORDER BY created_at DESC LIMIT 10",
            )?;
            let rows = stmt
                .query_map(params![cid, decision_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
                })?
                .filter_map(|r| r.ok());
            for (id, dtype, decision, status) in rows {
                items.push(format!("关联决策[{}] type={} status={} {}", id, dtype, status, decision));
            }
        }
    }

    // 有界截断
    if items.len() > MAX_CONTEXT_ITEMS {
        items.truncate(MAX_CONTEXT_ITEMS);
        items.push(format!("（上下文已截断至 {} 条）", MAX_CONTEXT_ITEMS));
    }

    Ok((snapshot, items))
}

/// 规则校验（始终执行；AI 不可用时作为降级校验）：日期格式 / 必填字段 / 引用存在性
fn rule_validate(conn: &Connection, snapshot: &DecisionSnapshot) -> Vec<String> {
    let mut gaps = Vec::new();

    if snapshot.decision.trim().is_empty() {
        gaps.push("决策内容为空".to_string());
    }
    if snapshot.decision_type.trim().is_empty() {
        gaps.push("决策类型为空".to_string());
    }

    // review_due 日期格式
    if let Some(rd) = &snapshot.review_due {
        if !rd.is_empty()
            && chrono::NaiveDate::parse_from_str(rd, "%Y-%m-%d").is_err()
        {
            gaps.push(format!("复核日期格式非法: {}", rd));
        }
    }

    // basis / source_ref 应为合法 JSON（schema 注释约定）
    for (label, val) in [("basis", &snapshot.basis), ("source_ref", &snapshot.source_ref)] {
        if let Some(v) = val {
            if !v.trim().is_empty() && serde_json::from_str::<serde_json::Value>(v).is_err() {
                gaps.push(format!("{} 不是合法 JSON，依据不可查", label));
            }
        }
    }

    // 引用实体存在性
    let exists_sql = match snapshot.entity_type.as_str() {
        "case" => Some("SELECT COUNT(*) FROM cases WHERE id = ?1"),
        "task" => Some("SELECT COUNT(*) FROM tasks WHERE id = ?1"),
        "knowledge" => Some("SELECT COUNT(*) FROM knowledge_items WHERE id = ?1"),
        "client" => Some("SELECT COUNT(*) FROM clients WHERE id = ?1"),
        _ => None,
    };
    if let Some(sql) = exists_sql {
        let count: i64 = conn
            .query_row(sql, params![snapshot.entity_id], |r| r.get(0))
            .unwrap_or(0);
        if count == 0 {
            gaps.push(format!(
                "关联实体不存在: {}[{}]",
                snapshot.entity_type, snapshot.entity_id
            ));
        }
    }

    gaps
}

/// 构建 AI 一致性核对提示词（独立提示词，不复用生成路径）
fn build_check_prompt(snapshot: &DecisionSnapshot, context_items: &[String]) -> String {
    format!(
        "请核对以下决策与数据是否一致，找出遗漏和矛盾。\n\n## 决策\n类型: {}\n内容: {}\n\n## 关联数据链\n{}",
        snapshot.decision_type,
        snapshot.decision,
        context_items.join("\n")
    )
}

/// 解析 AI 复核返回（宽松提取首个 JSON 对象）
fn parse_check_response(text: &str) -> Result<Vec<String>> {
    let trimmed = text.trim();
    let start = trimmed.find('{').unwrap_or(0);
    let end = trimmed.rfind('}').map(|i| i + 1).unwrap_or(trimmed.len());
    let parsed: serde_json::Value = serde_json::from_str(&trimmed[start..end])?;

    let gaps = parsed["gaps"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|g| g.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(gaps)
}

/// L3 决策确认后调用：执行递归确认核对（设计哲学 §11.5）
#[tauri::command]
pub async fn run_recursive_check(decision_id: String) -> Result<serde_json::Value, String> {
    let conn = crate::db::open_db().map_err(|e| e.to_string())?;
    recursive_check_decision(conn, &decision_id)
        .await
        .map_err(|e| e.to_string())
}

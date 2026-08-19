//! 隐性关联学习（设计哲学 §3.2 通道 B）
//!
//! 周期性把全局数据的有界摘要喂给 AI，产出"关联洞察"
//! （如"张三案与赵六案涉及同一专利""该客户历史常延期"），
//! 带置信度和来源落 `ai_insights`（status='pending'），用户确认后沉淀。
//!
//! 隐性关联本质是 AI 任务：AI 未配置时静默跳过（返回 Ok(0)），不做规则版。

use anyhow::Result;

/// 单轮最多落库的洞察条数
const MAX_INSIGHTS_PER_RUN: usize = 5;

/// 有界数据摘要（ContextPolicy 精神：各类条目硬上限）
fn collect_bounded_summary(conn: &rusqlite::Connection) -> Result<String> {
    let mut out = String::new();

    // 活跃案件 ≤30（名称/案号/客户/对方/轨道/状态）
    {
        let mut stmt = conn.prepare(
            "SELECT id, case_name, case_no, client_name, opponent_name, track, case_status
             FROM cases
             WHERE case_status != '已完结' OR case_status IS NULL
             ORDER BY updated_at DESC
             LIMIT 30",
        )?;
        let rows: Vec<String> = stmt
            .query_map([], |row| {
                let track: String = row.get(5)?;
                let track_label = match track.as_str() {
                    "patent_invalidation" => "专利无效",
                    "admin_litigation" => "行政诉讼",
                    "civil_tort" => "民事侵权",
                    _ => "其他",
                };
                Ok(format!(
                    "- [cases:{}] {} | 案号:{} | 客户:{} | 对方:{} | 轨道:{} | 状态:{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "无".into()),
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    track_label,
                    row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "未知".into()),
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        out.push_str("## 活跃案件\n");
        if rows.is_empty() {
            out.push_str("（无）\n");
        } else {
            out.push_str(&rows.join("\n"));
            out.push('\n');
        }
    }

    // 近 90 天 decisions ≤20
    {
        let cutoff = (chrono::Local::now() - chrono::Duration::days(90))
            .format("%Y-%m-%d")
            .to_string();
        let mut stmt = conn.prepare(
            "SELECT id, entity_type, decision_type, decision, created_at
             FROM decisions
             WHERE created_at >= ?1
             ORDER BY created_at DESC
             LIMIT 20",
        )?;
        let rows: Vec<String> = stmt
            .query_map(rusqlite::params![cutoff], |row| {
                Ok(format!(
                    "- [decisions:{}] 实体:{} | 类型:{} | 决策:{} | 时间:{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        out.push_str("\n## 近90天决策记录\n");
        if rows.is_empty() {
            out.push_str("（无）\n");
        } else {
            out.push_str(&rows.join("\n"));
            out.push('\n');
        }
    }

    // 近 30 天 task_events 统计（按事件类型聚合计数）
    {
        let cutoff = (chrono::Local::now() - chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();
        let mut stmt = conn.prepare(
            "SELECT event_type, COUNT(*) FROM task_events
             WHERE occurred_at >= ?1
             GROUP BY event_type
             ORDER BY COUNT(*) DESC",
        )?;
        let rows: Vec<String> = stmt
            .query_map(rusqlite::params![cutoff], |row| {
                Ok(format!(
                    "- {}: {} 次",
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        out.push_str("\n## 近30天任务事件统计\n");
        if rows.is_empty() {
            out.push_str("（无）\n");
        } else {
            out.push_str(&rows.join("\n"));
            out.push('\n');
        }
    }

    // waiting 超期清单（跟进日期已过且未完成，≤20 条）
    {
        let today = crate::db::today();
        let mut stmt = conn.prepare(
            "SELECT id, task_name, waiting_for, follow_up_date
             FROM tasks
             WHERE completed = 0 AND task_type = 'waiting'
               AND follow_up_date IS NOT NULL AND follow_up_date < ?1
             ORDER BY follow_up_date ASC
             LIMIT 20",
        )?;
        let rows: Vec<String> = stmt
            .query_map(rusqlite::params![today], |row| {
                Ok(format!(
                    "- [tasks:{}] {} | 等待:{} | 应跟进:{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "未知".into()),
                    row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        out.push_str("\n## waiting 超期清单\n");
        if rows.is_empty() {
            out.push_str("（无）\n");
        } else {
            out.push_str(&rows.join("\n"));
            out.push('\n');
        }
    }

    Ok(out)
}

/// AI 输出的单条洞察（宽松解析）
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawInsight {
    title: String,
    content: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
    #[serde(default)]
    source_refs: Vec<RawSourceRef>,
}

fn default_confidence() -> f64 {
    0.5
}

#[derive(Debug, serde::Deserialize)]
struct RawSourceRef {
    table: String,
    id: String,
}

/// 宽松解析 AI 响应中的 JSON 数组（允许前后有多余文字）
fn parse_insights(response: &str) -> Vec<RawInsight> {
    let start = response.find('[');
    let end = response.rfind(']');
    if let (Some(s), Some(e)) = (start, end) {
        if s < e {
            if let Ok(list) = serde_json::from_str::<Vec<RawInsight>>(&response[s..=e]) {
                return list;
            }
        }
    }
    Vec::new()
}

/// 生成隐性关联洞察：有界摘要 → AI → 落 ai_insights（pending，判重）
/// 返回新落库条数；AI 未配置/调用失败/解析失败均静默降级
pub fn generate_relation_insights(conn: &rusqlite::Connection) -> Result<usize> {
    let config = super::load_ai_config();
    if config.mode == "noop" {
        // AI 未配置：静默跳过（隐性关联本质是 AI 任务，不做规则版）
        return Ok(0);
    }

    let summary = collect_bounded_summary(conn)?;
    if summary.trim().is_empty() {
        return Ok(0);
    }

    let system_prompt = "你是专利律师工作中的关联分析助手。从给定的案件/决策/任务数据摘要中，\
        发掘人不易察觉的隐性关联，例如：不同案件涉及同一专利/同一当事人/同一对方、\
        某客户历史上的行为模式（常延期、常和解）、相似案件的不同走向等。\n\
        要求：\n\
        1. 只输出 JSON 数组，不要其他文字\n\
        2. 每条格式：{\"title\":\"一句话标题\",\"content\":\"具体说明（含推断依据）\",\
        \"confidence\":0.0-1.0,\"source_refs\":[{\"table\":\"cases\",\"id\":\"...\"}]}\n\
        3. source_refs 必须引用摘要中出现的 [表名:id] 标记，不得编造\n\
        4. 最多 5 条，宁缺毋滥；没有有价值关联时输出空数组 []";
    let user_prompt = format!("以下是工作数据摘要，请发掘隐性关联：\n\n{}", summary);

    use sha2::Digest;
    let input_hash = hex::encode(sha2::Sha256::digest(user_prompt.as_bytes()));
    let provider = config.mode.clone();
    let model = config.model.clone().unwrap_or_default();

    // 照 inbox.rs 的模式：在阻塞线程上起独立 runtime 调 async AI
    let backend = super::create_backend(&config);
    let rt = tokio::runtime::Runtime::new()?;
    let response = match rt.block_on(backend.chat_completion(system_prompt, &user_prompt)) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("关联洞察 AI 调用失败，本轮跳过: {}", e);
            let _ = crate::commands::ai_routes::log_ai_run(
                &provider, &model, "relation_insights", Some("v1"), &input_hash,
                None, "failed", Some(&e.to_string()),
            );
            return Ok(0);
        }
    };

    let output_hash = hex::encode(sha2::Sha256::digest(response.as_bytes()));
    let insights = parse_insights(&response);
    if insights.is_empty() {
        log::info!("关联洞察：AI 未产出有效结果");
        let _ = crate::commands::ai_routes::log_ai_run(
            &provider, &model, "relation_insights", Some("v1"), &input_hash,
            Some(&output_hash), "completed", None,
        );
        return Ok(0);
    }

    // 判重：同 title+entity 且未被 rejected 的不重复插
    let mut inserted = 0usize;
    for ins in insights.into_iter().take(MAX_INSIGHTS_PER_RUN) {
        if ins.title.trim().is_empty() || ins.content.trim().is_empty() {
            continue;
        }

        // entity 取首个来源引用（无来源则为空）
        let (entity_type, entity_id) = match ins.source_refs.first() {
            Some(r) => (Some(r.table.clone()), Some(r.id.clone())),
            None => (None, None),
        };

        let dup: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM ai_insights
                 WHERE title = ?1
                   AND COALESCE(entity_type, '') = COALESCE(?2, '')
                   AND COALESCE(entity_id, '') = COALESCE(?3, '')
                   AND status != 'rejected'",
                rusqlite::params![ins.title, entity_type, entity_id],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false);
        if dup {
            continue;
        }

        let source_ref = serde_json::to_string(&serde_json::json!(
            ins.source_refs
                .iter()
                .map(|r| serde_json::json!({"table": r.table, "id": r.id}))
                .collect::<Vec<_>>()
        ))
        .unwrap_or_default();

        conn.execute(
            "INSERT INTO ai_insights (id, insight_type, entity_type, entity_id, title, content,
             confidence, source_ref, status, ai_model)
             VALUES (?1, 'correlation', ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)",
            rusqlite::params![
                crate::db::new_id(),
                entity_type,
                entity_id,
                ins.title,
                ins.content,
                ins.confidence.clamp(0.0, 1.0),
                source_ref,
                model,
            ],
        )?;
        inserted += 1;
    }

    let _ = crate::commands::ai_routes::log_ai_run(
        &provider, &model, "relation_insights", Some("v1"), &input_hash,
        Some(&output_hash), "completed", None,
    );

    log::info!("关联洞察：落库 {} 条（待确认）", inserted);
    Ok(inserted)
}

// ============================================================
// 确认区（照 distillation 的 confirm/dismiss 模式）
// ============================================================

/// 列出待确认洞察
pub fn list_pending_insights(conn: &rusqlite::Connection) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, insight_type, entity_type, entity_id, title, content,
                confidence, source_ref, ai_model, created_at
         FROM ai_insights
         WHERE status = 'pending'
         ORDER BY created_at DESC",
    )?;

    let rows = stmt
        .query_map([], |row| {
            let source_ref: Option<String> = row.get(7)?;
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "insightType": row.get::<_, String>(1)?,
                "entityType": row.get::<_, Option<String>>(2)?,
                "entityId": row.get::<_, Option<String>>(3)?,
                "title": row.get::<_, String>(4)?,
                "content": row.get::<_, String>(5)?,
                "confidence": row.get::<_, f64>(6)?,
                "sourceRef": source_ref.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                "aiModel": row.get::<_, Option<String>>(8)?,
                "createdAt": row.get::<_, Option<String>>(9)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// 确认洞察：status → confirmed；可选沉淀 knowledge_items（经验类，照 confirm_memory）
pub fn confirm_insight(
    conn: &rusqlite::Connection,
    id: &str,
    sink_to_knowledge: bool,
) -> Result<serde_json::Value> {
    let (title, content): (String, String) = conn.query_row(
        "SELECT title, content FROM ai_insights WHERE id = ?1",
        rusqlite::params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    conn.execute(
        "UPDATE ai_insights SET status = 'confirmed' WHERE id = ?1",
        rusqlite::params![id],
    )?;

    let mut knowledge_id: Option<String> = None;
    if sink_to_knowledge {
        let kid = crate::db::new_id();
        conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content, tags, source_type, source_id)
             VALUES (?1, ?2, 'case_note', ?3, '隐性关联', 'insight', ?4)",
            rusqlite::params![kid, title, content, id],
        )?;
        knowledge_id = Some(kid);
    }

    log::info!("洞察已确认: {} (沉淀知识库: {})", id, sink_to_knowledge);
    Ok(serde_json::json!({
        "id": id,
        "status": "confirmed",
        "knowledgeId": knowledge_id,
    }))
}

/// 丢弃洞察：status → rejected
pub fn dismiss_insight(conn: &rusqlite::Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE ai_insights SET status = 'rejected' WHERE id = ?1",
        rusqlite::params![id],
    )?;
    log::info!("洞察已丢弃: {}", id);
    Ok(())
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_insights_clean_json() {
        let resp = r#"[{"title":"同一专利","content":"A案与B案涉及同一专利","confidence":0.8,"source_refs":[{"table":"cases","id":"c1"}]}]"#;
        let list = parse_insights(resp);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "同一专利");
        assert!((list[0].confidence - 0.8).abs() < 1e-9);
        assert_eq!(list[0].source_refs[0].id, "c1");
    }

    #[test]
    fn test_parse_insights_with_surrounding_text() {
        let resp = "好的，分析结果如下：\n[{\"title\":\"t\",\"content\":\"c\"}]\n以上。";
        let list = parse_insights(resp);
        assert_eq!(list.len(), 1);
        assert!((list[0].confidence - 0.5).abs() < 1e-9); // 缺省置信度
        assert!(list[0].source_refs.is_empty());
    }

    #[test]
    fn test_parse_insights_empty_and_garbage() {
        assert!(parse_insights("[]").is_empty());
        assert!(parse_insights("没有发现关联").is_empty());
        assert!(parse_insights("[{broken json]").is_empty());
    }
}

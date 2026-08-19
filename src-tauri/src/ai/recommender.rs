//! AI 推荐决策引擎（设计哲学 §11.6）
//!
//! 从 task_events + decisions 抽取信息 → 规则/AI 生成推荐 → 写回 today_index
//! 当前为规则排序版本，AI 推荐需配置后端后启用。

use anyhow::Result;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 推荐项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub task_id: String,
    pub task_name: String,
    pub case_id: Option<String>,
    pub case_name: Option<String>,
    pub reason: String,
    pub score: f64,
    pub priority: String,
    pub due_date: Option<String>,
    pub estimated_minutes: Option<i64>,
    pub context: Option<String>,
}

/// 推荐结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResult {
    pub recommendations: Vec<Recommendation>,
    pub followup_suggestions: Vec<FollowupSuggestion>,
    pub source: String,
    pub generated_at: String,
}

/// 等待跟进建议
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowupSuggestion {
    pub task_id: String,
    pub task_name: String,
    pub waiting_for: Option<String>,
    pub waiting_days: i64,
    pub reason: String,
    pub action: String,
}

/// 生成今日推荐（规则排序）
pub fn generate_today_recommendations(conn: &rusqlite::Connection) -> Result<RecommendationResult> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today_dt = NaiveDate::parse_from_str(&today, "%Y-%m-%d")?;

    // 获取所有未完成任务
    let mut stmt = conn.prepare(
        "SELECT t.id, t.task_name, t.case_id, t.priority, t.due_date, t.deadline,
                t.estimated_minutes, t.context, t.task_type, t.start_date, t.start_bucket,
                t.flagged, t.blocked, c.case_name
         FROM tasks t
         LEFT JOIN cases c ON c.id = t.case_id
         WHERE t.completed = 0
         ORDER BY t.priority, t.due_date"
    )?;

    let mut recommendations = Vec::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,      // id
            row.get::<_, String>(1)?,      // task_name
            row.get::<_, Option<String>>(2)?, // case_id
            row.get::<_, String>(3)?,      // priority
            row.get::<_, Option<String>>(4)?, // due_date
            row.get::<_, Option<String>>(5)?, // deadline
            row.get::<_, Option<i64>>(6)?, // estimated_minutes
            row.get::<_, Option<String>>(7)?, // context
            row.get::<_, String>(8)?,      // task_type
            row.get::<_, Option<String>>(9)?, // start_date
            row.get::<_, String>(10)?,     // start_bucket
            row.get::<_, i32>(11)?,        // flagged
            row.get::<_, i32>(12)?,        // blocked
            row.get::<_, Option<String>>(13)?, // case_name
        ))
    })?;

    for row in rows {
        let (id, name, case_id, priority, due_date, deadline, est_min, context,
             task_type, start_date, start_bucket, flagged, blocked, case_name) = row?;

        // 跳过已阻塞的任务
        if blocked != 0 { continue; }
        // 跳过收件箱和某天
        if start_bucket == "inbox" || start_bucket == "someday" { continue; }
        // 跳过等待类型
        if task_type == "waiting" { continue; }

        let effective_due = due_date.as_deref().or(deadline.as_deref());

        // 计算推荐分数
        let mut score = 0.0;
        let mut reasons = Vec::new();

        // 1. 到期日紧迫度
        if let Some(due) = effective_due {
            if let Ok(due_dt) = NaiveDate::parse_from_str(due, "%Y-%m-%d") {
                let days_left = (due_dt - today_dt).num_days();
                if days_left < 0 {
                    score += 100.0;
                    reasons.push(format!("已逾期 {} 天", -days_left));
                } else if days_left == 0 {
                    score += 80.0;
                    reasons.push("今天到期".to_string());
                } else if days_left <= 1 {
                    score += 60.0;
                    reasons.push("明天到期".to_string());
                } else if days_left <= 3 {
                    score += 40.0;
                    reasons.push(format!("{} 天后到期", days_left));
                }
            }
        }

        // 2. 旗标加分
        if flagged != 0 {
            score += 30.0;
            reasons.push("已标记重要".to_string());
        }

        // 3. 优先级加分
        match priority.as_str() {
            "urgent_important" => { score += 50.0; reasons.push("紧急重要".to_string()); }
            "urgent" => { score += 35.0; }
            "important" => { score += 25.0; }
            _ => {}
        }

        // 4. 今日桶加分
        if start_bucket == "today" {
            score += 20.0;
            reasons.push("已加入今日".to_string());
        }

        // 5. 开始日期已到
        if let Some(sd) = &start_date {
            if let Ok(start_dt) = NaiveDate::parse_from_str(sd, "%Y-%m-%d") {
                if start_dt <= today_dt {
                    score += 15.0;
                }
            }
        }

        // 基础分
        if score == 0.0 { score = 5.0; }

        let reason = if reasons.is_empty() {
            "可随时处理".to_string()
        } else {
            reasons.join(" · ")
        };

        recommendations.push(Recommendation {
            task_id: id,
            task_name: name,
            case_id: case_id.clone(),
            case_name: case_name.clone(),
            reason,
            score,
            priority,
            due_date: effective_due.map(String::from),
            estimated_minutes: est_min,
            context,
        });
    }

    // 按分数降序排列
    recommendations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    recommendations.truncate(5);

    // 等待跟进建议
    let followup_suggestions = generate_followup_suggestions(conn)?;

    Ok(RecommendationResult {
        recommendations,
        followup_suggestions,
        source: "rule_engine".to_string(),
        generated_at: chrono::Local::now().to_rfc3339(),
    })
}

/// 生成等待跟进建议
fn generate_followup_suggestions(conn: &rusqlite::Connection) -> Result<Vec<FollowupSuggestion>> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT t.id, t.task_name, t.waiting_for, t.follow_up_date, t.case_id
         FROM tasks t
         WHERE t.completed = 0 AND t.task_type = 'waiting'"
    )?;

    let mut suggestions = Vec::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for row in rows {
        let (id, name, waiting_for, follow_up_date, _case_id) = row?;

        let waiting_days = if let Some(ref fud) = follow_up_date {
            if let Ok(fup) = NaiveDate::parse_from_str(fud, "%Y-%m-%d") {
                let today_dt = NaiveDate::parse_from_str(&today, "%Y-%m-%d").unwrap();
                (today_dt - fup).num_days().max(0)
            } else { 0 }
        } else { 0 };

        if waiting_days >= 3 {
            let reason = format!(
                "已等 {} 天（等待 {}）",
                waiting_days,
                waiting_for.as_deref().unwrap_or("未知")
            );
            suggestions.push(FollowupSuggestion {
                task_id: id,
                task_name: name,
                waiting_for,
                waiting_days,
                reason,
                action: if waiting_days >= 7 { "建议催办".to_string() } else { "关注".to_string() },
            });
        }
    }

    suggestions.sort_by(|a, b| b.waiting_days.cmp(&a.waiting_days));
    Ok(suggestions)
}

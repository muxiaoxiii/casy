//! 自动报表系统（设计哲学 §11.3）
//!
//! 每日早报 / 每周总结
//! 对内结构化数据（daily_stats，纯 SQL 统计）→ 对外可读叙事（smart_summaries，规则拼接 Markdown）
//! 统计永远本地 SQL 计算，不经过 AI 路径，保证确定性。

use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// 每日早报
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyBrief {
    pub date: String,
    pub yesterday_review: YesterdayReview,
    pub today_focus: TodayFocus,
    pub waiting_alerts: Vec<WaitingAlert>,
    pub smart_suggestions: Vec<String>,
    /// 人读版 Markdown（同步写入 smart_summaries）
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YesterdayReview {
    pub tasks_completed: i64,
    pub tasks_total: i64,
    pub completion_rate: f64,
    pub compared_to_last_week: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayFocus {
    pub hard_schedule: Vec<ScheduleItem>,
    pub due_today: Vec<TaskItem>,
    pub next_actions: Vec<TaskItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleItem {
    pub title: String,
    pub time: String,
    pub case_id: Option<String>,
    pub event_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: String,
    pub name: String,
    pub case_id: Option<String>,
    pub due_date: Option<String>,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitingAlert {
    pub task_name: String,
    pub waiting_for: String,
    pub waiting_days: i64,
    pub suggestion: String,
}

/// 每周总结
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklySummary {
    pub week_start: String,
    pub week_end: String,
    pub tasks_completed: i64,
    pub overdue_count: i64,
    pub overdue_rate: f64,
    pub case_transitions: Vec<CaseTransition>,
    pub time_by_track: Vec<TrackTime>,
    pub next_week_hearings: Vec<ScheduleItem>,
    pub next_week_deadlines: Vec<DeadlineItem>,
    /// 人读版 Markdown（同步写入 smart_summaries）
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseTransition {
    pub case_id: String,
    pub case_name: Option<String>,
    pub track: String,
    pub from_status: Option<String>,
    pub to_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackTime {
    pub track: String,
    pub task_count: i64,
    pub actual_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineItem {
    pub id: String,
    pub case_id: String,
    pub deadline_name: String,
    pub due_date: String,
}

// ============================================================
// 每日早报
// ============================================================

/// 生成每日早报：先落 daily_stats（结构化统计），再落 smart_summaries（Markdown）
/// 重复生成同一天：UPDATE 而不是插重复行
pub fn generate_daily_brief(conn: &rusqlite::Connection, date: &str) -> Result<DailyBrief> {
    let date_dt = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
    let yesterday = (date_dt - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    // 昨日回顾
    let yesterday_review = generate_yesterday_review(conn, &yesterday)?;

    // 今日焦点
    let today_focus = generate_today_focus(conn, date)?;

    // 等待预警
    let waiting_alerts = generate_waiting_alerts(conn, date)?;

    // 智能建议
    let smart_suggestions = generate_smart_suggestions(conn, date)?;

    let mut brief = DailyBrief {
        date: date.to_string(),
        yesterday_review,
        today_focus,
        waiting_alerts,
        smart_suggestions,
        markdown: String::new(),
    };

    // Step 1: 结构化统计落库 daily_stats（UPSERT by date）
    upsert_daily_stats(conn, date, &brief)?;

    // Step 2: 人读版 Markdown 落库 smart_summaries（按 summary_type+period_start 判重）
    brief.markdown = render_daily_markdown(&brief);
    upsert_summary(
        conn,
        "daily",
        date,
        date,
        &format!("每日早报 {}", date),
        &brief.markdown,
        &serde_json::to_string(&brief).unwrap_or_default(),
    )?;

    log::info!("每日早报已生成并落库: {}", date);
    Ok(brief)
}

/// 当日结构化统计写入 daily_stats（对内，纯 SQL）
fn upsert_daily_stats(conn: &rusqlite::Connection, date: &str, brief: &DailyBrief) -> Result<()> {
    // 当日完成任务数（以 task_events 为准，tasks 表无 completed_at 列）
    let task_done: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_events WHERE event_type = 'completed' AND date(occurred_at) = ?1",
            params![date],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 截至当日累计任务数
    let task_total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE date(created_at) <= ?1",
            params![date],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 未完成任务中已逾期数量与累计逾期天数
    let (overdue_count, overdue_days): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(julianday(?1) - julianday(COALESCE(t.due_date, t.deadline))), 0)
             FROM tasks t
             WHERE t.completed = 0 AND COALESCE(t.due_date, t.deadline) IS NOT NULL
               AND COALESCE(t.due_date, t.deadline) < ?1",
            params![date],
            |r| Ok((r.get(0)?, r.get::<_, f64>(1)? as i64)),
        )
        .unwrap_or((0, 0));

    // 当日开庭数
    let hearing_count = brief.today_focus.hard_schedule.len() as i64;

    // 当日到期期限数
    let deadline_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM case_deadlines WHERE completed = 0 AND due_date = ?1",
            params![date],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 等待超过 3 天的任务数
    let waiting_overdue_3d = brief.waiting_alerts.len() as i64;

    // 当日案件状态变迁（JSON）
    let transitions = query_case_transitions(conn, date, date)?;
    let case_transitions = serde_json::to_string(&transitions).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO daily_stats (id, date, task_done, task_total, overdue_count, overdue_days,
                                  hearing_count, deadline_count, waiting_overdue_3d, case_transitions)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(date) DO UPDATE SET
           task_done = excluded.task_done,
           task_total = excluded.task_total,
           overdue_count = excluded.overdue_count,
           overdue_days = excluded.overdue_days,
           hearing_count = excluded.hearing_count,
           deadline_count = excluded.deadline_count,
           waiting_overdue_3d = excluded.waiting_overdue_3d,
           case_transitions = excluded.case_transitions",
        params![
            crate::db::new_id(),
            date,
            task_done,
            task_total,
            overdue_count,
            overdue_days,
            hearing_count,
            deadline_count,
            waiting_overdue_3d,
            case_transitions,
        ],
    )?;

    Ok(())
}

/// smart_summaries 判重写入：按 summary_type + period_start 查重，存在则 UPDATE
fn upsert_summary(
    conn: &rusqlite::Connection,
    summary_type: &str,
    period_start: &str,
    period_end: &str,
    title: &str,
    content: &str,
    structured_data: &str,
) -> Result<()> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM smart_summaries WHERE summary_type = ?1 AND period_start = ?2
             ORDER BY created_at DESC LIMIT 1",
            params![summary_type, period_start],
            |r| r.get(0),
        )
        .ok();

    if let Some(id) = existing {
        // 规则版重算时重置叙事来源，叙事层随后会再尝试覆盖（§11.3 / §12.5）
        conn.execute(
            "UPDATE smart_summaries SET title = ?1, content = ?2, structured_data = ?3,
                                      period_end = ?4, status = 'confirmed', narrative_source = 'rule'
             WHERE id = ?5",
            params![title, content, structured_data, period_end, id],
        )?;
    } else {
        conn.execute(
            "INSERT INTO smart_summaries (id, summary_type, title, content, structured_data,
                                          status, period_start, period_end, narrative_source)
             VALUES (?1, ?2, ?3, ?4, ?5, 'confirmed', ?6, ?7, 'rule')",
            params![
                crate::db::new_id(),
                summary_type,
                title,
                content,
                structured_data,
                period_start,
                period_end,
            ],
        )?;
    }

    Ok(())
}

// ============================================================
// 叙事层（设计哲学 §11.3 对外版 / §12.5 降级铁律）
// ============================================================

/// 叙事层：AI 可用时把规则版结构化数据改写为自然叙事版，覆盖 smart_summaries.content。
///
/// 调用前提：规则版已落库（确定性数据一定在）。
/// AI 未配置 / 调用失败 / 返回为空 → 静默回退规则版，返回 Ok(false)，不抛错。
/// 每次 AI 调用均写 ai_runs 审计（purpose 由调用方指定）。
pub async fn try_narrative_layer(
    summary_type: &str,
    period_start: &str,
    purpose: &str,
) -> Result<bool> {
    let config = crate::ai::load_ai_config();
    if config.mode == "noop" {
        // AI 未配置：静默使用规则版
        return Ok(false);
    }

    let conn = crate::db::open_db()?;
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT id, structured_data FROM smart_summaries
             WHERE summary_type = ?1 AND period_start = ?2
             ORDER BY created_at DESC LIMIT 1",
            params![summary_type, period_start],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();

    let Some((summary_id, structured)) = row else {
        return Ok(false);
    };
    let structured = structured.unwrap_or_default();
    if structured.is_empty() {
        return Ok(false);
    }

    let label = if summary_type == "daily" { "每日早报" } else { "每周总结" };
    let system_prompt = "你是专利律师的工作助手。把给定的结构化报表数据（JSON）改写为自然、连贯、易读的中文叙事版报告（Markdown）。\
        不得编造数据中没有的事实、数字或日期，保留所有关键数字与名称。只输出报告正文，不要额外解释。";
    let user_prompt = format!(
        "以下是{}的结构化数据，请改写为叙事版报告：\n\n{}",
        label,
        structured.chars().take(4000).collect::<String>()
    );

    use sha2::Digest;
    let input_hash = hex::encode(sha2::Sha256::digest(user_prompt.as_bytes()));
    let provider = config.mode.clone();
    let model = config.model.clone().unwrap_or_default();
    let backend = crate::ai::create_backend(&config);

    match backend.chat_completion(system_prompt, &user_prompt).await {
        Ok(narrative) if !narrative.trim().is_empty() => {
            let output_hash = hex::encode(sha2::Sha256::digest(narrative.as_bytes()));
            // 审计（失败不阻塞主流程）
            if let Err(e) = crate::commands::ai_routes::log_ai_run(
                &provider,
                &model,
                purpose,
                Some("v1"),
                &input_hash,
                Some(&output_hash),
                "completed",
                None,
            ) {
                log::warn!("AI 审计日志写入失败: {}", e);
            }

            let conn = crate::db::open_db()?;
            conn.execute(
                "UPDATE smart_summaries SET content = ?1, narrative_source = 'ai', ai_model = ?2
                 WHERE id = ?3",
                params![narrative, model, summary_id],
            )?;
            log::info!("{}叙事版已生成（AI: {}）", label, model);
            Ok(true)
        }
        Ok(_) => {
            log::warn!("叙事层返回空内容，回退规则版");
            let _ = crate::commands::ai_routes::log_ai_run(
                &provider, &model, purpose, Some("v1"), &input_hash, None, "failed",
                Some("AI 返回空内容"),
            );
            Ok(false)
        }
        Err(e) => {
            // §12.5 降级铁律：失败静默回退规则版，只记日志 + 审计
            log::warn!("叙事层生成失败，回退规则版: {}", e);
            let _ = crate::commands::ai_routes::log_ai_run(
                &provider, &model, purpose, Some("v1"), &input_hash, None, "failed",
                Some(&e.to_string()),
            );
            Ok(false)
        }
    }
}

fn generate_yesterday_review(conn: &rusqlite::Connection, yesterday: &str) -> Result<YesterdayReview> {
    // tasks 表没有 completed_at 列，完成数以 task_events 的 completed 事件为准
    let completed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_events WHERE event_type = 'completed' AND date(occurred_at) = ?1",
            params![yesterday],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE date(created_at) <= ?1",
            params![yesterday],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 上周同日完成数，用于对比
    let last_week_dt = NaiveDate::parse_from_str(yesterday, "%Y-%m-%d")
        .map(|d| d - chrono::Duration::days(7))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let last_week_completed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_events WHERE event_type = 'completed' AND date(occurred_at) = ?1",
            params![last_week_dt],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let completion_rate = if total > 0 { completed as f64 / total as f64 } else { 0.0 };
    let compared_to_last_week = if completed > last_week_completed {
        format!("较上周同日多完成 {} 项", completed - last_week_completed)
    } else if completed < last_week_completed {
        format!("较上周同日少完成 {} 项", last_week_completed - completed)
    } else {
        "持平".to_string()
    };

    Ok(YesterdayReview {
        tasks_completed: completed,
        tasks_total: total,
        completion_rate,
        compared_to_last_week,
    })
}

fn generate_today_focus(conn: &rusqlite::Connection, today: &str) -> Result<TodayFocus> {
    // 硬性日程：今日庭审（hearings 表真实列为 hearing_name / hearing_date）
    let mut stmt = conn.prepare(
        "SELECT id, hearing_name, hearing_date, case_id FROM hearings
         WHERE date(hearing_date) = ?1 ORDER BY hearing_date",
    )?;

    let hard_schedule: Vec<ScheduleItem> = stmt
        .query_map(params![today], |row| {
            let hearing_date: String = row.get(2)?;
            // hearing_date 可能含时间部分（YYYY-MM-DD HH:MM），提取时间
            let time = if hearing_date.len() > 10 {
                hearing_date[11..].to_string()
            } else {
                String::new()
            };
            Ok(ScheduleItem {
                title: row.get::<_, Option<String>>(1)?.unwrap_or_else(|| "庭审".to_string()),
                time,
                case_id: row.get(3)?,
                event_type: "hearing".to_string(),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 今日到期任务
    let mut stmt = conn.prepare(
        "SELECT id, task_name, case_id, COALESCE(due_date, deadline), COALESCE(priority, 'normal') FROM tasks
         WHERE completed = 0 AND (due_date = ?1 OR deadline = ?1)
         ORDER BY priority",
    )?;

    let due_today: Vec<TaskItem> = stmt
        .query_map(params![today], |row| {
            Ok(TaskItem {
                id: row.get(0)?,
                name: row.get(1)?,
                case_id: row.get(2)?,
                due_date: row.get(3)?,
                priority: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // 下一步行动
    let mut stmt = conn.prepare(
        "SELECT id, task_name, case_id, COALESCE(due_date, deadline), COALESCE(priority, 'normal') FROM tasks
         WHERE completed = 0 AND task_type = 'action' AND blocked = 0
         ORDER BY priority LIMIT 3",
    )?;

    let next_actions: Vec<TaskItem> = stmt
        .query_map([], |row| {
            Ok(TaskItem {
                id: row.get(0)?,
                name: row.get(1)?,
                case_id: row.get(2)?,
                due_date: row.get(3)?,
                priority: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(TodayFocus {
        hard_schedule,
        due_today,
        next_actions,
    })
}

fn generate_waiting_alerts(conn: &rusqlite::Connection, today: &str) -> Result<Vec<WaitingAlert>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_name, waiting_for, follow_up_date FROM tasks
         WHERE completed = 0 AND task_type = 'waiting'",
    )?;

    let mut alerts = Vec::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    let today_dt = NaiveDate::parse_from_str(today, "%Y-%m-%d")?;

    for row in rows {
        let (_id, name, waiting_for, follow_up_date) = row?;
        let waiting_days = if let Some(ref fud) = follow_up_date {
            if let Ok(fup) = NaiveDate::parse_from_str(fud, "%Y-%m-%d") {
                (today_dt - fup).num_days().max(0)
            } else {
                0
            }
        } else {
            0
        };

        if waiting_days >= 3 {
            alerts.push(WaitingAlert {
                task_name: name,
                waiting_for: waiting_for.unwrap_or_else(|| "未知".to_string()),
                waiting_days,
                suggestion: if waiting_days >= 7 { "建议催办".to_string() } else { "关注中".to_string() },
            });
        }
    }

    Ok(alerts)
}

fn generate_smart_suggestions(conn: &rusqlite::Connection, today: &str) -> Result<Vec<String>> {
    let mut suggestions = Vec::new();

    // 检查今日是否有多个庭审
    let hearing_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM hearings WHERE date(hearing_date) = ?1",
            params![today],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if hearing_count >= 2 {
        suggestions.push("今日有多个庭审，请提前准备材料".to_string());
    }

    // 检查是否有同客户的多个案件
    let mut stmt = conn.prepare(
        "SELECT client_name, COUNT(*) as cnt FROM cases
         WHERE case_status != '已完结'
         GROUP BY client_name HAVING cnt >= 3",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    for row in rows {
        let (client, count) = row?;
        suggestions.push(format!("{} 名下有 {} 个活跃案件，建议统一梳理", client, count));
    }

    Ok(suggestions)
}

// ============================================================
// 每周总结
// ============================================================

/// 生成每周总结（本周一至周日），全部 SQL 聚合 + 规则拼接 Markdown，不调 LLM
pub fn generate_weekly_summary(conn: &rusqlite::Connection) -> Result<WeeklySummary> {
    let today = chrono::Local::now().date_naive();
    let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let sunday = monday + chrono::Duration::days(6);
    let week_start = monday.format("%Y-%m-%d").to_string();
    let week_end = sunday.format("%Y-%m-%d").to_string();
    let today_s = today.format("%Y-%m-%d").to_string();

    // 本周完成任务数
    let tasks_completed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_events
             WHERE event_type = 'completed' AND date(occurred_at) BETWEEN ?1 AND ?2",
            params![week_start, week_end],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 当前逾期数（截至今天）
    let overdue_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE completed = 0 AND COALESCE(due_date, deadline) IS NOT NULL
               AND COALESCE(due_date, deadline) < ?1",
            params![today_s],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 逾期率 = 逾期数 /（本周完成 + 当前未完成且有截止日期的任务数）
    let open_with_due: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE completed = 0 AND COALESCE(due_date, deadline) IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let base = tasks_completed + open_with_due;
    let overdue_rate = if base > 0 { overdue_count as f64 / base as f64 } else { 0.0 };

    // 本周案件状态变迁（读 case_track_history）
    let case_transitions = query_case_transitions(conn, &week_start, &week_end)?;

    // 时间分布：本周完成任务按案件轨道聚合实际耗时
    let mut stmt = conn.prepare(
        "SELECT COALESCE(c.track, '未关联案件') as track, COUNT(*) as cnt,
                COALESCE(SUM(t.actual_minutes), 0) as minutes
         FROM task_events te
         JOIN tasks t ON t.id = te.task_id
         LEFT JOIN cases c ON c.id = t.case_id
         WHERE te.event_type = 'completed' AND date(te.occurred_at) BETWEEN ?1 AND ?2
         GROUP BY track ORDER BY minutes DESC",
    )?;
    let time_by_track: Vec<TrackTime> = stmt
        .query_map(params![week_start, week_end], |row| {
            Ok(TrackTime {
                track: row.get(0)?,
                task_count: row.get(1)?,
                actual_minutes: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 下周预告：开庭
    let next_monday = monday + chrono::Duration::days(7);
    let next_sunday = sunday + chrono::Duration::days(7);
    let nw_start = next_monday.format("%Y-%m-%d").to_string();
    let nw_end = next_sunday.format("%Y-%m-%d").to_string();

    let mut stmt = conn.prepare(
        "SELECT id, hearing_name, hearing_date, case_id FROM hearings
         WHERE date(hearing_date) BETWEEN ?1 AND ?2 ORDER BY hearing_date",
    )?;
    let next_week_hearings: Vec<ScheduleItem> = stmt
        .query_map(params![nw_start, nw_end], |row| {
            let hearing_date: String = row.get(2)?;
            let time = if hearing_date.len() > 10 {
                hearing_date[11..].to_string()
            } else {
                hearing_date.clone()
            };
            Ok(ScheduleItem {
                title: row.get::<_, Option<String>>(1)?.unwrap_or_else(|| "庭审".to_string()),
                time,
                case_id: row.get(3)?,
                event_type: "hearing".to_string(),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 下周预告：到期期限
    let mut stmt = conn.prepare(
        "SELECT id, case_id, deadline_name, due_date FROM case_deadlines
         WHERE completed = 0 AND due_date BETWEEN ?1 AND ?2 ORDER BY due_date",
    )?;
    let next_week_deadlines: Vec<DeadlineItem> = stmt
        .query_map(params![nw_start, nw_end], |row| {
            Ok(DeadlineItem {
                id: row.get(0)?,
                case_id: row.get(1)?,
                deadline_name: row.get(2)?,
                due_date: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut summary = WeeklySummary {
        week_start: week_start.clone(),
        week_end: week_end.clone(),
        tasks_completed,
        overdue_count,
        overdue_rate,
        case_transitions,
        time_by_track,
        next_week_hearings,
        next_week_deadlines,
        markdown: String::new(),
    };

    summary.markdown = render_weekly_markdown(&summary);
    upsert_summary(
        conn,
        "weekly",
        &week_start,
        &week_end,
        &format!("每周总结 {} ~ {}", week_start, week_end),
        &summary.markdown,
        &serde_json::to_string(&summary).unwrap_or_default(),
    )?;

    log::info!("每周总结已生成并落库: {} ~ {}", week_start, week_end);
    Ok(summary)
}

/// 查询区间内的案件状态变迁
fn query_case_transitions(
    conn: &rusqlite::Connection,
    start: &str,
    end: &str,
) -> Result<Vec<CaseTransition>> {
    let mut stmt = conn.prepare(
        "SELECT h.case_id, c.case_name, h.track, h.from_status, h.to_status
         FROM case_track_history h
         LEFT JOIN cases c ON c.id = h.case_id
         WHERE date(h.changed_at) BETWEEN ?1 AND ?2
         ORDER BY h.changed_at",
    )?;

    let transitions = stmt
        .query_map(params![start, end], |row| {
            Ok(CaseTransition {
                case_id: row.get(0)?,
                case_name: row.get(1)?,
                track: row.get(2)?,
                from_status: row.get(3)?,
                to_status: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(transitions)
}

// ============================================================
// Markdown 渲染（规则拼接，不调 LLM）
// ============================================================

fn render_daily_markdown(brief: &DailyBrief) -> String {
    let mut md = String::new();
    md.push_str(&format!("# 每日早报 {}\n\n", brief.date));

    md.push_str("## 昨日回顾\n");
    md.push_str(&format!(
        "- 完成任务 {}/{} 项（完成率 {:.0}%），{}\n\n",
        brief.yesterday_review.tasks_completed,
        brief.yesterday_review.tasks_total,
        brief.yesterday_review.completion_rate * 100.0,
        brief.yesterday_review.compared_to_last_week,
    ));

    md.push_str("## 今日焦点\n");
    if brief.today_focus.hard_schedule.is_empty() {
        md.push_str("- 今日无庭审安排\n");
    } else {
        for item in &brief.today_focus.hard_schedule {
            let time = if item.time.is_empty() { "全天".to_string() } else { item.time.clone() };
            md.push_str(&format!("- ⏰ {} {}\n", time, item.title));
        }
    }
    for item in &brief.today_focus.due_today {
        md.push_str(&format!("- 📌 今日到期：{}\n", item.name));
    }
    md.push('\n');

    if !brief.today_focus.next_actions.is_empty() {
        md.push_str("## 建议下一步行动\n");
        for item in &brief.today_focus.next_actions {
            md.push_str(&format!("- {}\n", item.name));
        }
        md.push('\n');
    }

    if !brief.waiting_alerts.is_empty() {
        md.push_str("## 等待预警\n");
        for alert in &brief.waiting_alerts {
            md.push_str(&format!(
                "- 「{}」等待 {} 已 {} 天（{}）\n",
                alert.task_name, alert.waiting_for, alert.waiting_days, alert.suggestion
            ));
        }
        md.push('\n');
    }

    if !brief.smart_suggestions.is_empty() {
        md.push_str("## 智能建议\n");
        for s in &brief.smart_suggestions {
            md.push_str(&format!("- {}\n", s));
        }
    }

    md
}

fn render_weekly_markdown(s: &WeeklySummary) -> String {
    let mut md = String::new();
    md.push_str(&format!("# 每周总结 {} ~ {}\n\n", s.week_start, s.week_end));

    md.push_str("## 本周概览\n");
    md.push_str(&format!("- 完成任务：{} 项\n", s.tasks_completed));
    md.push_str(&format!("- 当前逾期：{} 项（逾期率 {:.0}%）\n\n", s.overdue_count, s.overdue_rate * 100.0));

    if !s.case_transitions.is_empty() {
        md.push_str("## 案件状态变迁\n");
        for t in &s.case_transitions {
            md.push_str(&format!(
                "- {}（{}）：{} → {}\n",
                t.case_name.as_deref().unwrap_or(&t.case_id),
                t.track,
                t.from_status.as_deref().unwrap_or("（初始）"),
                t.to_status,
            ));
        }
        md.push('\n');
    }

    if !s.time_by_track.is_empty() {
        md.push_str("## 时间分布\n");
        for t in &s.time_by_track {
            if t.actual_minutes > 0 {
                md.push_str(&format!(
                    "- {}：{} 项任务，实际耗时 {:.1} 小时\n",
                    t.track,
                    t.task_count,
                    t.actual_minutes as f64 / 60.0
                ));
            } else {
                md.push_str(&format!("- {}：{} 项任务（未记录耗时）\n", t.track, t.task_count));
            }
        }
        md.push('\n');
    }

    md.push_str("## 下周预告\n");
    if s.next_week_hearings.is_empty() && s.next_week_deadlines.is_empty() {
        md.push_str("- 下周暂无开庭或到期期限\n");
    } else {
        for h in &s.next_week_hearings {
            md.push_str(&format!("- ⚖️ 开庭：{}（{}）\n", h.title, h.time));
        }
        for d in &s.next_week_deadlines {
            md.push_str(&format!("- 📅 期限：{} 到期日 {}\n", d.deadline_name, d.due_date));
        }
    }

    md
}

// ============================================================
// 查询命令支撑
// ============================================================

/// 读取今日早报（smart_summaries），没有则现算
pub fn get_today_brief(conn: &rusqlite::Connection) -> Result<serde_json::Value> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    if let Some(row) = get_summary_row(conn, "daily", Some(&today))? {
        return Ok(row);
    }

    // 没有则现算（现算会落库）
    generate_daily_brief(conn, &today)?;
    get_summary_row(conn, "daily", Some(&today))?
        .ok_or_else(|| anyhow::anyhow!("今日早报生成后未找到记录"))
}

/// 读取最新一期周报，没有则现算
pub fn get_latest_weekly_summary(conn: &rusqlite::Connection) -> Result<serde_json::Value> {
    if let Some(row) = get_summary_row(conn, "weekly", None)? {
        return Ok(row);
    }

    generate_weekly_summary(conn)?;
    get_summary_row(conn, "weekly", None)?
        .ok_or_else(|| anyhow::anyhow!("每周总结生成后未找到记录"))
}

/// 查询 smart_summaries 行；period_start 为 None 时取该类型最新一期
fn get_summary_row(
    conn: &rusqlite::Connection,
    summary_type: &str,
    period_start: Option<&str>,
) -> Result<Option<serde_json::Value>> {
    let (sql, params_vec): (&str, Vec<String>) = match period_start {
        Some(ps) => (
            "SELECT id, title, content, structured_data, period_start, period_end, created_at, narrative_source
             FROM smart_summaries
             WHERE summary_type = ?1 AND period_start = ?2
             ORDER BY created_at DESC LIMIT 1",
            vec![summary_type.to_string(), ps.to_string()],
        ),
        None => (
            "SELECT id, title, content, structured_data, period_start, period_end, created_at, narrative_source
             FROM smart_summaries
             WHERE summary_type = ?1
             ORDER BY period_start DESC, created_at DESC LIMIT 1",
            vec![summary_type.to_string()],
        ),
    };

    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "content": row.get::<_, Option<String>>(2)?,
            "structuredData": row.get::<_, Option<String>>(3)?,
            "periodStart": row.get::<_, Option<String>>(4)?,
            "periodEnd": row.get::<_, Option<String>>(5)?,
            "createdAt": row.get::<_, Option<String>>(6)?,
            "narrativeSource": row.get::<_, Option<String>>(7)?,
        }))
    })?;

    match rows.next() {
        Some(Ok(v)) => Ok(Some(v)),
        _ => Ok(None),
    }
}

/// 列出报表历史（smart_summaries），供前端报表浏览（§11.3）
/// summary_type 可选过滤（daily/weekly/monthly/project/client），limit 缺省 50、封顶 200
pub fn list_summaries(
    conn: &rusqlite::Connection,
    summary_type: Option<&str>,
    limit: Option<i64>,
) -> Result<Vec<serde_json::Value>> {
    let limit = limit.unwrap_or(50).clamp(1, 200);

    // limit 已 clamp 为 i64，直接内联（无注入风险）；summary_type 走参数绑定
    let (sql, params_vec): (String, Vec<String>) = match summary_type.filter(|s| !s.is_empty()) {
        Some(st) => (
            format!(
                "SELECT id, summary_type, entity_type, entity_id, title, content, structured_data,
                        period_start, period_end, narrative_source, created_at
                 FROM smart_summaries
                 WHERE summary_type = ?1
                 ORDER BY period_start DESC, created_at DESC
                 LIMIT {}",
                limit
            ),
            vec![st.to_string()],
        ),
        None => (
            format!(
                "SELECT id, summary_type, entity_type, entity_id, title, content, structured_data,
                        period_start, period_end, narrative_source, created_at
                 FROM smart_summaries
                 ORDER BY period_start DESC, created_at DESC
                 LIMIT {}",
                limit
            ),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params_vec), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "summaryType": row.get::<_, String>(1)?,
                "entityType": row.get::<_, Option<String>>(2)?,
                "entityId": row.get::<_, Option<String>>(3)?,
                "title": row.get::<_, String>(4)?,
                "content": row.get::<_, Option<String>>(5)?,
                "structuredData": row.get::<_, Option<String>>(6)?,
                "periodStart": row.get::<_, Option<String>>(7)?,
                "periodEnd": row.get::<_, Option<String>>(8)?,
                "narrativeSource": row.get::<_, Option<String>>(9)?,
                "createdAt": row.get::<_, Option<String>>(10)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(rows)
}

//! 多通道提醒系统 — ReminderEngine + Tauri Commands
//!
//! 全局横切能力：定时检查期限/开庭/任务，按规则分发到各通道。

use super::run_blocking;
use crate::db;
use anyhow::Result;
use chrono::{NaiveDate, Timelike};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

/// 日历通道所需的实体上下文（dispatch_reminder 的 calendar 通道使用）
pub struct CalendarJobCtx {
    /// 'case' | 'task' | 'hearing' | 'deadline'
    pub entity_type: &'static str,
    pub entity_id: String,
    /// YYYY-MM-DD
    pub due_date: String,
    /// 期限名 / 庭审名 / 任务名
    pub title: String,
}

// ============================================================
// 数据结构
// ============================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReminderRule {
    pub id: String,
    pub name: String,
    pub trigger_type: String,
    pub trigger_value: Option<i64>,
    pub channels: String, // JSON array
    pub message_template: Option<String>,
    pub case_types: Option<String>,
    pub enabled: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReminderLogEntry {
    pub id: String,
    pub rule_id: String,
    pub case_id: Option<String>,
    pub task_id: Option<String>,
    pub channel: String,
    pub message: String,
    pub level: Option<String>,
    pub status: String,
    pub sent_at: Option<String>,
}

/// ReminderEngine — 全局提醒调度器
pub struct ReminderEngine {
    #[allow(dead_code)]
    pub check_interval_secs: u64,
    pub running: Arc<AtomicBool>,
}

impl ReminderEngine {
    pub fn new(check_interval_secs: u64) -> Self {
        Self {
            check_interval_secs,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 启动周期性检查循环（在 tokio::spawn 中运行）
    pub fn start_loop(&self) -> Arc<AtomicBool> {
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);
        running
    }

    /// 一次性检查所有规则并触发提醒
    pub fn check_and_trigger(&self, conn: &Connection) -> Result<Vec<ReminderLogEntry>> {
        let rules = load_enabled_rules(conn)?;
        let today = db::today();
        let today_dt =
            NaiveDate::parse_from_str(&today, "%Y-%m-%d").unwrap_or_else(|_| chrono::Local::now().date_naive());

        let mut triggered = Vec::new();

        for rule in &rules {
            match rule.trigger_type.as_str() {
                "deadline_before" | "deadline_on" | "deadline_after" => {
                    let entries = check_deadline_rules(conn, rule, today_dt)?;
                    triggered.extend(entries);
                }
                "hearing_before" => {
                    let entries = check_hearing_rules(conn, rule, today_dt)?;
                    triggered.extend(entries);
                }
                "task_due" | "task_overdue" => {
                    let entries = check_task_rules(conn, rule, today_dt)?;
                    triggered.extend(entries);
                }
                _ => {}
            }
        }

        // 顺带派发"到点的 pending local 延迟作业"（时段外延迟的 R1/R2 提醒），不另起定时器
        let delivered = dispatch_due_local_jobs(conn)?;
        triggered.extend(delivered);

        Ok(triggered)
    }
}

// ============================================================
// 规则检查逻辑
// ============================================================

fn load_enabled_rules(conn: &Connection) -> Result<Vec<ReminderRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, trigger_type, trigger_value, channels, message_template, case_types, enabled, created_at
         FROM reminder_rules WHERE enabled = 1",
    )?;

    let rules = stmt
        .query_map([], |row| {
            Ok(ReminderRule {
                id: row.get(0)?,
                name: row.get(1)?,
                trigger_type: row.get(2)?,
                trigger_value: row.get(3)?,
                channels: row.get(4)?,
                message_template: row.get(5)?,
                case_types: row.get(6)?,
                enabled: row.get::<_, i32>(7)? != 0,
                created_at: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(rules)
}

fn check_deadline_rules(
    conn: &Connection,
    rule: &ReminderRule,
    today: NaiveDate,
) -> Result<Vec<ReminderLogEntry>> {
    let trigger_days = rule.trigger_value.unwrap_or(0);
    let mut triggered = Vec::new();

    // 查询 case_deadlines 中未完成的期限
    let mut stmt = conn.prepare(
        "SELECT cd.id, cd.case_id, cd.deadline_name, cd.due_date, c.case_name
         FROM case_deadlines cd
         JOIN cases c ON c.id = cd.case_id
         WHERE cd.completed = 0 AND cd.due_date IS NOT NULL AND cd.due_date != ''",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    for row in rows {
        let (deadline_id, case_id, deadline_name, due_date_str, case_name) = row?;
        let Ok(due_date) = NaiveDate::parse_from_str(&due_date_str, "%Y-%m-%d") else {
            continue;
        };

        let days_diff = (due_date - today).num_days();

        // 区间触发（设计哲学 §11.2 M0 补偿扫描）：
        // 应用离线期间错过的 T-N 天预警，重启后在区间内补发；
        // before 类规则由 already_sent 窗口去重（过去 trigger_days 天内只发一次），
        // 不会在 T-3/T-2/T-1/T-0 每天轰炸；on/after 类按日去重。
        let should_trigger = match rule.trigger_type.as_str() {
            "deadline_before" => (0..=trigger_days).contains(&days_diff),
            "deadline_on" => days_diff <= 0,
            "deadline_after" => days_diff <= -trigger_days,
            _ => false,
        };

        if should_trigger {
            // before 类规则用窗口去重（过去 trigger_days 天内已发则跳过，避免区间每日轰炸）；
            // on/after 保持按日去重（每天一次逾期提醒是合理的）
            let window = (rule.trigger_type == "deadline_before").then_some(trigger_days);
            if already_sent(conn, &rule.id, Some(&case_id), None, window)? {
                continue;
            }

            let remain_text = if days_diff < 0 {
                format!("已逾期 {} 天", -days_diff)
            } else {
                format!("剩余: {} 天", days_diff)
            };
            let message = format!(
                "案件: {}\n期限: {}\n截止日期: {}\n{}",
                case_name, deadline_name, due_date_str, remain_text
            );

            let level = compute_level(days_diff, false);
            let channels: Vec<String> =
                serde_json::from_str(&rule.channels).unwrap_or_default();

            for channel in &channels {
                let cal_ctx = CalendarJobCtx {
                    entity_type: "deadline",
                    entity_id: deadline_id.clone(),
                    due_date: due_date_str.clone(),
                    title: deadline_name.clone(),
                };
                let entry = dispatch_reminder(
                    conn,
                    &rule.id,
                    Some(&case_id),
                    None,
                    channel,
                    &message,
                    level,
                    Some(&cal_ctx),
                )?;
                triggered.push(entry);
            }
        }
    }

    Ok(triggered)
}

fn check_hearing_rules(
    conn: &Connection,
    rule: &ReminderRule,
    today: NaiveDate,
) -> Result<Vec<ReminderLogEntry>> {
    let trigger_days = rule.trigger_value.unwrap_or(0);
    let mut triggered = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT h.id, h.case_id, h.hearing_name, h.hearing_date, c.case_name
         FROM hearings h
         JOIN cases c ON c.id = h.case_id
         WHERE h.hearing_date IS NOT NULL AND h.hearing_date != ''",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    for row in rows {
        let (hearing_id, case_id, hearing_name, hearing_date_str, case_name) = row?;
        let Ok(hearing_date) = NaiveDate::parse_from_str(&hearing_date_str, "%Y-%m-%d") else {
            continue;
        };

        let days_diff = (hearing_date - today).num_days();

        // 区间触发（§11.2 补偿扫描）：开庭前 N 天内（含当天）均可触发，错过可补发
        if (0..=trigger_days).contains(&days_diff) {
            // hearing_before 属区间触发：窗口去重，过去 trigger_days 天内已发则跳过
            if already_sent(conn, &rule.id, Some(&case_id), None, Some(trigger_days))? {
                continue;
            }

            let hearing_label = hearing_name.unwrap_or_else(|| "开庭/口审".to_string());
            let message = format!(
                "案件: {}\n庭审: {}\n日期: {}\n剩余: {} 天",
                case_name, hearing_label, hearing_date_str, days_diff
            );

            let level = compute_level(days_diff, false);
            let channels: Vec<String> =
                serde_json::from_str(&rule.channels).unwrap_or_default();

            for channel in &channels {
                let cal_ctx = CalendarJobCtx {
                    entity_type: "hearing",
                    entity_id: hearing_id.clone(),
                    due_date: hearing_date_str.clone(),
                    title: hearing_label.clone(),
                };
                let entry = dispatch_reminder(
                    conn,
                    &rule.id,
                    Some(&case_id),
                    None,
                    channel,
                    &message,
                    level,
                    Some(&cal_ctx),
                )?;
                triggered.push(entry);
            }
        }
    }

    Ok(triggered)
}

fn check_task_rules(
    conn: &Connection,
    rule: &ReminderRule,
    today: NaiveDate,
) -> Result<Vec<ReminderLogEntry>> {
    let mut triggered = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT t.id, t.case_id, t.task_name, t.deadline, COALESCE(c.case_name, '')
         FROM tasks t
         LEFT JOIN cases c ON c.id = t.case_id
         WHERE t.completed = 0 AND t.deadline IS NOT NULL AND t.deadline != ''",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    for row in rows {
        let (task_id, case_id, task_name, deadline_str, case_name) = row?;
        let Ok(deadline) = NaiveDate::parse_from_str(&deadline_str, "%Y-%m-%d") else {
            continue;
        };

        let days_diff = (deadline - today).num_days();

        let should_trigger = match rule.trigger_type.as_str() {
            "task_due" => days_diff <= 0,
            "task_overdue" => days_diff < 0,
            _ => false,
        };

        if should_trigger {
            // task_due/task_overdue 保持按日去重
            if already_sent(conn, &rule.id, case_id.as_deref(), Some(&task_id), None)? {
                continue;
            }

            let status_text = if days_diff < 0 {
                format!("已逾期 {} 天", -days_diff)
            } else {
                "今日到期".to_string()
            };

            let message = format!(
                "任务: {}\n关联案件: {}\n截止日期: {}\n状态: {}",
                task_name, case_name, deadline_str, status_text
            );

            let level = compute_level(days_diff, rule.trigger_type == "task_overdue");
            let channels: Vec<String> =
                serde_json::from_str(&rule.channels).unwrap_or_default();

            for channel in &channels {
                let cal_ctx = CalendarJobCtx {
                    entity_type: "task",
                    entity_id: task_id.clone(),
                    due_date: deadline_str.clone(),
                    title: task_name.clone(),
                };
                let entry = dispatch_reminder(
                    conn,
                    &rule.id,
                    case_id.as_deref(),
                    Some(&task_id),
                    channel,
                    &message,
                    level,
                    Some(&cal_ctx),
                )?;
                triggered.push(entry);
            }
        }
    }

    Ok(triggered)
}

// ============================================================
// 通道分发
// ============================================================

/// 计算 R1-R4 预警等级
///
/// - R1 温和：T-3 至 T-2 天（截止前较多余量）
/// - R2 明确：T-1 天
/// - R3 强提醒：T=0 当天到期
/// - R4 逾期：T<0 已超过截止
fn compute_level(days_diff: i64, is_overdue: bool) -> &'static str {
    if is_overdue || days_diff < 0 {
        "R4"
    } else if days_diff == 0 {
        "R3"
    } else if days_diff <= 1 {
        "R2"
    } else {
        "R1"
    }
}

fn dispatch_reminder(
    conn: &Connection,
    rule_id: &str,
    case_id: Option<&str>,
    task_id: Option<&str>,
    channel: &str,
    message: &str,
    level: &str,
    cal_ctx: Option<&CalendarJobCtx>,
) -> Result<ReminderLogEntry> {
    dispatch_reminder_at(
        conn,
        rule_id,
        case_id,
        task_id,
        channel,
        message,
        level,
        cal_ctx,
        chrono::Local::now().naive_local(),
    )
}

/// 带显式当前时刻的分发实现（便于单测控制时间）
///
/// 提醒时机智能（设计哲学 §11.2）：R1（温和）/R2（明确）级提醒在画像工作时段外
/// （深夜/清晨）不打扰，写入 reminder_jobs（executor='local'，scheduled_at=下一工作
/// 时段起点，status='pending'），由 check_and_trigger 到点派发。
/// R3（到期当天）/R4（逾期）不受时段限制，立即发。
/// calendar 通道自带定时投递语义（由日历服务商准时推送），不参与时段延迟。
#[allow(clippy::too_many_arguments)]
fn dispatch_reminder_at(
    conn: &Connection,
    rule_id: &str,
    case_id: Option<&str>,
    task_id: Option<&str>,
    channel: &str,
    message: &str,
    level: &str,
    cal_ctx: Option<&CalendarJobCtx>,
    now: chrono::NaiveDateTime,
) -> Result<ReminderLogEntry> {
    if (level == "R1" || level == "R2") && channel != "calendar" {
        let (start_hour, end_hour) = work_hours(conn);
        if let Some(next_start) = next_work_start(now, start_hour, end_hour) {
            if let Some(ctx) = cal_ctx {
                return defer_reminder_job(
                    conn, rule_id, case_id, task_id, channel, message, level, ctx, next_start,
                );
            }
        }
    }

    let log_id = db::new_id();

    let result = send_via_channel(conn, rule_id, case_id, task_id, Some(&log_id), channel, message, level, cal_ctx);

    let status = match result {
        Ok(_) => "sent",
        Err(_) => "failed",
    };
    let entry = ReminderLogEntry {
        id: log_id.clone(),
        rule_id: rule_id.to_string(),
        case_id: case_id.map(|s| s.to_string()),
        task_id: task_id.map(|s| s.to_string()),
        channel: channel.to_string(),
        message: message.to_string(),
        level: Some(level.to_string()),
        status: status.to_string(),
        sent_at: Some(db::now_local()),
    };

    // 写入日志
    conn.execute(
        "INSERT INTO reminder_log (id, rule_id, case_id, task_id, channel, message, level, status, sent_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now','localtime'))",
        params![
            entry.id,
            entry.rule_id,
            entry.case_id,
            entry.task_id,
            entry.channel,
            entry.message,
            entry.level,
            entry.status,
        ],
    )?;

    Ok(entry)
}

/// 通道发送（不含时段判断与日志写入）
fn send_via_channel(
    conn: &Connection,
    rule_id: &str,
    case_id: Option<&str>,
    task_id: Option<&str>,
    reminder_log_id: Option<&str>,
    channel: &str,
    message: &str,
    level: &str,
    cal_ctx: Option<&CalendarJobCtx>,
) -> Result<()> {
    match channel {
        "local" => send_local_notification(message, task_id, reminder_log_id),
        "system" => send_system_notification(message),
        "calendar" => dispatch_calendar_channel(conn, rule_id, case_id, message, level, cal_ctx),
        "feishu_message" => {
            // 异步发送飞书消息（不阻塞引擎循环）
            let msg = message.to_string();
            let rule_id = rule_id.to_string();
            let log_rule_id = rule_id.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = send_feishu_reminder_async_generic(&msg).await {
                    log::error!("飞书提醒发送失败 (rule {}): {}", rule_id, e);
                }
            });
            log::info!("[提醒-飞书消息] 已入队: {}", log_rule_id);
            Ok(())
        }
        "feishu_task" => {
            // 异步创建飞书任务
            let msg = message.to_string();
            let rule_id = rule_id.to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = send_feishu_task_async_generic(&msg).await {
                    log::error!("飞书任务提醒创建失败 (rule {}): {}", rule_id, e);
                }
            });
            log::info!("[提醒-飞书任务] 已入队");
            Ok(())
        }
        _ => Ok(()),
    }
}

// ============================================================
// 提醒时机智能（设计哲学 §11.2：懂你的节奏）
// ============================================================

/// 读取画像工作时段（settings.lawyer_profile.work_hours.start_hour/end_hour）
///
/// 无画像数据 / 解析失败 / 字段缺失时默认 9-21。
fn work_hours(conn: &Connection) -> (u32, u32) {
    let profile = db::get_setting(conn, "lawyer_profile")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());

    let hours = profile.as_ref().and_then(|p| p.get("work_hours"));
    let start = hours
        .and_then(|h| h.get("start_hour"))
        .and_then(|v| v.as_u64())
        .unwrap_or(9) as u32;
    let end = hours
        .and_then(|h| h.get("end_hour"))
        .and_then(|v| v.as_u64())
        .unwrap_or(21) as u32;

    (start.min(23), end.min(24))
}

/// 若 now 在工作时段外，返回下一个工作时段开始时刻；时段内返回 None
///
/// start_hour >= end_hour 视为异常配置，不做延迟。
fn next_work_start(
    now: chrono::NaiveDateTime,
    start_hour: u32,
    end_hour: u32,
) -> Option<chrono::NaiveDateTime> {
    if start_hour >= end_hour {
        return None;
    }
    let hour = now.hour();
    let target_date = if hour < start_hour {
        now.date()
    } else if hour >= end_hour {
        now.date() + chrono::Duration::days(1)
    } else {
        return None;
    };
    target_date.and_hms_opt(start_hour, 0, 0)
}

/// 时段外延迟：写 reminder_jobs（executor='local'，status='pending'）+ reminder_log（status='deferred'）
///
/// 延迟日志写 sent_at=now，使 already_sent 同日去重继续生效，
/// 避免每个检查周期重复创建延迟作业。
#[allow(clippy::too_many_arguments)]
fn defer_reminder_job(
    conn: &Connection,
    rule_id: &str,
    case_id: Option<&str>,
    task_id: Option<&str>,
    channel: &str,
    message: &str,
    level: &str,
    ctx: &CalendarJobCtx,
    scheduled: chrono::NaiveDateTime,
) -> Result<ReminderLogEntry> {
    let job_id = db::new_id();
    let scheduled_str = scheduled.format("%Y-%m-%d %H:%M:%S").to_string();

    conn.execute(
        "INSERT INTO reminder_jobs (id, rule_id, entity_type, entity_id, channel, executor,
           scheduled_at, content, due_snapshot, offset_snapshot, status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'local', ?6, ?7, ?8, ?9, 'pending')",
        params![
            job_id,
            rule_id,
            ctx.entity_type,
            ctx.entity_id,
            channel,
            scheduled_str,
            message,
            ctx.due_date,
            level,
        ],
    )?;

    log::info!(
        "[提醒-延迟] 时段外 {} 级提醒延迟到 {} 再发: {}",
        level,
        scheduled_str,
        message.replace('\n', " | ")
    );

    let log_id = db::new_id();
    let entry = ReminderLogEntry {
        id: log_id.clone(),
        rule_id: rule_id.to_string(),
        case_id: case_id.map(|s| s.to_string()),
        task_id: task_id.map(|s| s.to_string()),
        channel: channel.to_string(),
        message: message.to_string(),
        level: Some(level.to_string()),
        status: "deferred".to_string(),
        sent_at: Some(db::now_local()),
    };

    conn.execute(
        "INSERT INTO reminder_log (id, rule_id, case_id, task_id, channel, message, level, status, sent_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'deferred', datetime('now','localtime'))",
        params![
            entry.id,
            entry.rule_id,
            entry.case_id,
            entry.task_id,
            entry.channel,
            entry.message,
            entry.level,
        ],
    )?;

    Ok(entry)
}

/// 派发到点的 pending local 延迟作业（由 check_and_trigger 顺带调用）
///
/// 到点即发，不再做时段判断（派发时刻本身就在工作时段起点之后）。
fn dispatch_due_local_jobs(conn: &Connection) -> Result<Vec<ReminderLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, rule_id, entity_type, entity_id, channel, content, offset_snapshot
         FROM reminder_jobs
         WHERE executor = 'local' AND status = 'pending'
           AND scheduled_at <= datetime('now','localtime')
         ORDER BY scheduled_at",
    )?;

    let jobs = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut delivered = Vec::new();

    for (job_id, rule_id, entity_type, entity_id, channel, content, level) in jobs {
        let message = content.unwrap_or_default();
        let rule_id = rule_id.unwrap_or_default();
        let level = level.unwrap_or_else(|| "R1".to_string());
        // entity_type='task' 时 entity_id 即 task_id；case_id 无法从作业还原，记 NULL
        let task_id = (entity_type == "task").then_some(entity_id.as_str());

        let result = send_via_channel(conn, &rule_id, None, None, None, &channel, &message, &level, None);

        let (job_status, last_error) = match &result {
            Ok(_) => ("sent", None),
            Err(e) => {
                log::error!("[提醒-延迟作业] 投递失败 (job {}): {}", job_id, e);
                ("dead_lettered", Some(e.to_string()))
            }
        };

        conn.execute(
            "UPDATE reminder_jobs SET status = ?1, last_error = ?2, attempts = attempts + 1 WHERE id = ?3",
            params![job_status, last_error, job_id],
        )?;

        // 投递审计日志
        let log_id = db::new_id();
        conn.execute(
            "INSERT INTO reminder_log (id, rule_id, case_id, task_id, channel, message, level, status, sent_at)
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, datetime('now','localtime'))",
            params![log_id, rule_id, task_id, channel, message, level, job_status],
        )?;

        delivered.push(ReminderLogEntry {
            id: log_id,
            rule_id,
            case_id: None,
            task_id: task_id.map(|s| s.to_string()),
            channel,
            message,
            level: Some(level),
            status: job_status.to_string(),
            sent_at: Some(db::now_local()),
        });
    }

    if !delivered.is_empty() {
        log::info!("[提醒-延迟作业] 派发 {} 条到点延迟提醒", delivered.len());
    }

    Ok(delivered)
}

// ============================================================
// 日历通道（设计哲学 §11.2 M1：CalDAV 离线准时提醒）
// ============================================================

/// calendar 通道分发
///
/// 执行方互斥（架构铁律）：
/// - 已启用日历同步 → 创建 reminder_job（executor='calendar'）并异步 PUT 到 CalDAV；
///   同步成功则本地不再重复发，由日历服务商负责准时推送；
///   同步失败由 execute_calendar_job 回退本地提醒 + 记录 job 状态，不阻塞引擎。
/// - 未启用/未配置 → 保持 M0 本地提醒行为不变。
fn dispatch_calendar_channel(
    conn: &Connection,
    rule_id: &str,
    case_id: Option<&str>,
    message: &str,
    level: &str,
    cal_ctx: Option<&CalendarJobCtx>,
) -> Result<()> {
    if !crate::sync::caldav::calendar_sync_enabled(conn) {
        log::info!("[提醒-日历] 日历同步未启用，回退本地提醒");
        return send_local_notification(message, None, None);
    }

    let Some(ctx) = cal_ctx else {
        log::warn!("[提醒-日历] 缺少实体上下文，回退本地提醒");
        return send_local_notification(message, None, None);
    };

    let Some(dtstart) = super::caldav::parse_due_morning(&ctx.due_date) else {
        log::warn!("[提醒-日历] 无法解析截止日期 {}，回退本地提醒", ctx.due_date);
        return send_local_notification(message, None, None);
    };

    let summary = masked_calendar_summary(conn, case_id, &ctx.title);
    let description = format!(
        "期限：{}\n截止：{}\n等级：{}\n由 Casy 同步",
        ctx.title, ctx.due_date, level
    );
    let alarm_minutes = super::caldav::alarm_minutes_for_level(level);

    // ICS UID = reminder_jobs.id（不可变），PUT 同 UID 天然幂等
    let job_id = db::new_id();
    let account = db::get_setting(conn, "caldav_user").ok().flatten();

    conn.execute(
        "INSERT INTO reminder_jobs (id, rule_id, entity_type, entity_id, channel, executor,
           scheduled_at, calendar_account, content, masked_content, due_snapshot, offset_snapshot, status)
         VALUES (?1, ?2, ?3, ?4, 'calendar', 'calendar', datetime('now','localtime'), ?5, ?6, ?7, ?8, ?9, 'pending')",
        params![
            job_id,
            rule_id,
            ctx.entity_type,
            ctx.entity_id,
            account,
            message,
            summary,
            ctx.due_date,
            level,
        ],
    )?;

    // 异步执行 CalDAV 同步，不阻塞引擎循环
    let payload = super::caldav::CalendarJobPayload {
        job_id: job_id.clone(),
        uid: job_id,
        summary,
        description,
        dtstart,
        alarm_minutes,
        fallback_message: message.to_string(),
    };
    tauri::async_runtime::spawn(async move {
        super::caldav::execute_calendar_job(payload).await;
    });

    log::info!("[提醒-日历] 已创建同步作业并入队");
    Ok(())
}

/// 生成脱敏的日历事件标题
///
/// 默认脱敏（settings.calendar_mask_case_name 非 'false'）：
/// 优先用内部卷号（internal_no，非客户敏感字段）标识，否则用通用文案
/// "案件提醒：{期限名}"，不含案件名/当事人名。
/// 用户显式关闭脱敏时带案件名。
fn masked_calendar_summary(conn: &Connection, case_id: Option<&str>, title: &str) -> String {
    let mask = db::get_setting(conn, "calendar_mask_case_name")
        .ok()
        .flatten()
        .map(|v| v != "false")
        .unwrap_or(true);

    if let Some(cid) = case_id {
        if !mask {
            if let Ok(name) = conn.query_row(
                "SELECT case_name FROM cases WHERE id = ?1",
                [cid],
                |r| r.get::<_, String>(0),
            ) {
                return format!("{}：{}", name, title);
            }
        } else {
            let internal_no: Option<String> = conn
                .query_row(
                    "SELECT internal_no FROM cases WHERE id = ?1",
                    [cid],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten();
            if let Some(no) = internal_no.filter(|s| !s.trim().is_empty()) {
                return format!("案件提醒[{}]：{}", no.trim(), title);
            }
        }
    }

    format!("案件提醒：{}", title)
}

/// 检查是否已为该规则+案件/任务发送过提醒
///
/// - `window_days = None`：仅按当日去重（deadline_on/deadline_after/task_due/task_overdue，
///   每天一次逾期提醒是合理的）。
/// - `window_days = Some(n)`：窗口去重（deadline_before/hearing_before 区间触发专用）——
///   过去 n 天（含今天）内已发过同 rule+case+task 的提醒即跳过，
///   避免 T-N 区间规则在 T-3/T-2/T-1/T-0 每天轰炸一次。
fn already_sent(
    conn: &Connection,
    rule_id: &str,
    case_id: Option<&str>,
    task_id: Option<&str>,
    window_days: Option<i64>,
) -> Result<bool> {
    let count: i64 = match window_days {
        Some(days) => conn.query_row(
            "SELECT COUNT(*) FROM reminder_log
             WHERE rule_id = ?1
               AND date(sent_at) >= date(?2, ?5)
               AND (case_id = ?3 OR (?3 IS NULL AND case_id IS NULL))
               AND (task_id = ?4 OR (?4 IS NULL AND task_id IS NULL))",
            params![rule_id, db::today(), case_id, task_id, format!("-{} days", days)],
            |row| row.get(0),
        )?,
        None => {
            let today = db::today();
            conn.query_row(
                "SELECT COUNT(*) FROM reminder_log
                 WHERE rule_id = ?1
                   AND date(sent_at) = ?2
                   AND (case_id = ?3 OR (?3 IS NULL AND case_id IS NULL))
                   AND (task_id = ?4 OR (?4 IS NULL AND task_id IS NULL))",
                params![rule_id, today, case_id, task_id],
                |row| row.get(0),
            )?
        }
    };
    Ok(count > 0)
}

// ============================================================
// 通道实现
// ============================================================

pub(crate) fn send_local_notification(message: &str, task_id: Option<&str>, reminder_log_id: Option<&str>) -> Result<()> {
    // 向前端 emit 事件（弹提醒面板，带实体上下文供反馈回收），并记录日志
    log::info!("[提醒-本地弹窗] {}", message.replace('\n', " | "));

    if let Some(handle) = crate::get_app_handle() {
        let _ = handle.emit("reminder:triggered", serde_json::json!({
            "message": message,
            "at": crate::db::now_local(),
            "taskId": task_id,
            "reminderLogId": reminder_log_id,
        }));
    }

    // macOS 系统通知（无论前端是否打开都可见）
    send_system_notification(message)
}

fn send_system_notification(message: &str) -> Result<()> {
    // macOS: 使用 osascript 发系统通知（Tauri 外部运行时回退方案）
    log::info!("[提醒-系统通知] {}", message.replace('\n', " | "));

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("osascript")
            .args([
                "-e",
                &format!(
                    "display notification \"{}\" with title \"Casy 期限提醒\" sound name \"default\"",
                    message.replace('"', "\\\"").replace('\n', "\\n")
                ),
            ])
            .output();
    }

    Ok(())
}

/// 构建飞书消息卡片 JSON
pub fn build_feishu_card_message(
    case_name: &str,
    deadline_name: &str,
    due_date: &str,
    days_left: i64,
) -> serde_json::Value {
    let content = format!(
        "**案件**: {}\n**期限**: {}\n**截止日期**: {}\n**剩余**: {} 天",
        case_name, deadline_name, due_date, days_left
    );

    serde_json::json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": {
                    "tag": "plain_text",
                    "content": "⏰ 期限提醒"
                },
                "template": if days_left <= 1 { "red" } else if days_left <= 3 { "orange" } else { "blue" }
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": content
                    }
                }
            ]
        }
    })
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command]
pub async fn list_reminder_rules() -> Result<Vec<ReminderRule>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, trigger_type, trigger_value, channels, message_template, case_types, enabled, created_at
             FROM reminder_rules ORDER BY created_at",
        )?;

        let rules = stmt
            .query_map([], |row| {
                Ok(ReminderRule {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    trigger_type: row.get(2)?,
                    trigger_value: row.get(3)?,
                    channels: row.get(4)?,
                    message_template: row.get(5)?,
                    case_types: row.get(6)?,
                    enabled: row.get::<_, i32>(7)? != 0,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rules)
    })
    .await
}

#[tauri::command]
pub async fn create_reminder_rule(data: serde_json::Value) -> Result<ReminderRule, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = data["id"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| db::new_id());

        let channels = data["channels"].to_string();

        conn.execute(
            "INSERT INTO reminder_rules (id, name, trigger_type, trigger_value, channels, message_template, case_types, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                data["name"].as_str().unwrap_or(""),
                data["triggerType"].as_str().unwrap_or("deadline_before"),
                data["triggerValue"].as_i64(),
                channels,
                data["messageTemplate"].as_str(),
                data["caseTypes"].as_str(),
                data["enabled"].as_i64().unwrap_or(1) as i32,
            ],
        )?;

        Ok(ReminderRule {
            id,
            name: data["name"].as_str().unwrap_or("").to_string(),
            trigger_type: data["triggerType"].as_str().unwrap_or("deadline_before").to_string(),
            trigger_value: data["triggerValue"].as_i64(),
            channels,
            message_template: data["messageTemplate"].as_str().map(|s| s.to_string()),
            case_types: data["caseTypes"].as_str().map(|s| s.to_string()),
            enabled: data["enabled"].as_i64().unwrap_or(1) != 0,
            created_at: Some(db::now_local()),
        })
    })
    .await
}

#[tauri::command]
pub async fn update_reminder_rule(id: String, data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let channels = data["channels"].to_string();

        conn.execute(
            "UPDATE reminder_rules SET name = ?1, trigger_type = ?2, trigger_value = ?3,
             channels = ?4, message_template = ?5, case_types = ?6, enabled = ?7
             WHERE id = ?8",
            params![
                data["name"].as_str().unwrap_or(""),
                data["triggerType"].as_str().unwrap_or("deadline_before"),
                data["triggerValue"].as_i64(),
                channels,
                data["messageTemplate"].as_str(),
                data["caseTypes"].as_str(),
                data["enabled"].as_i64().unwrap_or(1) as i32,
                id,
            ],
        )?;

        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_reminder_rule(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM reminder_rules WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn test_reminder(
    channels: Vec<String>,
    message: Option<String>,
) -> Result<Vec<ReminderLogEntry>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let test_msg = message.unwrap_or_else(|| "这是一条测试提醒消息".to_string());
        let mut results = Vec::new();

        for channel in &channels {
            match channel.as_str() {
                "local" => {
                    let _ = send_local_notification(&test_msg, None, None);
                }
                "system" => {
                    let _ = send_system_notification(&test_msg);
                }
                "feishu_message" | "feishu_task" => {
                    // 异步通道 — 仅记录日志
                    log::info!("[测试提醒-{}] {}", channel, test_msg);
                }
                _ => {}
            }

            let log_id = db::new_id();
            conn.execute(
                "INSERT INTO reminder_log (id, rule_id, case_id, task_id, channel, message, level, status)
                 VALUES (?1, 'test', NULL, NULL, ?2, ?3, 'R1', 'sent')",
                params![log_id, channel, test_msg],
            )?;

            results.push(ReminderLogEntry {
                id: log_id,
                rule_id: "test".to_string(),
                case_id: None,
                task_id: None,
                channel: channel.clone(),
                message: test_msg.clone(),
                level: Some("R1".to_string()),
                status: "sent".to_string(),
                sent_at: Some(db::now_local()),
            });
        }

        Ok(results)
    })
    .await
}

#[tauri::command]
pub async fn start_reminder_engine(interval_secs: Option<u64>) -> Result<(), String> {
    use std::sync::OnceLock;
    static ENGINE_RUNNING: OnceLock<Arc<AtomicBool>> = OnceLock::new();

    let interval = interval_secs.unwrap_or(300); // 默认 5 分钟

    // 防重复启动：已有运行中的引擎则直接返回
    if let Some(running) = ENGINE_RUNNING.get() {
        if running.load(Ordering::SeqCst) {
            log::info!("提醒引擎已在运行中，跳过重复启动");
            return Ok(());
        }
    }

    tokio::spawn(async move {
        let engine = ReminderEngine::new(interval);
        let running = engine.start_loop();
        let _ = ENGINE_RUNNING.set(running.clone());

        log::info!("提醒引擎启动，检查间隔: {}秒", interval);

        while running.load(Ordering::SeqCst) {
            match db::open_db() {
                Ok(conn) => match engine.check_and_trigger(&conn) {
                    Ok(triggered) => {
                        if !triggered.is_empty() {
                            log::info!("提醒引擎触发 {} 条提醒", triggered.len());
                        }
                    }
                    Err(e) => {
                        log::error!("提醒检查失败: {}", e);
                    }
                },
                Err(e) => {
                    log::error!("提醒引擎打开数据库失败: {}", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }

        log::info!("提醒引擎已停止");
    });

    Ok(())
}

#[tauri::command]
pub async fn get_reminder_log(limit: Option<i64>) -> Result<Vec<ReminderLogEntry>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let limit = limit.unwrap_or(100);

        let mut stmt = conn.prepare(
            "SELECT id, rule_id, case_id, task_id, channel, message, level, status, sent_at
             FROM reminder_log ORDER BY sent_at DESC LIMIT ?1",
        )?;

        let entries = stmt
            .query_map(params![limit], |row| {
                Ok(ReminderLogEntry {
                    id: row.get(0)?,
                    rule_id: row.get(1)?,
                    case_id: row.get(2)?,
                    task_id: row.get(3)?,
                    channel: row.get(4)?,
                    message: row.get(5)?,
                    level: row.get(6)?,
                    status: row.get(7)?,
                    sent_at: row.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    })
    .await
}

// ============================================================
// 异步飞书提醒发送（供 engine 调用）
// ============================================================

/// 从提醒消息文本构造飞书卡片并发送（通用入口）
async fn send_feishu_reminder_async_generic(message: &str) -> Result<()> {
    // 使用默认接收人（settings 中配置的 owner 或空则跳过）
    let conn = db::open_db()?;
    let receive_id = db::get_setting(&conn, "feishu_reminder_receive_id")
        .ok()
        .flatten()
        .unwrap_or_default();

    if receive_id.is_empty() {
        log::warn!("飞书提醒未配置 receive_id，跳过发送");
        return Ok(());
    }

    let card = serde_json::json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "tag": "plain_text", "content": "Casy 期限提醒" },
                "template": "red"
            },
            "elements": [
                { "tag": "div", "text": { "tag": "lark_md", "content": message } }
            ]
        }
    });

    crate::sync::feishu::send_feishu_message(&receive_id, "open_id", &card).await
}

/// 从提醒消息创建飞书任务（通用入口）
async fn send_feishu_task_async_generic(message: &str) -> Result<()> {
    let conn = db::open_db()?;
    let receive_id = db::get_setting(&conn, "feishu_reminder_receive_id")
        .ok()
        .flatten()
        .unwrap_or_default();

    if receive_id.is_empty() {
        log::warn!("飞书任务提醒未配置 receive_id，跳过创建");
        return Ok(());
    }

    let summary = message.lines().next().unwrap_or("Casy 提醒").to_string();
    let members = vec![receive_id.clone()];
    let due = chrono::Local::now().format("%Y-%m-%d").to_string();

    crate::sync::feishu::create_feishu_task(&summary, message, &due, &members)
        .await
        .map(|_| ())
}

/// 异步发送飞书消息提醒（由后台 task 调用）
#[allow(dead_code)]
pub async fn send_feishu_reminder_async(
    receive_id: &str,
    receive_id_type: &str,
    case_name: &str,
    deadline_name: &str,
    due_date: &str,
    days_left: i64,
) -> Result<()> {
    let card = build_feishu_card_message(case_name, deadline_name, due_date, days_left);
    crate::sync::feishu::send_feishu_message(receive_id, receive_id_type, &card).await
}

/// 异步创建飞书任务提醒
#[allow(dead_code)]
pub async fn create_feishu_task_reminder_async(
    summary: &str,
    description: &str,
    due_date: &str,
    members: &[String],
) -> Result<String> {
    crate::sync::feishu::create_feishu_task(summary, description, due_date, members).await
}

// ============================================================
// 分级预警 R1-R4（设计哲学 §11.2）
// ============================================================

/// 提醒等级
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ReminderLevel {
    /// R1 温和：截止前 T-3 天
    R1,
    /// R2 明确：截止前 T-1 天
    R2,
    /// R3 强提醒：到期当天
    R3,
    /// R4 逾期追踪：超过截止
    R4,
}

impl ReminderLevel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::R1 => "R1",
            Self::R2 => "R2",
            Self::R3 => "R3",
            Self::R4 => "R4",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::R1 => "温和",
            Self::R2 => "明确",
            Self::R3 => "强提醒",
            Self::R4 => "逾期",
        }
    }

    pub fn color(&self) -> &str {
        match self {
            Self::R1 => "#9BA2AF",
            Self::R2 => "#B0823A",
            Self::R3 => "#B4554F",
            Self::R4 => "#B4554F",
        }
    }
}

/// 计算期限的提醒等级
pub fn compute_reminder_level(days_left: i64) -> ReminderLevel {
    if days_left < 0 {
        ReminderLevel::R4
    } else if days_left == 0 {
        ReminderLevel::R3
    } else if days_left <= 1 {
        ReminderLevel::R2
    } else if days_left <= 3 {
        ReminderLevel::R1
    } else {
        ReminderLevel::R1 // 默认 R1
    }
}

/// 分级预警结果
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineWarning {
    pub deadline_id: String,
    pub case_id: String,
    pub case_name: String,
    pub deadline_name: String,
    pub due_date: String,
    pub days_left: i64,
    pub level: String,
    pub level_label: String,
    pub level_color: String,
    pub message: String,
}

/// 获取所有分级预警（设计哲学 §11.2）
#[tauri::command]
pub async fn get_deadline_warnings_with_levels() -> Result<Vec<DeadlineWarning>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let today = db::today();
        let today_dt = NaiveDate::parse_from_str(&today, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Local::now().date_naive());

        let mut stmt = conn.prepare(
            "SELECT cd.id, cd.case_id, cd.deadline_name, cd.due_date, c.case_name
             FROM case_deadlines cd
             JOIN cases c ON c.id = cd.case_id
             WHERE cd.completed = 0 AND cd.due_date IS NOT NULL AND cd.due_date != ''
             ORDER BY cd.due_date ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;

        let mut warnings = Vec::new();
        for row in rows {
            let (deadline_id, case_id, deadline_name, due_date_str, case_name) = row?;
            let Ok(due_date) = NaiveDate::parse_from_str(&due_date_str, "%Y-%m-%d") else {
                continue;
            };
            let days_left = (due_date - today_dt).num_days();
            let level = compute_reminder_level(days_left);

            let message = match &level {
                ReminderLevel::R1 => format!("{} 天后到期：{}", days_left, deadline_name),
                ReminderLevel::R2 => format!("明天到期：{}（{}）", deadline_name, case_name),
                ReminderLevel::R3 => format!("今天到期：{}（{}）", deadline_name, case_name),
                ReminderLevel::R4 => format!("已逾期 {} 天：{}（{}）", -days_left, deadline_name, case_name),
            };

            warnings.push(DeadlineWarning {
                deadline_id,
                case_id,
                case_name,
                deadline_name,
                due_date: due_date_str,
                days_left,
                level: level.as_str().to_string(),
                level_label: level.label().to_string(),
                level_color: level.color().to_string(),
                message,
            });
        }

        Ok(warnings)
    })
    .await
}

/// 提醒处理反馈回收（设计哲学 §11.2：提醒不是终点，反馈回收到行为数据）
/// status: handled（已处理）/ dismissed（忽略）/ snoozed（稍后）
/// 写 reminded 事件到 task_events，支撑"懂你的节奏"（何时提醒最有效）学习
#[tauri::command]
pub async fn record_reminder_feedback(
    reminder_log_id: Option<String>,
    task_id: Option<String>,
    status: String,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let now = db::now_local();

        // 更新 reminder_log 状态（snoozed 在现有枚举内）
        if let Some(log_id) = &reminder_log_id {
            if status == "snoozed" {
                let _ = conn.execute(
                    "UPDATE reminder_log SET status = 'snoozed' WHERE id = ?1",
                    rusqlite::params![log_id],
                );
            }
        }

        // 写 reminded 行为事件（task_events，支撑提醒时机学习）
        if let Some(tid) = task_id {
            let payload = serde_json::json!({
                "status": status,
                "reminderLogId": reminder_log_id,
            });
            conn.execute(
                "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'reminded', ?3, ?4, 'user')",
                rusqlite::params![db::new_id(), tid, now, serde_json::to_string(&payload).unwrap_or_default()],
            )?;
        }

        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_feishu_card_message() {
        let card = build_feishu_card_message("测试案件", "答辩期限", "2026-08-15", 7);
        assert_eq!(card["msg_type"], "interactive");
        assert_eq!(card["card"]["header"]["template"], "blue");

        let card_urgent = build_feishu_card_message("测试案件", "答辩期限", "2026-08-15", 1);
        assert_eq!(card_urgent["card"]["header"]["template"], "red");
    }

    #[test]
    fn test_reminder_rule_serde() {
        let rule = ReminderRule {
            id: "test-1".to_string(),
            name: "测试规则".to_string(),
            trigger_type: "deadline_before".to_string(),
            trigger_value: Some(7),
            channels: r#"["feishu_message"]"#.to_string(),
            message_template: None,
            case_types: None,
            enabled: true,
            created_at: None,
        };

        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("deadline_before"));
        assert!(json.contains("feishu_message"));
    }

    /// 区间触发的最小内存库（cases + case_deadlines + reminder_log + settings + reminder_jobs）
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE cases (id TEXT PRIMARY KEY, case_name TEXT NOT NULL);
             CREATE TABLE case_deadlines (
               id TEXT PRIMARY KEY, case_id TEXT NOT NULL, deadline_name TEXT NOT NULL,
               due_date TEXT, completed INTEGER DEFAULT 0
             );
             CREATE TABLE reminder_log (
               id TEXT PRIMARY KEY, rule_id TEXT, case_id TEXT, task_id TEXT,
               channel TEXT, message TEXT, level TEXT, status TEXT, sent_at TEXT
             );
             CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE reminder_jobs (
               id TEXT PRIMARY KEY, rule_id TEXT, entity_type TEXT, entity_id TEXT,
               channel TEXT, executor TEXT, scheduled_at TEXT, content TEXT,
               due_snapshot TEXT, offset_snapshot TEXT, status TEXT,
               attempts INTEGER DEFAULT 0, last_error TEXT
             );",
        )
        .unwrap();
        conn
    }

    fn test_rule(trigger_type: &str, trigger_value: i64) -> ReminderRule {
        ReminderRule {
            id: format!("test-{}", trigger_type),
            name: "测试".to_string(),
            trigger_type: trigger_type.to_string(),
            trigger_value: Some(trigger_value),
            // 未知通道：dispatch 只写日志，无外部副作用
            channels: r#"["test_channel"]"#.to_string(),
            message_template: None,
            case_types: None,
            enabled: true,
            created_at: None,
        }
    }

    fn insert_deadline(conn: &Connection, id: &str, days_from_today: i64) {
        let today = chrono::Local::now().date_naive();
        let due = (today + chrono::Duration::days(days_from_today))
            .format("%Y-%m-%d")
            .to_string();
        conn.execute(
            "INSERT INTO case_deadlines (id, case_id, deadline_name, due_date, completed)
             VALUES (?1, 'c1', '测试期限', ?2, 0)",
            params![id, due],
        )
        .unwrap();
    }

    #[test]
    fn test_deadline_before_interval_trigger() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO cases (id, case_name) VALUES ('c1', '测试案件')", [])
            .unwrap();
        let today = chrono::Local::now().date_naive();
        let rule = test_rule("deadline_before", 3);

        // T-2 天（区间内，补偿场景：严格相等时代不会触发）
        insert_deadline(&conn, "d1", 2);
        let triggered = check_deadline_rules(&conn, &rule, today).unwrap();
        assert_eq!(triggered.len(), 1, "T-2 在 [0,3] 区间内应补发");

        // 同日重跑：already_sent 去重，不重复发
        let triggered = check_deadline_rules(&conn, &rule, today).unwrap();
        assert_eq!(triggered.len(), 0, "同日不应重复触发");

        // T-5 天（区间外）
        insert_deadline(&conn, "d2", 5);
        let triggered = check_deadline_rules(&conn, &rule, today).unwrap();
        assert_eq!(triggered.len(), 0, "T-5 超出 [0,3] 区间不应触发");

        // 已逾期 1 天：before 规则不触发（由 on/after 覆盖）
        insert_deadline(&conn, "d3", -1);
        let triggered = check_deadline_rules(&conn, &rule, today).unwrap();
        assert_eq!(triggered.len(), 0, "逾期不应由 before 规则触发");
    }

    #[test]
    fn test_deadline_before_window_dedup_no_daily_bombardment() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO cases (id, case_name) VALUES ('c1', '测试案件')", [])
            .unwrap();
        let today = chrono::Local::now().date_naive();
        let rule = test_rule("deadline_before", 3);

        // T-3：首次进入区间，触发
        insert_deadline(&conn, "d1", 3);
        let triggered = check_deadline_rules(&conn, &rule, today).unwrap();
        assert_eq!(triggered.len(), 1, "T-3 应触发");

        // 模拟次日 T-2：窗口内已发过，不再轰炸
        let tomorrow = today + chrono::Duration::days(1);
        let triggered = check_deadline_rules(&conn, &rule, tomorrow).unwrap();
        assert_eq!(triggered.len(), 0, "窗口内已发过，T-2 不应再次触发");

        // 模拟 T-0 当天：同样跳过
        let day_t0 = today + chrono::Duration::days(3);
        let triggered = check_deadline_rules(&conn, &rule, day_t0).unwrap();
        assert_eq!(triggered.len(), 0, "窗口内已发过，T-0 不应再次触发");
    }

    #[test]
    fn test_already_sent_window_vs_daily() {
        let conn = setup_test_db();
        // 手工写入一条"昨天已发"的日志
        let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO reminder_log (id, rule_id, case_id, task_id, channel, message, level, status, sent_at)
             VALUES ('l1', 'r1', 'c1', NULL, 'test_channel', 'm', 'R1', 'sent', ?1)",
            params![yesterday],
        )
        .unwrap();

        // 按日去重（on/after/task 类）：昨天发过 → 今天仍可发
        assert!(!already_sent(&conn, "r1", Some("c1"), None, None).unwrap());
        // 窗口去重（before 类，3 天）：昨天在窗口内 → 跳过
        assert!(already_sent(&conn, "r1", Some("c1"), None, Some(3)).unwrap());
        // 窗口为 0 天：等同当日去重
        assert!(!already_sent(&conn, "r1", Some("c1"), None, Some(0)).unwrap());
        // 不同案件不受窗口去重影响
        assert!(!already_sent(&conn, "r1", Some("c2"), None, Some(3)).unwrap());
    }

    #[test]
    fn test_deadline_on_and_after_semantics() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO cases (id, case_name) VALUES ('c1', '测试案件')", [])
            .unwrap();
        let today = chrono::Local::now().date_naive();

        // deadline_on：到期日已到或已过（含离线错过的当天提醒）
        insert_deadline(&conn, "d1", -2);
        let rule_on = test_rule("deadline_on", 0);
        let triggered = check_deadline_rules(&conn, &rule_on, today).unwrap();
        assert_eq!(triggered.len(), 1, "错过的当天提醒应补发");
        assert_eq!(triggered[0].level.as_deref(), Some("R4"), "逾期应为 R4");

        // deadline_after：逾期天数 >= trigger_value 才触发
        conn.execute("INSERT INTO cases (id, case_name) VALUES ('c2', '测试案件2')", [])
            .unwrap();
        let today_plus = |id: &str, case: &str, days: i64| {
            let due = (today + chrono::Duration::days(days))
                .format("%Y-%m-%d")
                .to_string();
            conn.execute(
                "INSERT INTO case_deadlines (id, case_id, deadline_name, due_date, completed)
                 VALUES (?1, ?2, '测试期限', ?3, 0)",
                params![id, case, due],
            )
            .unwrap();
        };
        today_plus("d2", "c2", -3);
        let rule_after = test_rule("deadline_after", 2);
        let triggered = check_deadline_rules(&conn, &rule_after, today).unwrap();
        // d1(c1) 逾期 2 天 + d2(c2) 逾期 3 天，均 >= 2 → 两条
        assert_eq!(triggered.len(), 2);

        today_plus("d3", "c1", 1);
        let rule_after2 = test_rule("deadline_after", 2);
        let triggered = check_deadline_rules(&conn, &rule_after2, today).unwrap();
        assert_eq!(triggered.len(), 0, "未逾期不触发 after 规则");
    }

    #[test]
    fn test_level_uses_real_days_diff() {
        // level 按真实 days_diff 计算，与触发区间无关
        assert_eq!(compute_level(3, false), "R1");
        assert_eq!(compute_level(1, false), "R2");
        assert_eq!(compute_level(0, false), "R3");
        assert_eq!(compute_level(-2, false), "R4");
        assert_eq!(compute_level(5, true), "R4");
    }

    // --------------------------------------------------------
    // 提醒时机智能（§11.2）：时段外 R1/R2 延迟，R3/R4 立即
    // --------------------------------------------------------

    fn dt(y: i32, m: u32, d: u32, h: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, 30, 0)
            .unwrap()
    }

    fn at_hms(y: i32, m: u32, d: u32, h: u32) -> Option<chrono::NaiveDateTime> {
        chrono::NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, 0, 0)
    }

    fn test_cal_ctx() -> CalendarJobCtx {
        CalendarJobCtx {
            entity_type: "deadline",
            entity_id: "d1".to_string(),
            due_date: "2026-08-20".to_string(),
            title: "测试期限".to_string(),
        }
    }

    #[test]
    fn test_next_work_start_boundaries() {
        // 时段内 → None
        assert_eq!(next_work_start(dt(2026, 8, 19, 10), 9, 21), None);
        assert_eq!(next_work_start(dt(2026, 8, 19, 9), 9, 21), None);
        // 深夜（>= end）→ 次日 start
        assert_eq!(
            next_work_start(dt(2026, 8, 19, 23), 9, 21),
            at_hms(2026, 8, 20, 9)
        );
        // 清晨（< start）→ 当天 start
        assert_eq!(
            next_work_start(dt(2026, 8, 19, 6), 9, 21),
            at_hms(2026, 8, 19, 9)
        );
        // 异常配置（start >= end）不延迟
        assert_eq!(next_work_start(dt(2026, 8, 19, 23), 21, 9), None);
    }

    #[test]
    fn test_work_hours_default_and_profile() {
        let conn = setup_test_db();
        // 无画像数据 → 默认 9-21
        assert_eq!(work_hours(&conn), (9, 21));
        // 画像损坏 → 默认 9-21
        conn.execute("INSERT INTO settings (key, value) VALUES ('lawyer_profile', 'not-json')", [])
            .unwrap();
        assert_eq!(work_hours(&conn), (9, 21));
        // 有画像 → 用画像时段
        conn.execute(
            "UPDATE settings SET value = ?1 WHERE key = 'lawyer_profile'",
            params![r#"{"work_hours":{"start_hour":8,"end_hour":20}}"#],
        )
        .unwrap();
        assert_eq!(work_hours(&conn), (8, 20));
    }

    #[test]
    fn test_r1_deferred_outside_work_hours() {
        let conn = setup_test_db();
        let night = dt(2026, 8, 19, 23); // 默认 9-21 时段外

        let entry = dispatch_reminder_at(
            &conn, "rule-1", Some("c1"), None, "test_channel", "测试消息", "R1",
            Some(&test_cal_ctx()), night,
        )
        .unwrap();

        assert_eq!(entry.status, "deferred", "时段外 R1 应延迟");

        // 写入一条 pending local 作业，scheduled_at = 次日 09:00
        let (status, executor, scheduled): (String, String, String) = conn
            .query_row(
                "SELECT status, executor, scheduled_at FROM reminder_jobs",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(executor, "local");
        assert_eq!(scheduled, "2026-08-20 09:00:00");

        // 延迟日志写入（sent_at 非空，供 already_sent 同日去重）
        let log_status: String = conn
            .query_row("SELECT status FROM reminder_log WHERE id = ?1", params![entry.id], |r| r.get(0))
            .unwrap();
        assert_eq!(log_status, "deferred");

        // 把作业改到过去时刻 → check 循环顺带派发
        conn.execute(
            "UPDATE reminder_jobs SET scheduled_at = '2020-01-01 00:00:00'",
            [],
        )
        .unwrap();
        let delivered = dispatch_due_local_jobs(&conn).unwrap();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].status, "sent");
        let job_status: String = conn
            .query_row("SELECT status FROM reminder_jobs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(job_status, "sent");
    }

    #[test]
    fn test_r3_immediate_outside_work_hours() {
        let conn = setup_test_db();
        let night = dt(2026, 8, 19, 23);

        let entry = dispatch_reminder_at(
            &conn, "rule-1", Some("c1"), None, "test_channel", "到期提醒", "R3",
            Some(&test_cal_ctx()), night,
        )
        .unwrap();

        assert_eq!(entry.status, "sent", "时段外 R3 应立即发");
        let job_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM reminder_jobs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(job_count, 0, "R3 不应创建延迟作业");
    }

    #[test]
    fn test_r1_within_work_hours_unaffected() {
        let conn = setup_test_db();
        let daytime = dt(2026, 8, 19, 10); // 时段内

        let entry = dispatch_reminder_at(
            &conn, "rule-1", Some("c1"), None, "test_channel", "测试消息", "R1",
            Some(&test_cal_ctx()), daytime,
        )
        .unwrap();

        assert_eq!(entry.status, "sent", "时段内 R1 行为与现状一致");
        let job_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM reminder_jobs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(job_count, 0, "时段内不应创建延迟作业");
    }
}

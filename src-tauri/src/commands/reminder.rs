//! 多通道提醒系统 — ReminderEngine + Tauri Commands
//!
//! 全局横切能力：定时检查期限/开庭/任务，按规则分发到各通道。

use super::run_blocking;
use crate::db;
use anyhow::Result;
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

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
        let (_deadline_id, case_id, deadline_name, due_date_str, case_name) = row?;
        let Ok(due_date) = NaiveDate::parse_from_str(&due_date_str, "%Y-%m-%d") else {
            continue;
        };

        let days_diff = (due_date - today).num_days();

        let should_trigger = match rule.trigger_type.as_str() {
            "deadline_before" => days_diff == trigger_days,
            "deadline_on" => days_diff == 0,
            "deadline_after" => days_diff == -trigger_days,
            _ => false,
        };

        if should_trigger {
            // 检查是否已发送过（同一天、同规则、同案件）
            if already_sent(conn, &rule.id, Some(&case_id), None)? {
                continue;
            }

            let message = format!(
                "案件: {}\n期限: {}\n截止日期: {}\n剩余: {} 天",
                case_name, deadline_name, due_date_str, days_diff
            );

            let level = compute_level(days_diff, false);
            let channels: Vec<String> =
                serde_json::from_str(&rule.channels).unwrap_or_default();

            for channel in &channels {
                let entry = dispatch_reminder(conn, &rule.id, Some(&case_id), None, channel, &message, level)?;
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
        let (_hearing_id, case_id, hearing_name, hearing_date_str, case_name) = row?;
        let Ok(hearing_date) = NaiveDate::parse_from_str(&hearing_date_str, "%Y-%m-%d") else {
            continue;
        };

        let days_diff = (hearing_date - today).num_days();

        if days_diff == trigger_days {
            if already_sent(conn, &rule.id, Some(&case_id), None)? {
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
                let entry = dispatch_reminder(conn, &rule.id, Some(&case_id), None, channel, &message, level)?;
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
            if already_sent(conn, &rule.id, case_id.as_deref(), Some(&task_id))? {
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
                let entry = dispatch_reminder(
                    conn,
                    &rule.id,
                    case_id.as_deref(),
                    Some(&task_id),
                    channel,
                    &message,
                    level,
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
) -> Result<ReminderLogEntry> {
    let result = match channel {
        "local" => send_local_notification(message),
        "system" => send_system_notification(message),
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
    };

    let status = match result {
        Ok(_) => "sent",
        Err(_) => "failed",
    };

    let log_id = db::new_id();
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

/// 检查是否今天已经为该规则+案件/任务发送过提醒
fn already_sent(
    conn: &Connection,
    rule_id: &str,
    case_id: Option<&str>,
    task_id: Option<&str>,
) -> Result<bool> {
    let today = db::today();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM reminder_log
         WHERE rule_id = ?1
           AND date(sent_at) = ?2
           AND (case_id = ?3 OR (?3 IS NULL AND case_id IS NULL))
           AND (task_id = ?4 OR (?4 IS NULL AND task_id IS NULL))",
        params![rule_id, today, case_id, task_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

// ============================================================
// 通道实现
// ============================================================

fn send_local_notification(message: &str) -> Result<()> {
    // 向前端 emit 事件（弹提醒面板），并记录日志
    log::info!("[提醒-本地弹窗] {}", message.replace('\n', " | "));

    if let Some(handle) = crate::get_app_handle() {
        let _ = handle.emit("reminder:triggered", serde_json::json!({
            "message": message,
            "at": crate::db::now_local(),
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
                    let _ = send_local_notification(&test_msg);
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
}

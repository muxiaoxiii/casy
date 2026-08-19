//! 行为学习闭环（设计哲学 §11.9）
//!
//! 从 task_events 分析用户模式 → 预估校准 / 活跃时段 / 延期模式

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 任务耗时统计
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDurationStats {
    pub task_pattern: String,
    pub avg_estimated: f64,
    pub avg_actual: f64,
    pub sample_count: i64,
    pub accuracy: f64,
}

/// 活跃时段分析
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPattern {
    pub hour: i32,
    pub completions: i64,
    pub percentage: f64,
}

/// 延期模式分析
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelayPattern {
    pub case_type: String,
    pub avg_delay_days: f64,
    pub delay_count: i64,
    pub total_tasks: i64,
    pub delay_rate: f64,
}

/// 学习分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningAnalysis {
    pub duration_stats: Vec<TaskDurationStats>,
    pub activity_patterns: Vec<ActivityPattern>,
    pub delay_patterns: Vec<DelayPattern>,
    /// 上次校准已调整的任务数（来自 settings 持久化）
    pub calibrated_task_count: i64,
    /// 上次校准时间
    pub last_calibrated_at: Option<String>,
    /// 当前待校准（未完成任务中预估缺失或偏差 >50%）的任务数
    pub pending_calibration_count: i64,
    pub generated_at: String,
}

/// 预估校准结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationResult {
    pub calibrated_count: i64,
    pub groups: Vec<CalibrationGroup>,
    pub calibrated_at: String,
}

/// 单个分组的校准情况
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationGroup {
    pub task_pattern: String,
    pub context: String,
    pub avg_estimated: f64,
    pub avg_actual: f64,
    pub sample_count: i64,
    pub updated_tasks: i64,
}

/// 校准计划项（内部）
struct CalibrationPlanItem {
    task_id: String,
    new_estimate: i64,
    group_key: (String, String),
}

/// 分析任务耗时准确性
pub fn analyze_task_durations(conn: &rusqlite::Connection) -> Result<Vec<TaskDurationStats>> {
    let mut stmt = conn.prepare(
        "SELECT task_name, estimated_minutes, actual_minutes
         FROM tasks
         WHERE completed = 1 AND estimated_minutes > 0 AND actual_minutes > 0"
    )?;

    let mut pattern_map: std::collections::HashMap<String, Vec<(f64, f64)>> = std::collections::HashMap::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)? as f64,
            row.get::<_, i64>(2)? as f64,
        ))
    })?;

    for row in rows {
        let (name, est, actual) = row?;
        // 简单模式匹配：按任务名前缀分组
        let pattern = extract_task_pattern(&name);
        pattern_map.entry(pattern).or_default().push((est, actual));
    }

    let stats: Vec<TaskDurationStats> = pattern_map
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .map(|(pattern, values)| {
            let avg_est = values.iter().map(|(e, _)| e).sum::<f64>() / values.len() as f64;
            let avg_act = values.iter().map(|(_, a)| a).sum::<f64>() / values.len() as f64;
            let accuracy = if avg_est > 0.0 { (avg_est / avg_act).min(2.0) } else { 0.0 };
            TaskDurationStats {
                task_pattern: pattern,
                avg_estimated: avg_est,
                avg_actual: avg_act,
                sample_count: values.len() as i64,
                accuracy,
            }
        })
        .collect();

    Ok(stats)
}

/// 分析活跃时段
pub fn analyze_activity_patterns(conn: &rusqlite::Connection) -> Result<Vec<ActivityPattern>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', occurred_at) AS INTEGER) as hour, COUNT(*) as cnt
         FROM task_events
         WHERE event_type = 'completed'
         GROUP BY hour
         ORDER BY hour"
    )?;

    let mut patterns = Vec::new();
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM task_events WHERE event_type = 'completed'",
        [],
        |r| r.get(0),
    ).unwrap_or(1);

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?))
    })?;

    for row in rows {
        let (hour, count) = row?;
        patterns.push(ActivityPattern {
            hour,
            completions: count,
            percentage: count as f64 / total as f64 * 100.0,
        });
    }

    Ok(patterns)
}

/// 分析延期模式（tasks 表无 completed_at，完成时间以 task_events 为准）
pub fn analyze_delay_patterns(conn: &rusqlite::Connection) -> Result<Vec<DelayPattern>> {
    let mut stmt = conn.prepare(
        "SELECT c.track, COUNT(*) as total,
                SUM(CASE WHEN t.due_date < date(te.occurred_at) THEN 1 ELSE 0 END) as delayed,
                COALESCE(AVG(CASE WHEN t.due_date < date(te.occurred_at)
                     THEN julianday(date(te.occurred_at)) - julianday(t.due_date) END), 0) as avg_delay
         FROM task_events te
         JOIN tasks t ON t.id = te.task_id
         JOIN cases c ON c.id = t.case_id
         WHERE te.event_type = 'completed' AND t.due_date IS NOT NULL
         GROUP BY c.track"
    )?;

    let mut patterns = Vec::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, f64>(3)?,
        ))
    })?;

    for row in rows {
        let (track, total, delayed, avg_delay) = row?;
        let delay_rate = if total > 0 { delayed as f64 / total as f64 } else { 0.0 };
        patterns.push(DelayPattern {
            case_type: track,
            avg_delay_days: avg_delay,
            delay_count: delayed,
            total_tasks: total,
            delay_rate,
        });
    }

    Ok(patterns)
}

/// 生成完整学习分析
pub fn generate_learning_analysis(conn: &rusqlite::Connection) -> Result<LearningAnalysis> {
    // 上次校准信息（持久化在 settings 表）
    let last_cal: Option<serde_json::Value> = crate::db::get_setting(conn, "learning_last_calibration")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok());
    let calibrated_task_count = last_cal
        .as_ref()
        .and_then(|v| v["calibratedCount"].as_i64())
        .unwrap_or(0);
    let last_calibrated_at = last_cal
        .as_ref()
        .and_then(|v| v["calibratedAt"].as_str().map(|s| s.to_string()));

    // 当前待校准任务数（dry-run，不写入）
    let (plan, _) = plan_calibration(conn)?;
    let pending_calibration_count = plan.len() as i64;

    Ok(LearningAnalysis {
        duration_stats: analyze_task_durations(conn)?,
        activity_patterns: analyze_activity_patterns(conn)?,
        delay_patterns: analyze_delay_patterns(conn)?,
        calibrated_task_count,
        last_calibrated_at,
        pending_calibration_count,
        generated_at: chrono::Local::now().to_rfc3339(),
    })
}

/// 按任务模式 + 上下文分组，统计已完成任务的 avg(actual) vs avg(estimated)
/// 只保留样本数 >= 2 的组
fn group_duration_avgs(
    conn: &rusqlite::Connection,
) -> Result<std::collections::HashMap<(String, String), (f64, f64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT task_name, context, estimated_minutes, actual_minutes
         FROM tasks
         WHERE completed = 1 AND actual_minutes IS NOT NULL AND actual_minutes > 0",
    )?;

    let mut groups: std::collections::HashMap<(String, String), Vec<(f64, f64)>> =
        std::collections::HashMap::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<i64>>(2)?.unwrap_or(0) as f64,
            row.get::<_, i64>(3)? as f64,
        ))
    })?;

    for row in rows {
        let (name, context, est, actual) = row?;
        let pattern = extract_task_pattern(&name);
        groups.entry((pattern, context)).or_default().push((est, actual));
    }

    // (pattern, context) -> (avg_est, avg_actual, sample_count)
    Ok(groups
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .map(|(k, v)| {
            let avg_est = v.iter().map(|(e, _)| e).sum::<f64>() / v.len() as f64;
            let avg_act = v.iter().map(|(_, a)| a).sum::<f64>() / v.len() as f64;
            (k, (avg_est, avg_act, v.len() as i64))
        })
        .collect())
}

/// 计算校准计划：未完成任务中，未设预估或预估偏差 >50% 的，更新为历史均值
/// 只处理能算出历史均值的分组，不瞎填
fn plan_calibration(
    conn: &rusqlite::Connection,
) -> Result<(Vec<CalibrationPlanItem>, std::collections::HashMap<(String, String), (f64, f64, i64)>)> {
    let groups = group_duration_avgs(conn)?;
    if groups.is_empty() {
        return Ok((Vec::new(), groups));
    }

    let mut stmt = conn.prepare(
        "SELECT id, task_name, context, estimated_minutes
         FROM tasks
         WHERE completed = 0",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            row.get::<_, Option<i64>>(3)?,
        ))
    })?;

    let mut plan = Vec::new();

    for row in rows {
        let (id, name, context, est) = row?;
        let key = (extract_task_pattern(&name), context);
        let Some(&(_, avg_act, _)) = groups.get(&key) else { continue };

        let need_calibration = match est {
            None => true,
            Some(e) => {
                // 偏差 >50%（以历史实际均值为基准）
                ((e as f64 - avg_act).abs() / avg_act) > 0.5
            }
        };

        if need_calibration {
            plan.push(CalibrationPlanItem {
                task_id: id,
                new_estimate: avg_act.round() as i64,
                group_key: key,
            });
        }
    }

    Ok((plan, groups))
}

/// 执行预估校准：把未设预估或偏差 >50% 的未完成任务的 estimated_minutes 更新为历史均值
pub fn apply_estimation_calibration(conn: &rusqlite::Connection) -> Result<CalibrationResult> {
    let (plan, groups) = plan_calibration(conn)?;

    let mut updated_per_group: std::collections::HashMap<(String, String), i64> =
        std::collections::HashMap::new();
    let mut calibrated_count: i64 = 0;

    for item in &plan {
        conn.execute(
            "UPDATE tasks SET estimated_minutes = ?1 WHERE id = ?2",
            rusqlite::params![item.new_estimate, item.task_id],
        )?;
        *updated_per_group.entry(item.group_key.clone()).or_insert(0) += 1;
        calibrated_count += 1;
    }

    let calibrated_at = chrono::Local::now().to_rfc3339();

    let groups_out: Vec<CalibrationGroup> = groups
        .into_iter()
        .map(|((pattern, context), (avg_est, avg_act, samples))| {
            let updated = updated_per_group
                .get(&(pattern.clone(), context.clone()))
                .copied()
                .unwrap_or(0);
            CalibrationGroup {
                task_pattern: pattern,
                context,
                avg_estimated: avg_est,
                avg_actual: avg_act,
                sample_count: samples,
                updated_tasks: updated,
            }
        })
        .collect();

    // 持久化本次校准结果，供 get_learning_analysis 展示
    let record = serde_json::json!({
        "calibratedCount": calibrated_count,
        "calibratedAt": calibrated_at,
    });
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('learning_last_calibration', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![record.to_string()],
    )?;

    log::info!("预估校准完成：{} 个任务已按历史均值调整", calibrated_count);

    Ok(CalibrationResult {
        calibrated_count,
        groups: groups_out,
        calibrated_at,
    })
}

/// 提取任务模式（简单前缀匹配）
fn extract_task_pattern(name: &str) -> String {
    let patterns = [
        ("起草", "起草文书"),
        ("审核", "审核材料"),
        ("准备", "准备材料"),
        ("联系", "沟通联系"),
        ("整理", "整理归档"),
        ("提交", "提交材料"),
        ("跟进", "跟进进度"),
        ("核对", "核对检查"),
    ];

    for (prefix, pattern) in patterns {
        if name.starts_with(prefix) {
            return pattern.to_string();
        }
    }

    "其他".to_string()
}

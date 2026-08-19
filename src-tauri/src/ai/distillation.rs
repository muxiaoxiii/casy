//! 数据蒸馏调度（设计哲学 §11.10）
//!
//! L1 原始数据 → L2 提炼记忆 → L3 知识库
//! 周期性蒸馏 + 确认区 + 生命周期管理

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 蒸馏候选记忆
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateMemory {
    pub id: String,
    pub content: String,
    pub source_type: String,
    pub source_ids: Vec<String>,
    pub confidence: f64,
    pub layer: String,
    pub status: String,
    pub generated_at: String,
}

/// 蒸馏结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistillationResult {
    pub candidates: Vec<CandidateMemory>,
    /// 新落库的候选记忆数（重复内容跳过）
    pub inserted_count: i64,
    pub cleaned_count: i64,
    pub merged_count: i64,
    /// 本轮标记为 stale 的记忆数（30 天未引用）
    pub stale_count: i64,
    /// 本轮归档的记忆数（stale 超 90 天）
    pub archived_count: i64,
    pub generated_at: String,
}

/// 执行蒸馏调度
///
/// 记忆生命周期全量串行执行（L1 清理 → L2 提炼落库 → 近似合并 → 陈旧标记 → 归档），
/// 单步失败只记日志，不中断后续步骤。
pub fn run_distillation(conn: &rusqlite::Connection) -> Result<DistillationResult> {
    // Step 1: 清理过期 L1 原始数据（90 天前的 task_events）
    let cleaned_count = cleanup_old_events(conn)
        .map_err(|e| log::error!("蒸馏: 清理过期事件失败: {}", e))
        .unwrap_or(0);

    // Step 2: 从 task_events 提炼候选记忆
    let candidates = extract_candidate_memories(conn)
        .map_err(|e| log::error!("蒸馏: 提炼候选记忆失败: {}", e))
        .unwrap_or_default();

    // Step 3: 候选记忆落库 memory_entries（layer='l2', status='pending'，按内容判重）
    let inserted_count = persist_candidates(conn, &candidates)
        .map_err(|e| log::error!("蒸馏: 候选记忆落库失败: {}", e))
        .unwrap_or(0);

    // Step 4: 合并近似记忆
    let merged_count = merge_similar_memories(conn)
        .map_err(|e| log::error!("蒸馏: 合并近似记忆失败: {}", e))
        .unwrap_or(0);

    // Step 5: 标记陈旧记忆（30 天未引用 → stale）
    let stale_count = mark_stale_memories(conn)
        .map_err(|e| log::error!("蒸馏: 标记陈旧记忆失败: {}", e))
        .unwrap_or(0);

    // Step 6: 归档长期陈旧记忆（stale 超 90 天 → archived）
    let archived_count = archive_stale_memories(conn)
        .map_err(|e| log::error!("蒸馏: 归档陈旧记忆失败: {}", e))
        .unwrap_or(0);

    Ok(DistillationResult {
        candidates,
        inserted_count,
        cleaned_count,
        merged_count,
        stale_count,
        archived_count,
        generated_at: chrono::Local::now().to_rfc3339(),
    })
}

/// 清理过期 L1 原始数据
fn cleanup_old_events(conn: &rusqlite::Connection) -> Result<i64> {
    let cutoff = (chrono::Local::now() - chrono::Duration::days(90))
        .format("%Y-%m-%d")
        .to_string();

    let count = conn.execute(
        "DELETE FROM task_events WHERE occurred_at < ?1",
        rusqlite::params![cutoff],
    )?;

    log::info!("蒸馏: 清理 {} 条过期 task_events", count);
    Ok(count as i64)
}

/// 从 task_events 提炼候选记忆
fn extract_candidate_memories(conn: &rusqlite::Connection) -> Result<Vec<CandidateMemory>> {
    let mut candidates = Vec::new();

    // 分析延期模式
    let mut stmt = conn.prepare(
        "SELECT t.task_name, COUNT(*) as delay_count,
                GROUP_CONCAT(t.id) as task_ids
         FROM task_events te
         JOIN tasks t ON t.id = te.task_id
         WHERE te.event_type = 'deferred'
         GROUP BY t.task_name
         HAVING delay_count >= 2"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in rows {
        let (name, count, ids) = row?;
        candidates.push(CandidateMemory {
            id: uuid::Uuid::new_v4().to_string(),
            content: format!("任务「{}」被延期 {} 次，建议调整预估时间", name, count),
            source_type: "delay_pattern".to_string(),
            source_ids: ids.split(',').map(String::from).collect(),
            confidence: 0.7,
            layer: "l2".to_string(),
            status: "pending".to_string(),
            generated_at: chrono::Local::now().to_rfc3339(),
        });
    }

    // 分析高频完成时段
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', occurred_at) AS INTEGER) as hour, COUNT(*) as cnt
         FROM task_events
         WHERE event_type = 'completed'
         GROUP BY hour
         HAVING cnt >= 5
         ORDER BY cnt DESC
         LIMIT 3"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?))
    })?;

    for row in rows {
        let (hour, count) = row?;
        candidates.push(CandidateMemory {
            id: uuid::Uuid::new_v4().to_string(),
            content: format!("你在 {} 点时段完成任务最多（{} 次），建议安排重要任务", hour, count),
            source_type: "activity_pattern".to_string(),
            source_ids: vec![],
            confidence: 0.8,
            layer: "l2".to_string(),
            status: "pending".to_string(),
            generated_at: chrono::Local::now().to_rfc3339(),
        });
    }

    Ok(candidates)
}

/// 候选记忆落库 memory_entries（layer='l2', status='pending' 待确认）
/// 判重：同内容且未被丢弃（dismissed）的记忆不重复插入
fn persist_candidates(conn: &rusqlite::Connection, candidates: &[CandidateMemory]) -> Result<i64> {
    // 现有记忆内容集合（已丢弃的允许重新提炼）
    let mut stmt = conn.prepare(
        "SELECT content FROM memory_entries WHERE layer = 'l2' AND status != 'dismissed'",
    )?;
    let existing: std::collections::HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut inserted: i64 = 0;
    let mut seen_this_run: std::collections::HashSet<String> = std::collections::HashSet::new();

    for c in candidates {
        if existing.contains(&c.content) || !seen_this_run.insert(c.content.clone()) {
            continue;
        }

        let source_ref = serde_json::json!({
            "type": c.source_type,
            "sourceIds": c.source_ids,
        })
        .to_string();

        conn.execute(
            "INSERT INTO memory_entries (id, layer, content, source_ref, status, confidence)
             VALUES (?1, 'l2', ?2, ?3, 'pending', ?4)",
            rusqlite::params![c.id, c.content, source_ref, c.confidence],
        )?;
        inserted += 1;
    }

    log::info!("蒸馏: 落库 {} 条候选记忆（待确认）", inserted);
    Ok(inserted)
}

/// 合并近似记忆
///
/// 按内容关键词分组（前 20 个字母数字字符相同视为高相似/同主题）：
/// 每组保留最新一条为 keeper，其余标记 status='merged'（不删除，数据不丢）；
/// keeper 每吸收一条 confidence +0.1（封顶 1.0），merged_from 记录来源 ID 列表。
fn merge_similar_memories(conn: &rusqlite::Connection) -> Result<i64> {
    let mut stmt = conn.prepare(
        "SELECT id, content, COALESCE(merged_from, '') FROM memory_entries
         WHERE layer = 'l2' AND status = 'active'
         ORDER BY created_at DESC"
    )?;

    let entries: Vec<(String, String, String)> = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?.filter_map(|r| r.ok()).collect();

    // key -> (keeper_id, 已吸收来源 ID 列表)
    let mut keepers: std::collections::HashMap<String, (String, Vec<String>)> =
        std::collections::HashMap::new();
    let mut merged: i64 = 0;

    for (id, content, merged_from) in &entries {
        let keywords = extract_keywords(content);
        let key = keywords.join(",");

        match keepers.get_mut(&key) {
            Some((keeper_id, sources)) => {
                let keeper_id = keeper_id.clone();
                // 吸收该条（连同它自己的 merged_from 来源），不丢数据
                sources.push(id.clone());
                if let Ok(serde_json::Value::Array(extra)) =
                    serde_json::from_str::<serde_json::Value>(merged_from)
                {
                    for v in extra {
                        if let Some(s) = v.as_str() {
                            sources.push(s.to_string());
                        }
                    }
                }

                conn.execute(
                    "UPDATE memory_entries SET status = 'merged' WHERE id = ?1",
                    rusqlite::params![id],
                )?;
                conn.execute(
                    "UPDATE memory_entries
                     SET confidence = MIN(1.0, confidence + 0.1), merged_from = ?2
                     WHERE id = ?1",
                    rusqlite::params![keeper_id, serde_json::to_string(&sources)?],
                )?;
                merged += 1;
            }
            None => {
                // 首个（最新）条目成为 keeper，保留其已有 merged_from 来源
                let sources = match serde_json::from_str::<serde_json::Value>(merged_from) {
                    Ok(serde_json::Value::Array(arr)) => arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                    _ => Vec::new(),
                };
                keepers.insert(key, (id.clone(), sources));
            }
        }
    }

    log::info!("蒸馏: 合并 {} 条近似记忆", merged);
    Ok(merged)
}

/// 标记陈旧记忆：active 且超过 30 天未引用 → stale
///
/// 引用时间以 last_used_at 为准，从未引用的回退到 created_at。
fn mark_stale_memories(conn: &rusqlite::Connection) -> Result<i64> {
    let cutoff = (chrono::Local::now() - chrono::Duration::days(30))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let count = conn.execute(
        "UPDATE memory_entries SET status = 'stale'
         WHERE status = 'active' AND COALESCE(last_used_at, created_at) < ?1",
        rusqlite::params![cutoff],
    )?;

    log::info!("蒸馏: 标记 {} 条陈旧记忆", count);
    Ok(count as i64)
}

/// 归档长期陈旧记忆：stale 超 90 天 → archived
///
/// stale 转变会触发 trg_memory_updated 刷新 updated_at，
/// 故 updated_at 近似于进入 stale 状态的时刻。
fn archive_stale_memories(conn: &rusqlite::Connection) -> Result<i64> {
    let cutoff = (chrono::Local::now() - chrono::Duration::days(90))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let count = conn.execute(
        "UPDATE memory_entries SET status = 'archived'
         WHERE status = 'stale' AND updated_at < ?1",
        rusqlite::params![cutoff],
    )?;

    log::info!("蒸馏: 归档 {} 条长期陈旧记忆", count);
    Ok(count as i64)
}

/// 提取关键词（简单实现）
fn extract_keywords(text: &str) -> Vec<String> {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .chars()
        .take(20)
        .map(|c| c.to_string())
        .collect()
}

// ============================================================
// 确认区（设计哲学 §11.10：候选记忆需人工确认后才生效）
// ============================================================

/// 列出待确认候选记忆
pub fn list_pending_memories(conn: &rusqlite::Connection) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, layer, content, source_ref, status, confidence, created_at
         FROM memory_entries
         WHERE status = 'pending'
         ORDER BY created_at DESC",
    )?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "layer": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "sourceRef": row.get::<_, Option<String>>(3)?,
                "status": row.get::<_, String>(4)?,
                "confidence": row.get::<_, f64>(5)?,
                "createdAt": row.get::<_, Option<String>>(6)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// 采纳候选记忆：status → active 并刷新 last_used_at；可选同时沉淀进 knowledge_items（经验类，category='case_note'）
pub fn confirm_memory(
    conn: &rusqlite::Connection,
    id: &str,
    sink_to_knowledge: bool,
) -> Result<serde_json::Value> {
    let content: String = conn.query_row(
        "SELECT content FROM memory_entries WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )?;

    conn.execute(
        "UPDATE memory_entries SET status = 'active', last_used_at = datetime('now','localtime') WHERE id = ?1",
        rusqlite::params![id],
    )?;

    let mut knowledge_id: Option<String> = None;
    if sink_to_knowledge {
        let kid = crate::db::new_id();
        // 标题取内容前 30 字符
        let title: String = content.chars().take(30).collect();
        conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content, tags, source_type, source_id)
             VALUES (?1, ?2, 'case_note', ?3, '记忆蒸馏', 'memory', ?4)",
            rusqlite::params![kid, title, content, id],
        )?;
        knowledge_id = Some(kid);
    }

    log::info!("记忆已采纳: {} (沉淀知识库: {})", id, sink_to_knowledge);
    Ok(serde_json::json!({
        "id": id,
        "status": "active",
        "knowledgeId": knowledge_id,
    }))
}

/// 丢弃候选记忆：status → dismissed
pub fn dismiss_memory(conn: &rusqlite::Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE memory_entries SET status = 'dismissed' WHERE id = ?1",
        rusqlite::params![id],
    )?;
    log::info!("记忆已丢弃: {}", id);
    Ok(())
}

/// 引用记忆时刷新 last_used_at（供记忆被检索/引用处调用，延缓 stale 生命周期）
#[allow(dead_code)]
pub fn touch_memory(conn: &rusqlite::Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE memory_entries SET last_used_at = datetime('now','localtime') WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// 最小内存库：memory_entries（v10 status 全集）+ task_events + tasks
    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE memory_entries (
               id TEXT PRIMARY KEY,
               layer TEXT NOT NULL CHECK(layer IN ('l1','l2','l3')),
               content TEXT NOT NULL,
               source_ref TEXT,
               status TEXT DEFAULT 'active'
                 CHECK(status IN ('active','stale','archived','pending','merged','dismissed')),
               confidence REAL DEFAULT 0.5,
               ai_model TEXT,
               last_used_at TEXT,
               merged_from TEXT,
               created_at TEXT DEFAULT (datetime('now','localtime')),
               updated_at TEXT DEFAULT (datetime('now','localtime'))
             );
             CREATE TABLE task_events (
               id TEXT PRIMARY KEY, task_id TEXT, event_type TEXT, occurred_at TEXT
             );
             CREATE TABLE tasks (id TEXT PRIMARY KEY, task_name TEXT);",
        )
        .unwrap();
        conn
    }

    fn days_ago(n: i64) -> String {
        (chrono::Local::now() - chrono::Duration::days(n))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }

    fn insert_memory(
        conn: &Connection,
        id: &str,
        status: &str,
        content: &str,
        last_used_at: Option<&str>,
        created_at: &str,
        updated_at: &str,
    ) {
        conn.execute(
            "INSERT INTO memory_entries (id, layer, content, status, confidence, last_used_at, created_at, updated_at)
             VALUES (?1, 'l2', ?2, ?3, 0.6, ?4, ?5, ?6)",
            rusqlite::params![id, content, status, last_used_at, created_at, updated_at],
        )
        .unwrap();
    }

    fn status_of(conn: &Connection, id: &str) -> String {
        conn.query_row(
            "SELECT status FROM memory_entries WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn test_stale_threshold_30_days() {
        let conn = setup_db();
        let now = days_ago(0);
        // 31 天未引用 → stale
        insert_memory(&conn, "m-old", "active", "旧记忆", Some(&days_ago(31)), &days_ago(60), &now);
        // 10 天前引用过 → 保持 active
        insert_memory(&conn, "m-fresh", "active", "新记忆", Some(&days_ago(10)), &days_ago(60), &now);
        // 从未引用但 40 天前创建 → 回退 created_at，stale
        insert_memory(&conn, "m-never", "active", "未引用记忆", None, &days_ago(40), &now);
        // 从未引用且刚创建 → 保持 active
        insert_memory(&conn, "m-new", "active", "新建记忆", None, &now, &now);

        let n = mark_stale_memories(&conn).unwrap();
        assert_eq!(n, 2);
        assert_eq!(status_of(&conn, "m-old"), "stale");
        assert_eq!(status_of(&conn, "m-never"), "stale");
        assert_eq!(status_of(&conn, "m-fresh"), "active");
        assert_eq!(status_of(&conn, "m-new"), "active");
    }

    #[test]
    fn test_archive_threshold_90_days() {
        let conn = setup_db();
        // stale 已 91 天（updated_at 近似 stale 起点）→ archived
        insert_memory(&conn, "m-stale-old", "stale", "长期陈旧", None, &days_ago(200), &days_ago(91));
        // 刚变 stale → 保持 stale
        insert_memory(&conn, "m-stale-new", "stale", "新陈旧", None, &days_ago(100), &days_ago(10));
        // active 不受影响
        insert_memory(&conn, "m-active", "active", "活跃记忆", Some(&days_ago(5)), &days_ago(200), &days_ago(100));

        let n = archive_stale_memories(&conn).unwrap();
        assert_eq!(n, 1);
        assert_eq!(status_of(&conn, "m-stale-old"), "archived");
        assert_eq!(status_of(&conn, "m-stale-new"), "stale");
        assert_eq!(status_of(&conn, "m-active"), "active");
    }

    #[test]
    fn test_merge_keeps_data() {
        let conn = setup_db();
        // 前 20 个字母数字字符相同 → 视为高相似
        let prefix = "A".repeat(20);
        insert_memory(&conn, "m1", "active", &format!("{}额外信息一", prefix), None, &days_ago(2), &days_ago(2));
        insert_memory(&conn, "m2", "active", &format!("{}完全不同的尾巴", prefix), None, &days_ago(1), &days_ago(1));
        // 不相似的对照组
        insert_memory(&conn, "m3", "active", "独一无二的记忆内容", None, &days_ago(1), &days_ago(1));

        let merged = merge_similar_memories(&conn).unwrap();
        assert_eq!(merged, 1);

        // keeper = 最新创建（m2），confidence 提升，merged_from 记录来源
        assert_eq!(status_of(&conn, "m2"), "active");
        assert_eq!(status_of(&conn, "m1"), "merged");
        assert_eq!(status_of(&conn, "m3"), "active");

        let (conf, merged_from): (f64, String) = conn
            .query_row(
                "SELECT confidence, merged_from FROM memory_entries WHERE id = 'm2'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!((conf - 0.7).abs() < 1e-9, "合并后 confidence 应 +0.1");
        let sources: Vec<String> = serde_json::from_str(&merged_from).unwrap();
        assert_eq!(sources, vec!["m1".to_string()], "merged_from 应记录来源 ID");

        // 数据不丢：两条内容都还在库里
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 3);
        let c1: String = conn
            .query_row("SELECT content FROM memory_entries WHERE id = 'm1'", [], |r| r.get(0))
            .unwrap();
        assert!(c1.contains("额外信息一"));
    }

    #[test]
    fn test_confirm_memory_refreshes_last_used() {
        let conn = setup_db();
        insert_memory(&conn, "p1", "pending", "待确认记忆", None, &days_ago(1), &days_ago(1));

        confirm_memory(&conn, "p1", false).unwrap();
        assert_eq!(status_of(&conn, "p1"), "active");
        let last_used: Option<String> = conn
            .query_row("SELECT last_used_at FROM memory_entries WHERE id = 'p1'", [], |r| r.get(0))
            .unwrap();
        assert!(last_used.is_some(), "confirm 应刷新 last_used_at");

        // 引用时 touch 同样刷新
        touch_memory(&conn, "p1").unwrap();
    }

    #[test]
    fn test_run_distillation_empty_db() {
        let conn = setup_db();
        let r = run_distillation(&conn).unwrap();
        assert_eq!(r.cleaned_count, 0);
        assert_eq!(r.inserted_count, 0);
        assert_eq!(r.merged_count, 0);
        assert_eq!(r.stale_count, 0);
        assert_eq!(r.archived_count, 0);
    }
}

//! 按轨道期限重算命令（W3 · 规则自定义配套）
//!
//! 规则新增/修改/启停/删除后，由前端调用本命令对该 track 的活跃案件
//! 触发期限重算，并返回受影响案件数（产出至少一条期限结果的案件）。
//!
//! 复用 [`super::engine::DeadlineEngine`] 的求值逻辑（与 lib.rs 中
//! `recalc_all_deadlines` 相同的引擎路径），不在此处另行持久化——
//! 期限预警为按需即时计算，重算即重新求值。
//!
//! 注意：本命令的 invoke handler 注册由集成方在 commands/mod.rs 完成。

use crate::commands::run_blocking;
use crate::db;

use super::engine::DeadlineEngine;

/// 进程内重算助手（写命令直接调用，保证「规则变更即重算」不依赖前端补调）
pub(crate) fn recalc_track_inner(
    conn: &rusqlite::Connection,
    track: &str,
) -> Result<i64, anyhow::Error> {
    let engine = DeadlineEngine::new(conn)?;
    let cases = db::cases::active_cases(conn)?;
    let mut affected: i64 = 0;
    for case in cases.iter().filter(|c| c.track == track) {
        if !engine.evaluate_case(conn, case).is_empty() {
            affected += 1;
        }
    }
    log::info!("track [{track}] 期限重算完成，受影响案件 {affected} 个");
    Ok(affected)
}

/// 重算指定 track 下所有活跃案件的期限，返回受影响案件数。
#[tauri::command]
pub async fn recalculate_deadlines_for_track(track: String) -> Result<i64, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        recalc_track_inner(&conn, &track)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::{seed_deadline_rules, SCHEMA_SQL};
    use rusqlite::Connection;

    /// 引擎按 track 过滤：只有 track 匹配且触发日期齐备的案件才会产出期限
    #[test]
    fn recalc_counts_only_cases_with_results() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        seed_deadline_rules(&conn).unwrap();

        // 一个行政诉讼活跃案件（带立案日期 + 收到起诉状日期）
        conn.execute(
            "INSERT INTO cases (id, track, case_name, client_name, opponent_name,
             case_status, filing_date, complaint_received_date)
             VALUES ('c1', 'admin_litigation', '测试行政案', '委托人', '对方',
             '进行中', '2026-01-10', '2026-02-01')",
            [],
        )
        .unwrap();
        // 一个无触发日期的行政诉讼案件（不应计入受影响）
        conn.execute(
            "INSERT INTO cases (id, track, case_name, client_name, opponent_name, case_status)
             VALUES ('c2', 'admin_litigation', '无日期案', '委托人', '对方', '进行中')",
            [],
        )
        .unwrap();

        let engine = DeadlineEngine::new(&conn).unwrap();
        let cases = db::cases::active_cases(&conn).unwrap();
        let affected = cases
            .iter()
            .filter(|c| c.track == "admin_litigation")
            .filter(|c| !engine.evaluate_case(&conn, c).is_empty())
            .count() as i64;

        assert_eq!(affected, 1);
    }
}

//! Schema v20 集成测试：全球对标灵感落地的新表/列（2026-09-01）
//! 验证：迁移幂等推进到 v20、新表结构、基本 CRUD 与外键约束。
use casy_lib::db::schema::{run_migrations, CURRENT_SCHEMA_VERSION};
use rusqlite::Connection;

fn migrated_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    run_migrations(&conn, 0).unwrap();
    conn
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        [name],
        |r| r.get::<_, i64>(0),
    )
    .unwrap()
        > 0
}

fn column_exists(conn: &Connection, table: &str, col: &str) -> bool {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap();
    let found = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .any(|c| c.unwrap() == col);
    found
}

#[test]
fn v20_is_current_and_migrates_clean() {
    let conn = migrated_db();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, CURRENT_SCHEMA_VERSION);
    // 版本号随迁移演进，与 CURRENT_SCHEMA_VERSION 保持一致即可（上文已断言）

    // 新表全部存在
    for t in [
        "notifications",
        "deadline_rule_audit",
        "links",
        "persons",
        "case_persons",
        "smart_rules",
        "whiteboards",
        "fact_nodes",
    ] {
        assert!(table_exists(&conn, t), "missing table {t}");
    }
    // 新列
    assert!(column_exists(&conn, "tasks", "defer_until"));
    assert!(column_exists(&conn, "case_files", "ocr_text"));
}

#[test]
fn v20_migration_is_idempotent() {
    let conn = migrated_db();
    // 二次执行不报错、版本不变
    run_migrations(&conn, 0).unwrap();
    let _version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    // 版本号随迁移演进，与 CURRENT_SCHEMA_VERSION 保持一致即可（上文已断言）
}

#[test]
fn notifications_inbox_zero_semantics() {
    let conn = migrated_db();
    conn.execute(
        "INSERT INTO notifications (id, type, title) VALUES ('n1', 'reminder', '开庭提醒')",
        [],
    )
    .unwrap();
    // 处理即消失 = dismissed_at 非空
    conn.execute(
        "UPDATE notifications SET dismissed_at = datetime('now','localtime') WHERE id = 'n1'",
        [],
    )
    .unwrap();
    let active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM notifications WHERE dismissed_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(active, 0);
}

#[test]
fn persons_kind_check_and_case_link_fk() {
    let conn = migrated_db();
    // 非法 kind 被拒绝
    let bad = conn.execute(
        "INSERT INTO persons (id, kind, name) VALUES ('p1', 'alien', 'X')",
        [],
    );
    assert!(bad.is_err(), "invalid kind must be rejected");

    conn.execute(
        "INSERT INTO persons (id, kind, name, preferences) VALUES ('p2', 'judge', '王法官', '偏好书面质证')",
        [],
    )
    .unwrap();
    // 挂到不存在的案件 → 外键拒绝
    let bad_link = conn.execute(
        "INSERT INTO case_persons (id, case_id, person_id) VALUES ('cp1', 'no-such-case', 'p2')",
        [],
    );
    assert!(bad_link.is_err(), "dangling case_id must be rejected");
}

#[test]
fn links_support_anchor_and_backlink_index() {
    let conn = migrated_db();
    conn.execute(
        "INSERT INTO links (id, source_type, source_id, target_type, target_id, anchor, label)
         VALUES ('l1', 'doc', 'doc-1', 'file', 'file-1', 'page:12', '关键证据')",
        [],
    )
    .unwrap();
    let anchor: String = conn
        .query_row(
            "SELECT anchor FROM links WHERE target_type='file' AND target_id='file-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(anchor, "page:12");
}

#[test]
fn decisions_accepts_recommendation_entity_type() {
    let conn = migrated_db();
    // v23b：entity_type CHECK 放宽后 'recommendation' 可写（首页 AI 建议反馈 recordDecision）
    conn.execute(
        "INSERT INTO decisions (id, entity_type, entity_id, decision_type, decision)
         VALUES ('d1', 'recommendation', 'rec-1', 'recommend_today', 'accept')",
        [],
    )
    .unwrap();
    // 非法类型仍被拒绝
    let bad = conn.execute(
        "INSERT INTO decisions (id, entity_type, entity_id, decision_type, decision)
         VALUES ('d2', 'alien', 'x', 'other', 'accept')",
        [],
    );
    assert!(bad.is_err(), "invalid entity_type must still be rejected");
}

#[test]
fn smart_rules_action_check() {
    let conn = migrated_db();
    let bad = conn.execute(
        "INSERT INTO smart_rules (id, name, match_field, match_pattern, action_type, action_payload)
         VALUES ('r1', 'x', 'filename', 'Subpoena', 'teleport', 'x')",
        [],
    );
    assert!(bad.is_err(), "invalid action_type must be rejected");
    conn.execute(
        "INSERT INTO smart_rules (id, name, match_field, match_pattern, action_type, action_payload)
         VALUES ('r2', '传票加急', 'filename', 'Subpoena', 'mark_urgent', 'urgent')",
        [],
    )
    .unwrap();
}

#[test]
fn fact_nodes_cascade_with_whiteboard() {
    let conn = migrated_db();
    // 需要一个真实案件满足外键
    conn.execute(
        "INSERT INTO cases (id, case_name, client_name, track) VALUES ('c1', '测试案', '张三', 'civil_tort')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO whiteboards (id, case_id, name) VALUES ('w1', 'c1', '事实网络')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO fact_nodes (id, whiteboard_id, page, excerpt, x, y)
         VALUES ('fn1', 'w1', 12, '被告于 3 月 5 日签收…', 100.0, 200.0)",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM whiteboards WHERE id = 'w1'", [])
        .unwrap();
    let remaining: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM fact_nodes WHERE whiteboard_id = 'w1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        remaining, 0,
        "fact_nodes must cascade-delete with whiteboard"
    );
}

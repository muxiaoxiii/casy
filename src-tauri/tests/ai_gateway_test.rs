use casy_lib::ai::gateway::{
    approve_proposal, compute_entity_hash, create_proposal, validate_and_consume_token,
    verify_ai_mutation_authorized,
};
use rusqlite::Connection;

fn setup_gateway_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE ai_proposals (
            id TEXT PRIMARY KEY,
            tool_name TEXT NOT NULL,
            target_entity_type TEXT NOT NULL,
            target_entity_id TEXT,
            pre_state_hash TEXT,
            payload_json TEXT NOT NULL,
            auth_token TEXT NOT NULL UNIQUE,
            expires_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL,
            executed_at TEXT
        );",
    )
    .unwrap();
    conn
}

#[test]
fn test_unauthorized_ai_write_rejected_by_default() {
    let conn = setup_gateway_db();

    // 1. 无 token 直接调用 AI 写操作 -> 硬拒绝
    let res = verify_ai_mutation_authorized(
        &conn,
        Some("ai"),
        None,
        "update_task",
        "task",
        Some("task-100"),
        None,
        &serde_json::json!({"dueDate":"2026-09-01"}),
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("PermissionDenied: AI write operation requires an approved proposal token"));

    // 2. 空 token 调用 -> 硬拒绝
    let res_empty = verify_ai_mutation_authorized(
        &conn,
        Some("ai"),
        Some("   "),
        "update_task",
        "task",
        Some("task-100"),
        None,
        &serde_json::json!({}),
    );
    assert!(res_empty.is_err());

    // 3. 正常用户直接操作 (origin: "user" 或 None) -> 直接通过
    let res_user = verify_ai_mutation_authorized(
        &conn,
        Some("user"),
        None,
        "update_task",
        "task",
        Some("task-100"),
        None,
        &serde_json::json!({}),
    );
    assert!(res_user.is_ok());
}

#[test]
fn test_proposal_lifecycle_and_single_use_token() {
    let conn = setup_gateway_db();

    let initial_data = serde_json::json!({
        "taskName": "初审答辩",
        "status": "todo"
    });
    let state_hash = compute_entity_hash(&initial_data);

    // 1. AI 发起提案
    let proposal = create_proposal(
        &conn,
        "update_task",
        "task",
        Some("task-200"),
        Some(&state_hash),
        &serde_json::json!({"completed": 1}).to_string(),
        Some(300),
    )
    .unwrap();

    assert_eq!(proposal.status, "pending");

    // 2. 用户尚未在 UI 确认前，直接拿 token 调用写操作 -> 拒绝 (not approved)
    let early_consume = validate_and_consume_token(
        &conn,
        &proposal.auth_token,
        "update_task",
        "task",
        Some("task-200"),
        Some(&state_hash),
        &serde_json::json!({"completed": 1}),
    );
    assert!(early_consume.is_err());
    assert!(early_consume
        .unwrap_err()
        .to_string()
        .contains("Proposal is not approved"));

    // 3. 用户在 UI 显式确认批准
    let token = approve_proposal(&conn, &proposal.id).unwrap();
    assert_eq!(token, proposal.auth_token);

    // 4. 服务端写命令携带 token 执行 -> 校验通过并原子消费
    let consumed = validate_and_consume_token(
        &conn,
        &token,
        "update_task",
        "task",
        Some("task-200"),
        Some(&state_hash),
        &serde_json::json!({"completed": 1}),
    )
    .unwrap();
    assert_eq!(consumed.id, proposal.id);

    // 5. 再次使用同一 token (重放攻击或重复调用) -> 拒绝 (已被消费，单次幂等)
    let replay = validate_and_consume_token(
        &conn,
        &token,
        "update_task",
        "task",
        Some("task-200"),
        Some(&state_hash),
        &serde_json::json!({"completed": 1}),
    );
    assert!(replay.is_err());
    assert!(replay
        .unwrap_err()
        .to_string()
        .contains("Token has already been consumed"));
}

#[test]
fn test_state_hash_mismatch_prevents_race_condition() {
    let conn = setup_gateway_db();

    let old_state = serde_json::json!({ "dueDate": "2026-08-30" });
    let old_hash = compute_entity_hash(&old_state);

    // 提案基于 old_hash
    let proposal = create_proposal(
        &conn,
        "update_task",
        "task",
        Some("task-300"),
        Some(&old_hash),
        &serde_json::json!({"dueDate": "2026-09-05"}).to_string(),
        Some(300),
    )
    .unwrap();

    approve_proposal(&conn, &proposal.id).unwrap();

    // 在消费前，数据已被其他端修改为 new_hash
    let new_state = serde_json::json!({ "dueDate": "2026-09-01" });
    let new_hash = compute_entity_hash(&new_state);

    let res = validate_and_consume_token(
        &conn,
        &proposal.auth_token,
        "update_task",
        "task",
        Some("task-300"),
        Some(&new_hash),
        &serde_json::json!({"dueDate": "2026-09-05"}),
    );

    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("State hash mismatch"));
}

#[test]
fn test_payload_mismatch_is_rejected_without_consuming_token() {
    let conn = setup_gateway_db();
    let proposal = create_proposal(
        &conn,
        "update_task",
        "task",
        Some("task-400"),
        None,
        &serde_json::json!({"dueDate": "2026-09-05"}).to_string(),
        Some(300),
    )
    .unwrap();
    approve_proposal(&conn, &proposal.id).unwrap();

    let mismatch = validate_and_consume_token(
        &conn,
        &proposal.auth_token,
        "update_task",
        "task",
        Some("task-400"),
        None,
        &serde_json::json!({"dueDate": "2026-10-01"}),
    );
    assert!(mismatch
        .unwrap_err()
        .to_string()
        .contains("Payload mismatch"));

    let valid = validate_and_consume_token(
        &conn,
        &proposal.auth_token,
        "update_task",
        "task",
        Some("task-400"),
        None,
        &serde_json::json!({"dueDate": "2026-09-05"}),
    );
    assert!(valid.is_ok());
}

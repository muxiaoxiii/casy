use casy_lib::{db, commands};
#[tokio::test]
async fn direct_mcp_writes_require_confirmation() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let conn = db::open_db().unwrap(); db::init_db(&conn).unwrap();
    conn.execute("INSERT INTO cases(id,case_name,client_name) VALUES('c','Case','Client')", []).unwrap();
    let result = commands::mcp_execute_tool("case_create_task".into(), serde_json::json!({"case_id":"c","task_name":"must not execute"})).await.unwrap();
    assert_eq!(result["Ok"]["status"], "pending_confirmation");
    assert_eq!(commands::list_mcp_pending_writes().await.unwrap().len(), 1);
    assert_eq!(conn.query_row("SELECT count(*) FROM tasks", [], |r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(conn.query_row("SELECT count(*) FROM audit_events WHERE event_type='mcp_write_submitted'", [], |r|r.get::<_,i64>(0)).unwrap(),1);
    let id = result["Ok"]["write_id"].as_str().unwrap().to_owned();
    let (first, second) = tokio::join!(commands::approve_mcp_write(id.clone()), commands::approve_mcp_write(id));
    assert_ne!(first.is_ok(), second.is_ok(), "first={first:?}; second={second:?}");
    assert_eq!(conn.query_row("SELECT count(*) FROM tasks", [], |r|r.get::<_,i64>(0)).unwrap(),1);
    assert!(commands::list_mcp_pending_writes().await.unwrap().is_empty());
    drop(conn); db::reset_shared_conn();
}

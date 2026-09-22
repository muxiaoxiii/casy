use casy_lib::{
    commands::{editor_tasks as e, tasks},
    db,
};
#[tokio::test]
async fn checklist_links_are_idempotent_bidirectional_and_do_not_delete_tasks() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute_batch("INSERT INTO knowledge_items(id,title,content,category) VALUES('note','笔记','正文','reference'); INSERT INTO drafts(id,title,content) VALUES('draft','文书','');").unwrap();
    let bind = || {
        e::bind_editor_task(
            "knowledge".into(),
            "note".into(),
            "same-block".into(),
            "核实转文".into(),
            None,
            None,
        )
    };
    let id = bind().await.unwrap();
    assert_eq!(bind().await.unwrap(), id);
    assert_eq!(
        conn.query_row("SELECT count(*) FROM tasks", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    e::set_editor_task_completed("knowledge".into(), "note".into(), id.clone(), true, false)
        .await
        .unwrap();
    assert!(
        e::get_editor_tasks("knowledge".into(), "note".into())
            .await
            .unwrap()[0]
            .completed
    );
    tasks::toggle_task(id.clone(), None, None, None)
        .await
        .unwrap();
    assert!(
        !e::get_editor_tasks("knowledge".into(), "note".into())
            .await
            .unwrap()[0]
            .completed
    );
    assert!(
        e::set_editor_task_completed("doc".into(), "draft".into(), id.clone(), true, false)
            .await
            .is_err()
    );
    e::bind_editor_task(
        "doc".into(),
        "draft".into(),
        "existing".into(),
        "核实转文".into(),
        Some(id.clone()),
        None,
    )
    .await
    .unwrap();
    e::set_editor_task_completed("doc".into(), "draft".into(), id.clone(), true, false)
        .await
        .unwrap();
    assert!(
        e::get_editor_tasks("knowledge".into(), "note".into())
            .await
            .unwrap()[0]
            .completed
    );
    conn.execute("UPDATE knowledge_items SET content='' WHERE id='note'", [])
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM tasks WHERE deleted_at IS NULL",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    conn.execute(
        "UPDATE tasks SET deleted_at='2026-09-08' WHERE id=?1",
        [&id],
    )
    .unwrap();
    assert!(
        e::get_editor_tasks("knowledge".into(), "note".into())
            .await
            .unwrap()[0]
            .missing
    );
    assert!(
        e::set_editor_task_completed("knowledge".into(), "note".into(), id, false, true)
            .await
            .is_err()
    );
}

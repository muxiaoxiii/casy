use casy_lib::{
    commands::{
        deadline_rules, drafts, knowledge, linking, notifications, relations, smart_rules,
        whiteboard,
    },
    db,
};

#[tokio::test]
async fn readers_reject_corrupt_rows_and_relation_detection_rolls_back() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute_batch("INSERT INTO cases(id,case_name,client_name,patent_app_no) VALUES
        ('a','A','Client','P'),('b','B','Client','P');
        INSERT INTO drafts(id,title,content) VALUES('d',X'FF','text');
        INSERT INTO notifications(id,type,title) VALUES('n','system',X'FF');
        INSERT INTO whiteboards(id,case_id,name) VALUES('w','a',X'FF');
        INSERT INTO smart_rules(id,name,match_field,match_pattern,action_type,action_payload)
            VALUES('s',X'FF','filename','x','mark_urgent','');
        INSERT INTO knowledge_items(id,title,content,category) VALUES('k','Parent','text','reference');
        INSERT INTO knowledge_items(id,title,content,category,parent_id) VALUES('child',X'FF','text','reference','k');").unwrap();
    conn.execute_batch("INSERT INTO deadline_rule_audit(id,rule_id,action,actor) VALUES('audit','rule','create',X'FF');
        INSERT INTO links(id,source_type,source_id,target_type,target_id,label) VALUES('link','case','a','case','b',X'FF');
        INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f','a','x.txt','/missing/x.txt','evidence');").unwrap();
    assert!(deadline_rules::list_deadline_rule_audit(None)
        .await
        .is_err());
    assert!(linking::list_links_for("case".into(), "a".into())
        .await
        .is_err());
    assert!(smart_rules::run_smart_rules_for_all()
        .await
        .unwrap_err()
        .contains("失败 1"));
    assert!(drafts::list_drafts().await.is_err());
    assert!(notifications::list_notifications().await.is_err());
    assert!(whiteboard::list_whiteboards("a".into()).await.is_err());
    assert!(smart_rules::list_smart_rules().await.is_err());
    assert!(knowledge::get_knowledge_with_blocks("k".into())
        .await
        .is_err());
    conn.execute_batch(
        "CREATE TRIGGER fail_second_relation BEFORE INSERT ON case_relations
        WHEN NEW.relation_type='same_party' BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
    )
    .unwrap();
    assert!(relations::detect_relations("a".into()).await.is_err());
    let count: i64 = conn
        .query_row("SELECT count(*) FROM case_relations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 0,
        "failed detection must not leave the first relation committed"
    );
    conn.execute_batch(
        "DROP TRIGGER fail_second_relation;
        UPDATE drafts SET title='Draft' WHERE id='d';
        UPDATE notifications SET title='Notice' WHERE id='n';
        UPDATE whiteboards SET name='Board' WHERE id='w';
        UPDATE smart_rules SET name='Rule' WHERE id='s';
        UPDATE knowledge_items SET title='Child' WHERE id='child';",
    )
    .unwrap();
    assert_eq!(drafts::list_drafts().await.unwrap().len(), 1);
    assert!(!notifications::list_notifications()
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        whiteboard::list_whiteboards("a".into())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(smart_rules::list_smart_rules().await.unwrap().len(), 1);
    assert!(knowledge::get_knowledge_with_blocks("k".into())
        .await
        .is_ok());
    assert_eq!(
        relations::detect_relations("a".into()).await.unwrap().len(),
        2
    );
    assert!(relations::detect_relations("a".into())
        .await
        .unwrap()
        .is_empty());
    conn.execute_batch(
        "UPDATE smart_rules SET action_type='add_keyword',action_payload='new' WHERE id='s';
        UPDATE case_files SET knowledge_keywords=X'FF' WHERE id='f';",
    )
    .unwrap();
    assert!(smart_rules::apply_smart_rules("f".into()).await.is_err());
    let kind: String = conn
        .query_row(
            "SELECT typeof(knowledge_keywords) FROM case_files WHERE id='f'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        kind, "blob",
        "failed keyword decoding must not overwrite the original value"
    );
    drop(conn);
    db::reset_shared_conn();
}

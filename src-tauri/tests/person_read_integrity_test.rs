use casy_lib::{commands::{persons, cases}, db};

#[tokio::test]
async fn corrupt_person_rows_are_errors_not_successful_partial_lists() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO persons(id,kind,name) VALUES('good','contact','Good');
        INSERT INTO persons(id,kind,name) VALUES('bad','contact',X'FF');
        INSERT INTO cases(id,case_name,client_name) VALUES('case','Case','Client');
        INSERT INTO case_persons(id,case_id,person_id) VALUES('link','case','bad');",
    )
    .unwrap();
    assert!(persons::list_persons(None, None).await.is_err());
    assert!(persons::list_case_persons("case".into()).await.is_err());
    conn.execute("UPDATE persons SET name='Repaired' WHERE id='bad'", [])
        .unwrap();
    assert_eq!(persons::list_persons(None, None).await.unwrap().len(), 2);
    let filter = serde_json::json!({"caseType": "' OR 1=1 --"});
    assert!(cases::get_case_unified_view(Some(filter)).await.unwrap().is_empty());
    let quoted = "client's dispute";
    conn.execute("UPDATE cases SET cause_action=?1 WHERE id='case'", [quoted]).unwrap();
    let filter = serde_json::json!({"caseType": quoted});
    assert!(!cases::get_case_unified_view(Some(filter)).await.unwrap().is_empty());
    assert_eq!(
        persons::list_case_persons("case".into())
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        persons::attach_person_to_case("case".into(), "bad".into(), None)
            .await
            .is_err()
    );
    assert!(
        persons::attach_person_to_case("case".into(), "bad".into(), Some("  ".into()))
            .await
            .is_err()
    );
    persons::attach_person_to_case("case".into(), "bad".into(), Some("client".into()))
        .await
        .unwrap();
    assert!(
        persons::attach_person_to_case("case".into(), "bad".into(), Some(" client ".into()))
            .await
            .is_err()
    );
    assert_eq!(
        persons::list_case_persons("case".into())
            .await
            .unwrap()
            .len(),
        2
    );
    drop(conn);
    db::reset_shared_conn();
}

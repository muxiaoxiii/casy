use casy_lib::{db, commands::{self, deadline_rules}};
#[tokio::test]
async fn invalid_rules_and_corrupt_calendars_are_not_silent_successes() {
    let root=tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR",root.path()); db::enable_test_mode();
    let conn=db::open_db().unwrap(); db::init_db(&conn).unwrap();
    let count=||conn.query_row("SELECT count(*) FROM deadline_rules",[],|r|r.get::<_,i64>(0)).unwrap();
    let before=count();
    for (field, amount, unit, method) in [("filingDate",15,"day","civil"),("filing_date",0,"day","civil"),("filing_date",121,"calendar_month","civil"),("filing_date",15,"day","typo")] {
        assert!(deadline_rules::upsert_deadline_rule(None,"civil_tort".into(),"Test".into(),"Test".into(),field.into(),amount,unit.into(),method.into(),None,"recommended".into(),1).await.is_err());
    }
    assert_eq!(count(),before);
    db::set_setting(&conn,"holidays_json","{broken").unwrap();
    assert!(commands::get_deadline_warnings().await.is_err());
    drop(conn);db::reset_shared_conn();
}

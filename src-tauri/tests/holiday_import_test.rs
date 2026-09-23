use casy_lib::{commands::{inbox, settings}, db};
use serde_json::json;
#[tokio::test]
async fn reviewed_holidays_commit_with_receipt_and_are_visible_to_calendar() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path()); db::enable_test_mode();
    { let conn = db::open_db().unwrap(); db::init_db(&conn).unwrap(); }
    let parsed = inbox::parse_holiday_notice("2027年1月1日至3日放假调休，1月4日上班。2月6日、2月7日上班。".into()).await.unwrap();
    assert_eq!(parsed.holidays, vec!["2027-01-01", "2027-01-02", "2027-01-03"]);
    assert_eq!(parsed.workdays, vec!["2027-01-04", "2027-02-06", "2027-02-07"]);
    let id = inbox::add_inbox_item("note".into(), None, Some("AI 或人工核对后的通知原文".into()), None).await.unwrap();
    let invalid = json!({"year":2027,"holidays":["2027-02-29"],"workdays":[]});
    assert!(inbox::confirm_inbox_action(id.clone(), "update_holidays".into(), None, None, Some(invalid)).await.is_err());
    assert!(inbox::get_inbox_action_result(id.clone()).await.unwrap().is_none());
    assert_eq!(db::open_db().unwrap().query_row("SELECT status FROM inbox_items WHERE id=?1",[&id],|r|r.get::<_,String>(0)).unwrap(), "pending");
    let reviewed = json!({"year":2027,"holidays":["2027-01-01","2027-01-02"],"workdays":["2027-01-03"]});
    let receipt = inbox::confirm_inbox_action(id.clone(), "update_holidays".into(), None, None, Some(reviewed.clone())).await.unwrap();
    assert_eq!(receipt["holidaysCount"], 2);
    assert_eq!(receipt["holidays"], reviewed["holidays"]);
    assert_eq!(inbox::get_inbox_action_result(id.clone()).await.unwrap(), Some(receipt.clone()));
    assert_eq!(inbox::confirm_inbox_action(id, "update_holidays".into(), None, None, Some(reviewed)).await.unwrap(), receipt);
    let entries = settings::get_holiday_calendar(2027).await.unwrap();
    assert!(entries["entries"].as_array().unwrap().iter().any(|e|e["date"]=="2027-01-02" && e["kind"]=="holiday"));
    assert!(entries["entries"].as_array().unwrap().iter().any(|e|e["date"]=="2027-01-03" && e["kind"]=="workday"));
    // Personal schedules coexist with statutory dates without changing legal deadlines.
    let official_before = db::get_setting(&db::open_db().unwrap(), "holidays_json").unwrap();
    let personal = json!([{"date":"2027-01-02","kind":"workday","name":"个人补班"},{"date":"2027-01-04","kind":"holiday","name":"个人休息"}]);
    settings::save_settings(std::collections::HashMap::from([("personal_calendar_days".into(),personal.clone())])).await.unwrap();
    let entries = settings::get_holiday_calendar(2027).await.unwrap();
    let same_day: Vec<_> = entries["entries"].as_array().unwrap().iter().filter(|e|e["date"] == "2027-01-02").collect();
    assert_eq!(same_day.len(),2);
    assert!(same_day.iter().any(|e|e["source"] == "official" && e["kind"] == "holiday"));
    assert!(same_day.iter().any(|e|e["source"] == "personal" && e["kind"] == "workday"));
    assert_eq!(db::get_setting(&db::open_db().unwrap(), "holidays_json").unwrap(), official_before);
    let invalid = json!([{"date":"2027-02-29","kind":"holiday","name":""}]);
    assert!(settings::save_settings(std::collections::HashMap::from([("personal_calendar_days".into(),invalid)])).await.is_err());
    assert_eq!(settings::get_settings().await.unwrap()["personal_calendar_days"],personal);
    settings::save_settings(std::collections::HashMap::from([("personal_calendar_days".into(),json!([]))])).await.unwrap();
    assert_eq!(settings::get_holiday_calendar(2027).await.unwrap()["entries"].as_array().unwrap().iter().filter(|e|e["date"]=="2027-01-02").count(),1);
    db::reset_shared_conn();
}

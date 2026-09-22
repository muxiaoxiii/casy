use casy_lib::{commands::{calendar,calendar_events,cases,drafts,tasks},db};
use serde_json::json;
#[tokio::test]
async fn modular_integrity_and_connection_lifecycle() {
 let profile=tempfile::tempdir().unwrap();std::env::set_var("CASY_TEST_DATA_DIR",profile.path());db::enable_test_mode();
 let conn=db::open_db().unwrap();db::init_db(&conn).unwrap();
 conn.execute_batch("INSERT INTO cases(id,case_name,client_name,civil_status) VALUES('c','核对案件','客户','filed'); INSERT INTO knowledge_items(id,title,content,category) VALUES('k','笔记','内容','reference'); INSERT INTO tasks(id,task_name,case_id,knowledge_id,created_date) VALUES('t','准备','c','k','2026-09-01');").unwrap();
 let event=calendar_events::create_calendar_event(json!({"title":"开会","eventDate":"2026-09-17","startTime":"09:00","endTime":"10:00","caseId":"c","taskId":"t"})).await.unwrap();
 calendar_events::update_calendar_event(event.id.clone(),json!({"location":"会议室"})).await.unwrap();
 assert!(calendar_events::update_calendar_event(event.id.clone(),json!({"endTime":"08:00"})).await.is_err());
 let rows=calendar_events::list_calendar_events("2026-09-17".into(),"2026-09-17".into()).await.unwrap();
 assert_eq!(rows[0].location.as_deref(),Some("会议室"));assert_eq!(rows[0].end_time.as_deref(),Some("10:00"));
 conn.execute("UPDATE tasks SET deleted_at='2026-09-17' WHERE id='t'",[]).unwrap();
 assert!(calendar_events::list_calendar_events("2026-09-17".into(),"2026-09-17".into()).await.unwrap().is_empty());
 assert!(!calendar::get_calendar_events(2026,8,Some(3)).await.unwrap().iter().any(|e|e.id==event.id));
 conn.execute("DELETE FROM knowledge_items WHERE id='k'",[]).unwrap();
 assert!(conn.query_row("SELECT knowledge_id IS NULL FROM tasks WHERE id='t'",[],|r|r.get::<_,bool>(0)).unwrap());
 let draft=drafts::create_draft("文书".into(),Some("正文".into()),Some("c".into()),None).await.unwrap();
 let draft=drafts::update_draft(draft.id,None,Some("修订".into()),None,None,Some(draft.version),Some(true)).await.unwrap();assert!(draft.case_id.is_none());
 conn.execute_batch("CREATE TRIGGER fail_history BEFORE INSERT ON case_track_history BEGIN SELECT RAISE(ABORT,'test history failure'); END;").unwrap();
 assert!(cases::update_case_status("c".into(),"civil_status".into(),"closed".into(),None).await.is_err());
 assert_eq!(conn.query_row("SELECT civil_status FROM cases WHERE id='c'",[],|r|r.get::<_,String>(0)).unwrap(),"filed");
 conn.execute_batch("DROP TRIGGER fail_history; CREATE TRIGGER fail_prep BEFORE INSERT ON tasks WHEN NEW.task_name LIKE '%人员%' BEGIN SELECT RAISE(ABORT,'test prep failure'); END;").unwrap();
 let before:i64=conn.query_row("SELECT count(*) FROM tasks",[],|r|r.get(0)).unwrap();
 assert!(tasks::generate_hearing_prep_tasks("c".into(),"h".into(),"2026-10-17T09:30:00+08:00".into()).await.is_err());
 assert_eq!(conn.query_row("SELECT count(*) FROM tasks",[],|r|r.get::<_,i64>(0)).unwrap(),before);
 conn.execute_batch("DROP TRIGGER fail_prep").unwrap();
 assert_eq!(tasks::generate_hearing_prep_tasks("c".into(),"h".into(),"2026-10-17 09:30".into()).await.unwrap().len(),6);
 let independent=calendar_events::create_calendar_event(json!({"title":"独立日程","eventDate":"2026-09-20","caseId":"c"})).await.unwrap();
 // Revalidation refreshes the hash and invalidates the old token without authorizing execution.
 let proposal=casy_lib::commands::ai_routes::create_ai_proposal("update_case".into(),"case".into(),Some("c".into()),None,"{\"caseName\":\"新名称\"}".into(),None).await.unwrap();
 conn.execute("UPDATE cases SET notes='新材料' WHERE id='c'",[]).unwrap();
 let renewed=casy_lib::commands::ai_routes::renew_ai_proposal(proposal.id).await.unwrap();
 assert_eq!(renewed.status,"pending");assert_ne!(renewed.pre_state_hash,proposal.pre_state_hash);assert_ne!(renewed.auth_token,proposal.auth_token);
 // Reading settings never returns password content or keychain references.
 db::set_setting(&conn,"smtp_pass","fixture-secret").unwrap();
 let public=casy_lib::commands::settings::get_settings().await.unwrap();assert_eq!(public["smtp_pass"],json!(""));assert_eq!(public["smtp_pass_configured"],json!(true));
 assert!(!serde_json::to_string(&public).unwrap().contains("fixture-secret"));
 // A failed migration leaves the original secret intact; explicit null clears it without Keychain access.
 let failed=std::collections::HashMap::from([("smtp_pass".into(),json!("replacement"))]);
 assert!(casy_lib::commands::settings::save_settings(failed).await.is_err());assert_eq!(db::get_setting(&conn,"smtp_pass").unwrap().as_deref(),Some("fixture-secret"));
 casy_lib::commands::settings::save_settings(std::collections::HashMap::from([("smtp_pass".into(),serde_json::Value::Null)])).await.unwrap();
 assert_eq!(db::get_setting(&conn,"smtp_pass").unwrap().as_deref(),Some(""));
 db::cases::delete_case(&conn,"c").unwrap();assert!(conn.query_row("SELECT case_id IS NULL FROM calendar_events WHERE id=?1",[&independent.id],|r|r.get::<_,bool>(0)).unwrap());
 assert_eq!(conn.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);drop(conn);
 let conn=db::open_db().unwrap();conn.execute_batch("CREATE TEMP TABLE pool_probe(value INTEGER); INSERT INTO pool_probe VALUES(7);").unwrap();drop(conn);
 let conn=db::open_db().unwrap();assert_eq!(conn.query_row("SELECT value FROM pool_probe",[],|r|r.get::<_,i32>(0)).unwrap(),7);drop(conn);
 let maintenance=db::enter_maintenance().unwrap();assert!(db::open_db().is_err());drop(maintenance);
 let conn=db::open_db().unwrap();assert!(conn.prepare("SELECT * FROM pool_probe").is_err());drop(conn);db::reset_shared_conn();
}

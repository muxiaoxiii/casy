use casy_lib::{commands::{inbox, calendar_events}, db};
use rusqlite::params;
use serde_json::{json, Value};

async fn confirm(id: &str, action: &str, intent: Value) -> Result<Value, String> {
    inbox::confirm_inbox_action(id.into(), action.into(), Some("capture-case".into()), None, Some(intent)).await
}

#[tokio::test]
async fn reviewed_capture_is_atomic_retryable_and_preserves_sources() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let conn = db::open_db().unwrap();
    for (version, sql) in db::schema::MIGRATIONS.iter().take_while(|(version,_)|version.parse::<i64>().unwrap()<=30) {
        conn.execute_batch(sql).unwrap();
        conn.pragma_update(None,"user_version",version.parse::<i64>().unwrap()).unwrap();
    }
    conn.execute_batch("INSERT INTO inbox_items(id,source_type,title,status) VALUES('upgrade-source','note','升级前原文','pending');").unwrap();
    db::init_db(&conn).unwrap();
    assert_eq!(conn.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),db::schema::CURRENT_SCHEMA_VERSION);
    assert_eq!(conn.query_row("SELECT title FROM inbox_items WHERE id='upgrade-source'",[],|r|r.get::<_,String>(0)).unwrap(),"升级前原文");
    assert_eq!(conn.query_row("SELECT count(*) FROM inbox_action_results",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    let original = profile.path().join("captured.txt");
    std::fs::write(&original,b"durable captured file").unwrap();
    let captured = inbox::add_inbox_item("file".into(),None,None,Some(original.to_string_lossy().into())).await.unwrap();
    let captured_path: String = conn.query_row("SELECT source_path FROM inbox_items WHERE id=?1",[&captured],|r|r.get(0)).unwrap();
    std::fs::remove_file(&original).unwrap();
    assert_eq!(std::fs::read(captured_path).unwrap(),b"durable captured file");
    conn.execute("INSERT INTO cases(id,case_name,client_name) VALUES('capture-case','合成捕获案件','合成客户')", []).unwrap();
    conn.execute("INSERT INTO knowledge_items(id,title,content,category) VALUES('link-target','链接目标','目标正文','reference')",[]).unwrap();
    for id in ["task", "event", "knowledge", "case", "project", "race", "file-a", "file-b", "holiday"] {
        conn.execute("INSERT INTO inbox_items(id,source_type,title,content_text,status) VALUES(?1,'note','原始标题','完整原始正文','pending')", [id]).unwrap();
    }
    let task = json!({"taskName":"等待提交","dueDate":"2026-09-18","dueTime":"15:37","taskType":"waiting","waitingFor":"合成客户","followUpDate":"2026-09-17","estimatedMinutes":20});
    conn.execute_batch("CREATE TRIGGER qa_fail_receipt BEFORE INSERT ON inbox_action_results BEGIN SELECT RAISE(ABORT,'receipt failure'); END;").unwrap();
    assert!(confirm("task", "create_task", task.clone()).await.unwrap_err().contains("receipt failure"));
    assert_eq!(conn.query_row("SELECT count(*) FROM tasks", [], |r|r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(conn.query_row("SELECT count(*) FROM task_events", [], |r|r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(conn.query_row("SELECT count(*) FROM inbox_feedback", [], |r|r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(conn.query_row("SELECT status FROM inbox_items WHERE id='task'", [], |r|r.get::<_,String>(0)).unwrap(), "pending");
    conn.execute_batch("DROP TRIGGER qa_fail_receipt").unwrap();
    let first = confirm("task", "create_task", task.clone()).await.unwrap();
    assert_eq!(confirm("task", "create_task", task).await.unwrap(), first);
    assert!(inbox::process_inbox_item("task".into()).await.is_err());
    conn.execute("UPDATE inbox_items SET status='pending' WHERE id='task'", []).unwrap();
    assert_eq!(confirm("task", "create_task", json!({})).await.unwrap(), first);
    assert_eq!(conn.query_row("SELECT status FROM inbox_items WHERE id='task'", [], |r|r.get::<_,String>(0)).unwrap(), "filed");
    assert_eq!(conn.query_row("SELECT count(*) FROM tasks WHERE inbox_source_id='task'", [], |r|r.get::<_,i64>(0)).unwrap(), 1);
    let task_id = first["task"]["id"].as_str().unwrap();
    let actual: (String,String,String,String,i64) = conn.query_row("SELECT inbox_source_id,due_time,description,waiting_for,estimated_minutes FROM tasks WHERE id=?1", [task_id], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
    assert_eq!(actual, ("task".into(),"15:37".into(),"完整原始正文".into(),"合成客户".into(),20));
    assert!(confirm("task", "save_knowledge", json!({})).await.is_err());
    assert_eq!(conn.query_row("SELECT linked_case_id FROM inbox_items WHERE id='task'", [], |r|r.get::<_,String>(0)).unwrap(), "capture-case");

    let (a,b) = tokio::join!(confirm("race","create_task",json!({"taskName":"并发确认"})), confirm("race","create_task",json!({"taskName":"并发确认"})));
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(conn.query_row("SELECT count(*) FROM tasks WHERE inbox_source_id='race'", [], |r|r.get::<_,i64>(0)).unwrap(), 1);

    assert!(confirm("event","create_event",json!({"title":"预约"})).await.is_err());
    assert!(confirm("event","create_event",json!({"title":"预约","eventDate":"2026-02-30"})).await.is_err());
    assert!(confirm("event","create_event",json!({"title":"预约","eventDate":"2026-09-18","startTime":"25:00"})).await.is_err());
    let event = confirm("event","create_event",json!({"title":"预约","eventDate":"2026-09-18","startTime":"15:37","endTime":"16:10","location":"会议室"})).await.unwrap();
    assert_eq!(event["event"]["startTime"], "15:37");
    assert_eq!(event["event"]["notes"], "完整原始正文");
    let events = calendar_events::list_calendar_events("2026-09-18".into(),"2026-09-18".into()).await.unwrap();
    assert_eq!(events.len(),1);
    assert!(!events[0].all_day);

    let knowledge = confirm("knowledge","save_knowledge",json!({"title":"研究","content":"# 原始 Markdown\n\n正文  \n换行 [[链接目标]]\n"})).await.unwrap();
    let saved: (String,String,String,String) = conn.query_row("SELECT content,source_id,linked_case_id,category FROM knowledge_items WHERE id=?1",[knowledge["knowledgeId"].as_str().unwrap()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(saved,("# 原始 Markdown\n\n正文  \n换行 [[链接目标]]\n".into(),"knowledge".into(),"capture-case".into(),"reference".into()));
    assert_eq!(conn.query_row("SELECT count(*) FROM links WHERE source_id=?1 AND target_id='link-target'",[knowledge["knowledgeId"].as_str().unwrap()],|r|r.get::<_,i64>(0)).unwrap(),1);
    let case = confirm("case","create_case",json!({"caseName":"捕获关联案","track":"civil_tort","clientName":"客户甲","opponentName":"相对方","caseAmount":12345,"thirdParties":[{"name":"第三人甲","role":"第三人"}]})).await.unwrap();
    assert!(case["case"]["thirdParties"].to_string().contains("第三人甲"));
    assert_eq!(case["case"]["caseAmount"],"12345");
    assert_eq!(confirm("case","create_case",json!({"caseName":"重试不新建"})).await.unwrap(),case);
    let project = confirm("project","create_project",json!({"name":"合成项目"})).await.unwrap();
    assert_eq!(confirm("project","create_project",json!({"name":"合成项目"})).await.unwrap(),project);

    conn.execute("UPDATE inbox_items SET content_text='2026年国庆节：10月1日至7日放假，10月10日上班。' WHERE id='holiday'",[]).unwrap();
    let holiday = confirm("holiday","update_holidays",json!({})).await.unwrap();
    assert_eq!(confirm("holiday","update_holidays",json!({})).await.unwrap(),holiday);

    let dir_a = profile.path().join("a"); let dir_b = profile.path().join("b");
    std::fs::create_dir_all(&dir_a).unwrap(); std::fs::create_dir_all(&dir_b).unwrap();
    for (id,dir,bytes) in [("file-a",dir_a,b"first contents".as_slice()),("file-b",dir_b,b"second contents".as_slice())] {
        let path = dir.join("evidence.txt"); std::fs::write(&path,bytes).unwrap();
        conn.execute("UPDATE inbox_items SET source_type='file',source_path=?2 WHERE id=?1",params![id,path.to_str().unwrap()]).unwrap();
    }
    inbox::file_inbox_item("file-a".into(),"capture-case".into(),"02_证据".into()).await.unwrap();
    inbox::file_inbox_item("file-a".into(),"capture-case".into(),"evidence".into()).await.unwrap();
    let first_path: String = conn.query_row("SELECT file_path FROM case_files",[],|r|r.get(0)).unwrap();
    conn.execute_batch("CREATE TRIGGER qa_fail_filed BEFORE UPDATE OF status ON inbox_items WHEN NEW.id='file-b' BEGIN SELECT RAISE(ABORT,'file status failure'); END;").unwrap();
    assert!(inbox::file_inbox_item("file-b".into(),"capture-case".into(),"evidence".into()).await.is_err());
    assert_eq!(std::fs::read(&first_path).unwrap(),b"first contents");
    assert_eq!(std::fs::read_dir(std::path::Path::new(&first_path).parent().unwrap()).unwrap().count(),1);
    conn.execute_batch("DROP TRIGGER qa_fail_filed").unwrap();
    inbox::file_inbox_item("file-b".into(),"capture-case".into(),"evidence".into()).await.unwrap();
    let paths: Vec<String> = conn.prepare("SELECT file_path FROM case_files").unwrap().query_map([],|r|r.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(paths.len(),2);
    assert_ne!(paths[0],paths[1]);
    assert_eq!(std::fs::read(&first_path).unwrap(),b"first contents");
    let second = paths.iter().find(|p|p.as_str()!=first_path).unwrap();
    assert_eq!(std::fs::read(second).unwrap(),b"second contents");
    assert_eq!(conn.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}

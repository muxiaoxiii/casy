use casy_lib::{commands::deadline_rules, db, sync};

#[tokio::test]
async fn deadline_mutations_rollback_when_audit_fails_and_sync_reports_observed_state() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    std::env::remove_var("TEST_ENV"); // Exercise real SQLCipher in this isolated profile.
    { let conn = db::open_db().unwrap(); db::init_db(&conn).unwrap(); }
    async fn save(id: Option<String>, name: &str) -> Result<String, String> {
        deadline_rules::upsert_deadline_rule(id, "民事诉讼".into(), name.into(), "测试依据".into(), "filing_date".into(), 10, "day".into(), "civil".into(), None, "recommended".into(), 1).await
    }
    let id = save(None, "原规则").await.unwrap();
    {
        let conn = db::open_db().unwrap();
        conn.execute_batch("CREATE TRIGGER fail_rule_audit BEFORE INSERT ON deadline_rule_audit BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;").unwrap();
    }
    assert!(save(Some(id.clone()), "不应保存").await.is_err());
    assert!(save(None, "不应新增").await.is_err());
    assert!(deadline_rules::toggle_deadline_rule(id.clone(), false).await.is_err());
    assert!(deadline_rules::delete_deadline_rule(id.clone()).await.is_err());
    {
        let mut conn = db::open_db().unwrap();
        let rule: (String, bool) = conn.query_row("SELECT rule_name,auto_calculate FROM deadline_rules WHERE id=?1", [&id], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(rule, ("原规则".into(),true));
        assert_eq!(conn.query_row("SELECT count(*) FROM deadline_rules WHERE rule_name='不应新增'", [], |r|r.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(conn.query_row("SELECT count(*) FROM deadline_rule_audit WHERE rule_id=?1", [&id], |r|r.get::<_,i64>(0)).unwrap(),1);
        conn.execute_batch("DROP TRIGGER fail_rule_audit").unwrap();
        assert!(!sync::get_sync_status(&conn).unwrap().configured);
        db::set_setting(&conn,"webdavUrl","https://example.invalid/dav").unwrap();
        db::set_setting(&conn,"webdavUsername","test-user").unwrap();
        assert_eq!(sync::get_sync_status(&conn).unwrap().connection_state,"unknown");
        sync::record_webdav_status(&mut conn,"https://example.invalid/dav","test-user",None,true,Some("test-etag")).unwrap();
        let status = sync::get_sync_status(&conn).unwrap();
        assert!(status.webdav_connected);assert!(status.last_sync_at.is_some());assert!(status.pending_changes.is_none());
        sync::record_webdav_status(&mut conn,"https://example.invalid/dav","test-user",Some("offline"),false,None).unwrap();
        let status = sync::get_sync_status(&conn).unwrap();assert!(!status.webdav_connected);assert_eq!(status.last_error.as_deref(),Some("offline"));
        db::set_setting(&conn,"webdavUrl","https://different.invalid/dav").unwrap();
        let status = sync::get_sync_status(&conn).unwrap();assert_eq!(status.connection_state,"unknown");assert!(status.last_sync_at.is_none());assert!(status.remote_etag.is_none());
        sync::record_webdav_status(&mut conn,"https://different.invalid/dav","test-user",None,false,None).unwrap();
        let status = sync::get_sync_status(&conn).unwrap();assert!(status.webdav_connected);assert!(status.last_sync_at.is_none());assert!(status.remote_etag.is_none());
    }
    save(Some(id.clone()), "已更新").await.unwrap();
    deadline_rules::delete_deadline_rule(id).await.unwrap();
    // Rejected remote bytes must never overwrite the local key or database.
    let original_key = db::get_or_create_encryption_key().unwrap();
    let (url, server) = serve_database(b"not an encrypted database".to_vec());
    let error = sync::manual_sync_pull(&url, "test", "test", &db::get_db_path()).await.unwrap_err();
    assert!(error.to_string().contains("密钥不匹配"));
    server.join().unwrap();
    assert_eq!(db::get_or_create_encryption_key().unwrap(), original_key);
    assert!(db::open_db().unwrap().query_row("SELECT count(*) FROM settings", [], |r| r.get::<_, i64>(0)).unwrap() > 0);
    // A valid same-key snapshot restores through the maintenance/rollback path.
    let backup = casy_lib::commands::backup::create_backup().await.unwrap();
    let bytes = std::fs::read(casy_lib::commands::backup::backups_dir().join(backup.filename)).unwrap();
    { let conn = db::open_db().unwrap(); db::set_setting(&conn, "after_snapshot", "local").unwrap(); }
    let (url, server) = serve_database(bytes);
    let restored = sync::manual_sync_pull(&url, "test", "test", &db::get_db_path()).await.unwrap();
    assert!(restored.success);
    server.join().unwrap();
    assert!(db::get_setting(&db::open_db().unwrap(), "after_snapshot").unwrap().is_none());
    assert_eq!(db::get_or_create_encryption_key().unwrap(), original_key);
    // Queued cancellation survives a subsequent start request; publication is not cancellable.
    let jobs = casy_lib::commands::processing::register_conversion_batch(vec!["/tmp/cancelled.md".into()]).await.unwrap();
    let job = jobs[0].clone();
    casy_lib::commands::processing::cancel_conversion(job.clone()).await.unwrap();
    assert!(casy_lib::commands::conversion::convert_file_to_markdown("/tmp/cancelled.md".into(), "/tmp".into(), Some(job), None).await.unwrap_err().contains("已取消"));
    db::reset_shared_conn();
}

fn serve_database(bytes: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") { stream.read_exact(&mut byte).unwrap(); request.push(byte[0]); }
        assert!(request.starts_with(b"GET /casy.db "));
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: test-version\r\nConnection: close\r\n\r\n", bytes.len()).unwrap();
        stream.write_all(&bytes).unwrap();
    });
    (url, handle)
}

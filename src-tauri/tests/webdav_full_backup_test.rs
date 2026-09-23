use casy_lib::{commands::sync::{webdav_backup_full, webdav_restore_full}, db};
use std::{collections::HashMap, io::{Read, Write}, sync::{Arc, Mutex}};

#[tokio::test]
async fn complete_webdav_backup_restores_cases_tasks_settings_and_attachments() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    std::env::remove_var("TEST_ENV");
    let documents = profile.path().join("documents");
    std::fs::create_dir_all(&documents).unwrap();
    let attachment = documents.join("证据.txt");
    std::fs::write(&attachment, "原始证据 · WebDAV 完整恢复").unwrap();
    {
        let conn = db::open_db().unwrap(); db::init_db(&conn).unwrap();
        conn.execute("INSERT INTO cases(id,case_name,client_name,folder_path) VALUES('qa-case','恢复案件','测试客户',?1)", [documents.to_str().unwrap()]).unwrap();
        conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('qa-file','qa-case','证据.txt',?1,'evidence')", [attachment.to_str().unwrap()]).unwrap();
        conn.execute("INSERT INTO tasks(id,task_name,case_id,created_date) VALUES('qa-task','备份时的任务','qa-case','2026-09-23')", []).unwrap();
        db::set_setting(&conn, "theme", "rice-paper").unwrap();
    }
    let original_key = db::get_or_create_encryption_key().unwrap();
    let files = Arc::new(Mutex::new(HashMap::<String, Vec<u8>>::new()));
    let server_files = files.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(60))).unwrap();
            let mut header = vec![]; let mut byte = [0];
            while !header.ends_with(b"\r\n\r\n") { stream.read_exact(&mut byte).unwrap(); header.push(byte[0]); }
            let header = String::from_utf8(header).unwrap();
            let mut first = header.lines().next().unwrap().split_whitespace();
            let method = first.next().unwrap(); let path = first.next().unwrap();
            let field = |key: &str| header.lines().find_map(|line| { let (name, value) = line.split_once(':')?; name.eq_ignore_ascii_case(key).then(|| value.trim().to_owned()) });
            let mut body = vec![0; field("content-length").and_then(|v|v.parse().ok()).unwrap_or(0)];
            stream.read_exact(&mut body).unwrap();
            let mut files = server_files.lock().unwrap();
            let output = match method {
                "PUT" => { assert!(path.ends_with(".upload")); files.insert(path.into(), body); vec![] },
                "MOVE" => { let dest = reqwest::Url::parse(&field("destination").unwrap()).unwrap(); let body = files.remove(path).unwrap(); files.insert(dest.path().into(), body); vec![] },
                "GET" => files.get(path).unwrap().clone(),
                _ => panic!("unexpected {method}"),
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", output.len()).unwrap();
            stream.write_all(&output).unwrap();
        }
    });
    let passphrase = "qa-backup-passphrase-123";
    webdav_backup_full(url.clone(), "qa".into(), "qa".into(), passphrase.into()).await.unwrap();
    {
        let files = files.lock().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files["/casy-full-backup.casy"].starts_with(b"age-encryption.org/v1"));
    }
    {
        let conn = db::open_db().unwrap();
        conn.execute("UPDATE tasks SET task_name='备份之后的修改' WHERE id='qa-task'", []).unwrap();
        db::set_setting(&conn, "theme", "dark").unwrap();
    }
    std::fs::write(&attachment, "备份后的原件修改，恢复不能删除它").unwrap();
    assert!(webdav_restore_full(url.clone(), "qa".into(), "qa".into(), "wrong-password".into()).await.is_err());
    assert_eq!(db::get_setting(&db::open_db().unwrap(), "theme").unwrap().as_deref(), Some("dark"));
    assert_eq!(db::get_or_create_encryption_key().unwrap(), original_key);
    webdav_restore_full(url, "qa".into(), "qa".into(), passphrase.into()).await.unwrap();
    server.join().unwrap();
    {
        let conn = db::open_db().unwrap();
        assert_eq!(db::get_setting(&conn, "theme").unwrap().as_deref(), Some("rice-paper"));
        assert_eq!(conn.query_row("SELECT task_name FROM tasks WHERE id='qa-task'", [], |r|r.get::<_,String>(0)).unwrap(), "备份时的任务");
        let path: String = conn.query_row("SELECT file_path FROM case_files WHERE id='qa-file'", [], |r|r.get(0)).unwrap();
        assert!(std::path::Path::new(&path).starts_with(profile.path().join("restored")));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "原始证据 · WebDAV 完整恢复");
    }
    assert_eq!(std::fs::read_to_string(attachment).unwrap(), "备份后的原件修改，恢复不能删除它");
    assert_eq!(db::get_or_create_encryption_key().unwrap(), original_key);
    assert!(std::fs::read_dir(profile.path().join("backups")).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("pre-restore-")));
    db::reset_shared_conn();
}

use casy_lib::db;
#[test]
fn encrypted_wal_database_is_snapshotted_before_upgrade() {
    let root=tempfile::tempdir().unwrap();let path=root.path().join("old.db");
    let key="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let conn=rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(&format!("PRAGMA key=\"x'{key}'\"; PRAGMA journal_mode=WAL;")).unwrap();
    conn.execute_batch(db::schema::SCHEMA_SQL).unwrap();
    conn.execute_batch("PRAGMA user_version=1; INSERT INTO cases(id,case_name,client_name) VALUES('c','Original','Client');").unwrap();
    db::init_db(&conn).unwrap();
    let files:Vec<_>=std::fs::read_dir(root.path().join("backups")).unwrap().map(|e|e.unwrap().path()).collect();
    assert_eq!(files.len(),1);
    let restored=rusqlite::Connection::open(&files[0]).unwrap();
    restored.execute_batch(&format!("PRAGMA key=\"x'{key}'\";")).unwrap();
    assert_eq!(restored.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert_eq!(restored.query_row("SELECT case_name FROM cases WHERE id='c'",[],|r|r.get::<_,String>(0)).unwrap(),"Original");
    assert_ne!(&std::fs::read(&files[0]).unwrap()[..16], b"SQLite format 3\0");
    db::init_db(&conn).unwrap();
    assert_eq!(std::fs::read_dir(root.path().join("backups")).unwrap().count(),1);
}

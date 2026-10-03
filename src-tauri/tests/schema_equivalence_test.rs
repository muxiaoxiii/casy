use casy_lib::db;
use rusqlite::Connection;
fn structure(conn: &Connection) -> Vec<(String,String,String)> {
    let mut stmt=conn.prepare("SELECT type,name,COALESCE(sql,'') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name").unwrap();
    stmt.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap()
}
#[test]
fn fresh_and_incrementally_migrated_databases_have_equal_structure() {
    let fresh=Connection::open_in_memory().unwrap();db::init_db(&fresh).unwrap();
    let expected=structure(&fresh);
    for cutoff in [1,20,30,40] {
        let old=Connection::open_in_memory().unwrap();
        old.execute_batch(db::schema::SCHEMA_SQL).unwrap();
        old.execute_batch("PRAGMA user_version=1;").unwrap();
        for (version,sql) in db::schema::MIGRATIONS {
            let version=version.parse::<i64>().unwrap();
            if version>1 && version<=cutoff {
                old.execute_batch(sql).unwrap();
                old.pragma_update(None,"user_version",version).unwrap();
            }
        }
        db::init_db(&old).unwrap();
        assert_eq!(structure(&old),expected,"upgrade from {cutoff} differs from fresh install");
        db::init_db(&old).unwrap();
        assert_eq!(structure(&old),expected,"second initialization differs");
    }
}

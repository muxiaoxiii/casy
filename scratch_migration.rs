use rusqlite::Connection;

fn main() {
    let conn = Connection::open_in_memory().unwrap();
    // Simulate v18
    conn.execute_batch("
        CREATE TABLE cases (
            id TEXT PRIMARY KEY,
            track TEXT CHECK(track IN ('civil', 'other'))
        );
        CREATE TABLE proceedings (
            id TEXT PRIMARY KEY,
            case_id TEXT REFERENCES cases(id) ON DELETE CASCADE
        );
        INSERT INTO cases (id, track) VALUES ('c1', 'civil');
        INSERT INTO proceedings (id, case_id) VALUES ('p1', 'c1');
    ").unwrap();

    // Now try to remove CHECK constraint by recreating table
    conn.execute_batch("
        PRAGMA foreign_keys=OFF;
        BEGIN TRANSACTION;
        CREATE TABLE cases_new (
            id TEXT PRIMARY KEY,
            track TEXT
        );
        INSERT INTO cases_new SELECT * FROM cases;
        DROP TABLE cases;
        ALTER TABLE cases_new RENAME TO cases;
        COMMIT;
        PRAGMA foreign_keys=ON;
    ").unwrap();

    // Test inserting an invalid track that used to be blocked
    conn.execute("INSERT INTO cases (id, track) VALUES ('c2', 'criminal')", []).unwrap();
    println!("Migration successful!");
}

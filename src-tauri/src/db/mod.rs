pub mod cases;
pub mod schema;
pub mod search;

use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "com.casy.db";
const KEYRING_ACCOUNT: &str = "encryption-key";

/// 打开数据库连接（自动加密，兼容旧版明文 DB 迁移）
pub fn open_db() -> Result<Connection> {
    open_db_encrypted()
}

/// 获取或生成数据库加密密钥（存储在 OS Keychain）
fn get_or_create_encryption_key() -> Result<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| anyhow::anyhow!("keyring entry: {:?}", e))?;
    match entry.get_password() {
        Ok(key) => Ok(key),
        Err(_) => {
            let mut buf = [0u8; 32];
            let _ = getrandom::getrandom(&mut buf);
            let key = hex::encode(buf);
            entry.set_password(&key)
                .map_err(|e| anyhow::anyhow!("keyring save: {:?}", e))?;
            log::info!("Generated new database encryption key and stored in keychain");
            Ok(key)
        }
    }
}

/// 打开加密数据库连接
pub fn open_db_encrypted() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let key = get_or_create_encryption_key()?;

    // 如果数据库文件不存在，直接创建加密数据库
    if !path.exists() {
        let conn = Connection::open(&path)?;
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")?;
        return Ok(conn);
    }

    // 尝试以加密方式打开
    let conn = Connection::open(&path)?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key))?;
    conn.execute_batch("PRAGMA busy_timeout=5000;")?;

    // 验证密钥是否正确：尝试读取 sqlite_master
    let test: Result<i64, _> = conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0));
    match test {
        Ok(_) => {
            // 密钥正确，数据库已加密
            conn.execute_batch("PRAGMA journal_mode=WAL;")?;
            conn.execute_batch("PRAGMA foreign_keys=ON;")?;
            Ok(conn)
        }
        Err(_) => {
            // 密钥不匹配 → 数据库是明文，需要迁移
            log::info!("Database is plaintext, migrating to encrypted...");
            drop(conn);
            migrate_to_encrypted(&key)?;
            // 重新打开加密后的数据库
            let conn = Connection::open(&path)?;
            conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key))?;
            conn.execute_batch("PRAGMA journal_mode=WAL;")?;
            conn.execute_batch("PRAGMA foreign_keys=ON;")?;
            conn.execute_batch("PRAGMA busy_timeout=5000;")?;
            Ok(conn)
        }
    }
}

/// 将现有明文数据库迁移为加密数据库
fn migrate_to_encrypted(key: &str) -> Result<()> {
    let path = db_path();
    let backup_path = path.with_extension("db.bak");
    let temp_encrypted = path.with_extension("db.encrypted");

    // 备份原数据库
    std::fs::copy(&path, &backup_path)?;
    log::info!("Backed up plaintext database to {:?}", backup_path);

    // 打开明文数据库，附加加密目标，导出数据
    let plain_conn = Connection::open(&path)?;
    plain_conn.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS encrypted KEY \"x'{}'\";",
        temp_encrypted.to_string_lossy().replace('\'', "''"),
        key
    ))?;
    plain_conn.execute_batch("SELECT sqlcipher_export('encrypted');")?;
    plain_conn.execute_batch("DETACH DATABASE encrypted;")?;
    drop(plain_conn);

    // 替换原文件
    std::fs::rename(&temp_encrypted, &path)?;

    log::info!("Database migration to encrypted format completed");
    Ok(())
}

/// 数据库文件路径
fn db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Casy")
        .join("casy.db")
}

/// 获取数据库文件路径（公开接口）
pub fn get_db_path() -> PathBuf {
    db_path()
}

/// 从 settings 表获取设置值
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query_map([key], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(Ok(val)) => Ok(Some(val)),
        _ => Ok(None),
    }
}

/// 保存设置到 settings 表
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

/// 初始化数据库（建表 + 种子数据）
pub fn init_db(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version == 0 {
        conn.execute_batch(schema::SCHEMA_SQL)?;
        schema::seed_deadline_rules(conn)?;
        conn.execute_batch("PRAGMA user_version = 1;")?;
        log::info!("Database initialized (v1)");
    }
    Ok(())
}

/// 生成 UUID v4
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 当前时间（本地时区）
pub fn now_local() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// 今天日期
pub fn today() -> String {
    chrono::Local::now()
        .naive_local()
        .date()
        .format("%Y-%m-%d")
        .to_string()
}

/// 某月最后一天
#[allow(dead_code)]
pub fn days_in_month(year: i32, month: u32) -> u32 {
    if month == 12 {
        31
    } else {
        let next = chrono::NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
        let curr = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        (next - curr).num_days() as u32
    }
}

/// 通用行到结构体转换辅助
pub fn row_get_string(row: &rusqlite::Row, col: &str) -> rusqlite::Result<Option<String>> {
    row.get::<_, Option<String>>(col)
}

pub fn row_get_string_or(row: &rusqlite::Row, col: &str) -> rusqlite::Result<String> {
    row.get::<_, Option<String>>(col).map(|v| v.unwrap_or_default())
}

#[allow(dead_code)]
pub fn row_get_i32(row: &rusqlite::Row, col: &str) -> rusqlite::Result<i32> {
    row.get::<_, Option<i32>>(col).map(|v| v.unwrap_or(0))
}

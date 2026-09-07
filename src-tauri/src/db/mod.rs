pub mod cases;
pub mod intake;
pub mod knowledge_index;
pub mod schema;
pub mod search;

use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

const KEYRING_SERVICE: &str = "com.casy.db";
const KEYRING_ACCOUNT: &str = "encryption-key";
/// 密钥文件（与应用数据同目录，权限 0600）
const KEY_FILE_NAME: &str = "casy.db.key";

/// 全局加密密钥（进程内只读取/生成一次）
static ENCRYPTION_KEY: OnceLock<String> = OnceLock::new();

/// 数据库维护模式标识（排他维护锁，用于备份恢复/重建期间阻止并发读写）
static MAINTENANCE_MODE: AtomicBool = AtomicBool::new(false);

/// 共享连接（B1 连接池底座）：SQLCipher 每次开连接都要重做 PBKDF2 密钥推导，
/// 逐命令开关的开销随命令数线性放大；共享单连接 + 互斥串行化是当前规模下
/// 最小侵入的池化形态。存量 open_db() 调用点不受影响，按域逐步迁移。
static SHARED_CONN: std::sync::Mutex<Option<Connection>> = std::sync::Mutex::new(None);
static ACTIVE_CONNECTIONS: std::sync::Mutex<usize> = std::sync::Mutex::new(0);
static CONNECTIONS_CLOSED: std::sync::Condvar = std::sync::Condvar::new();

#[derive(Debug)]
pub struct ManagedConnection {
    connection: Option<Connection>,
}

impl std::ops::Deref for ManagedConnection {
    type Target = Connection;
    fn deref(&self) -> &Connection { self.connection.as_ref().unwrap() }
}

impl std::ops::DerefMut for ManagedConnection {
    fn deref_mut(&mut self) -> &mut Connection { self.connection.as_mut().unwrap() }
}

impl Drop for ManagedConnection {
    fn drop(&mut self) {
        // Close SQLite before allowing a restore to replace its files.
        drop(self.connection.take());
        let mut count = ACTIVE_CONNECTIONS.lock().unwrap_or_else(|e| e.into_inner());
        *count -= 1;
        CONNECTIONS_CLOSED.notify_all();
    }
}

/// 检查系统当前是否处于独占维护模式
pub fn is_maintenance_mode() -> bool {
    MAINTENANCE_MODE.load(Ordering::SeqCst)
}

/// 重置/清空共享连接（关闭底层句柄以释放磁盘文件锁定）
pub fn reset_shared_conn() {
    if let Ok(mut guard) = SHARED_CONN.lock() {
        *guard = None;
    }
}

/// 维护模式 RAII 守卫：退出作用域时自动解除维护模式
pub struct MaintenanceGuard;

impl Drop for MaintenanceGuard {
    fn drop(&mut self) {
        MAINTENANCE_MODE.store(false, Ordering::SeqCst);
        log::info!("Exited database maintenance mode");
    }
}

/// 申请独占维护模式（用于备份恢复或库文件重置，阻止常规 IPC 操作）
pub fn enter_maintenance() -> Result<MaintenanceGuard> {
    if MAINTENANCE_MODE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        anyhow::bail!("系统已处于维护模式中，请勿重复执行维护操作");
    }

    let guard = MaintenanceGuard;
    // 释放共享连接池持有的文件句柄
    reset_shared_conn();
    let count = ACTIVE_CONNECTIONS.lock().map_err(|_| anyhow::anyhow!("数据库连接计数不可用"))?;
    let (count, _) = CONNECTIONS_CLOSED
        .wait_timeout_while(count, std::time::Duration::from_secs(30), |count| *count > 0)
        .map_err(|_| anyhow::anyhow!("等待数据库连接关闭失败"))?;
    if *count > 0 {
        MAINTENANCE_MODE.store(false, Ordering::SeqCst);
        anyhow::bail!("仍有操作进行中，请稍后重试备份或恢复");
    }
    log::info!("Entered exclusive database maintenance mode");
    Ok(guard)
}

/// 打开数据库连接（自动加密，兼容旧版明文 DB 迁移，受维护模式守卫拦截）
pub fn open_db() -> Result<ManagedConnection> {
    let mut count = ACTIVE_CONNECTIONS.lock().map_err(|_| anyhow::anyhow!("数据库连接计数不可用"))?;
    if is_maintenance_mode() {
        anyhow::bail!("数据库处于维护模式（正在进行备份恢复），暂不可用");
    }
    *count += 1;
    drop(count);
    let mut managed = ManagedConnection { connection: None };
    managed.connection = Some(open_db_encrypted()?);
    Ok(managed)
}

/// 在共享连接上执行一个数据库操作单元（B1 底座，新代码优先使用）
///
/// - 连接懒初始化且进程内复用（密钥推导只发生一次）
/// - `Mutex` 串行化：同一时刻仅一个命令持有连接，天然规避写冲突
/// - 错误类型与既有 anyhow 链路一致
pub fn with_conn<T>(f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    if is_maintenance_mode() {
        anyhow::bail!("数据库处于维护模式（正在进行备份恢复），暂不可用");
    }
    let mut guard = SHARED_CONN
        .lock()
        .map_err(|e| anyhow::anyhow!("数据库连接锁中毒: {e}"))?;
    if is_maintenance_mode() {
        anyhow::bail!("数据库处于维护模式（正在进行备份恢复），暂不可用");
    }
    if guard.is_none() {
        let conn = open_db_encrypted()?;
        *guard = Some(conn);
    }
    let conn_ref = guard.as_ref().unwrap();
    f(conn_ref)
}

/// 获取或生成数据库加密密钥
///
/// 优先尝试 OS Keychain（正式发布环境），失败则回退到本地密钥文件。
/// 本地密钥文件与数据库同目录（`~/Library/Application Support/Casy/casy.db.key`），权限 0600。
pub fn get_or_create_encryption_key() -> Result<String> {
    if let Some(key) = ENCRYPTION_KEY.get() {
        return Ok(key.clone());
    }

    // 隔离 profile 不读取或写入正式 Keychain，密钥只保存在测试目录。
    if crate::runtime_paths::isolated_data_root().is_some() {
        let key = if let Some(key) = read_key_file()? {
            key
        } else {
            let mut buffer = [0u8; 32];
            getrandom::getrandom(&mut buffer)
                .map_err(|error| anyhow::anyhow!("生成测试数据库密钥失败: {error}"))?;
            let key = hex::encode(buffer);
            write_key_file(&key)?;
            key
        };
        let _ = ENCRYPTION_KEY.set(key.clone());
        return Ok(key);
    }

    // 1. 尝试 keychain（发布环境优先）
    if let Ok(key) = keychain_get() {
        let _ = ENCRYPTION_KEY.set(key.clone());
        return Ok(key);
    }

    // 2. 回退：本地密钥文件
    let key = if let Some(k) = read_key_file()? {
        k
    } else {
        let mut buf = [0u8; 32];
        getrandom::getrandom(&mut buf)
            .map_err(|error| anyhow::anyhow!("生成数据库密钥失败: {error}"))?;
        let key = hex::encode(buf);
        write_key_file(&key)?;
        log::info!("Generated new database encryption key (file storage)");
        key
    };

    // 3. 尝试把新密钥同步到 keychain（尽力而为，失败不阻塞）
    if let Err(e) = keychain_set(&key) {
        log::warn!("Keychain save failed, using file storage: {}", e);
    }

    let _ = ENCRYPTION_KEY.set(key.clone());
    Ok(key)
}

/// 从 keychain 读取密钥
fn keychain_get() -> Result<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| anyhow::anyhow!("keyring entry: {:?}", e))?;
    entry
        .get_password()
        .map_err(|e| anyhow::anyhow!("keychain get: {:?}", e))
}

/// 写入 keychain
fn keychain_set(key: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| anyhow::anyhow!("keyring entry: {:?}", e))?;
    entry
        .set_password(key)
        .map_err(|e| anyhow::anyhow!("keychain set: {:?}", e))
}

/// 密钥文件路径
fn key_file_path() -> PathBuf {
    crate::runtime_paths::data_root().join(KEY_FILE_NAME)
}

/// 读取本地密钥文件
fn read_key_file() -> Result<Option<String>> {
    let path = key_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let key = std::fs::read_to_string(&path)?.trim().to_string();
    if key.is_empty() {
        return Ok(None);
    }
    Ok(Some(key))
}

/// 写入本地密钥文件（Unix 权限 0600；Windows 无该 API，依赖目录 ACL）
#[cfg(unix)]
fn write_key_file_impl(path: &std::path::Path, key: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, key)?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    Ok(())
}

#[cfg(not(unix))]
fn write_key_file_impl(path: &std::path::Path, key: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, key)?;
    Ok(())
}

fn write_key_file(key: &str) -> Result<()> {
    let path = key_file_path();
    write_key_file_impl(&path, key)
}

/// 打开加密数据库连接
pub(crate) fn open_db_encrypted() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if IS_TEST_MODE.load(Ordering::SeqCst) || std::env::var("TEST_ENV").is_ok() {
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        return Ok(conn);
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
    let test: Result<i64, _> =
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0));
    match test {
        Ok(_) => {
            // 密钥正确（或数据库是明文但可读），直接使用
            conn.execute_batch("PRAGMA journal_mode=WAL;")?;
            conn.execute_batch("PRAGMA foreign_keys=ON;")?;
            Ok(conn)
        }
        Err(_) => {
            // 密钥不匹配 → 尝试不带密钥打开，确认是明文 DB
            drop(conn);
            let test_conn = Connection::open(&path)?;
            let is_plaintext = test_conn
                .query_row("SELECT count(*) FROM sqlite_master", [], |r| {
                    r.get::<_, i64>(0)
                })
                .is_ok();
            drop(test_conn);

            if is_plaintext {
                log::info!("Database is plaintext, migrating to encrypted...");
                migrate_to_encrypted(&key)?;
                let conn = Connection::open(&path)?;
                conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key))?;
                conn.execute_batch("PRAGMA journal_mode=WAL;")?;
                conn.execute_batch("PRAGMA foreign_keys=ON;")?;
                conn.execute_batch("PRAGMA busy_timeout=5000;")?;
                Ok(conn)
            } else {
                // 数据库已加密但密钥不对——可能是 keychain 里的密钥过期
                anyhow::bail!(
                    "数据库已加密但密钥不匹配，请检查 keychain 或删除数据库文件: {:?}",
                    path
                );
            }
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

static IS_TEST_MODE: AtomicBool = AtomicBool::new(false);
static TEST_DB_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// 启用测试模式（仅内存/独立临时文件，不走 Keychain）
pub fn enable_test_mode() {
    IS_TEST_MODE.store(true, Ordering::SeqCst);
}

/// 数据库文件路径
fn db_path() -> PathBuf {
    if IS_TEST_MODE.load(Ordering::SeqCst) || std::env::var("TEST_ENV").is_ok() {
        return TEST_DB_PATH
            .get_or_init(|| {
                std::env::temp_dir().join(format!("casy_test_{}.db", uuid::Uuid::new_v4()))
            })
            .clone();
    }
    crate::runtime_paths::data_root().join("casy.db")
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

/// 初始化数据库（建表 + 种子数据 + 迁移）
pub fn init_db(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version == 0 {
        // 全新数据库：建表 + 种子
        conn.execute_batch(schema::SCHEMA_SQL)?;
        schema::seed_deadline_rules(conn)?;
        conn.execute_batch("PRAGMA user_version = 1;")?;
        log::info!("Database initialized (v1)");
    }
    // 对已有数据库运行增量迁移
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current < schema::CURRENT_SCHEMA_VERSION {
        schema::run_migrations(conn, current)?;
        log::info!(
            "Database migrated from v{} to v{}",
            current,
            schema::CURRENT_SCHEMA_VERSION
        );
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
    row.get::<_, Option<String>>(col)
        .map(|v| v.unwrap_or_default())
}

#[allow(dead_code)]
pub fn row_get_i32(row: &rusqlite::Row, col: &str) -> rusqlite::Result<i32> {
    row.get::<_, Option<i32>>(col).map(|v| v.unwrap_or(0))
}

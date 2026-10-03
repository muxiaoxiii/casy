use casy_lib::commands::backup::{
    calculate_file_sha256, validate_backup_filename, verify_db_file_integrity,
};
use casy_lib::db::{enter_maintenance, open_db};
use rusqlite::Connection;

#[test]
fn test_path_traversal_rejection() {
    // 恶意注入与越界路径测试
    let malicious = [
        "../../etc/passwd",
        "../casy.db",
        "casy-backup-20260829/../../../root.db",
        "casy-backup-20260829-120000.db/extra",
        "casy-backup-20260829\\win.db",
        "casy-backup-20260829[0].db",
        "casy-backup-20260829\0inject.db",
        "random-file.txt",
        "casy-backup-20260829.db.bak",
        "",
        "casy-backup-.db",
    ];

    for name in &malicious {
        assert!(
            validate_backup_filename(name).is_err(),
            "Path traversal or invalid filename should be rejected: {}",
            name
        );
    }

    assert!(validate_backup_filename("pre-migration-v20-abc123.db").is_ok());
    assert!(validate_backup_filename("pre-migration-../../outside.db").is_err());

    // 合法文件名
    assert!(validate_backup_filename("casy-backup-20260829-183000.db").is_ok());
    assert!(validate_backup_filename("pre-restore-20260829-183000.db").is_ok());
}

#[test]
fn test_maintenance_mode_lifecycle_and_blocking() {
    // 1. 正常状态下可打开数据库
    // 2. 申请独占维护锁
    let guard = enter_maintenance().expect("Should enter maintenance mode");

    // 3. 维护期间任何打开连接或操作请求必须被硬拦截
    let res = open_db();
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("维护模式"));

    // 4. 重复申请维护锁必须被拒绝（防并发重入冲突）
    let re_enter = enter_maintenance();
    assert!(re_enter.is_err());

    // 5. 释放维护守卫
    drop(guard);

    // 6. 维护模式自动解除
    assert!(!casy_lib::db::is_maintenance_mode());
}

#[test]
fn test_encrypted_backup_integrity_and_sha256() {
    let temp_dir = std::env::temp_dir().join(format!("casy_test_backup_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let db_file = temp_dir.join("live.db");
    let backup_file = temp_dir.join("casy-backup-test.db");
    let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    // 1. 创建加密源数据库并写入数据
    {
        let conn = Connection::open(&db_file).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key))
            .unwrap();
        conn.execute_batch(
            "CREATE TABLE cases (id TEXT PRIMARY KEY, title TEXT);
             INSERT INTO cases (id, title) VALUES ('c1', '重大民商事二审');",
        )
        .unwrap();

        // 执行 VACUUM INTO 生成备份
        conn.execute(
            "VACUUM INTO ?1",
            rusqlite::params![backup_file.to_string_lossy().as_ref()],
        )
        .unwrap();
    }

    // 2. 校验备份文件的完整性（使用正确密钥）
    assert!(verify_db_file_integrity(&backup_file, key).is_ok());

    // 3. 使用错误密钥校验（应被拒绝）
    let wrong_key = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    assert!(verify_db_file_integrity(&backup_file, wrong_key).is_err());

    // 4. 计算 SHA-256 校验和
    let sha = calculate_file_sha256(&backup_file).unwrap();
    assert_eq!(sha.len(), 64);

    // 5. 制造文件损坏（写入垃圾字节）
    let corrupted_file = temp_dir.join("corrupted.db");
    std::fs::write(&corrupted_file, b"corrupted sqlite header garbage data").unwrap();

    // 验证损坏文件被快速完整性校验拦截
    assert!(verify_db_file_integrity(&corrupted_file, key).is_err());

    // 清理测试目录
    let _ = std::fs::remove_dir_all(&temp_dir);
}

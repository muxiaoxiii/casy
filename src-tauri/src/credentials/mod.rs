//! 凭据安全存储模块
//!
//! 将敏感凭据（IMAP 密码、API Key 等）从 base64 迁移到 OS Keychain。
//! 使用 keyring crate 访问系统钥匙链（macOS Keychain / Windows Credential Manager / Linux Secret Service）。

use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

/// 凭据类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CredentialType {
    /// IMAP 邮箱密码
    ImapPassword,
    /// AI API Key
    AiApiKey,
    /// WebDAV 密码
    WebDavPassword,
    /// 飞书 App Token
    FeishuToken,
    /// SMTP 发信密码
    SmtpPassword,
    /// CalDAV 日历密码
    CaldavPassword,
}

impl CredentialType {
    pub fn service_name(&self) -> &str {
        match self {
            Self::ImapPassword => "casy-imap",
            Self::AiApiKey => "casy-ai",
            Self::WebDavPassword => "casy-webdav",
            Self::FeishuToken => "casy-feishu",
            Self::SmtpPassword => "casy-smtp",
            Self::CaldavPassword => "casy-caldav",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::ImapPassword => "IMAP 邮箱密码",
            Self::AiApiKey => "AI API Key",
            Self::WebDavPassword => "WebDAV 密码",
            Self::FeishuToken => "飞书 App Token",
            Self::SmtpPassword => "SMTP 发信密码",
            Self::CaldavPassword => "CalDAV 日历密码",
        }
    }
}

/// 存储凭据到 Keychain
pub fn store_credential(cred_type: CredentialType, account: &str, secret: &str) -> Result<()> {
    let entry = Entry::new(cred_type.service_name(), account)
        .with_context(|| format!("创建 keychain 条目失败: {}/{}", cred_type.service_name(), account))?;

    entry.set_password(secret)
        .with_context(|| format!("存储凭据到 keychain 失败: {}", cred_type.label()))?;

    log::info!("凭据已存储到 keychain: {} ({})", cred_type.label(), account);
    Ok(())
}

/// 从 Keychain 读取凭据
pub fn get_credential(cred_type: CredentialType, account: &str) -> Result<Option<String>> {
    let entry = Entry::new(cred_type.service_name(), account)
        .with_context(|| format!("创建 keychain 条目失败: {}/{}", cred_type.service_name(), account))?;

    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("读取 keychain 凭据失败: {}", e)),
    }
}

/// 删除 Keychain 中的凭据
pub fn delete_credential(cred_type: CredentialType, account: &str) -> Result<()> {
    let entry = Entry::new(cred_type.service_name(), account)
        .with_context(|| format!("创建 keychain 条目失败: {}/{}", cred_type.service_name(), account))?;

    match entry.delete_credential() {
        Ok(()) => {
            log::info!("凭据已从 keychain 删除: {} ({})", cred_type.label(), account);
            Ok(())
        }
        Err(keyring::Error::NoEntry) => Ok(()), // 不存在也视为成功
        Err(e) => Err(anyhow::anyhow!("删除 keychain 凭据失败: {}", e)),
    }
}

/// 检查凭据是否已存储在 Keychain 中
pub fn has_credential(cred_type: CredentialType, account: &str) -> bool {
    get_credential(cred_type, account).unwrap_or(None).is_some()
}

// ═══════════════════════════════════════════════════════════
// 迁移：base64 → Keychain
// ═══════════════════════════════════════════════════════════

/// 迁移结果
#[derive(Debug, Serialize, Deserialize)]
pub struct MigrationResult {
    pub total: usize,
    pub migrated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// 将 imap_accounts 表中的 base64 密码迁移到 Keychain
pub fn migrate_imap_passwords_to_keychain() -> Result<MigrationResult> {
    use base64::Engine;
    use rusqlite::params;

    let conn = crate::db::open_db()?;
    let mut result = MigrationResult {
        total: 0,
        migrated: 0,
        skipped: 0,
        failed: 0,
        errors: Vec::new(),
    };

    // 读取所有 IMAP 账号
    let mut stmt = conn.prepare(
        "SELECT id, email_address, password_enc FROM imap_accounts WHERE password_enc IS NOT NULL AND password_enc != ''"
    )?;

    let accounts: Vec<(String, String, String)> = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?.collect::<std::result::Result<Vec<_>, _>>()?;

    result.total = accounts.len();

    for (id, email, password_enc) in &accounts {
        // 检查是否已在 keychain 中
        if has_credential(CredentialType::ImapPassword, email) {
            result.skipped += 1;
            // 已在 keychain：同样清掉数据库里的 base64 明文
            if password_enc != "keychain" {
                let _ = conn.execute(
                    "UPDATE imap_accounts SET password_enc = 'keychain' WHERE id = ?1",
                    params![id],
                );
            }
            continue;
        }

        // 解码 base64
        let password = match base64::engine::general_purpose::STANDARD.decode(password_enc) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!("账号 {} base64 解码失败: {}", email, e));
                continue;
            }
        };

        // 存储到 keychain
        match store_credential(CredentialType::ImapPassword, email, &password) {
            Ok(()) => {
                result.migrated += 1;
                // 迁移成功：password_enc 改写为 'keychain' 标记，清掉 base64 明文
                let _ = conn.execute(
                    "UPDATE imap_accounts SET password_enc = 'keychain' WHERE id = ?1",
                    params![id],
                );
            }
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!("账号 {} keychain 存储失败: {}", email, e));
            }
        }
    }

    log::info!(
        "IMAP 密码迁移完成: 总计 {}, 迁移 {}, 跳过 {}, 失败 {}",
        result.total, result.migrated, result.skipped, result.failed
    );

    Ok(result)
}

/// 获取 IMAP 密码（优先 keychain，回退 base64）
pub fn get_imap_password(email: &str, password_enc: &str) -> Result<String> {
    use base64::Engine;

    // 优先从 keychain 读取
    if let Ok(Some(password)) = get_credential(CredentialType::ImapPassword, email) {
        return Ok(password);
    }

    // password_enc 为 'keychain' 标记：明文已迁移清除，keychain 又读不到 → 明确报错，
    // 不能拿 "keychain" 字符串去 base64 解码
    if password_enc == "keychain" {
        anyhow::bail!("凭据已迁移至钥匙串但读取失败，请重新输入密码");
    }

    // 回退到 base64 解码
    let password = base64::engine::general_purpose::STANDARD
        .decode(password_enc)
        .map_err(|e| anyhow::anyhow!("base64 解码失败: {}", e))
        .and_then(|bytes| String::from_utf8(bytes).map_err(|e| anyhow::anyhow!("UTF-8 解码失败: {}", e)))?;

    Ok(password)
}

/// 获取 AI API Key（优先 keychain，回退配置文件）
pub fn get_ai_api_key(config_key: &str) -> Result<Option<String>> {
    // 优先从 keychain 读取
    if let Ok(Some(key)) = get_credential(CredentialType::AiApiKey, "default") {
        return Ok(Some(key));
    }

    // 回退到配置文件中的值
    if !config_key.is_empty() {
        return Ok(Some(config_key.to_string()));
    }

    Ok(None)
}

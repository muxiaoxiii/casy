//! IMAP 邮件监听模块
//!
//! - ImapWatcher: async-imap + IDLE 模式监听新邮件
//! - 白名单过滤（sender/subject）
//! - 新邮件自动解析 → 添加到收件箱
//! - SMTP 发送（ICS 日历邀请）
//! - 29 分钟 IDLE 超时重连

pub mod smtp;

use anyhow::{Context, Result};
use async_imap::Session;
use futures_util::StreamExt;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_native_tls::TlsConnector;

use crate::db::{new_id, now_local};

// ============================================================
// IMAP 账号配置
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImapAccountConfig {
    pub id: Option<String>,
    pub email_address: String,
    pub imap_server: String,
    pub imap_port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
    pub watch_folders: String,
    pub filter_from: Option<String>,
    pub filter_subject: Option<String>,
    pub enabled: bool,
}

/// 保存 IMAP 账号配置到 imap_accounts 表
pub fn save_imap_account(config: &ImapAccountConfig) -> Result<String> {
    let conn = crate::db::open_db()?;
    let id = config.id.clone().unwrap_or_else(new_id);

    // 密码优先写入 OS keychain（service "casy-imap"，account 为邮箱地址）；
    // keychain 不可用时回退 base64，保证保存不失败（兼容旧逻辑）
    let password_enc = if config.password.is_empty() {
        // 空密码：保留原值（更新账号时不改密码的场景由前端避免传空，这里兜底）
        base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            config.password.as_bytes(),
        )
    } else {
        match crate::credentials::store_credential(
            crate::credentials::CredentialType::ImapPassword,
            &config.email_address,
            &config.password,
        ) {
            Ok(()) => "keychain".to_string(),
            Err(e) => {
                log::warn!("keychain 存储失败，回退 base64 存储: {}", e);
                base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    config.password.as_bytes(),
                )
            }
        }
    };

    conn.execute(
        "INSERT INTO imap_accounts (id, email_address, imap_server, imap_port, username, password_enc, use_tls, watch_folders, filter_from, filter_subject, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
           email_address=excluded.email_address,
           imap_server=excluded.imap_server,
           imap_port=excluded.imap_port,
           username=excluded.username,
           password_enc=excluded.password_enc,
           use_tls=excluded.use_tls,
           watch_folders=excluded.watch_folders,
           filter_from=excluded.filter_from,
           filter_subject=excluded.filter_subject,
           enabled=excluded.enabled",
        params![
            id,
            config.email_address,
            config.imap_server,
            config.imap_port,
            config.username,
            password_enc,
            config.use_tls as i32,
            config.watch_folders,
            config.filter_from,
            config.filter_subject,
            config.enabled as i32,
        ],
    )?;

    Ok(id)
}

/// 保存 IMAP 账号命令（返回 String）
pub fn save_imap_account_cmd(config: &ImapAccountConfig) -> anyhow::Result<String> {
    save_imap_account(config)
}

/// 从数据库加载所有启用的 IMAP 账号
pub fn load_enabled_accounts() -> Result<Vec<ImapAccountConfig>> {
    let conn = crate::db::open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, email_address, imap_server, imap_port, username, password_enc, use_tls, watch_folders, filter_from, filter_subject, enabled
         FROM imap_accounts WHERE enabled = 1",
    )?;

    let accounts = stmt
        .query_map([], |row| {
            let email_address: String = row.get("email_address")?;
            let password_enc: String = row.get("password_enc")?;
            // 优先 keychain，回退旧 base64 路径（兼容旧数据）
            let password = crate::credentials::get_imap_password(&email_address, &password_enc)
                .unwrap_or_default();

            Ok(ImapAccountConfig {
                id: Some(row.get("id")?),
                email_address,
                imap_server: row.get("imap_server")?,
                imap_port: row.get::<_, u16>("imap_port")?,
                username: row.get("username")?,
                password,
                use_tls: row.get::<_, i32>("use_tls")? != 0,
                watch_folders: row.get("watch_folders")?,
                filter_from: row.get("filter_from")?,
                filter_subject: row.get("filter_subject")?,
                enabled: row.get::<_, i32>("enabled")? != 0,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(accounts)
}

// ============================================================
// IMAP 连接与邮件解析
// ============================================================

type ImapSession = Session<tokio_native_tls::TlsStream<tokio::net::TcpStream>>;

/// 建立 IMAP 连接
async fn connect_imap(config: &ImapAccountConfig) -> Result<ImapSession> {
    let tcp = TcpStream::connect(format!("{}:{}", config.imap_server, config.imap_port))
        .await
        .context("TCP 连接失败")?;

    let session = if config.use_tls {
        let native_tls_connector = native_tls::TlsConnector::builder()
            .build()
            .context("创建 TLS 连接器失败")?;
        let tls = TlsConnector::from(native_tls_connector);
        let tls_stream = tls
            .connect(&config.imap_server, tcp)
            .await
            .context("TLS 握手失败")?;
        async_imap::Client::new(tls_stream)
            .login(&config.username, &config.password)
            .await
            .map_err(|(e, _)| anyhow::anyhow!("IMAP 登录失败: {}", e))?
    } else {
        anyhow::bail!("不支持非 TLS 连接（安全考虑）");
    };

    Ok(session)
}

/// 检查发件人/主题白名单过滤
fn passes_whitelist(config: &ImapAccountConfig, from: &str, subject: &str) -> bool {
    // 发件人过滤
    if let Some(ref filter_from) = config.filter_from {
        if !filter_from.is_empty() {
            let from_lower = from.to_lowercase();
            let matched = filter_from
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .any(|pattern| !pattern.is_empty() && from_lower.contains(&pattern));
            if !matched {
                return false;
            }
        }
    }

    // 主题过滤
    if let Some(ref filter_subject) = config.filter_subject {
        if !filter_subject.is_empty() {
            let subject_lower = subject.to_lowercase();
            let matched = filter_subject
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .any(|pattern| !pattern.is_empty() && subject_lower.contains(&pattern));
            if !matched {
                return false;
            }
        }
    }

    true
}

/// 解析邮件正文（从 mailparse 提取纯文本）
fn extract_body(mail: &mailparse::ParsedMail) -> Result<(Option<String>, Option<String>)> {
    let content_type = mail
        .headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "content-type")
        .map(|h| h.get_value())
        .unwrap_or_default();

    if mail.subparts.is_empty() {
        let body = mail.get_body()?;
        if content_type.to_lowercase().contains("text/html") {
            Ok((None, Some(body)))
        } else {
            Ok((Some(body), None))
        }
    } else {
        let mut text = None;
        let mut html = None;
        for part in &mail.subparts {
            let (t, h) = extract_body(part)?;
            if t.is_some() && text.is_none() {
                text = t;
            }
            if h.is_some() && html.is_none() {
                html = h;
            }
        }
        Ok((text, html))
    }
}

/// 解析单封邮件并写入收件箱
fn process_email(config: &ImapAccountConfig, raw_email: &[u8], uid: u32) -> Result<bool> {
    let mail = mailparse::parse_mail(raw_email).context("邮件解析失败")?;

    // 提取头部
    let subject = mail
        .headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "subject")
        .map(|h| h.get_value())
        .unwrap_or_else(|| "(无主题)".into());

    let from = mail
        .headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "from")
        .map(|h| h.get_value())
        .unwrap_or_default();

    let message_id = mail
        .headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "message-id")
        .map(|h| h.get_value());

    let date_str = mail
        .headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "date")
        .and_then(|h| {
            mailparse::dateparse(&h.get_value()).ok().and_then(|ts| {
                chrono::DateTime::from_timestamp_millis(ts * 1000)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            })
        })
        .unwrap_or_else(now_local);

    let mut connection = crate::db::open_db()?;
    let conn = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    // 白名单过滤
    if !passes_whitelist(config, &from, &subject) {
        log::debug!("邮件被白名单过滤: from={}, subject={}", from, subject);
        if let Some(account_id) = &config.id {
            conn.execute("UPDATE imap_accounts SET last_sync_uid=?1 WHERE id=?2 AND CAST(COALESCE(last_sync_uid,'0') AS INTEGER) < ?3", params![uid.to_string(), account_id, uid])?;
        }
        conn.commit()?;
        return Ok(false);
    }

    // 提取正文
    let (body_text, body_html) = extract_body(&mail)?;

    // 解析发件人姓名和地址
    let (from_name, from_addr) = parse_from_address(&from);

    // 使用规则分类
    let text_for_classify = body_text.as_deref().unwrap_or(&subject);
    let parsed = crate::parse::classify_document(text_for_classify);

    // 写入 email_records 表
    let email_id = new_id();

    // 检查 message_id 去重
    if let Some(ref mid) = message_id {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM email_records WHERE message_id = ?1",
                params![mid],
                |r| r.get(0),
            )
            .context("邮件去重检查失败")?;
        if exists {
            log::debug!("邮件已存在，跳过: {}", mid);
            return Ok(false);
        }
    }

    // 检查 UID 去重
    let last_uid: Option<String> = conn
        .query_row(
            "SELECT last_sync_uid FROM imap_accounts WHERE id = ?1",
            params![config.id.as_deref().unwrap_or("")],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    if let Some(ref last) = last_uid {
        if let Ok(last_num) = last.parse::<u32>() {
            if uid <= last_num {
                return Ok(false);
            }
        }
    }

    conn.execute(
        "INSERT INTO email_records (id, message_id, subject, from_address, from_name, date, body_text, body_html, email_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            email_id,
            message_id,
            subject,
            from_addr,
            from_name,
            date_str,
            body_text,
            body_html,
            classify_email_type(&subject, text_for_classify),
            now_local(),
        ],
    )?;

    // 同时添加到收件箱
    let inbox_id = new_id();
    conn.execute(
        "INSERT INTO inbox_items (id, source_type, title, content_text, ai_category, ai_confidence, status, created_at)
         VALUES (?1, 'email', ?2, ?3, ?4, ?5, 'pending', ?6)",
        params![
            inbox_id,
            subject,
            body_text.as_deref().unwrap_or(""),
            parsed.doc_type,
            parsed.confidence,
            now_local(),
        ],
    )?;

    // 更新 IMAP 账号的 last_sync_uid
    if let Some(ref account_id) = config.id {
        conn.execute(
            "UPDATE imap_accounts SET last_sync_uid = ?1 WHERE id = ?2",
            params![uid.to_string(), account_id],
        )?;
    }

    conn.commit()?;
    if let Some(app) = crate::get_app_handle() {
        use tauri::Emitter;
        let _ = app.emit("inbox:new_item", &inbox_id);
    }
    log::info!("新邮件已处理: {} (uid={})", subject, uid);
    Ok(true)
}

/// 解析发件人地址 "Name <email>" → (name, email)
fn parse_from_address(from: &str) -> (Option<String>, String) {
    if let Some(angle_start) = from.find('<') {
        let name = from[..angle_start].trim();
        let addr = from[angle_start + 1..].trim_end_matches('>');
        let name = if name.is_empty() {
            None
        } else {
            Some(name.trim_matches('"').to_string())
        };
        (name, addr.to_string())
    } else {
        (None, from.trim().to_string())
    }
}

/// 根据主题/内容分类邮件类型
fn classify_email_type(subject: &str, body: &str) -> &'static str {
    let combined = format!("{} {}", subject, body).to_lowercase();
    if combined.contains("传票") || combined.contains("开庭") || combined.contains("口审") {
        "court_notice"
    } else if combined.contains("委托") || combined.contains("指示") || combined.contains("指令")
    {
        "client_instruction"
    } else if combined.contains("对方") || combined.contains("答辩") || combined.contains("代理词")
    {
        "opposing_counsel"
    } else if combined.contains("函") || combined.contains("沟通") || combined.contains("协商")
    {
        "correspondence"
    } else {
        "other"
    }
}

// ============================================================
// ImapWatcher — IDLE 监听器
// ============================================================

#[allow(dead_code)]
const IDLE_TIMEOUT_SECS: u64 = 29 * 60; // 29 分钟

/// 启动连通性探测超时（S-7：启动即验证，但不阻塞应用启动）
const IMAP_PROBE_TIMEOUT_SECS: u64 = 20;

/// 单个 IMAP 账号的连接态（S-7：启动探测 + 运行期错误可观测）
#[derive(Debug, Clone, Default)]
pub struct ImapAccountConnection {
    /// 最近一次探测/监听是否成功连通
    pub connected: bool,
    /// 最近一次错误（成功时为 None）
    pub last_error: Option<String>,
}

pub struct ImapWatcher {
    running: Arc<AtomicBool>,
    handle: Option<tokio::task::JoinHandle<()>>,
    /// 账号连接态：email_address → 状态。
    /// std Mutex：临界区只做一次浅拷贝，不跨 await 持有。
    connections: Arc<std::sync::Mutex<HashMap<String, ImapAccountConnection>>>,
}

impl ImapWatcher {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            connections: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// 记录账号连接态（状态 API 与日志观测用）
    fn record_connection(&self, key: &str, connected: bool, last_error: Option<String>) {
        if let Ok(mut states) = self.connections.lock() {
            states.insert(
                key.to_string(),
                ImapAccountConnection {
                    connected,
                    last_error,
                },
            );
        }
    }

    /// 查询账号连接态（键为邮箱地址）
    pub fn account_connection(&self, key: &str) -> Option<ImapAccountConnection> {
        self.connections
            .lock()
            .ok()
            .and_then(|states| states.get(key).cloned())
    }

    /// 启动监听
    ///
    /// S-7：启动时对每个账号做一次真实连接 + 拉取探测（而不是 spawn 后立即
    /// 返回 Ok），结果写入 connections 供 get_email_monitor_status 观测。
    /// 探测失败不阻断启动：watch_account 自带 30s 重试，网络恢复后自动连上。
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let accounts = load_enabled_accounts()?;
        if accounts.is_empty() {
            anyhow::bail!("没有启用的 IMAP 账号，请先配置");
        }

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let connections = self.connections.clone();

        // 启动连通性探测：真实建立连接、选择文件夹并拉取一次邮件列表
        for account in &accounts {
            match probe_account(account).await {
                Ok(()) => {
                    log::info!("IMAP 启动探测成功: {}", account.email_address);
                    self.record_connection(&account.email_address, true, None);
                }
                Err(e) => {
                    log::warn!("IMAP 启动探测失败 ({}): {}", account.email_address, e);
                    self.record_connection(&account.email_address, false, Some(e.to_string()));
                }
            }
        }

        let handle = tokio::spawn(async move {
            for account in accounts {
                let running_clone = running.clone();
                let connections_clone = connections.clone();
                tokio::spawn(async move {
                    if let Err(e) = watch_account(account, running_clone, connections_clone).await {
                        crate::processing::service("email","邮件监听","failed","邮件监听异常",Some(&e.to_string()));
                        log::error!("IMAP 监听错误: {}", e);
                    }
                });
            }

            // 主循环：保持任务存活
            while running.load(Ordering::SeqCst) {
                sleep(Duration::from_secs(5)).await;
            }
        });

        self.handle = Some(handle);
        crate::processing::service("email","邮件监听","waiting","等待新邮件通知",None);
        log::info!("IMAP 邮件监听已启动");
        Ok(())
    }

    /// 停止监听
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
        crate::processing::service("email","邮件监听","disabled","已停止监听",None);
        log::info!("IMAP 邮件监听已停止");
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// 启动连通性探测：真实建立一次 IMAP 连接、选择监听文件夹并拉取一次邮件
///
/// 与 watch_account 的区别：不进入 IDLE，探测完即注销，只用于启动时验证
/// 服务器/凭据/文件夹是否可用，并把结果写入 watcher 连接态（S-7）。
async fn probe_account(config: &ImapAccountConfig) -> Result<()> {
    let mut session = tokio::time::timeout(
        Duration::from_secs(IMAP_PROBE_TIMEOUT_SECS),
        connect_imap(config),
    )
    .await
    .map_err(|_| anyhow::anyhow!("IMAP 连接超时（{}s）", IMAP_PROBE_TIMEOUT_SECS))??;

    let folder = config.watch_folders.split(',').next().unwrap_or("INBOX");
    session
        .select(folder)
        .await
        .context("选择文件夹失败")?;
    // 一次真实拉取：验证邮箱可读（凭据/权限在此暴露）
    session
        .uid_search("UID 1:*")
        .await
        .context("搜索邮件失败")?;
    session.logout().await.ok();
    Ok(())
}

/// 监听单个 IMAP 账号
async fn watch_account(
    config: ImapAccountConfig,
    running: Arc<AtomicBool>,
    connections: Arc<std::sync::Mutex<HashMap<String, ImapAccountConnection>>>,
) -> Result<()> {
    log::info!(
        "开始监听邮箱: {} ({}:{})",
        config.email_address,
        config.imap_server,
        config.imap_port
    );

    while running.load(Ordering::SeqCst) {
        match connect_and_idle(&config, &running).await {
            Ok(_) => {
                log::info!("IMAP IDLE 正常结束，准备重连");
                record_connection(&connections, &config.email_address, true, None);
            }
            Err(e) => {
                log::error!("IMAP 连接错误: {}, 30 秒后重试", e);
                record_connection(&connections, &config.email_address, false, Some(e.to_string()));
                sleep(Duration::from_secs(30)).await;
            }
        }
    }

    Ok(())
}

/// 写入账号连接态（锁 poisoning 时静默跳过，不影响监听主流程）
fn record_connection(
    connections: &std::sync::Mutex<HashMap<String, ImapAccountConnection>>,
    key: &str,
    connected: bool,
    last_error: Option<String>,
) {
    if let Ok(mut states) = connections.lock() {
        states.insert(
            key.to_string(),
            ImapAccountConnection {
                connected,
                last_error,
            },
        );
    }
}

/// 连接并进入 IDLE 模式
async fn connect_and_idle(config: &ImapAccountConfig, running: &Arc<AtomicBool>) -> Result<()> {
    let mut session = Some(connect_imap(config).await?);

    // 选择监听文件夹
    let folder = config.watch_folders.split(',').next().unwrap_or("INBOX");
    session
        .as_mut()
        .unwrap()
        .select(folder)
        .await
        .context("选择文件夹失败")?;

    // 先拉取未读邮件
    fetch_new_emails(session.as_mut().unwrap(), config).await?;

    // 使用 IDLE 模式监听新邮件
    // 29 分钟超时后重连（IMAP 服务器通常 30 分钟超时）
    let idle_timeout = Duration::from_secs(IDLE_TIMEOUT_SECS);

    loop {
        if !running.load(Ordering::SeqCst) {
            break;
        }

        // 进入 IDLE 模式（take session 所有权）
        let mut idle = session.take().unwrap().idle();
        idle.init().await.context("初始化 IDLE 模式失败")?;

        // 等待新邮件通知或超时
        let (wait_future, _stop_source) = idle.wait_with_timeout(idle_timeout);

        let should_break;
        tokio::select! {
            result = wait_future => {
                match result {
                    Ok(async_imap::extensions::idle::IdleResponse::NewData(_)) => {
                        // 收到新邮件通知
                        log::debug!("收到 IDLE 通知，检查新邮件");
                        session = Some(idle.done().await.context("退出 IDLE 模式失败")?);
                        if let Err(e) = fetch_new_emails(session.as_mut().unwrap(), config).await {
                            log::warn!("检查新邮件失败: {}", e);
                            should_break = true;
                        } else {
                            should_break = false;
                        }
                    }
                    Ok(async_imap::extensions::idle::IdleResponse::Timeout) => {
                        // 超时，重新连接
                        log::debug!("IDLE 超时，重新连接");
                        session = Some(idle.done().await.context("退出 IDLE 模式失败")?);
                        should_break = false;
                    }
                    Ok(async_imap::extensions::idle::IdleResponse::ManualInterrupt) => {
                        // 手动中断
                        log::debug!("IDLE 手动中断");
                        session = Some(idle.done().await.context("退出 IDLE 模式失败")?);
                        should_break = true;
                    }
                    Err(e) => {
                        log::warn!("IDLE 等待错误: {}", e);
                        should_break = true;
                    }
                }
            }
            _ = async {
                // 检查运行状态
                while running.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            } => {
                // 收到停止信号
                log::info!("收到停止信号，退出 IDLE");
                if let Ok(s) = idle.done().await {
                    session = Some(s);
                }
                should_break = true;
            }
        }

        if should_break {
            break;
        }
    }

    if let Some(mut s) = session {
        s.logout().await.ok();
    }
    Ok(())
}

/// 拉取新邮件
async fn fetch_new_emails(session: &mut ImapSession, config: &ImapAccountConfig) -> Result<()> {
    crate::processing::tracked("email","收取新邮件",fetch_new_emails_inner(session,config)).await
}
async fn fetch_new_emails_inner(session: &mut ImapSession, config: &ImapAccountConfig) -> Result<()> {
    // 获取上次同步的 UID
    let last_uid = {
        let conn = crate::db::open_db()?;
        conn.query_row(
            "SELECT last_sync_uid FROM imap_accounts WHERE id = ?1",
            params![config.id.as_deref().unwrap_or("")],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0)
    };

    // 搜索新邮件
    let search_query = if last_uid > 0 {
        format!("UID {}:*", last_uid + 1)
    } else {
        "ALL".to_string()
    };

    let uids = session
        .uid_search(&search_query)
        .await
        .context("搜索邮件失败")?;

    for uid in uids.iter() {
        if config.id.is_none() {
            continue;
        }

        // 拉取邮件原始内容
        let mut fetches = session
            .uid_fetch(uid.to_string(), "RFC822")
            .await
            .context("拉取邮件失败")?;

        while let Some(fetch) = fetches.next().await {
            let fetch = fetch.context("获取邮件数据失败")?;
            if let Some(body) = fetch.body() {
                if let Err(e) = process_email(config, body, *uid) {
                    log::error!("处理邮件失败 (uid={}): {}", uid, e);
                }
            }
        }
    }

    Ok(())
}

// ============================================================
// Tauri 全局状态
// ============================================================

/// 全局 ImapWatcher 实例
static IMAP_WATCHER: std::sync::OnceLock<Arc<Mutex<ImapWatcher>>> = std::sync::OnceLock::new();

fn get_watcher() -> &'static Arc<Mutex<ImapWatcher>> {
    IMAP_WATCHER.get_or_init(|| Arc::new(Mutex::new(ImapWatcher::new())))
}

// ============================================================
// Tauri 命令
// ============================================================

/// 保存 IMAP 账号配置
#[tauri::command]
pub async fn configure_imap(account: ImapAccountConfig) -> Result<String, String> {
    crate::commands::run_blocking(move || save_imap_account_cmd(&account)).await
}

/// 启动邮件监听
#[tauri::command]
pub async fn start_email_monitor() -> Result<String, String> {
    let watcher = get_watcher();
    let mut w = watcher.lock().await;
    w.start().await.map_err(|e| e.to_string())?;
    Ok("邮件监听已启动".into())
}

/// 停止邮件监听
#[tauri::command]
pub async fn stop_email_monitor() -> Result<String, String> {
    let watcher = get_watcher();
    let mut w = watcher.lock().await;
    w.stop();
    Ok("邮件监听已停止".into())
}

/// 列出所有 IMAP 账号（含禁用的）
#[tauri::command]
pub async fn list_imap_accounts() -> Result<Vec<serde_json::Value>, String> {
    crate::commands::run_blocking(move || {
        let conn = crate::db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, email_address, imap_server, imap_port, use_tls, watch_folders, filter_from, filter_subject, enabled
             FROM imap_accounts ORDER BY created_at DESC",
        )?;
        let accounts: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>("id")?,
                    "emailAddress": row.get::<_, String>("email_address")?,
                    "imapServer": row.get::<_, String>("imap_server")?,
                    "imapPort": row.get::<_, u16>("imap_port")?,
                    "useTls": row.get::<_, i32>("use_tls")? != 0,
                    "watchFolders": row.get::<_, String>("watch_folders")?,
                    "filterFrom": row.get::<_, Option<String>>("filter_from")?,
                    "filterSubject": row.get::<_, Option<String>>("filter_subject")?,
                    "enabled": row.get::<_, i32>("enabled")? != 0,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(accounts)
    })
    .await
}

/// 删除 IMAP 账号
#[tauri::command]
pub async fn delete_imap_account(email_address: String) -> Result<String, String> {
    crate::commands::run_blocking(move || {
        let conn = crate::db::open_db()?;
        let deleted = conn.execute(
            "DELETE FROM imap_accounts WHERE email_address = ?1",
            rusqlite::params![email_address],
        )?;
        if deleted > 0 {
            // 同步清理 keychain 中的密码
            let _ = crate::credentials::delete_credential(
                crate::credentials::CredentialType::ImapPassword,
                &email_address,
            );
            Ok(format!("已删除账号: {}", email_address))
        } else {
            Err(anyhow::anyhow!("未找到账号: {}", email_address))
        }
    })
    .await
}

/// 获取邮件监听状态
///
/// S-7：除 running / 账号数外，逐个账号报告连接态与最近一次错误，
/// 前端可据此显示"未连接/凭据错误"而不是只有一个开关。
#[tauri::command]
pub async fn get_email_monitor_status() -> Result<serde_json::Value, String> {
    let watcher = get_watcher();
    let w = watcher.lock().await;
    let accounts = load_enabled_accounts().unwrap_or_default();

    Ok(status_json(w.is_running(), &accounts, |key| {
        w.account_connection(key)
    }))
}

/// 组装监听状态 JSON（独立函数便于测试，不依赖数据库）
fn status_json<F>(running: bool, accounts: &[ImapAccountConfig], connection_of: F) -> serde_json::Value
where
    F: Fn(&str) -> Option<ImapAccountConnection>,
{
    serde_json::json!({
        "running": running,
        "accountCount": accounts.len(),
        "accounts": accounts.iter().map(|a| {
            let connection = connection_of(&a.email_address);
            serde_json::json!({
                "id": a.id,
                "email": a.email_address,
                "server": a.imap_server,
                "enabled": a.enabled,
                "connected": connection.as_ref().map(|c| c.connected).unwrap_or(false),
                "lastError": connection.and_then(|c| c.last_error),
            })
        }).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod status_shape_tests {
    use super::*;

    fn account(email: &str) -> ImapAccountConfig {
        ImapAccountConfig {
            id: Some(format!("id-{email}")),
            email_address: email.into(),
            imap_server: "imap.example.invalid".into(),
            imap_port: 993,
            username: "user".into(),
            password: String::new(),
            use_tls: true,
            watch_folders: "INBOX".into(),
            filter_from: None,
            filter_subject: None,
            enabled: true,
        }
    }

    /// 状态 API 必须报告每个账号的连接态与最近错误（S-7）
    #[test]
    fn monitor_status_reports_per_account_connection_state() {
        let accounts = vec![account("a@example.invalid"), account("b@example.invalid")];
        let states: HashMap<String, ImapAccountConnection> = HashMap::from([
            (
                "a@example.invalid".to_string(),
                ImapAccountConnection {
                    connected: true,
                    last_error: None,
                },
            ),
            (
                "b@example.invalid".to_string(),
                ImapAccountConnection {
                    connected: false,
                    last_error: Some("IMAP 登录失败".into()),
                },
            ),
        ]);
        let value = status_json(true, &accounts, |key| states.get(key).cloned());
        assert_eq!(value["running"], serde_json::json!(true));
        assert_eq!(value["accountCount"], serde_json::json!(2));
        let list = value["accounts"].as_array().unwrap();
        assert_eq!(list[0]["email"], serde_json::json!("a@example.invalid"));
        assert_eq!(list[0]["connected"], serde_json::json!(true));
        assert_eq!(list[0]["lastError"], serde_json::json!(null));
        assert_eq!(list[1]["connected"], serde_json::json!(false));
        assert_eq!(
            list[1]["lastError"],
            serde_json::json!("IMAP 登录失败")
        );

        // 未探测过的账号：connected 为 false，不臆造成功
        let fresh = status_json(false, &accounts[..1], |_| None);
        assert_eq!(fresh["running"], serde_json::json!(false));
        assert_eq!(fresh["accounts"][0]["connected"], serde_json::json!(false));
    }
}

#[cfg(test)]
mod atomic_intake_tests {
    use super::*;
    #[test]
    fn message_and_inbox_commit_together_and_filtered_uid_advances() {
        let root=tempfile::tempdir().unwrap();
        std::env::set_var("CASY_TEST_DATA_DIR",root.path());
        crate::db::enable_test_mode();
        let conn=crate::db::open_db().unwrap();crate::db::init_db(&conn).unwrap();
        conn.execute_batch("INSERT INTO imap_accounts(id,email_address,imap_server,username,password_enc) VALUES('atomic-mail','test@example.invalid','example.invalid','user','');
            CREATE TRIGGER fail_mail_inbox BEFORE INSERT ON inbox_items WHEN NEW.source_type='email' BEGIN SELECT RAISE(ABORT,'mail failure'); END;").unwrap();
        let mut config=ImapAccountConfig{id:Some("atomic-mail".into()),email_address:"test@example.invalid".into(),imap_server:"example.invalid".into(),imap_port:993,username:"user".into(),password:String::new(),use_tls:true,watch_folders:"INBOX".into(),filter_from:None,filter_subject:None,enabled:true};
        let raw=b"From: Sender <sender@example.invalid>\r\nSubject: Atomic mail\r\nMessage-ID: <atomic@example.invalid>\r\n\r\nBody";
        assert!(process_email(&config,raw,7).is_err());
        assert_eq!(conn.query_row("SELECT count(*) FROM email_records WHERE message_id='<atomic@example.invalid>'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        conn.execute_batch("DROP TRIGGER fail_mail_inbox").unwrap();
        assert!(process_email(&config,raw,7).unwrap());
        assert!(!process_email(&config,raw,7).unwrap());
        config.filter_from=Some("allowed.invalid".into());
        assert!(!process_email(&config,raw,8).unwrap());
        assert_eq!(conn.query_row("SELECT last_sync_uid FROM imap_accounts WHERE id='atomic-mail'",[],|r|r.get::<_,String>(0)).unwrap(),"8");
    }
}

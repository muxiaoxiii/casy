//! SMTP 发送模块（设计哲学 §11.2）
//!
//! 为日历同步提供 ICS 邀请发送能力。
//! 支持发送日程邀请邮件（.ics 附件）给当事人/同事。
//!
//! 最小 SMTP 客户端实现：
//! - 465 端口：隐式 TLS
//! - 其他端口（如 587）：明文连接后 STARTTLS 升级
//! - 流程：EHLO → (STARTTLS → EHLO) → AUTH PLAIN → MAIL FROM → RCPT TO → DATA → QUIT
//! - 整体超时 30s

use anyhow::{Context, Result};
use chrono::{NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::TlsConnector;

/// SMTP 整体超时
const SMTP_TIMEOUT_SECS: u64 = 30;

/// ICS 内容行最大字节数（RFC 5545 §3.1：单行不应超过 75 字节）
const ICS_LINE_MAX_OCTETS: usize = 75;

/// SMTP 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmtpConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
    pub from_address: String,
    pub from_name: String,
}

/// 日程事件（用于生成 ICS）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub dtstart: NaiveDateTime,
    pub dtend: NaiveDateTime,
    pub all_day: bool,
    /// 提醒提前分钟数
    pub alarm_minutes: Option<i64>,
}

/// 本地浮动时间 → RFC 5545 UTC DATE-TIME（`YYYYMMDDTHHMMSSZ`）
///
/// 输入 NaiveDateTime 视为本地时间；统一转 UTC 而不是发 VTIMEZONE，
/// 这样报文不含时区定义也能被所有日历客户端正确解析（S-6）。
/// 夏令时缺口等无法映射的时刻按原值兜底，避免 panic。
fn to_utc_ics(dt: &NaiveDateTime) -> String {
    let utc = match chrono::Local.from_local_datetime(dt) {
        chrono::LocalResult::Single(local)
        | chrono::LocalResult::Ambiguous(local, _) => local.with_timezone(&chrono::Utc),
        // 夏令时缺口：不存在对应的本地时刻，按原值输出（调用方传入的即显示意图）
        chrono::LocalResult::None => return dt.format("%Y%m%dT%H%M%SZ").to_string(),
    };
    utc.format("%Y%m%dT%H%M%SZ").to_string()
}

/// RFC 5545 折行：超过 75 字节的内容行拆成多行，续行以单个空格开头
///
/// 按字符（char）边界累加字节数，绝不切断多字节 UTF-8 字符，
/// 因此中文摘要/地点折行后仍是合法 UTF-8。
fn fold_ics_line(line: &str) -> String {
    let mut folded = String::with_capacity(line.len() + 16);
    let mut used = 0usize; // 当前行已占字节数（不含续行前导空格）
    for ch in line.chars() {
        let len = ch.len_utf8();
        // 续行前导空格占 1 字节，故续行正文最多 74 字节
        let limit = if used == 0 {
            ICS_LINE_MAX_OCTETS
        } else {
            ICS_LINE_MAX_OCTETS - 1
        };
        if used + len > limit {
            folded.push_str("\r\n ");
            used = 0;
        }
        folded.push(ch);
        used += len;
    }
    folded
}

/// 生成 ICS 日历内容
pub fn generate_ics(event: &CalendarEvent) -> String {
    // 非全天事件统一转 UTC（带 Z）；全天事件用 DATE 值并显式声明 VALUE=DATE
    let (dtstart_prop, dtend_prop) = if event.all_day {
        (
            format!("DTSTART;VALUE=DATE:{}", event.dtstart.format("%Y%m%d")),
            format!("DTEND;VALUE=DATE:{}", event.dtend.format("%Y%m%d")),
        )
    } else {
        (
            format!("DTSTART:{}", to_utc_ics(&event.dtstart)),
            format!("DTEND:{}", to_utc_ics(&event.dtend)),
        )
    };

    // DTSTAMP 必须是 UTC（RFC 5545 §3.8.7.2），带 Z 后缀
    let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();

    let alarm = if let Some(minutes) = event.alarm_minutes {
        format!(
            "BEGIN:VALARM\r\n\
             TRIGGER:-PT{}M\r\n\
             ACTION:DISPLAY\r\n\
             {}\r\n\
             END:VALARM\r\n",
            minutes,
            ics_content_line("DESCRIPTION", &event.summary)
        )
    } else {
        String::new()
    };

    format!(
        "BEGIN:VCALENDAR\r\n\
         VERSION:2.0\r\n\
         PRODID:-//Casy//Casy Calendar//CN\r\n\
         CALSCALE:GREGORIAN\r\n\
         METHOD:REQUEST\r\n\
         BEGIN:VEVENT\r\n\
         UID:{}\r\n\
         DTSTAMP:{}\r\n\
         {}\r\n\
         {}\r\n\
         {}\r\n\
         {}\r\n\
         {}\r\n\
         STATUS:CONFIRMED\r\n\
         {}\
         END:VEVENT\r\n\
         END:VCALENDAR\r\n",
        event.uid,
        now,
        dtstart_prop,
        dtend_prop,
        ics_content_line("SUMMARY", &event.summary),
        ics_content_line("DESCRIPTION", event.description.as_deref().unwrap_or("")),
        ics_content_line("LOCATION", event.location.as_deref().unwrap_or("")),
        alarm,
    )
}

/// 转义 ICS 特殊字符（不折行；折行必须覆盖属性名前缀，见 ics_content_line）
fn escape_ics(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

/// 组装并按 75 字节折行的完整内容行（属性名 + 冒号 + 转义后的值）
fn ics_content_line(property: &str, value: &str) -> String {
    fold_ics_line(&format!("{property}:{}", escape_ics(value)))
}

// ============================================================
// 邮件头注入防护
// ============================================================

/// 校验邮件头字段：拒绝 CR/LF 与控制字符（防 SMTP 头注入）
fn sanitize_header_value(value: &str, field: &str) -> Result<String> {
    if value
        .chars()
        .any(|c| c == '\r' || c == '\n' || (c.is_control() && c != '\t'))
    {
        anyhow::bail!("邮件{}包含非法控制字符（疑似头注入），已拒绝发送", field);
    }
    Ok(value.to_string())
}

/// RFC 2047 encoded-word：非 ASCII 头字段做 Base64 编码，纯 ASCII 原样返回
fn encode_header(value: &str) -> String {
    if value.is_ascii() {
        value.to_string()
    } else {
        format!("=?UTF-8?B?{}?=", base64_encode(value.as_bytes()))
    }
}

/// 基本邮箱格式校验：单个 @、纯 ASCII、无空白/控制字符/地址分隔符
fn validate_email(addr: &str) -> Result<()> {
    let ok = !addr.is_empty()
        && addr.len() <= 254
        && addr.is_ascii()
        && addr.matches('@').count() == 1
        && !addr.starts_with('@')
        && !addr.ends_with('@')
        && !addr
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '<' | '>' | ',' | ';' | '(' | ')' | '"'));
    if ok {
        Ok(())
    } else {
        Err(anyhow::anyhow!("邮箱地址格式不合法: {:?}", addr))
    }
}

/// RFC 2822 `Date:` 头（如 `Wed, 08 Oct 2026 09:15:00 +0800`）
fn rfc2822_now() -> String {
    chrono::Local::now()
        .format("%a, %d %b %Y %H:%M:%S %z")
        .to_string()
}

/// Message-ID 域名：取发件地址的域名，非法时回退 casy.local
fn message_id_domain(from_address: &str) -> String {
    let domain = from_address
        .rsplit('@')
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        .collect::<String>();
    let domain = domain.trim_matches(['.', '-']).to_string();
    if domain.is_empty() {
        "casy.local".to_string()
    } else {
        domain
    }
}

/// 构建 ICS 邀请邮件的 MIME 报文（含 RFC 2822 Date / Message-ID 头）
fn build_invitation_email(
    config: &SmtpConfig,
    from_name: &str,
    to_name: &str,
    to_email: &str,
    subject: &str,
    event: &CalendarEvent,
) -> String {
    let ics_content = generate_ics(event);

    // 构建 MIME 邮件
    let boundary = "----=_Part_001_Casy_Calendar";
    let body_text = format!(
        "您有一个新的日程邀请：\n\n\
         事项：{}\n\
         时间：{} - {}\n\
         {}\n\
         {}\n\n\
         请查看附件中的日历文件以添加到您的日历。",
        event.summary,
        event.dtstart.format("%Y-%m-%d %H:%M"),
        event.dtend.format("%Y-%m-%d %H:%M"),
        event
            .location
            .as_deref()
            .map(|l| format!("地点：{}", l))
            .unwrap_or_default(),
        event
            .description
            .as_deref()
            .map(|d| format!("说明：{}", d))
            .unwrap_or_default(),
    );

    // RFC 2822 必需头：Date（发送时间）与 Message-ID（全局唯一，便于线程/去重）
    let date = rfc2822_now();
    let message_id = format!(
        "<{}@{}>",
        uuid::Uuid::new_v4(),
        message_id_domain(&config.from_address)
    );

    format!(
        "From: {} <{}>\r\n\
         To: {} <{}>\r\n\
         Subject: {}\r\n\
         Date: {}\r\n\
         Message-ID: {}\r\n\
         MIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"{}\"\r\n\
         \r\n\
         --{}\r\n\
         Content-Type: text/plain; charset=\"UTF-8\"\r\n\
         Content-Transfer-Encoding: base64\r\n\
         \r\n\
         {}\r\n\
         \r\n\
         --{}\r\n\
         Content-Type: text/calendar; method=REQUEST; name=\"invite.ics\"\r\n\
         Content-Disposition: attachment; filename=\"invite.ics\"\r\n\
         Content-Transfer-Encoding: base64\r\n\
         \r\n\
         {}\r\n\
         \r\n\
         --{}--\r\n",
        from_name,
        config.from_address,
        to_name,
        to_email,
        subject,
        date,
        message_id,
        boundary,
        boundary,
        base64_encode(body_text.as_bytes()),
        boundary,
        base64_encode(ics_content.as_bytes()),
        boundary,
    )
}

/// 发送 ICS 邀请邮件
pub async fn send_ics_invitation(
    config: &SmtpConfig,
    to_email: &str,
    to_name: &str,
    event: &CalendarEvent,
) -> Result<()> {
    // ── 头注入防护：信封地址格式校验 + 头字段控制字符拒绝 + 非 ASCII 头 RFC2047 编码 ──
    validate_email(to_email)?;
    validate_email(&config.from_address)?;
    let to_name = encode_header(&sanitize_header_value(to_name, "收件人名称")?);
    let from_name = encode_header(&sanitize_header_value(&config.from_name, "发件人名称")?);
    let subject = encode_header(&sanitize_header_value(
        &format!("日程邀请: {}", event.summary),
        "主题",
    )?);

    let email_content =
        build_invitation_email(config, &from_name, &to_name, to_email, &subject, event);

    // 发送邮件
    send_raw_email(config, to_email, email_content.as_bytes()).await
}

// ============================================================
// 最小 SMTP 客户端
// ============================================================

/// SMTP 传输流：明文或 TLS
enum SmtpStream {
    Plain(TcpStream),
    Tls(tokio_native_tls::TlsStream<TcpStream>),
}

impl SmtpStream {
    async fn write_all(&mut self, data: &[u8]) -> Result<()> {
        match self {
            Self::Plain(s) => s.write_all(data).await?,
            Self::Tls(s) => s.write_all(data).await?,
        }
        Ok(())
    }

    /// 发送一行 SMTP 命令（自动补 CRLF）
    async fn write_line(&mut self, line: &str) -> Result<()> {
        self.write_all(format!("{}\r\n", line).as_bytes()).await
    }

    /// 读取一行响应（以 \n 结尾）
    async fn read_line(&mut self) -> Result<String> {
        let mut buf: Vec<u8> = Vec::with_capacity(256);
        let mut byte = [0u8; 1];
        loop {
            let n = match self {
                Self::Plain(s) => s.read(&mut byte).await?,
                Self::Tls(s) => s.read(&mut byte).await?,
            };
            if n == 0 {
                anyhow::bail!("SMTP 连接被服务器关闭");
            }
            buf.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
            if buf.len() > 8192 {
                anyhow::bail!("SMTP 响应行过长");
            }
        }
        Ok(String::from_utf8_lossy(&buf).to_string())
    }

    /// 读取完整 SMTP 响应（兼容 "250-..." 多行格式），返回 (状态码, 全文)
    async fn read_response(&mut self) -> Result<(u16, String)> {
        let mut full = String::new();
        loop {
            let line = self.read_line().await?;
            full.push_str(&line);
            let trimmed = line.trim_end_matches(['\r', '\n']);
            // 非标准行（不足 4 字符）视为结束，避免死等
            if trimmed.len() < 4 {
                break;
            }
            let is_last = trimmed.as_bytes()[3] == b' ';
            if is_last {
                break;
            }
        }
        let code: u16 = full
            .get(..3)
            .and_then(|c| c.parse().ok())
            .context("解析 SMTP 响应码失败")?;
        Ok((code, full))
    }

    /// 升级为 TLS 连接（STARTTLS）
    async fn upgrade_tls(self, host: &str) -> Result<SmtpStream> {
        match self {
            Self::Plain(tcp) => {
                let connector = TlsConnector::from(
                    native_tls::TlsConnector::builder()
                        .build()
                        .context("创建 TLS 连接器失败")?,
                );
                let tls = connector
                    .connect(host, tcp)
                    .await
                    .context("STARTTLS 握手失败")?;
                Ok(SmtpStream::Tls(tls))
            }
            Self::Tls(_) => Ok(self),
        }
    }
}

/// 校验 SMTP 响应码
fn expect_code(code: u16, expected: &[u16], resp: &str, step: &str) -> Result<()> {
    if expected.contains(&code) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "SMTP {} 失败（期望 {:?}，收到 {}）: {}",
            step,
            expected,
            code,
            resp.trim()
        ))
    }
}

/// SMTP 会话主流程
async fn smtp_session(config: &SmtpConfig, to_email: &str, content: &[u8]) -> Result<()> {
    let addr = format!("{}:{}", config.smtp_server, config.smtp_port);
    let tcp = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("连接 SMTP 服务器失败: {}", addr))?;

    // 465 = 隐式 TLS；其余端口先明文，再 STARTTLS
    let implicit_tls = config.smtp_port == 465;
    let mut stream = if implicit_tls {
        let connector = TlsConnector::from(
            native_tls::TlsConnector::builder()
                .build()
                .context("创建 TLS 连接器失败")?,
        );
        SmtpStream::Tls(
            connector
                .connect(&config.smtp_server, tcp)
                .await
                .context("TLS 握手失败")?,
        )
    } else {
        SmtpStream::Plain(tcp)
    };

    // 服务器问候
    let (code, resp) = stream.read_response().await?;
    expect_code(code, &[220], &resp, "连接问候")?;

    // EHLO
    stream.write_line("EHLO casy.local").await?;
    let (code, resp) = stream.read_response().await?;
    expect_code(code, &[250], &resp, "EHLO")?;

    // STARTTLS（非隐式 TLS 端口）
    if !implicit_tls {
        stream.write_line("STARTTLS").await?;
        let (code, resp) = stream.read_response().await?;
        expect_code(code, &[220], &resp, "STARTTLS")?;
        stream = stream.upgrade_tls(&config.smtp_server).await?;
        // TLS 后重新 EHLO
        stream.write_line("EHLO casy.local").await?;
        let (code, resp) = stream.read_response().await?;
        expect_code(code, &[250], &resp, "EHLO(TLS)")?;
    }

    // AUTH PLAIN
    let auth_token =
        base64_encode(format!("\0{}\0{}", config.username, config.password).as_bytes());
    stream
        .write_line(&format!("AUTH PLAIN {}", auth_token))
        .await?;
    let (code, resp) = stream.read_response().await?;
    expect_code(code, &[235], &resp, "AUTH PLAIN")?;

    // 信封
    stream
        .write_line(&format!("MAIL FROM:<{}>", config.from_address))
        .await?;
    let (code, resp) = stream.read_response().await?;
    expect_code(code, &[250], &resp, "MAIL FROM")?;

    stream
        .write_line(&format!("RCPT TO:<{}>", to_email))
        .await?;
    let (code, resp) = stream.read_response().await?;
    expect_code(code, &[250, 251], &resp, "RCPT TO")?;

    // DATA
    stream.write_line("DATA").await?;
    let (code, resp) = stream.read_response().await?;
    expect_code(code, &[354], &resp, "DATA")?;

    // 归一化为 CRLF 并做 dot-stuffing（行首 '.' 需双写），以 "\r\n.\r\n" 结束
    let text = String::from_utf8_lossy(content);
    let normalized = text.replace("\r\n", "\n").replace('\n', "\r\n");
    let mut stuffed = String::with_capacity(normalized.len() + 64);
    for line in normalized.split("\r\n") {
        if line.starts_with('.') {
            stuffed.push('.');
        }
        stuffed.push_str(line);
        stuffed.push_str("\r\n");
    }
    stuffed.push_str(".\r\n");
    stream.write_all(stuffed.as_bytes()).await?;
    let (code, resp) = stream.read_response().await?;
    expect_code(code, &[250], &resp, "邮件数据")?;

    // QUIT
    stream.write_line("QUIT").await?;
    let _ = stream.read_response().await;

    log::info!("SMTP: 邮件已发送到 {}", to_email);
    Ok(())
}

/// 发送原始邮件（整体 30s 超时）
pub async fn send_raw_email(config: &SmtpConfig, to_email: &str, content: &[u8]) -> Result<()> {
    // 信封地址直接拼进 MAIL FROM / RCPT TO 命令，必须校验，防命令注入
    validate_email(to_email)?;
    validate_email(&config.from_address)?;
    log::info!(
        "SMTP: 发送邮件到 {}（服务器 {}:{}）",
        to_email,
        config.smtp_server,
        config.smtp_port
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(SMTP_TIMEOUT_SECS),
        smtp_session(config, to_email, content),
    )
    .await
    .map_err(|_| anyhow::anyhow!("SMTP 发送超时（{}s）", SMTP_TIMEOUT_SECS))?
}

/// Base64 编码
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// 生成日程提醒邮件（非 ICS 邀请，纯文本提醒）
// 预留：邮件提醒通道（R 级别邮件模板）
#[allow(dead_code)]
pub fn generate_reminder_email(event: &CalendarEvent, days_until: i64) -> (String, String) {
    let subject = if days_until == 0 {
        format!("今日日程: {}", event.summary)
    } else if days_until == 1 {
        format!("明日日程: {}", event.summary)
    } else {
        format!("{}天后日程: {}", days_until, event.summary)
    };

    let body = format!(
        "日程提醒\n\n\
         事项：{}\n\
         时间：{} - {}\n\
         {}\n\
         {}\n\n\
         距离现在还有 {} 天。",
        event.summary,
        event.dtstart.format("%Y-%m-%d %H:%M"),
        event.dtend.format("%Y-%m-%d %H:%M"),
        event
            .location
            .as_deref()
            .map(|l| format!("地点：{}", l))
            .unwrap_or_default(),
        event
            .description
            .as_deref()
            .map(|d| format!("说明：{}", d))
            .unwrap_or_default(),
        days_until,
    );

    (subject, body)
}

// ============================================================
// Tauri 命令
// ============================================================

/// 从 settings 表 + keychain 加载 SMTP 配置
///
/// 键名：smtp_host / smtp_port / smtp_user / smtp_pass
/// 密码优先走 keychain（service "casy-smtp"，account 为 smtp_user），
/// keychain 没有则回退 settings 表 smtp_pass；两者都无则报错。
fn load_smtp_config() -> Result<SmtpConfig> {
    let conn = crate::db::open_db()?;
    let get = |key: &str| -> Option<String> {
        crate::db::get_setting(&conn, key)
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
    };

    let host = get("smtp_host")
        .ok_or_else(|| anyhow::anyhow!("未配置 SMTP 服务器，请前往设置页配置（smtp_host）"))?;
    let user = get("smtp_user")
        .ok_or_else(|| anyhow::anyhow!("未配置 SMTP 用户名，请前往设置页配置（smtp_user）"))?;
    let port: u16 = get("smtp_port").and_then(|p| p.parse().ok()).unwrap_or(465);

    let password = crate::credentials::resolve_settings_secret(&conn,"smtp_pass")?
        .ok_or_else(|| anyhow::anyhow!("未配置 SMTP 密码，请前往设置页配置"))?;

    Ok(SmtpConfig {
        smtp_server: host,
        smtp_port: port,
        username: user.clone(),
        password,
        use_tls: true,
        from_address: user.clone(),
        from_name: user,
    })
}

/// 解析 ISO 时间（支持 RFC3339 与常见本地格式）
fn parse_start_time(start_iso: &str) -> Result<NaiveDateTime> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(start_iso) {
        return Ok(dt.naive_local());
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(start_iso, fmt) {
            return Ok(dt);
        }
    }
    Err(anyhow::anyhow!("无法解析开始时间: {}", start_iso))
}

/// 发送 ICS 日程邀请邮件
///
/// SMTP 配置从 settings 表读取（smtp_host/smtp_port/smtp_user/smtp_pass），
/// 密码优先走 keychain。未配置时返回友好错误，不 panic。
#[tauri::command]
pub async fn send_ics_invitation_cmd(
    to: String,
    subject: String,
    description: Option<String>,
    start_iso: String,
    duration_minutes: Option<i64>,
    alarm_minutes: Option<i64>,
) -> Result<String, String> {
    let config = crate::commands::run_blocking(load_smtp_config).await?;

    let dtstart = parse_start_time(&start_iso).map_err(|e| e.to_string())?;
    let dtend = dtstart + chrono::Duration::minutes(duration_minutes.unwrap_or(60));

    let event = CalendarEvent {
        uid: format!("{}@casy", crate::db::new_id()),
        summary: subject,
        description,
        location: None,
        dtstart,
        dtend,
        all_day: false,
        alarm_minutes,
    };

    send_ics_invitation(&config, &to, &to, &event)
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!("ICS 邀请已发送至 {}", to))
}

#[cfg(test)]
mod rfc_compliance_tests {
    use super::*;

    fn event() -> CalendarEvent {
        CalendarEvent {
            uid: "casy-uid-1".into(),
            summary: "案件提醒：答辩期限（北京市海淀区人民法院，案号（2026）京0108民初12345号）".into(),
            description: Some("请在开庭前提交答辩状与证据目录，注意举证期限为收到本通知之日起十五日内。".into()),
            location: Some("北京市海淀区人民法院第三法庭".into()),
            dtstart: NaiveDateTime::parse_from_str("2026-08-20 09:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            dtend: NaiveDateTime::parse_from_str("2026-08-20 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            all_day: false,
            alarm_minutes: Some(1440),
        }
    }

    fn config() -> SmtpConfig {
        SmtpConfig {
            smtp_server: "smtp.example.invalid".into(),
            smtp_port: 465,
            username: "user".into(),
            password: "pass".into(),
            use_tls: true,
            from_address: "casy@example.invalid".into(),
            from_name: "Casy".into(),
        }
    }

    /// RFC 2822 必需头：Date 可解析、Message-ID 形如 <uuid@domain>
    #[test]
    fn invitation_email_carries_date_and_message_id() {
        let email = build_invitation_email(&config(), "Casy", "收件人", "to@example.invalid", "日程邀请: 测试", &event());
        let headers = email.split("\r\n\r\n").next().unwrap();
        let date = headers.lines().find_map(|l| l.strip_prefix("Date: ")).expect("缺少 Date 头");
        assert!(chrono::DateTime::parse_from_rfc2822(date).is_ok(), "Date 头不是合法 RFC 2822 时间: {date}");
        let message_id = headers.lines().find_map(|l| l.strip_prefix("Message-ID: ")).expect("缺少 Message-ID 头");
        assert!(message_id.starts_with('<') && message_id.ends_with('>'), "Message-ID 格式不合法: {message_id}");
        let inner = &message_id[1..message_id.len() - 1];
        let (local, domain) = inner.split_once('@').expect("Message-ID 缺少 @domain");
        assert_eq!(local.len(), 36, "Message-ID 本地部分应为 UUID");
        assert_eq!(domain, "example.invalid");
    }

    /// ICS 合规：每行 ≤ 75 字节、续行以单空格开头、DTSTAMP/DTSTART/DTEND 均为 UTC Z
    #[test]
    fn ics_lines_are_folded_and_all_date_times_are_utc() {
        let ics = generate_ics(&event());
        for line in ics.split("\r\n") {
            assert!(line.len() <= 75, "ICS 行超过 75 字节: {line}");
        }
        let dtstamp = ics.lines().find(|l| l.starts_with("DTSTAMP:")).unwrap();
        assert!(dtstamp.ends_with('Z'), "DTSTAMP 必须是 UTC: {dtstamp}");
        assert!(chrono::NaiveDateTime::parse_from_str(&dtstamp["DTSTAMP:".len()..dtstamp.len() - 1], "%Y%m%dT%H%M%S").is_ok());
        // 与测试机时区无关：按本地时区换算后应与 DTSTART/DTEND 一致
        let expected_start = chrono::Local
            .from_local_datetime(&event().dtstart)
            .latest()
            .expect("测试时间应可映射到本地时区")
            .with_timezone(&chrono::Utc)
            .format("%Y%m%dT%H%M%SZ")
            .to_string();
        let expected_end = chrono::Local
            .from_local_datetime(&event().dtend)
            .latest()
            .expect("测试时间应可映射到本地时区")
            .with_timezone(&chrono::Utc)
            .format("%Y%m%dT%H%M%SZ")
            .to_string();
        assert!(ics.contains(&format!("DTSTART:{expected_start}")), "DTSTART 应为 UTC Z 形式: {ics}");
        assert!(ics.contains(&format!("DTEND:{expected_end}")), "DTEND 应为 UTC Z 形式: {ics}");
        // 折行后续行必须能被 RFC 5545 反折行还原（去掉 CRLF + 单个前导空格）
        let unfolded = ics.replace("\r\n ", "");
        assert!(unfolded.contains("SUMMARY:案件提醒：答辩期限"));
        assert!(unfolded.contains("DESCRIPTION:请在开庭前提交答辩状与证据目录"));
        // 全天事件用 DATE 值
        let mut all_day = event();
        all_day.all_day = true;
        let all_day_ics = generate_ics(&all_day);
        assert!(all_day_ics.contains("DTSTART;VALUE=DATE:20260820"));
    }
}

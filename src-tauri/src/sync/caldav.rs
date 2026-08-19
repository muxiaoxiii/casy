//! CalDAV 客户端（设计哲学 §11.2 M1）
//!
//! 把提醒固化为日历事件，通过 CalDAV 同步到用户已有的
//! Google / Apple(iCloud) / Outlook 日历，由日历服务商负责准时推送。
//!
//! 最小闭环：
//! - test_connection: OPTIONS + PROPFIND(Depth: 0) 探测
//! - upsert_event:    PUT {caldav_url}/{uid}.ics（同 UID 天然幂等）
//! - delete_event:    DELETE
//! - get_event_etag:  HEAD（对账 / delivery_unknown 判定）
//!
//! 配置从 settings 表读取（caldav_url / caldav_user），
//! 密码优先走 keychain（service "casy-caldav"），回退 settings 表 caldav_pass。

use anyhow::Result;
use chrono::NaiveDateTime;
use rusqlite::Connection;
use reqwest::Client;
use std::time::Duration;

/// CalDAV 请求超时
const CALDAV_TIMEOUT_SECS: u64 = 30;

/// CalDAV 结构化错误（区分认证失败 / 网络 / 4xx / 5xx / 结果不明）
#[derive(Debug, thiserror::Error)]
pub enum CalDavError {
    /// 认证失败（401/403）
    #[error("CalDAV 认证失败: {0}")]
    Auth(String),
    /// 网络/连接错误（请求可能未到达服务器）
    #[error("CalDAV 网络错误: {0}")]
    Network(String),
    /// 客户端错误（4xx，除认证外）
    #[error("CalDAV 请求被拒绝（{0}）: {1}")]
    Client(u16, String),
    /// 服务器错误（5xx）
    #[error("CalDAV 服务器错误（{0}）: {1}")]
    Server(u16, String),
    /// 结果不明（超时等，事件可能已写入 — 重试前必须先 get_event_etag 对账）
    #[error("CalDAV 投递结果不明: {0}")]
    Unknown(String),
}

impl CalDavError {
    /// 投递结果是否不明（网络错误/超时：PUT 可能已到达服务器）
    /// 为 true 时不允许盲目重 PUT，必须先对账
    pub fn is_delivery_unknown(&self) -> bool {
        matches!(self, Self::Network(_) | Self::Unknown(_))
    }
}

/// 从 reqwest 错误分类
fn classify_reqwest(e: reqwest::Error) -> CalDavError {
    if e.is_timeout() {
        CalDavError::Unknown(format!("请求超时（{}s）: {}", CALDAV_TIMEOUT_SECS, e))
    } else if e.is_connect() || e.is_request() {
        CalDavError::Network(e.to_string())
    } else {
        CalDavError::Network(e.to_string())
    }
}

/// 按 HTTP 状态码分类
fn classify_status(status: reqwest::StatusCode, context: &str) -> CalDavError {
    let code = status.as_u16();
    if code == 401 || code == 403 {
        CalDavError::Auth(format!("{} (HTTP {})", context, code))
    } else if (400..500).contains(&code) {
        CalDavError::Client(code, context.to_string())
    } else {
        CalDavError::Server(code, context.to_string())
    }
}

/// CalDAV 配置
#[derive(Debug, Clone)]
pub struct CalDavConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}

/// 日历同步是否启用（settings.calendar_sync_enabled == 'true' 且已配置 URL）
pub fn calendar_sync_enabled(conn: &Connection) -> bool {
    let enabled = crate::db::get_setting(conn, "calendar_sync_enabled")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);
    if !enabled {
        return false;
    }
    crate::db::get_setting(conn, "caldav_url")
        .ok()
        .flatten()
        .map(|u| !u.trim().is_empty())
        .unwrap_or(false)
}

/// 从 settings + keychain 加载 CalDAV 配置
///
/// 未配置（缺 URL / 用户名 / 密码）时返回 Ok(None)。
pub fn load_caldav_config(conn: &Connection) -> Result<Option<CalDavConfig>> {
    let get = |key: &str| -> Option<String> {
        crate::db::get_setting(conn, key)
            .ok()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let Some(url) = get("caldav_url") else {
        return Ok(None);
    };
    let Some(user) = get("caldav_user") else {
        return Ok(None);
    };

    // 密码：优先 keychain（service "casy-caldav"，account 为 caldav_user），回退 settings 表
    let password = crate::credentials::get_credential(
        crate::credentials::CredentialType::CaldavPassword,
        &user,
    )
    .ok()
    .flatten()
    .or_else(|| get("caldav_pass"));

    let Some(password) = password else {
        return Ok(None);
    };

    Ok(Some(CalDavConfig { url, user, password }))
}

/// CalDAV 客户端
pub struct CalDavClient {
    /// 日历集合 URL（以 '/' 结尾）
    base_url: String,
    username: String,
    password: String,
    client: Client,
}

impl CalDavClient {
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(CALDAV_TIMEOUT_SECS))
            .timeout(Duration::from_secs(CALDAV_TIMEOUT_SECS))
            .build()?;
        let mut base = base_url.trim().to_string();
        if !base.ends_with('/') {
            base.push('/');
        }
        Ok(Self {
            base_url: base,
            username: username.to_string(),
            password: password.to_string(),
            client,
        })
    }

    pub fn from_config(config: &CalDavConfig) -> Result<Self> {
        Self::new(&config.url, &config.user, &config.password)
    }

    /// 事件资源 URL：{caldav_url}/{uid}.ics
    fn event_url(&self, uid: &str) -> String {
        format!("{}{}.ics", self.base_url, uid)
    }

    /// 测试连接：OPTIONS 探测 DAV 能力，再 PROPFIND(Depth: 0) 验证认证
    pub async fn test_connection(&self) -> std::result::Result<String, CalDavError> {
        // 1. OPTIONS — 验证服务器可达且声明 DAV 支持
        let resp = self
            .client
            .request(reqwest::Method::OPTIONS, &self.base_url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(classify_reqwest)?;

        let status = resp.status();
        if !status.is_success() {
            return Err(classify_status(status, "OPTIONS 探测失败"));
        }
        let dav = resp
            .headers()
            .get("dav")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // 2. PROPFIND Depth:0 — 验证认证通过且资源是日历集合
        let propfind = reqwest::Method::from_bytes(b"PROPFIND")
            .map_err(|e| CalDavError::Client(0, e.to_string()))?;
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<propfind xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <prop>
    <resourcetype/>
    <displayname/>
  </prop>
</propfind>"#;
        let resp = self
            .client
            .request(propfind, &self.base_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "0")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .await
            .map_err(classify_reqwest)?;

        let status = resp.status();
        // 207 Multi-Status 或 200 均视为成功
        if status.as_u16() != 207 && !status.is_success() {
            return Err(classify_status(status, "PROPFIND 验证失败"));
        }

        Ok(format!(
            "连接成功（DAV: {}）",
            if dav.is_empty() { "未声明".to_string() } else { dav }
        ))
    }

    /// 创建/覆盖日历事件（PUT 同 UID 天然幂等），返回服务端 ETag
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_event(
        &self,
        uid: &str,
        summary: &str,
        description: &str,
        dtstart: NaiveDateTime,
        duration_minutes: i64,
        alarm_minutes: i64,
    ) -> std::result::Result<Option<String>, CalDavError> {
        let ics = build_reminder_ics(uid, summary, description, dtstart, duration_minutes, alarm_minutes);

        let resp = self
            .client
            .put(self.event_url(uid))
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .body(ics)
            .send()
            .await
            .map_err(classify_reqwest)?;

        let status = resp.status();
        // 201 Created / 204 No Content / 200 OK 均为成功
        if !status.is_success() {
            return Err(classify_status(status, "PUT 事件失败"));
        }

        Ok(resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()))
    }

    /// 删除日历事件（404 视为已删除，幂等）
    /// 任务完成/删除、期限失效后清理日历事件（commands::caldav::cancel_jobs_for_entity）
    pub async fn delete_event(&self, uid: &str) -> std::result::Result<(), CalDavError> {
        let resp = self
            .client
            .delete(self.event_url(uid))
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(classify_reqwest)?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(());
        }
        if !status.is_success() {
            return Err(classify_status(status, "DELETE 事件失败"));
        }
        Ok(())
    }

    /// 查询事件 ETag（对账用）：存在返回 Some(etag)，不存在返回 None
    pub async fn get_event_etag(
        &self,
        uid: &str,
    ) -> std::result::Result<Option<String>, CalDavError> {
        let resp = self
            .client
            .head(self.event_url(uid))
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(classify_reqwest)?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(classify_status(status, "HEAD 事件失败"));
        }

        Ok(Some(
            resp.headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string(),
        ))
    }
}

// ============================================================
// ICS 生成
// ============================================================

/// 生成提醒事件的 iCalendar body（含 VALARM）
///
/// 与 email/smtp.rs 的 generate_ics 不同：不带 METHOD:REQUEST
/// （那是邮件邀请语义；CalDAV PUT 的是纯日历对象）。
/// DTSTAMP 用 UTC；DTSTART/DTEND 用本地浮动时间（与现有 smtp.rs 一致）。
fn build_reminder_ics(
    uid: &str,
    summary: &str,
    description: &str,
    dtstart: NaiveDateTime,
    duration_minutes: i64,
    alarm_minutes: i64,
) -> String {
    let dtend = dtstart + chrono::Duration::minutes(duration_minutes.max(1));
    let dtstamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");

    format!(
        "BEGIN:VCALENDAR\r\n\
         VERSION:2.0\r\n\
         PRODID:-//Casy//Casy Calendar//CN\r\n\
         CALSCALE:GREGORIAN\r\n\
         BEGIN:VEVENT\r\n\
         UID:{uid}@casy.local\r\n\
         DTSTAMP:{dtstamp}\r\n\
         DTSTART:{dtstart}\r\n\
         DTEND:{dtend}\r\n\
         SUMMARY:{summary}\r\n\
         DESCRIPTION:{description}\r\n\
         STATUS:CONFIRMED\r\n\
         BEGIN:VALARM\r\n\
         TRIGGER:-PT{alarm}M\r\n\
         ACTION:DISPLAY\r\n\
         DESCRIPTION:{summary}\r\n\
         END:VALARM\r\n\
         END:VEVENT\r\n\
         END:VCALENDAR\r\n",
        uid = escape_ics(uid),
        dtstamp = dtstamp,
        dtstart = dtstart.format("%Y%m%dT%H%M%S"),
        dtend = dtend.format("%Y%m%dT%H%M%S"),
        summary = escape_ics(summary),
        description = escape_ics(description),
        alarm = alarm_minutes.max(0),
    )
}

/// 转义 ICS 特殊字符
fn escape_ics(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_reminder_ics() {
        let ics = build_reminder_ics(
            "job-123",
            "案件提醒：答辩期限",
            "截止：2026-08-20",
            NaiveDateTime::parse_from_str("2026-08-20 09:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            30,
            1440,
        );
        assert!(ics.contains("UID:job-123@casy.local"));
        assert!(ics.contains("DTSTART:20260820T090000"));
        assert!(ics.contains("DTEND:20260820T093000"));
        assert!(ics.contains("TRIGGER:-PT1440M"));
        assert!(ics.contains("SUMMARY:案件提醒：答辩期限"));
        assert!(!ics.contains("METHOD:REQUEST"));
    }

    #[test]
    fn test_delivery_unknown_classification() {
        assert!(CalDavError::Network("x".into()).is_delivery_unknown());
        assert!(CalDavError::Unknown("x".into()).is_delivery_unknown());
        assert!(!CalDavError::Auth("x".into()).is_delivery_unknown());
        assert!(!CalDavError::Client(404, "x".into()).is_delivery_unknown());
        assert!(!CalDavError::Server(500, "x".into()).is_delivery_unknown());
    }
}

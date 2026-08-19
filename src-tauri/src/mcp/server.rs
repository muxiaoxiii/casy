//! MCP HTTP JSON-RPC Server（设计哲学 §11.11）
//!
//! 极简 HTTP/1.1 + JSON-RPC 2.0 服务，绑定 127.0.0.1:37877（仅回环，不对外）。
//! 手写最小 HTTP 解析（请求行 + header + Content-Length body），不引框架。
//! 写操作工具（case_create_task / task_update_status）在此通道下不直接执行：
//! 进入 mcp_pending_writes 待确认队列，返回 pending_confirmation，
//! 由用户在应用内确认后才真正执行（确认链路见 commands 模块）。

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpServer};

/// 监听地址（仅回环）
pub const MCP_BIND_ADDR: &str = "127.0.0.1:37877";

/// 监听端口（供 get_mcp_server_info 返回）
pub const MCP_PORT: u16 = 37877;

/// 单个连接的处理超时，避免慢连接占用资源
const CONN_TIMEOUT_SECS: u64 = 30;
/// 请求头/请求体大小上限
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;

/// 鉴权 token（内存缓存；settings 表 key='mcp_auth_token' 持久化，供用户配置外部工具）
static AUTH_TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// 获取或生成 MCP 鉴权 token
///
/// 优先读 settings 表 mcp_auth_token；没有则生成随机 token（uuid v4）并写回 settings，
/// 同时缓存到内存。数据库不可用时退回一次性随机 token（仅内存，不持久化）。
pub fn auth_token() -> String {
    if let Some(t) = AUTH_TOKEN.get() {
        return t.clone();
    }

    let token = (|| -> Option<String> {
        let conn = crate::db::open_db().ok()?;
        if let Some(existing) = crate::db::get_setting(&conn, "mcp_auth_token").ok().flatten() {
            if !existing.trim().is_empty() {
                return Some(existing);
            }
        }
        let fresh = uuid::Uuid::new_v4().to_string();
        if let Err(e) = crate::db::set_setting(&conn, "mcp_auth_token", &fresh) {
            log::warn!("MCP token 写入 settings 失败（仅内存生效）: {}", e);
        }
        Some(fresh)
    })()
    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // 并发首次设置时只会有一个生效，两者内容同源无妨
    let _ = AUTH_TOKEN.set(token.clone());
    token
}

/// 启动 MCP HTTP server。绑定失败返回 Err（由调用方记日志，不 panic）。
pub async fn run() -> Result<()> {
    let token = auth_token();
    let listener = TcpListener::bind(MCP_BIND_ADDR)
        .await
        .with_context(|| format!("绑定 {} 失败（端口可能被占用）", MCP_BIND_ADDR))?;
    log::info!(
        "MCP server listening on {}（已启用 Bearer token 鉴权，token 见 settings.mcp_auth_token 或 get_mcp_server_info）",
        MCP_BIND_ADDR
    );

    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                let token = token.clone();
                tokio::spawn(async move {
                    let result = tokio::time::timeout(
                        std::time::Duration::from_secs(CONN_TIMEOUT_SECS),
                        handle_connection(stream, &token),
                    )
                    .await;
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => log::debug!("MCP 连接处理结束: {}", e),
                        Err(_) => log::debug!("MCP 连接超时关闭"),
                    }
                });
            }
            Err(e) => {
                log::warn!("MCP accept 失败: {}", e);
            }
        }
    }
}

/// 处理单个 HTTP 连接（一请求一响应，Connection: close）
async fn handle_connection(stream: TcpStream, token: &str) -> Result<()> {
    // 每个连接独立的 server 状态（initialized 标志按连接维护）
    handle_connection_with(stream, McpServer::new_readonly(), token).await
}

/// 校验 Authorization 头：必须是 `Bearer <token>`，不匹配返回 false
fn check_auth(header_text: &str, expected_token: &str) -> bool {
    for line in header_text.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("authorization") {
                return v.trim() == format!("Bearer {}", expected_token);
            }
        }
    }
    false
}

/// 处理单个 HTTP 连接（注入 server 实例与期望 token，测试可替换写队列提交器）
async fn handle_connection_with(mut stream: TcpStream, mut server: McpServer, expected_token: &str) -> Result<()> {
    // ── 读取请求头（直到 \r\n\r\n） ──
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            anyhow::bail!("连接已关闭");
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > MAX_HEADER_BYTES {
            write_response(&mut stream, 431, "Request Header Fields Too Large", b"")
                .await?;
            return Ok(());
        }
    };

    let header_text = String::from_utf8_lossy(&buf[..header_end]).to_string();

    // ── 鉴权：所有请求必须携带 Authorization: Bearer <token>，否则 401 ──
    if !check_auth(&header_text, expected_token) {
        write_response(&mut stream, 401, "Unauthorized", b"Unauthorized").await?;
        return Ok(());
    }

    let mut lines = header_text.lines();
    let request_line = lines.next().unwrap_or("");
    let method = request_line.split_whitespace().next().unwrap_or("");

    if method != "POST" {
        write_response(&mut stream, 405, "Method Not Allowed", b"Method Not Allowed").await?;
        return Ok(());
    }

    let mut content_length = 0usize;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_length = v.trim().parse().unwrap_or(0);
            }
        }
    }
    if content_length > MAX_BODY_BYTES {
        write_response(&mut stream, 413, "Payload Too Large", b"Payload Too Large").await?;
        return Ok(());
    }

    // ── 读取 body ──
    let mut body: Vec<u8> = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    // ── 解析并执行 JSON-RPC ──
    match serde_json::from_slice::<JsonRpcRequest>(&body) {
        Ok(req) => {
            let is_notification = req.id.is_none();
            let response = server.handle_request(req).await;

            if is_notification {
                // JSON-RPC 通知不需要响应体
                write_response(&mut stream, 202, "Accepted", b"").await?;
            } else {
                let payload = serde_json::to_vec(&response).unwrap_or_default();
                write_json(&mut stream, 200, "OK", &payload).await?;
            }
        }
        Err(_) => {
            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: "Parse error: 请求体不是合法的 JSON-RPC 2.0 请求".to_string(),
                    data: None,
                }),
            };
            let payload = serde_json::to_vec(&response).unwrap_or_default();
            write_json(&mut stream, 200, "OK", &payload).await?;
        }
    }

    Ok(())
}

/// 写 JSON 响应（Content-Type: application/json）
async fn write_json(stream: &mut TcpStream, code: u16, reason: &str, body: &[u8]) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        code,
        reason,
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

/// 写纯文本/空响应
async fn write_response(stream: &mut TcpStream, code: u16, reason: &str, body: &[u8]) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        code,
        reason,
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

/// 获取 MCP server 信息（供设置页展示 / 用户配置外部工具）
///
/// 返回 { port, token, enabled }；token 即 HTTP 鉴权所需的 Bearer token
///（持久化在 settings.mcp_auth_token，外部 MCP 客户端请求头需带 Authorization: Bearer <token>）。
#[tauri::command]
pub async fn get_mcp_server_info() -> Result<serde_json::Value, String> {
    crate::commands::run_blocking(|| {
        let conn = crate::db::open_db()?;
        let enabled = crate::db::get_setting(&conn, "mcp_server_enabled")
            .ok()
            .flatten()
            .map(|v| v != "false")
            .unwrap_or(true);
        let token = auth_token();
        Ok(serde_json::json!({
            "port": MCP_PORT,
            "token": token,
            "enabled": enabled,
        }))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// 测试用鉴权 token
    const TEST_TOKEN: &str = "test-token";

    /// 在临时端口起一个连接处理任务，发送一个 HTTP 请求并读回完整响应
    async fn roundtrip(request: &[u8]) -> String {
        roundtrip_with_server(McpServer::new_readonly(), request).await
    }

    /// 同上，但注入自定义 server（测试写队列替身用）
    async fn roundtrip_with_server(server: McpServer, request: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection_with(stream, server, TEST_TOKEN).await.unwrap();
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(request).await.unwrap();
        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await.unwrap();
        server_task.await.unwrap();
        String::from_utf8_lossy(&buf).to_string()
    }

    /// 鉴权头（既有测试统一携带）
    fn auth() -> String {
        format!("Authorization: Bearer {}\r\n", TEST_TOKEN)
    }

    #[tokio::test]
    async fn ping_returns_jsonrpc_result() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let req = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\n{}Content-Length: {}\r\n\r\n{}",
            auth(),
            body.len(),
            body
        );
        let resp = roundtrip(req.as_bytes()).await;
        assert!(resp.starts_with("HTTP/1.1 200 OK"), "响应行错误: {}", resp);
        assert!(resp.contains(r#""jsonrpc":"2.0""#), "缺少 jsonrpc 字段: {}", resp);
        assert!(resp.contains(r#""id":1"#), "缺少 id: {}", resp);
        assert!(resp.contains(r#""result""#), "ping 应返回 result: {}", resp);
    }

    #[tokio::test]
    async fn invalid_json_returns_parse_error() {
        let body = "not json";
        let req = format!(
            "POST / HTTP/1.1\r\n{}Content-Length: {}\r\n\r\n{}",
            auth(),
            body.len(),
            body
        );
        let resp = roundtrip(req.as_bytes()).await;
        assert!(resp.contains(r#""code":-32700"#), "应返回 -32700: {}", resp);
    }

    #[tokio::test]
    async fn get_method_rejected() {
        let req = format!("GET / HTTP/1.1\r\nHost: x\r\n{}\r\n", auth());
        let resp = roundtrip(req.as_bytes()).await;
        assert!(resp.starts_with("HTTP/1.1 405"), "GET 应返回 405: {}", resp);
    }

    #[tokio::test]
    async fn write_tool_queued_for_confirmation_in_readonly_mode() {
        let body = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"task_update_status","arguments":{"task_id":"t1","action":"complete"}}}"#;
        let req = format!(
            "POST / HTTP/1.1\r\n{}Content-Length: {}\r\n\r\n{}",
            auth(),
            body.len(),
            body
        );
        // 注入写队列替身，避免测试触碰真实数据库
        let server = McpServer::new_readonly_with_submitter(|_tool, _args| {
            Ok("test-write-id".to_string())
        });
        let resp = roundtrip_with_server(server, req.as_bytes()).await;
        assert!(resp.contains(r#""result""#), "写工具应返回正常响应: {}", resp);
        assert!(
            resp.contains("pending_confirmation"),
            "写工具应进入待确认队列: {}",
            resp
        );
        assert!(resp.contains("test-write-id"), "应返回 write_id: {}", resp);
        assert!(!resp.contains("-32001"), "不应再返回 -32001 拒绝: {}", resp);
    }

    #[tokio::test]
    async fn notification_returns_202() {
        let body = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req = format!(
            "POST / HTTP/1.1\r\n{}Content-Length: {}\r\n\r\n{}",
            auth(),
            body.len(),
            body
        );
        let resp = roundtrip(req.as_bytes()).await;
        assert!(resp.starts_with("HTTP/1.1 202"), "通知应返回 202: {}", resp);
    }

    #[tokio::test]
    async fn missing_token_returns_401() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let req = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let resp = roundtrip(req.as_bytes()).await;
        assert!(
            resp.starts_with("HTTP/1.1 401"),
            "无 token 应返回 401: {}",
            resp
        );
    }

    #[tokio::test]
    async fn wrong_token_returns_401() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let req = format!(
            "POST /mcp HTTP/1.1\r\nAuthorization: Bearer wrong-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let resp = roundtrip(req.as_bytes()).await;
        assert!(
            resp.starts_with("HTTP/1.1 401"),
            "错误 token 应返回 401: {}",
            resp
        );
    }
}


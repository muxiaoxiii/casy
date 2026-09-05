use casy_lib::ai::{
    profiles::{self, AiProfile, AiProfiles},
    AiBackend, ChatMessage, OllamaBackend, OpenAiBackend,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};

fn profile() -> AiProfile {
    AiProfile {
        id: "one".into(),
        name: "Local test".into(),
        mode: "openai".into(),
        api_url: "http://127.0.0.1:9999/v1/".into(),
        model: "custom-model".into(),
        has_api_key: false,
        api_key: None,
    }
}

#[test]
fn profiles_validate_identity_and_endpoint() {
    let mut state = AiProfiles {
        profiles: vec![profile()],
        active_id: Some("one".into()),
        ..Default::default()
    };
    profiles::validate(&mut state).unwrap();
    assert_eq!(state.profiles[0].api_url, "http://127.0.0.1:9999/v1");
    for url in [
        "https://user:pass@example.com/v1",
        "file:///tmp/model",
        "https://example.com/v1?key=x",
        "https://example.com/v1/chat/completions",
        "http://example.com/v1",
    ] {
        let mut invalid = state.clone();
        invalid.profiles[0].api_url = url.into();
        assert!(profiles::validate(&mut invalid).is_err());
    }
    state.profiles.push(profile());
    assert!(profiles::validate(&mut state).is_err());
    state.profiles.pop();
    state.active_id = Some("missing".into());
    assert!(profiles::validate(&mut state).is_err());
}

#[test]
fn keyless_profiles_roundtrip_and_invalid_save_preserves_state() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .unwrap();
    let original = AiProfiles {
        profiles: vec![profile()],
        active_id: Some("one".into()),
        ..Default::default()
    };
    let saved = profiles::save(&mut conn, original).unwrap();
    assert!(!saved.profiles[0].has_api_key);
    assert_eq!(
        profiles::resolve(&conn, None).unwrap().model.as_deref(),
        Some("custom-model")
    );
    let mut invalid = saved.clone();
    invalid.profiles[0].model.clear();
    assert!(profiles::save(&mut conn, invalid).is_err());
    assert_eq!(
        profiles::read(&conn).unwrap().profiles[0].model,
        saved.profiles[0].model
    );
    assert!(profiles::resolve(&conn, Some("missing")).is_err());
}

fn server(status: &str, body: &str) -> (String, mpsc::Receiver<String>) {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        casy_lib::db::enable_test_mode();
        casy_lib::db::init_db(&casy_lib::db::open_db().unwrap()).unwrap();
    });
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = vec![];
        loop {
            let mut buf = [0; 1024];
            let n = stream.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&bytes);
            if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                let len = headers
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if body.len() >= len {
                    break;
                }
            }
        }
        stream.write_all(response.as_bytes()).unwrap();
        tx.send(String::from_utf8(bytes).unwrap()).unwrap();
    });
    (url, rx)
}

#[tokio::test]
async fn compatible_api_sends_model_key_and_messages() {
    let (url, rx) = server(
        "200 OK",
        r#"{"choices":[{"message":{"content":"API response"}}]}"#,
    );
    let backend = OpenAiBackend::new(
        &format!("{url}/v1/"),
        "synthetic-test-key",
        "custom-legal-model",
    );
    let messages = vec![ChatMessage {
        role: "user".into(),
        content: "Synthetic question".into(),
    }];
    assert_eq!(
        backend.chat_messages(&messages).await.unwrap(),
        "API response"
    );
    let request = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request.starts_with("POST /v1/chat/completions "));
    assert!(request
        .to_lowercase()
        .contains("authorization: bearer synthetic-test-key"));
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["model"], "custom-legal-model");
    assert_eq!(body["messages"][0]["content"], "Synthetic question");
}

#[tokio::test]
async fn local_ollama_does_not_require_a_key() {
    let (url, rx) = server("200 OK", r#"{"message":{"content":"Local response"}}"#);
    assert_eq!(
        OllamaBackend::new(&url, "local-model")
            .chat_completion("", "Test")
            .await
            .unwrap(),
        "Local response"
    );
    let request = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request.starts_with("POST /api/chat "));
    assert!(!request.to_lowercase().contains("authorization:"));
}

#[tokio::test]
async fn empty_and_http_error_responses_are_not_success() {
    for (status, body) in [
        ("200 OK", "{}"),
        ("200 OK", r#"{"choices":[{"message":{"content":" "}}]}"#),
        ("401 Unauthorized", "secret-echo"),
    ] {
        let (url, rx) = server(status, body);
        let error = OpenAiBackend::new(&url, "secret-echo", "model")
            .chat_completion("", "test")
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains("secret-echo"));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
}

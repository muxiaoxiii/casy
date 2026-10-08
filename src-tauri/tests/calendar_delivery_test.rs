use casy_lib::{commands::caldav, db};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};

#[tokio::test]
async fn retry_keeps_actual_time_and_cancelled_remote_deletion_survives_failure() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/calendar/", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    let server = std::thread::spawn(move || {
        for status in ["201 Created", "503 Service Unavailable", "204 No Content"] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
                let text = String::from_utf8_lossy(&bytes);
                if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break;
                    }
                }
            }
            sender.send(String::from_utf8(bytes).unwrap()).unwrap();
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nETag: \"test\"\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
        }
    });
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    for (key, value) in [
        ("calendar_sync_enabled", "true"),
        ("caldav_url", url.as_str()),
        ("caldav_user", "isolated-delivery-test"),
        ("caldav_pass", "synthetic-password"),
    ] {
        db::set_setting(&conn, key, value).unwrap();
    }
    let id = db::new_id();
    conn.execute("INSERT INTO reminder_jobs(id,entity_type,entity_id,channel,executor,scheduled_at,due_snapshot,status,attempts,next_attempt_at) VALUES(?1,'task',?1,'calendar','calendar','2026-09-30','2026-10-01T14:45:00','sync_failed',5,'2000-01-01')",[&id]).unwrap();
    drop(conn);
    let (first, second) = tokio::join!(
        caldav::sync_reminders_to_calendar(),
        caldav::sync_reminders_to_calendar()
    );
    assert!(first.is_ok() && second.is_ok());
    let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request.starts_with("PUT "));
    // DTSTART 现为 UTC（带 Z）：按测试机本地时区换算后期望值一致
    let expected_utc = chrono::TimeZone::from_local_datetime(
        &chrono::Local,
        &chrono::NaiveDateTime::parse_from_str("2026-10-01T14:45:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
    )
    .unwrap()
    .with_timezone(&chrono::Utc)
    .format("%Y%m%dT%H%M%SZ")
    .to_string();
    assert!(
        request.contains(&expected_utc),
        "PUT 请求应包含 UTC 时间 {expected_utc}: {request}"
    );
    assert_eq!(
        caldav::cancel_jobs_for_entity("task", &id).await.unwrap(),
        1
    );
    assert!(receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .starts_with("DELETE "));
    let conn = db::open_db().unwrap();
    let (status, uid, error): (String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT status,calendar_event_id,last_error FROM reminder_jobs WHERE id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(status, "cancelled");
    assert_eq!(uid.as_deref(), Some(id.as_str()));
    assert!(error.is_some());
    conn.execute(
        "UPDATE reminder_jobs SET next_attempt_at='2000-01-01' WHERE id=?1",
        [&id],
    )
    .unwrap();
    drop(conn);
    caldav::sync_reminders_to_calendar().await.unwrap();
    assert!(receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .starts_with("DELETE "));
    server.join().unwrap();
    let conn = db::open_db().unwrap();
    let pending: bool = conn
        .query_row(
            "SELECT calendar_event_id IS NOT NULL FROM reminder_jobs WHERE id=?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!pending);
}

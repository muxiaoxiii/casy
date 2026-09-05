use anyhow::{ensure, Context, Result};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

const USAGE_KEY: &str = "ai_usage_v1";

#[derive(Default, Deserialize, Serialize)]
struct Counter {
    date: String,
    used: u64,
}

fn read(conn: &Connection, date: &str) -> Result<Counter> {
    let stored: Option<String> = conn.query_row("SELECT value FROM settings WHERE key=?1", [USAGE_KEY], |row| row.get(0)).optional()?;
    let counter: Counter = stored
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .context("AI 调用计数损坏，无法确认剩余额度")?
        .unwrap_or_default();
    Ok(if counter.date == date { counter } else { Counter { date: date.into(), used: 0 } })
}

fn reserve(conn: &mut Connection, date: &str) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let limit = super::profiles::read(&tx)?.daily_limit as u64;
    let mut counter = read(&tx, date)?;
    ensure!(limit == 0 || counter.used < limit, "AI 调用已达每日限额 ({}/{})", counter.used, limit);
    counter.used = counter.used.checked_add(1).context("AI 调用计数超出范围")?;
    tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [USAGE_KEY, &serde_json::to_string(&counter)?])?;
    tx.commit()?;
    Ok(())
}

pub async fn current() -> Result<(u64, u64)> {
    tokio::task::spawn_blocking(|| {
        let conn = crate::db::open_db()?;
        let date = crate::db::today();
        Ok((read(&conn, &date)?.used, super::profiles::read(&conn)?.daily_limit as u64))
    }).await?
}

// Count dispatched attempts, including failures: a lost response may still have incurred cost.
pub async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
    let (client, request) = request.build_split();
    let request = request.context("AI 请求参数无效")?;
    tokio::task::spawn_blocking(|| reserve(&mut crate::db::open_db()?, &crate::db::today())).await??;
    client.execute(request).await.map_err(|_| anyhow::anyhow!("AI 接口连接失败或超时"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_counter_respects_dates_limits_and_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.db");
        let mut conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO settings VALUES('ai_daily_limit','2');").unwrap();
        reserve(&mut conn, "2026-09-06").unwrap();
        drop(conn);
        let mut conn = Connection::open(&path).unwrap();
        reserve(&mut conn, "2026-09-06").unwrap();
        assert!(reserve(&mut conn, "2026-09-06").is_err());
        assert_eq!(read(&conn, "2026-09-06").unwrap().used, 2);
        reserve(&mut conn, "2026-09-07").unwrap();
        assert_eq!(read(&conn, "2026-09-07").unwrap().used, 1);
        conn.execute("UPDATE settings SET value='0' WHERE key='ai_daily_limit'", []).unwrap();
        for _ in 0..3 { reserve(&mut conn, "2026-09-07").unwrap(); }
        assert_eq!(read(&conn, "2026-09-07").unwrap().used, 4);
        conn.execute("UPDATE settings SET value=x'00' WHERE key=?1", [USAGE_KEY]).unwrap();
        assert!(reserve(&mut conn, "2026-09-07").is_err());
        conn.execute("UPDATE settings SET value='broken' WHERE key=?1", [USAGE_KEY]).unwrap();
        assert!(reserve(&mut conn, "2026-09-07").is_err());
    }

    #[test]
    fn concurrent_connections_cannot_exceed_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.db");
        Connection::open(&path).unwrap().execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO settings VALUES('ai_daily_limit','3');").unwrap();
        let workers: Vec<_> = (0..12).map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut conn = Connection::open(path).unwrap();
                conn.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
                reserve(&mut conn, "2026-09-06").is_ok()
            })
        }).collect();
        assert_eq!(workers.into_iter().map(|worker| usize::from(worker.join().unwrap())).sum::<usize>(), 3);
    }
}

use casy_lib::{
    ai::{
        embeddings::{EmbeddingClient, EmbeddingPlan},
        profiles::{self, AiProfile, AiProfiles, EmbeddingSettings},
    },
    db::{self, knowledge_index as index, search},
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

struct Server {
    url: String,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    mode: Arc<AtomicUsize>,
    stop: Arc<AtomicUsize>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mode = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicUsize::new(0));
        let (log, behavior, stopped) = (requests.clone(), mode.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while stopped.load(Ordering::SeqCst) == 0 {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let (headers, body) = loop {
                    let mut buf = [0; 4096];
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&bytes);
                    if let Some((header, body)) = text.split_once("\r\n\r\n") {
                        let len = header
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|n| n.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        if body.len() >= len {
                            break (
                                header.to_owned(),
                                serde_json::from_str::<Value>(body).unwrap(),
                            );
                        }
                    }
                };
                log.lock().unwrap().push((headers.clone(), body.clone()));
                let behavior = behavior.load(Ordering::SeqCst);
                if behavior == 3 {
                    db::open_db().unwrap().execute("UPDATE knowledge_items SET content='修订后仅保留新事实' WHERE id='long'", []).unwrap();
                }
                if behavior == 4 {
                    db::open_db().unwrap().execute("UPDATE knowledge_index_jobs SET status='cancelled' WHERE status='running'", []).unwrap();
                }
                let vectors: Vec<Value> = body["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| {
                        if behavior == 2 {
                            json!([1, 0, 0])
                        } else if s.as_str().unwrap().contains("赔偿金额")
                            || s.as_str().unwrap().contains("损失数额")
                        {
                            json!([1, 0])
                        } else {
                            json!([0, 1])
                        }
                    })
                    .collect();
                let response = if headers.starts_with("POST /api/embed ") {
                    json!({"embeddings": vectors})
                } else {
                    json!({"data": vectors.into_iter().enumerate().rev().map(|(index,embedding)| json!({"index":index,"embedding":embedding})).collect::<Vec<_>>()})
                };
                let (status, response) = if behavior == 1 {
                    (
                        "500 Internal Server Error",
                        "synthetic-key-do-not-echo".into(),
                    )
                } else {
                    ("200 OK", response.to_string())
                };
                let wire = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len());
                let _ = stream.write_all(wire.as_bytes());
            }
        });
        Self {
            url,
            requests,
            mode,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(1, Ordering::SeqCst);
        let _ = self.thread.take().unwrap().join();
    }
}

fn configure(conn: &mut Connection, url: &str, model: &str) -> EmbeddingPlan {
    profiles::save(
        conn,
        AiProfiles {
            profiles: vec![AiProfile {
                id: "embedding".into(),
                name: "本地合成接口".into(),
                mode: "openai".into(),
                api_url: format!("{url}/v1"),
                model: "chat-model".into(),
                has_api_key: false,
                api_key: None,
            }],
            active_id: Some("embedding".into()),
            daily_limit: 0,
            embedding: Some(EmbeddingSettings {
                profile_id: "embedding".into(),
                model: model.into(),
                chunk_chars: 256,
            }),
            ..Default::default()
        },
    )
    .unwrap();
    EmbeddingPlan::require(conn).unwrap()
}

fn job_status(conn: &Connection, id: &str) -> String {
    conn.query_row(
        "SELECT status FROM knowledge_index_jobs WHERE id=?1",
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

#[tokio::test]
async fn embedding_protocols_preserve_model_credentials_and_input_order() {
    let server = Server::start();
    for mode in ["openai", "ollama"] {
        let plan = EmbeddingPlan {
            profile_id: "test".into(),
            mode: mode.into(),
            api_url: if mode == "openai" {
                format!("{}/v1", server.url)
            } else {
                server.url.clone()
            },
            model: "separate-vector-model".into(),
            chunk_chars: 256,
            fingerprint: "test".into(),
        };
        let client = EmbeddingClient::new(&plan, Some("synthetic-key")).unwrap();
        assert_eq!(
            client
                .embed(&["赔偿金额".into(), "普通材料".into()])
                .await
                .unwrap(),
            vec![vec![1.0, 0.0], vec![0.0, 1.0]]
        );
        let log = server.requests.lock().unwrap();
        let (headers, body) = log.last().unwrap();
        assert!(headers.starts_with(if mode == "openai" {
            "POST /v1/embeddings "
        } else {
            "POST /api/embed "
        }));
        assert!(headers
            .to_lowercase()
            .contains("authorization: bearer synthetic-key"));
        assert_eq!(body["model"], "separate-vector-model");
        assert_eq!(body["input"], json!(["赔偿金额", "普通材料"]));
        if mode == "ollama" {
            assert_eq!(body["truncate"], false)
        } else {
            assert_eq!(body["encoding_format"], "float")
        }
    }
}

#[test]
fn migration_backfills_cjk_search_and_invalidates_only_changed_vectors() {
    let conn = Connection::open_in_memory().unwrap();
    db::schema::run_migrations(&conn, 0).unwrap();
    // Recreate the v27 boundary with a pre-existing note, then use the normal upgrader.
    conn.execute_batch("DROP TRIGGER trg_knowledge_vector_invalidate; DROP TRIGGER trg_knowledge_trigram_insert; DROP TRIGGER trg_knowledge_trigram_delete; DROP TRIGGER trg_knowledge_trigram_update; DROP TABLE knowledge_trigram; DROP TABLE knowledge_index_chunks; DROP TABLE knowledge_index_jobs; PRAGMA user_version=27;").unwrap();
    conn.execute("INSERT INTO knowledge_items(id,title,content,category) VALUES('note','侵权研究','依法确认第三人的损害赔偿金额与诉讼费用','reference')", []).unwrap();
    db::schema::run_migrations(&conn, 27).unwrap();
    assert_eq!(
        search::fts_search(&conn, "损害赔偿金额", 10).unwrap()[0].0,
        "note"
    );
    assert_eq!(search::fts_search(&conn, "费用", 10).unwrap()[0].0, "note");
    assert!(search::fts_search(&conn, "%_", 10).unwrap().is_empty());
    let changes = conn.total_changes();
    db::schema::run_migrations(&conn, 27).unwrap();
    assert_eq!(changes, conn.total_changes());
    conn.execute("INSERT INTO knowledge_index_jobs(id,item_id,source_hash,config_hash,model,status,total_chunks,completed_chunks,dimension) VALUES('job','note','source','config','model','completed',1,1,2)", []).unwrap();
    conn.execute(
        "INSERT INTO knowledge_index_chunks VALUES('job',0,'损害赔偿金额',?1)",
        [vec![0u8; 8]],
    )
    .unwrap();
    conn.execute(
        "UPDATE knowledge_items SET content=content,tags='更新标签' WHERE id='note'",
        [],
    )
    .unwrap();
    assert_eq!(job_status(&conn, "job"), "completed");
    conn.execute(
        "UPDATE knowledge_items SET content='全新材料' WHERE id='note'",
        [],
    )
    .unwrap();
    assert_eq!(job_status(&conn, "job"), "stale");
    assert!(search::fts_search(&conn, "损害赔偿金额", 10)
        .unwrap()
        .is_empty());
    assert_eq!(
        conn.query_row("SELECT count(*) FROM knowledge_index_chunks", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    conn.execute("DELETE FROM knowledge_items WHERE id='note'", [])
        .unwrap();
    assert!(search::fts_search(&conn, "全新材料", 10)
        .unwrap()
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn durable_index_handles_full_text_retry_cancel_edits_and_model_changes() {
    db::enable_test_mode();
    let mut conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute("DELETE FROM knowledge_items", []).unwrap();
    let server = Server::start();
    let first_plan = configure(&mut conn, &server.url, "vector-v1");
    casy_lib::ai::get_token_budget().set_daily_limit(0);
    let long = "普通材料已整理。".repeat(700) + "第三人的赔偿金额为125000.25元。";
    conn.execute("INSERT INTO knowledge_items(id,title,content,category) VALUES('long','长文测试',?1,'reference')", [&long]).unwrap();
    conn.execute("INSERT INTO knowledge_items(id,title,content,category) VALUES('empty','空白测试','','reference')", []).unwrap();
    assert!(index::embed_knowledge("empty".into(), None).await.is_err());
    let queued = index::embed_all_knowledge().await.unwrap();
    assert_eq!(
        (queued.queued, queued.up_to_date, queued.already_queued),
        (1, 0, 0)
    );
    assert_eq!(
        index::embed_all_knowledge().await.unwrap().already_queued,
        1
    );
    assert!(index::process_next().await.unwrap());
    let status = index::status_inner(&conn).unwrap();
    assert_eq!(status.indexed, 1);
    let original = status.jobs[0].id.clone();
    assert!(status.jobs[0].total_chunks > 20);
    assert_eq!(index::embed_all_knowledge().await.unwrap().up_to_date, 1);
    let seen = server.requests.lock().unwrap();
    assert!(seen
        .iter()
        .any(|(_, body)| body["input"].to_string().contains("125000.25")));
    drop(seen);
    let found = search::search("损失数额", 10, true).await.unwrap();
    assert_eq!(found.semantic_status, "ready");
    assert!(found.results[0].content.contains("125000.25"));
    assert_eq!(found.results[0].source, "semantic");
    server.mode.store(1, Ordering::SeqCst);
    let fallback = search::search("赔偿金额", 10, true).await.unwrap();
    assert_eq!(fallback.semantic_status, "unavailable");
    assert!(fallback.results[0].content.contains("125000.25"));
    assert!(!fallback.warning.unwrap().contains("synthetic-key"));
    server.mode.store(2, Ordering::SeqCst);
    let changed_dimension = search::search("赔偿金额", 10, true).await.unwrap();
    assert_eq!(changed_dimension.semantic_status, "unavailable");
    assert!(changed_dimension.warning.unwrap().contains("维数"));
    assert!(changed_dimension.results[0].content.contains("125000.25"));
    server.mode.store(0, Ordering::SeqCst);
    // Missing stored chunks must be repaired, even when the source/model hashes match.
    conn.execute(
        "DELETE FROM knowledge_index_chunks WHERE job_id=?1 AND chunk_index=0",
        [&original],
    )
    .unwrap();
    let repaired = index::embed_knowledge("long".into(), None).await.unwrap();
    assert_ne!(original, repaired);
    index::cancel_knowledge_index_job(repaired.clone())
        .await
        .unwrap();
    assert_eq!(job_status(&conn, &repaired), "cancelled");
    assert!(!index::process_next().await.unwrap());
    let failed = index::embed_knowledge("long".into(), None).await.unwrap();
    server.mode.store(1, Ordering::SeqCst);
    index::process_next().await.unwrap();
    assert_eq!(job_status(&conn, &failed), "failed");
    server.mode.store(0, Ordering::SeqCst);
    let retry = index::embed_knowledge("long".into(), None).await.unwrap();
    index::process_next().await.unwrap();
    assert_eq!(job_status(&conn, &retry), "completed");
    let next_plan = configure(&mut conn, &server.url, "vector-v2");
    assert_ne!(first_plan.fingerprint, next_plan.fingerprint);
    assert!(
        search::semantic_search(&conn, &[1.0, 0.0], &next_plan.fingerprint, 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        search::search("赔偿金额", 10, true)
            .await
            .unwrap()
            .semantic_status,
        "not_indexed"
    );
    assert_eq!(index::status_inner(&conn).unwrap().jobs[0].status, "stale");
    let cancelled = index::embed_knowledge("long".into(), None).await.unwrap();
    server.mode.store(4, Ordering::SeqCst);
    index::process_next().await.unwrap();
    assert_eq!(job_status(&conn, &cancelled), "cancelled");
    let edited = index::embed_knowledge("long".into(), None).await.unwrap();
    server.mode.store(3, Ordering::SeqCst);
    index::process_next().await.unwrap();
    assert_eq!(job_status(&conn, &edited), "stale");
    assert!(
        search::semantic_search(&conn, &[1.0, 0.0], &first_plan.fingerprint, 10)
            .unwrap()
            .is_empty()
    );
    server.mode.store(0, Ordering::SeqCst);
    let current = index::embed_knowledge("long".into(), None).await.unwrap();
    index::process_next().await.unwrap();
    assert_eq!(job_status(&conn, &current), "completed");
    let forced = index::embed_knowledge("long".into(), Some(true))
        .await
        .unwrap();
    assert_ne!(current, forced);
    assert_eq!(index::status_inner(&conn).unwrap().indexed, 0);
    conn.execute(
        "UPDATE knowledge_index_jobs SET status='running' WHERE id=?1",
        [&forced],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO knowledge_index_chunks VALUES(?1,0,'partial',?2)",
        params![forced, vec![0u8; 8]],
    )
    .unwrap();
    index::recover_interrupted(&conn).unwrap();
    assert_eq!(job_status(&conn, &forced), "failed");
    assert_eq!(
        conn.query_row("SELECT count(*) FROM knowledge_index_chunks", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
}

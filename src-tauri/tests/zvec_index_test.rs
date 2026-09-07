use casy_lib::{
    ai::embeddings::{compact_vector, stored_cosine},
    db::{schema, vector_index::VectorIndex},
};
use rusqlite::{params, Connection};
use std::{collections::HashSet, time::Instant};

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    schema::run_migrations(&conn, 0).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    conn
}
fn item(conn: &Connection, id: &str, vectors: &[Vec<f32>], fingerprint: &str) {
    conn.execute(
        "INSERT INTO knowledge_items(id,title,category,content) VALUES(?1,?1,'note','合成正文')",
        [id],
    )
    .unwrap();
    conn.execute("INSERT INTO knowledge_index_jobs(id,item_id,source_hash,config_hash,model,status,total_chunks,completed_chunks,dimension)
        VALUES(?1,?1,'sha',?2,'test','completed',?3,?3,?4)", params![id,fingerprint,vectors.len(),vectors[0].len()]).unwrap();
    for (n, v) in vectors.iter().enumerate() {
        conn.execute("INSERT INTO knowledge_index_chunks(job_id,chunk_index,content,embedding) VALUES(?1,?2,?3,?4)",
            params![id,n,format!("{id} passage {n}"),compact_vector(v)]).unwrap();
    }
}

#[test]
fn zvec_handles_edits_archival_deletion_reopening_and_corrupt_cache() {
    let root = tempfile::tempdir().unwrap();
    let conn = database();
    item(&conn, "a", &[vec![1., 0., 0., 0.]], "test");
    item(&conn, "b", &[vec![0., 1., 0., 0.]], "test");
    let mut index = VectorIndex::open(root.path()).unwrap();
    let search = |index: &mut VectorIndex, q: &[f32]| index.search(&conn, q, "test", 10).unwrap();
    assert_eq!(search(&mut index, &[1., 0., 0., 0.])[0].0, "a");
    assert!(
        VectorIndex::open(root.path()).is_err(),
        "single writer lock"
    );
    drop(index);
    let mut index = VectorIndex::open(root.path()).unwrap();
    assert_eq!(search(&mut index, &[1., 0., 0., 0.])[0].0, "a");
    conn.execute(
        "UPDATE knowledge_index_chunks SET embedding=?1 WHERE job_id='a'",
        [compact_vector(&[0., 1., 0., 0.])],
    )
    .unwrap();
    assert!(search(&mut index, &[1., 0., 0., 0.]).is_empty());
    conn.execute(
        "UPDATE knowledge_items SET status='archived' WHERE id='b'",
        [],
    )
    .unwrap();
    assert_eq!(search(&mut index, &[0., 1., 0., 0.]).len(), 1);
    conn.execute("DELETE FROM knowledge_items WHERE id='a'", [])
        .unwrap();
    assert!(search(&mut index, &[0., 1., 0., 0.]).is_empty());
    conn.execute(
        "UPDATE knowledge_items SET status='current' WHERE id='b'",
        [],
    )
    .unwrap();
    assert_eq!(search(&mut index, &[0., 1., 0., 0.])[0].0, "b");
    drop(index);
    std::fs::write(root.path().join("manifest.json"), "interrupted write").unwrap();
    let mut index = VectorIndex::open(root.path()).unwrap();
    assert_eq!(search(&mut index, &[0., 1., 0., 0.])[0].0, "b");
    // A model change must not reuse the previous collection, even at the same dimension.
    assert!(index
        .search(&conn, &[0., 1., 0., 0.], "changed-model", 10)
        .unwrap()
        .is_empty());
    assert_eq!(search(&mut index, &[0., 1., 0., 0.])[0].0, "b");
    conn.execute(
        "UPDATE knowledge_items SET content='changed source' WHERE id='b'",
        [],
    )
    .unwrap();
    assert!(search(&mut index, &[0., 1., 0., 0.]).is_empty());
}

#[test]
fn zvec_diversifies_long_documents_and_recovers_missing_collection() {
    let root = tempfile::tempdir().unwrap();
    let conn = database();
    item(&conn, "long", &vec![vec![1., 0., 0., 0.]; 700], "test");
    item(&conn, "other'quoted", &[vec![0.9, 0.1, 0., 0.]], "test");
    let mut index = VectorIndex::open(root.path()).unwrap();
    let hits = index.search(&conn, &[1., 0., 0., 0.], "test", 10).unwrap();
    assert!(hits.iter().any(|h| h.0 == "other'quoted"));
    assert_eq!(hits.len(), 2);
    drop(index);
    std::fs::remove_dir_all(root.path().join("collection")).unwrap();
    let mut index = VectorIndex::open(root.path()).unwrap();
    assert_eq!(
        index
            .search(&conn, &[1., 0., 0., 0.], "test", 10)
            .unwrap()
            .len(),
        2
    );
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}

#[test]
fn zvec_resynchronizes_after_restoring_an_older_database_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let conn = database();
    item(&conn, "a", &[vec![1., 0., 0., 0.]], "test");
    let snapshot = root.path().join("snapshot.sqlite");
    conn.execute("VACUUM INTO ?1", [snapshot.to_str().unwrap()])
        .unwrap();
    let mut index = VectorIndex::open(&root.path().join("index")).unwrap();
    assert_eq!(
        index.search(&conn, &[1., 0., 0., 0.], "test", 10).unwrap()[0].0,
        "a"
    );
    conn.execute(
        "UPDATE knowledge_index_chunks SET embedding=?1,content='newer text' WHERE job_id='a'",
        [compact_vector(&[0., 1., 0., 0.])],
    )
    .unwrap();
    assert_eq!(
        index.search(&conn, &[0., 1., 0., 0.], "test", 10).unwrap()[0].2,
        "newer text"
    );
    let restored = Connection::open(snapshot).unwrap();
    assert!(index
        .search(&restored, &[0., 1., 0., 0.], "test", 10)
        .unwrap()
        .is_empty());
    let hits = index
        .search(&restored, &[1., 0., 0., 0.], "test", 10)
        .unwrap();
    assert_eq!(hits[0].2, "a passage 0");
}

fn bytes(path: &std::path::Path) -> u64 {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                bytes(&path)
            } else {
                path.metadata().unwrap().len()
            }
        })
        .sum()
}

#[test]
#[ignore = "nontrivial HNSW recall and disk benchmark; run explicitly"]
fn zvec_recall_latency_and_total_disk() {
    let root = tempfile::tempdir().unwrap();
    let conn = database();
    let mut seed = 123456789u64;
    let mut vectors = Vec::new();
    conn.execute_batch("BEGIN").unwrap();
    for n in 0..6000 {
        let mut v: Vec<f32> = (0..768)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                ((seed >> 32) as u32 as f32 / u32::MAX as f32) - 0.5
            })
            .collect();
        // Independent clustered passages give a meaningful, reproducible near-neighbor task.
        v[n % 32] += 5.0;
        item(&conn, &format!("item-{n}"), &[v.clone()], "test");
        vectors.push(v);
    }
    conn.execute_batch("COMMIT").unwrap();
    let mut index = VectorIndex::open(root.path()).unwrap();
    let build = Instant::now();
    index.search(&conn, &vectors[0], "test", 10).unwrap();
    let build_ms = build.elapsed().as_millis();
    let mut recalled = 0;
    let mut ann_ms = Vec::new();
    let mut linear_ms = Vec::new();
    let compact: Vec<_> = vectors.iter().map(|v| compact_vector(v)).collect();
    for n in (0..6000).step_by(300) {
        let mut query = vectors[n].clone();
        query[(n + 7) % 768] += 0.5;
        let start = Instant::now();
        let mut exact: Vec<_> = compact
            .iter()
            .enumerate()
            .map(|(i, v)| (i, stored_cosine(&query, v)))
            .collect();
        exact.sort_by(|a, b| b.1.total_cmp(&a.1));
        linear_ms.push(start.elapsed().as_micros());
        let targets: HashSet<_> = exact[..10]
            .iter()
            .map(|(i, _)| format!("item-{i}"))
            .collect();
        let start = Instant::now();
        let hits = index.search(&conn, &query, "test", 10).unwrap();
        ann_ms.push(start.elapsed().as_micros());
        recalled += hits.iter().filter(|h| targets.contains(&h.0)).count();
    }
    let report = serde_json::json!({"vectors":6000,"dimension":768,"queries":20,"recallAt10":recalled as f64/200.0,
        "buildMs":build_ms,"annMicros":ann_ms,"linearMicros":linear_ms,"zvecDirectoryBytes":bytes(root.path()),"sqliteVectorPayloadBytes":6000*772});
    println!("{report}");
    assert!(recalled >= 190, "recall@10 below 95%: {report}");
    if let Ok(directory) = std::env::var("CASY_ZVEC_QA_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            std::path::Path::new(&directory).join("zvec-benchmark.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
}

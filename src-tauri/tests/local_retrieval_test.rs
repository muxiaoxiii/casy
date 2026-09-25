use casy_lib::{
    ai::{
        embeddings::{self, EmbeddingPlan},
        local_embedding,
        profiles::{self, AiProfiles, EmbeddingSettings},
    },
    db::{self, knowledge_index, search},
};
use rusqlite::params;
use serde_json::json;
use std::time::Instant;

#[tokio::test]
#[ignore = "requires bundled E5-base model and CASY_DOC_ENGINE"]
async fn local_multilingual_retrieval_and_compact_storage() {
    let root = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", root.path());
    db::enable_test_mode();
    let mut conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    let config = AiProfiles {
        embedding: Some(EmbeddingSettings {
            profile_id: local_embedding::PROFILE.into(),
            model: local_embedding::MODEL.into(),
            chunk_chars: 384,
        }),
        ..AiProfiles::default()
    };
    profiles::save(&mut conn, config).unwrap();
    let passages = [
        (
            "patent",
            "材料甲",
            "未经专利权人许可制造、销售被诉产品构成专利侵权，应当赔偿损失并停止侵害。",
        ),
        (
            "lease",
            "材料乙",
            "房屋租赁合同到期后，承租人应腾退房屋，出租人退还押金。",
        ),
        (
            "employment",
            "材料丙",
            "劳动者被违法解除劳动合同，可以请求继续履行合同或者支付赔偿金。",
        ),
        (
            "inheritance",
            "材料丁",
            "遗产由继承人依法分配，遗嘱应符合法定形式。",
        ),
        (
            "software",
            "材料戊",
            "软件版本升级后支持数据备份，用户可恢复已经保存的设置。",
        ),
    ];
    for (id, title, content) in passages {
        conn.execute(
            "INSERT INTO knowledge_items(id,title,content,category) VALUES(?1,?2,?3,'reference')",
            params![id, title, content],
        )
        .unwrap();
    }
    let started = Instant::now();
    assert_eq!(
        knowledge_index::embed_all_knowledge().await.unwrap().queued,
        5
    );
    while knowledge_index::process_next().await.unwrap() {}
    let build_ms = started.elapsed().as_millis();
    let status = knowledge_index::get_knowledge_index_status().await.unwrap();
    assert_eq!(
        status.indexed,
        5,
        "{}",
        serde_json::to_string(&status).unwrap()
    );
    let plan = EmbeddingPlan::require(&conn).unwrap();
    let client = plan.client(&conn).unwrap();
    let mut results = Vec::new();
    for query in [
        "未经许可使用专利如何赔偿",
        "Compensation for patent infringement",
        "Schadensersatz wegen Patentverletzung",
        "Indemnisation pour contrefaçon de brevet",
        "特許侵害による損害賠償",
    ] {
        let start = Instant::now();
        let found = search::search(query, 5, true).await.unwrap();
        assert_eq!(found.semantic_status, "ready", "{:?}", found.warning);
        assert_eq!(found.results[0].id, "patent", "{query}");
        results.push(
            json!({"query":query,"top":found.results[0].id,"ms":start.elapsed().as_millis()}),
        );
    }
    let vectors = client
        .embed(
            &passages
                .iter()
                .map(|(_, _, content)| content.to_string())
                .collect::<Vec<_>>(),
        )
        .await
        .unwrap();
    let query = client
        .embed_query("Patent infringement damages")
        .await
        .unwrap()
        .remove(0);
    let cancelled = tokio::time::timeout(
        std::time::Duration::from_millis(1),
        client.embed(&["法院材料".repeat(1000)]),
    )
    .await;
    assert!(cancelled.is_err());
    let after_cancel = client
        .embed_query("Patent infringement damages")
        .await
        .unwrap()
        .remove(0);
    assert!(
        embeddings::cosine(&query, &after_cancel) > 0.99999,
        "cancelled requests must not leak stale responses"
    );
    let mut fp: Vec<_> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i, embeddings::cosine(&query, v)))
        .collect();
    let mut compact: Vec<_> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                i,
                embeddings::stored_cosine(&query, &embeddings::compact_vector(v)),
            )
        })
        .collect();
    fp.sort_by(|a, b| b.1.total_cmp(&a.1));
    compact.sort_by(|a, b| b.1.total_cmp(&a.1));
    assert_eq!(
        fp.iter().map(|v| v.0).collect::<Vec<_>>(),
        compact.iter().map(|v| v.0).collect::<Vec<_>>()
    );
    // A single input exceeding 512 tokens must include its tail instead of silently truncating.
    assert_eq!(
        client
            .embed(&["法院审理合同事实。".repeat(200) + "特许侵害による損害賠償"])
            .await
            .unwrap()[0]
            .len(),
        768
    );
    let (bytes, chunks): (usize, usize) = conn
        .query_row(
            "SELECT sum(length(embedding)),count(*) FROM knowledge_index_chunks",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(bytes, chunks * 772);
    let job: String = conn
        .query_row(
            "SELECT id FROM knowledge_index_jobs WHERE item_id='patent'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let blob = embeddings::compact_vector(&vectors[0]);
    {
        let tx = conn.transaction().unwrap();
        let mut insert=tx.prepare("INSERT INTO knowledge_index_chunks(job_id,chunk_index,content,embedding) VALUES(?1,?2,'性能测试合成分段',?3)").unwrap();
        for n in 1..1000 {
            insert.execute(params![job, n, blob]).unwrap();
        }
        drop(insert);
        tx.execute("UPDATE knowledge_index_jobs SET total_chunks=1000,completed_chunks=1000 WHERE id=?1", [&job]).unwrap();
        tx.commit().unwrap();
    }
    let start = Instant::now();
    assert_eq!(
        search::semantic_search(&conn, &query, &plan.fingerprint, 20).unwrap()[0].0,
        "patent"
    );
    let sync_ms = start.elapsed().as_millis();
    let report = json!({"buildMs":build_ms,"queries":results,"vectorBytes":bytes,"chunks":chunks,"dimension":768,"floatBytesPerVector":3072,"compactBytesPerVector":772,"longDocumentChunks":1004,"longDocumentSyncAndSearchMs":sync_ms,"engine":"zvec-hnsw-fp16","profile":root.path()});
    println!("{report}");
    if let Ok(dir) = std::env::var("CASY_RETRIEVAL_QA_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            std::path::Path::new(&dir).join("retrieval.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
}

//! Isolated knowledge QA bridge. HTTP requests must use a loopback test server.
use anyhow::{bail, Result};
use casy_lib::{
    ai::{embeddings, profiles},
    commands::knowledge,
    db::{self, knowledge_index as index, search},
};
use serde_json::{json, Value};
use std::io::Read;

#[tokio::main]
async fn main() -> Result<()> {
    if !cfg!(debug_assertions) || std::env::var_os("CASY_TEST_DATA_DIR").is_none() {
        bail!("Requires debug build and CASY_TEST_DATA_DIR");
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let request: Value = serde_json::from_str(&input)?;
    let args = &request["args"];
    let mut conn = db::open_db()?;
    db::init_db(&conn)?;
    if let Some(plan) = embeddings::EmbeddingPlan::load(&conn)? {
        if !plan.api_url.starts_with("http://127.0.0.1:") {
            bail!("Expected loopback embedding server");
        }
    }
    let result: Result<Value, String> = match request["command"].as_str().unwrap_or("") {
        "qa_seed" => {
            conn.execute("DELETE FROM knowledge_items", [])?;
            conn.execute("INSERT INTO knowledge_items(id,title,content,category,updated_at) VALUES('qa-long','第三人赔偿研究',?1,'reference','2000-01-01')", ["程序材料已经核对。".repeat(500) + "第三人的赔偿金额为125000.25元。"])?;
            conn.execute("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<2000)
                INSERT INTO knowledge_items(id,title,content,category,status,block_type)
                SELECT 'archived-'||i,'历史分段'||i,'历史材料','reference','archived','block' FROM n", [])?;
            Ok(json!(true))
        }
        "qa_process_next" => index::process_next()
            .await
            .map(|v| json!(v))
            .map_err(|e| e.to_string()),
        "qa_audit" => {
            let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            let fk: i64 =
                conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
            Ok(json!({"integrity":integrity,"foreignKeyErrors":fk}))
        }
        "get_ai_profiles" => profiles::get_ai_profiles().await.map(|v| json!(v)),
        "save_ai_profiles" => {
            let config: profiles::AiProfiles = serde_json::from_value(args["config"].clone())?;
            if config
                .profiles
                .iter()
                .any(|p| !p.api_url.starts_with("http://127.0.0.1:"))
            {
                bail!("Expected loopback profiles");
            }
            profiles::save(&mut conn, config)
                .map(|v| json!(v))
                .map_err(|e| e.to_string())
        }
        "test_embedding_connection" => embeddings::test_embedding_connection()
            .await
            .map(|v| json!(v)),
        "embed_knowledge" => index::embed_knowledge(
            args["itemId"].as_str().unwrap_or("").into(),
            args["force"].as_bool(),
        )
        .await
        .map(|v| json!(v)),
        "embed_all_knowledge" => index::embed_all_knowledge().await.map(|v| json!(v)),
        "get_knowledge_index_status" => index::get_knowledge_index_status().await.map(|v| json!(v)),
        "cancel_knowledge_index_job" => {
            index::cancel_knowledge_index_job(args["jobId"].as_str().unwrap_or("").into())
                .await
                .map(|v| json!(v))
        }
        "search_knowledge_index" => search::search_knowledge_index(
            args["query"].as_str().unwrap_or("").into(),
            args["useSemantic"].as_bool().unwrap_or(false),
        )
        .await
        .map(|v| json!(v)),
        "list_knowledge" => knowledge::list_knowledge(None).await.map(|v| json!(v)),
        "get_knowledge_with_blocks" => {
            knowledge::get_knowledge_with_blocks(args["id"].as_str().unwrap_or("").into())
                .await
                .map(|v| json!(v))
        }
        _ => Err("Command outside knowledge QA allowlist".into()),
    };
    println!(
        "{}",
        match result {
            Ok(data) => json!({"ok":true,"data":data}),
            Err(error) => json!({"ok":false,"error":error}),
        }
    );
    Ok(())
}

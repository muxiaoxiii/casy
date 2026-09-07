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
        "qa_seed_workspace" => {
            conn.execute("INSERT INTO cases(id,case_name,client_name,case_no) VALUES('workspace-case','本地同步验收案','本地客户','2026-QA')",[])?;
            let dirs=casy_lib::commands::files::list_case_dirs("workspace-case".into()).await.map_err(anyhow::Error::msg)?;
            let root=std::path::PathBuf::from(&dirs[0].absolute_path);
            let path=root.join("证据材料.md");
            std::fs::write(&path,"# 多语种证据\n\nPrüfung français 日本語\n\n> 来源一\n\n>> 来源二\n\n>>> 来源三\n\n>>>> 来源四\n")?;
            conn.execute("INSERT INTO settings(key,value) VALUES('workspace_sync',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[json!({"register":true,"ocr":true,"knowledge":true}).to_string()])?;
            let mut sync=casy_lib::workspace_sync::Reconciler::default();sync.tick()?;sync.tick()?;
            casy_lib::background_jobs::process_next_document_job().await?;sync.tick()?;
            Ok(json!({"caseId":"workspace-case","path":path}))
        }
        "qa_sync_workspace" => {
            let mut sync=casy_lib::workspace_sync::Reconciler::default();sync.tick()?;sync.tick()?;
            casy_lib::background_jobs::process_next_document_job().await?;sync.tick()?;
            Ok(json!(true))
        }
        "list_workspace_sources" => casy_lib::workspace_sync::list_workspace_sources(serde_json::from_value(args["caseIds"].clone())?).await.map(|v|json!(v)),
        "get_workspace_document" => casy_lib::workspace_sync::get_workspace_document(args["fileId"].as_str().unwrap_or("").into()).await,
        "get_workspace_sync_status" => casy_lib::workspace_sync::get_workspace_sync_status().await,
        "import_pageindex_to_knowledge" => knowledge::import_pageindex_to_knowledge(args["fileId"].as_str().unwrap_or("").into()).await.map(|v|json!(v)),
        "save_settings" => casy_lib::commands::settings::save_settings(serde_json::from_value(args["settings"].clone())?).await.map(|v|json!(v)),
        "get_settings" => casy_lib::commands::settings::get_settings().await.map(|v|json!(v)),
        "save_editor_recovery" => casy_lib::commands::editor_recovery::save_editor_recovery(args["sessionId"].as_str().unwrap_or("").into(), args.get("draft").filter(|v| !v.is_null()).cloned()).await.map(|v|json!(v)),
        "recover_editor_drafts" => casy_lib::commands::editor_recovery::recover_editor_drafts().await.map(|v|json!(v)),
        "qa_seed_editing" => {
            conn.execute("INSERT INTO knowledge_items(id,title,content,category,status,block_type,updated_at) VALUES('edit-a','甲研究',?1,'reference','current','page','2099-01-01')", ["# 甲研究\n\n* 原始项目\n\n保留两个空格  \n保留换行\n"])?;
            conn.execute("INSERT INTO knowledge_items(id,title,content,category,status,block_type) VALUES('edit-b','乙研究','乙笔记原文','reference','current','page')", [])?;
            let long = (0..3000).map(|n|format!("## 材料第{n}节\n\n第三人代理人与法院核对送达地址，保留关联案件及证据编号。\n\n")).collect::<String>() + "末尾完整性标记-END";
            conn.execute("INSERT INTO knowledge_items(id,title,content,category,status,block_type) VALUES('edit-long','长篇材料',?1,'reference','current','page')", [&long])?;
            Ok(json!({"bytes":long.len()}))
        }
        "create_knowledge" => knowledge::create_knowledge(serde_json::from_value(args["data"].clone())?).await.map(|v|json!(v)),
        "update_knowledge" => knowledge::update_knowledge(args["id"].as_str().unwrap_or("").into(),args["data"].clone()).await.map(|v|json!(v)),
        "delete_knowledge" => knowledge::delete_knowledge(args["id"].as_str().unwrap_or("").into()).await.map(|v|json!(v)),
        "list_knowledge_versions" => knowledge::list_knowledge_versions(args["itemId"].as_str().unwrap_or("").into()).await.map(|v|json!(v)),
        "diff_knowledge_with_current" => knowledge::diff_knowledge_with_current(args["versionId"].as_str().unwrap_or("").into(),args["itemId"].as_str().unwrap_or("").into()).await.map(|v|json!(v)),
        "restore_knowledge_version" => knowledge::restore_knowledge_version(args["itemId"].as_str().unwrap_or("").into(),args["versionId"].as_str().unwrap_or("").into()).await.map(|v|json!(v)),
        "export_knowledge_markdown" => knowledge::export_knowledge_markdown(args["itemId"].as_str().unwrap_or("").into(),args["outputPath"].as_str().unwrap_or("").into()).await.map(|v|json!(v)),
        "list_links_for" => casy_lib::commands::linking::list_links_for(args["sourceType"].as_str().unwrap_or("").into(),args["sourceId"].as_str().unwrap_or("").into()).await.map(|v|json!(v)),
        "get_backlinks" => casy_lib::commands::linking::get_backlinks(args["targetType"].as_str().unwrap_or("").into(),args["targetId"].as_str().unwrap_or("").into()).await.map(|v|json!(v)),
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

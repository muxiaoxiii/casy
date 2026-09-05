//! Offline QA bridge for browser interaction tests against the real Rust commands.
//! Requires an isolated debug data directory; never starts sync/background workers.
use anyhow::{bail, Result};
use casy_lib::{
    commands::{cases, feishu_snapshot, persons, relations, tasks, timeline},
    db,
};
use serde_json::{json, Value};
use std::io::Read;

fn audit() -> Result<Value> {
    use sha2::{Digest, Sha256};
    let conn = db::open_db()?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    let foreign_keys: i64 =
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })?;
    let files: Vec<(String, String, i64)> = conn
        .prepare("SELECT id,file_path,file_size FROM case_files")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let assets: Vec<(String, Vec<u8>)> = conn
        .prepare("SELECT file_token,content FROM imported_assets")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut verified = 0;
    for (id, path, size) in &files {
        let bytes = std::fs::read(path)?;
        if bytes.len() as i64 != *size {
            bail!("File size mismatch");
        }
        let expected = assets
            .iter()
            .find(|(token, _)| id.ends_with(token))
            .ok_or_else(|| anyhow::anyhow!("Asset missing"))?;
        if Sha256::digest(&bytes) != Sha256::digest(&expected.1) {
            bail!("File hash mismatch");
        }
        verified += 1;
    }
    Ok(
        json!({"integrity":integrity,"foreignKeyErrors":foreign_keys,"verifiedFiles":verified,"assets":assets.len()}),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    if !cfg!(debug_assertions) || std::env::var_os("CASY_TEST_DATA_DIR").is_none() {
        bail!("This QA bridge requires a debug build and CASY_TEST_DATA_DIR");
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let request: Value = serde_json::from_str(&input)?;
    db::init_db(&db::open_db()?)?;
    let p = &request["args"];
    let result: Result<Value, String> = match request["command"].as_str().unwrap_or("") {
        "qa_integrity" => audit().map_err(|e| e.to_string()),
        "list_case_files" => casy_lib::commands::files::list_case_files(
            p["caseId"].as_str().unwrap_or("").into(),
            None,
        )
        .await
        .map(|v| json!(v)),
        "create_case" => cases::create_case(p["data"].clone())
            .await
            .map(|v| json!(v)),
        "update_case" => {
            cases::update_case(p["id"].as_str().unwrap_or("").into(), p["data"].clone())
                .await
                .map(|v| json!(v))
        }
        "get_case" => cases::get_case(p["id"].as_str().unwrap_or("").into())
            .await
            .map(|v| json!(v)),
        "list_cases" => {
            cases::list_cases(serde_json::from_value(p["filter"].clone()).unwrap_or_default())
                .await
                .map(|v| json!(v))
        }
        "search_cases" => cases::search_cases(p["query"].as_str().unwrap_or("").into())
            .await
            .map(|v| json!(v)),
        "get_relations" => relations::get_relations(p["caseId"].as_str().unwrap_or("").into())
            .await
            .map(|v| json!(v)),
        "list_case_hearings" => {
            cases::list_case_hearings(p["caseId"].as_str().unwrap_or("").into())
                .await
                .map(|v| json!(v))
        }
        "get_case_timeline" => {
            timeline::get_case_timeline(p["caseId"].as_str().unwrap_or("").into())
                .await
                .map(|v| json!(v))
        }
        "list_tasks" => tasks::list_tasks(serde_json::from_value(p["filter"].clone()).ok())
            .await
            .map(|v| json!(v)),
        "create_task" => tasks::create_task(p["data"].clone()).await.map(|v|json!(v)),
        "update_task" => tasks::update_task(p["data"].clone()).await.map(|v|json!(v)),
        "snooze_task" => tasks::snooze_task(p["id"].as_str().unwrap_or("").into(),p["option"].as_str().map(str::to_owned),p["newDueDate"].as_str().map(str::to_owned)).await.map(|v|json!(v)),
        "delete_task" => tasks::delete_task(p["id"].as_str().unwrap_or("").into(),None,None).await.map(|v|json!(v)),
        "restore_task" => tasks::restore_task(p["snapshot"].clone()).await.map(|v|json!(v)),
        "toggle_task" => {
            tasks::toggle_task(p["id"].as_str().unwrap_or("").into(), None, None, None)
                .await
                .map(|v| json!(v))
        }
        "list_case_persons" => {
            persons::list_case_persons(p["caseId"].as_str().unwrap_or("").into())
                .await
                .map(|v| json!(v))
        }
        "feishu_import_snapshot" => feishu_snapshot::feishu_import_snapshot(
            p["snapshot"].clone(),
            p["caseTableId"].as_str().unwrap_or("").into(),
            serde_json::from_value(p["selectedRecordIds"].clone()).unwrap_or_default(),
        )
        .await
        .map(|v| json!(v)),
        "get_case_source_records" => {
            feishu_snapshot::get_case_source_records(p["caseId"].as_str().unwrap_or("").into())
                .await
        }
        "get_imported_asset" => {
            feishu_snapshot::get_imported_asset(
                p["source"].as_str().unwrap_or("").into(),
                p["fileToken"].as_str().unwrap_or("").into(),
            )
            .await
        }
        _ => Err("Command is outside the offline QA allowlist".into()),
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

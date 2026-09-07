//! Local delivery verification only; every command uses an isolated profile.
use casy_lib::{
    commands::{backup, drafts, editor_recovery, portable_backup},
    db,
};
use serde_json::{json, Value};
use std::io::Read;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    anyhow::ensure!(
        cfg!(debug_assertions) && std::env::var_os("CASY_TEST_DATA_DIR").is_some(),
        "Requires isolated debug profile"
    );
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let request: Value = serde_json::from_str(&input)?;
    {
        let conn = db::open_db()?;
        db::init_db(&conn)?;
    }
    let args = &request["args"];
    let string = |key: &str| args[key].as_str().unwrap_or("").to_string();
    let optional = |key: &str| args[key].as_str().map(str::to_string);
    let result: Result<Value, String> = match request["command"].as_str().unwrap_or("") {
        "list_backups" => backup::list_backups().await.map(|v| json!(v)),
        "export_full_backup" => {
            portable_backup::export_full_backup(string("destination"), string("password"))
                .await
                .map(|v| json!(v))
        }
        "import_full_backup" => {
            portable_backup::import_full_backup(string("source"), string("password"))
                .await
                .map(|v| json!(v))
        }
        "list_drafts" => drafts::list_drafts().await.map(|v| json!(v)),
        "get_draft" => drafts::get_draft(string("id")).await.map(|v| json!(v)),
        "create_draft" => drafts::create_draft(
            string("title"),
            optional("content"),
            optional("caseId"),
            optional("templatePath"),
        )
        .await
        .map(|v| json!(v)),
        "update_draft" => drafts::update_draft(
            string("id"),
            optional("title"),
            optional("content"),
            optional("status"),
            optional("caseId"),
            args["expectedVersion"].as_i64().map(|v| v as i32),
        )
        .await
        .map(|v| json!(v)),
        "save_editor_recovery" => editor_recovery::save_editor_recovery(
            string("sessionId"),
            args.get("draft").filter(|v| !v.is_null()).cloned(),
        )
        .await
        .map(|v| json!(v)),
        "recover_editor_drafts" => editor_recovery::recover_editor_drafts()
            .await
            .map(|v| json!(v)),
        "qa_audit" => {
            let conn = db::open_db()?;
            let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            let foreign_keys: i64 =
                conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
            Ok(json!({"integrity":integrity,"foreignKeyErrors":foreign_keys}))
        }
        other => Err(format!("Unsupported delivery QA command: {other}")),
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

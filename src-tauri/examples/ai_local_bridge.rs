//! Isolated AI configuration and HTTP integration bridge. No background workers.
use anyhow::{bail, Result};
use casy_lib::db;
use serde_json::{json, Value};
use std::io::Read;

#[tokio::main]
async fn main() -> Result<()> {
    if !cfg!(debug_assertions) || std::env::var_os("CASY_TEST_DATA_DIR").is_none() {
        bail!("Requires a debug build and CASY_TEST_DATA_DIR");
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let request: Value = serde_json::from_str(&input)?;
    db::init_db(&*db::open_db()?)?;
    let p = &request["args"];
    let result: Result<Value, String> = match request["command"].as_str().unwrap_or("") {
        "qa_ai_storage" => {
            let conn = db::open_db()?;
            let leaked: i64 = conn.query_row("SELECT count(*) FROM settings WHERE value LIKE '%synthetic-key%'", [], |r| r.get(0))?;
            let state: Value = serde_json::from_str(&db::get_setting(&conn, "ai_profiles_v1")?.unwrap_or_else(|| "{}".into()))?;
            Ok(json!({"leakedSettings":leaked,"credentialAccounts":state["credentials"]}))
        }
        "qa_ai_credential_exists" => {
            let account = p["account"].as_str().unwrap_or("");
            if !account.starts_with("profile-") { bail!("Expected QA profile credential account"); }
            let exists = match keyring::Entry::new("casy-ai", account)?.get_password() {
                Ok(_) => true,
                Err(keyring::Error::NoEntry) => false,
                Err(error) => return Err(error.into()),
            };
            Ok(json!(exists))
        }
        "get_ai_profiles" => casy_lib::ai::profiles::get_ai_profiles()
            .await
            .map(|v| json!(v)),
        "save_ai_profiles" => {
            casy_lib::ai::profiles::save_ai_profiles(serde_json::from_value(p["config"].clone())?)
                .await
                .map(|v| json!(v))
        }
        "test_ai_profile" => {
            casy_lib::ai::profiles::test_ai_profile(serde_json::from_value(p["profile"].clone())?)
                .await
                .map(|v| json!(v))
        }
        "ai_chat" => casy_lib::ai::ai_chat(
            serde_json::from_value(p["messages"].clone())?,
            Some("local_qa".into()),
            p["mode"].as_str().map(Into::into),
            p["apiUrl"].as_str().map(Into::into),
            p["model"].as_str().map(Into::into),
            None,
            p["profileId"].as_str().map(Into::into),
        )
        .await
        .map(|v| json!(v)),
        "get_ai_config" => casy_lib::ai::get_ai_config().await.map(|v| json!(v)),
        "get_ai_usage" => casy_lib::ai::get_ai_usage().await,
        "call_llm_json" => casy_lib::ai::call_llm_json("Synthetic JSON test", "Return a JSON object").await,
        "generate_writing_suggestion" => casy_lib::ai::generate_writing_suggestion("Synthetic writing test".into(), None, None, None).await.map(|v| json!(v)),
        "get_settings" => casy_lib::commands::settings::get_settings()
            .await
            .map(|v| json!(v)),
        _ => Err("Command is outside the AI QA allowlist".into()),
    };
    println!("{}", match result {
        Ok(data) => json!({"ok":true,"data":data}),
        Err(error) => json!({"ok":false,"error":error}),
    });
    Ok(())
}

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::AiConfig;
use crate::credentials::{self, CredentialType};

const SETTINGS_KEY: &str = "ai_profiles_v1";

#[derive(Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiProfile {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub api_url: String,
    pub model: String,
    #[serde(default)]
    pub has_api_key: bool,
    /// None preserves the stored key; an empty string removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiProfiles {
    pub profiles: Vec<AiProfile>,
    pub active_id: Option<String>,
    pub daily_limit: u32,
    pub system_prompt: String,
    #[serde(default)]
    pub embedding: Option<EmbeddingSettings>,
}

#[derive(Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSettings {
    pub profile_id: String,
    pub model: String,
    pub chunk_chars: u32,
}

impl Default for AiProfiles {
    fn default() -> Self {
        Self {
            profiles: vec![],
            active_id: None,
            daily_limit: 50,
            system_prompt: "你是一个专业的法律 AI 助手。".into(),
            embedding: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct StoredProfiles {
    config: AiProfiles,
    credentials: std::collections::HashMap<String, String>,
}

fn stored(conn: &Connection) -> Result<Option<StoredProfiles>> {
    crate::db::get_setting(conn, SETTINGS_KEY)?
        .map(|s| serde_json::from_str(&s).context("AI 配置损坏，请在设置中重新保存"))
        .transpose()
}

pub fn read(conn: &Connection) -> Result<AiProfiles> {
    if let Some(state) = stored(conn)? {
        return Ok(state.config);
    }
    let mut config = AiProfiles::default();
    let get = |key| crate::db::get_setting(conn, key).ok().flatten();
    let mode = get("ai_mode").unwrap_or_default();
    config.daily_limit = get("ai_daily_limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    if mode == "openai" || mode == "ollama" {
        config.active_id = Some("legacy".into());
        config.profiles.push(AiProfile {
            id: "legacy".into(),
            name: "现有配置".into(),
            api_url: get("ai_api_url").unwrap_or_else(|| {
                if mode == "ollama" {
                    "http://localhost:11434".into()
                } else {
                    "https://api.openai.com/v1".into()
                }
            }),
            model: get("ai_model").unwrap_or_default(),
            mode,
            has_api_key: get("ai_api_key").is_some_and(|k| !k.is_empty()),
            api_key: None,
        });
    }
    Ok(config)
}

pub fn validate(config: &mut AiProfiles) -> Result<()> {
    let mut ids = std::collections::HashSet::new();
    for p in &mut config.profiles {
        p.name = p.name.trim().to_owned();
        p.model = p.model.trim().to_owned();
        p.api_url = p.api_url.trim().trim_end_matches('/').to_owned();
        if p.id.is_empty() || p.id == super::local_embedding::PROFILE || !ids.insert(p.id.clone()) {
            bail!("配置 ID 为空或重复");
        }
        if p.name.is_empty() || p.model.is_empty() {
            bail!("配置名称和模型不能为空");
        }
        if !matches!(p.mode.as_str(), "openai" | "ollama") {
            bail!("不支持的 API 协议");
        }
        let url = reqwest::Url::parse(&p.api_url).context("API 地址无效")?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("API 地址须为不含凭据、查询参数或片段的 HTTP(S) 地址");
        }
        if url.scheme() == "http"
            && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        {
            bail!("远程 API 请使用 HTTPS；本机接口可使用 HTTP");
        }
        if p.api_url.ends_with("/chat/completions") || p.api_url.ends_with("/api/chat") {
            bail!("请填写 API 基础地址，不含 /chat/completions 或 /api/chat");
        }
    }
    if config
        .active_id
        .as_ref()
        .is_some_and(|id| !ids.contains(id))
    {
        bail!("默认配置不存在");
    }
    if let Some(embedding) = &mut config.embedding {
        embedding.model = embedding.model.trim().to_owned();
        if embedding.profile_id == super::local_embedding::PROFILE {
            embedding.model = super::local_embedding::MODEL.into();
        } else if !ids.contains(&embedding.profile_id) || embedding.model.is_empty() {
            bail!("请选择向量接口配置并填写向量模型 ID");
        }
        if !(128..=4000).contains(&embedding.chunk_chars) {
            bail!("向量分段长度须为 128 至 4000 字");
        }
    }
    Ok(())
}

fn key_for(conn: &Connection, state: Option<&StoredProfiles>, id: &str) -> Result<Option<String>> {
    if let Some(account) = state.and_then(|s| s.credentials.get(id)) {
        return credentials::get_credential(CredentialType::AiApiKey, account)?
            .map(Some)
            .ok_or_else(|| anyhow::anyhow!("该配置的密钥无法读取，请重新录入"));
    }
    if state.is_none() && id == "legacy" {
        return crate::db::get_setting(conn, "ai_api_key");
    }
    Ok(None)
}

/// Startup/UI metadata must not unlock Keychain just to discard the secret.
pub fn public_config(conn: &Connection) -> Result<AiConfig> {
    let config=read(conn)?;
    let Some(id)=config.active_id.as_deref() else {return Ok(AiConfig::default());};
    let p=config.profiles.iter().find(|p|p.id==id).context("AI 配置不存在，请重新选择")?;
    Ok(AiConfig{mode:p.mode.clone(),api_url:Some(p.api_url.clone()),api_key:None,model:Some(p.model.clone()),daily_limit:Some(config.daily_limit)})
}

pub fn resolve(conn: &Connection, profile_id: Option<&str>) -> Result<AiConfig> {
    let state = stored(conn)?;
    let config = match &state {
        Some(state) => state.config.clone(),
        None => read(conn)?,
    };
    let id = profile_id.or(config.active_id.as_deref());
    let Some(id) = id else {
        return Ok(AiConfig::default());
    };
    let p = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| anyhow::anyhow!("AI 配置不存在，请重新选择"))?;
    Ok(AiConfig {
        mode: p.mode.clone(),
        api_url: Some(p.api_url.clone()),
        api_key: key_for(conn, state.as_ref(), id)?,
        model: Some(p.model.clone()),
        daily_limit: Some(config.daily_limit),
    })
}

pub fn save(conn: &mut Connection, mut config: AiProfiles) -> Result<AiProfiles> {
    validate(&mut config)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let old = stored(&tx)?;
    let mut accounts = std::collections::HashMap::new();
    let mut created = vec![];
    // New immutable credential entries let SQLite commit select one consistent version.
    let result: Result<()> = (|| {
        for p in &mut config.profiles {
            let change = p.api_key.take().or_else(|| {
                if old.is_none() && p.id == "legacy" {
                    crate::db::get_setting(&tx, "ai_api_key").ok().flatten()
                } else {
                    None
                }
            });
            if let Some(key) = change.as_ref().filter(|k| !k.is_empty()) {
                let account = format!("profile-{}", uuid::Uuid::new_v4());
                credentials::store_credential(CredentialType::AiApiKey, &account, key.as_str())?;
                created.push(account.clone());
                if credentials::get_credential(CredentialType::AiApiKey, &account)?.as_deref()
                    != Some(key.as_str())
                {
                    bail!("密钥存储读回失败，配置未保存");
                }
                accounts.insert(p.id.clone(), account);
            } else if change.is_none() {
                if let Some(account) = old.as_ref().and_then(|s| s.credentials.get(&p.id)) {
                    accounts.insert(p.id.clone(), account.clone());
                }
            }
            p.has_api_key = accounts.contains_key(&p.id);
        }
        let state = StoredProfiles {
            config: config.clone(),
            credentials: accounts.clone(),
        };
        tx.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![SETTINGS_KEY, serde_json::to_string(&state)?])?;
        tx.execute("DELETE FROM settings WHERE key='ai_api_key'", [])?;
        Ok(())
    })();
    let committed = result.and_then(|_| tx.commit().map_err(Into::into));
    if let Err(error) = committed {
        for account in created {
            let _ = credentials::delete_credential(CredentialType::AiApiKey, &account);
        }
        return Err(error);
    }
    if let Some(old) = old {
        for account in old
            .credentials
            .values()
            .filter(|a| !accounts.values().any(|v| v == *a))
        {
            if let Err(e) = credentials::delete_credential(CredentialType::AiApiKey, account) {
                log::warn!("清理旧 AI 凭据失败: {e}");
            }
        }
    }
    Ok(config)
}

#[tauri::command]
pub async fn get_ai_profiles() -> Result<AiProfiles, String> {
    crate::commands::run_blocking(|| read(&*crate::db::open_db()?)).await
}

#[tauri::command]
pub async fn save_ai_profiles(config: AiProfiles) -> Result<AiProfiles, String> {
    let saved =
        crate::commands::run_blocking(move || save(&mut *crate::db::open_db()?, config)).await?;
    Ok(saved)
}

#[tauri::command]
pub async fn test_ai_profile(mut profile: AiProfile) -> Result<String, String> {
    let mut candidate = AiProfiles {
        profiles: vec![profile.clone()],
        ..Default::default()
    };
    validate(&mut candidate).map_err(|e| e.to_string())?;
    profile = candidate.profiles.remove(0);
    let key = if let Some(key) = profile.api_key {
        Some(key)
    } else {
        let id = profile.id.clone();
        crate::commands::run_blocking(move || {
            let conn = crate::db::open_db()?;
            key_for(&conn, stored(&conn)?.as_ref(), &id)
        })
        .await?
    };
    let config = AiConfig {
        mode: profile.mode,
        api_url: Some(profile.api_url),
        api_key: key,
        model: Some(profile.model),
        daily_limit: None,
    };
    let response = super::create_backend(&config)
        .chat_completion("", "Reply with OK.")
        .await
        .map_err(|e| e.to_string())?;
    if response.trim().is_empty() {
        return Err("API 返回空内容，请检查模型与接口协议".into());
    }
    Ok(format!("连接成功：{}", config.model.unwrap_or_default()))
}

#[cfg(test)]
mod public_config_tests {
    use super::*;
    #[test]
    fn startup_metadata_does_not_require_the_credential() {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT)").unwrap();
        let config=AiProfiles{profiles:vec![AiProfile{id:"p".into(),name:"示例".into(),mode:"openai".into(),api_url:"https://example.com/v1".into(),model:"test-model".into(),has_api_key:true,api_key:None}],active_id:Some("p".into()),..Default::default()};
        let state=StoredProfiles{config,credentials:std::collections::HashMap::from([("p".into(),"credential-not-loaded-for-metadata".into())])};
        conn.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",params![SETTINGS_KEY,serde_json::to_string(&state).unwrap()]).unwrap();
        let public=public_config(&conn).unwrap();
        assert_eq!(public.model.as_deref(),Some("test-model"));
        assert!(public.api_key.is_none());
    }
}

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::time::Duration;

use super::profiles;

#[derive(Clone)]
pub struct EmbeddingPlan {
    pub profile_id: String,
    pub mode: String,
    pub api_url: String,
    pub model: String,
    pub chunk_chars: usize,
    pub fingerprint: String,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl EmbeddingPlan {
    pub fn load(conn: &Connection) -> Result<Option<Self>> {
        let mut config = profiles::read(conn)?;
        profiles::validate(&mut config)?;
        let Some(settings) = config.embedding else {
            return Ok(None);
        };
        let profile = config
            .profiles
            .iter()
            .find(|p| p.id == settings.profile_id)
            .context("向量接口配置不存在")?;
        let fingerprint = digest(&serde_json::to_vec(&(
            "knowledge-chunks-v1",
            &profile.id,
            &profile.mode,
            &profile.api_url,
            &settings.model,
            settings.chunk_chars,
        ))?);
        Ok(Some(Self {
            profile_id: settings.profile_id,
            mode: profile.mode.clone(),
            api_url: profile.api_url.clone(),
            model: settings.model,
            chunk_chars: settings.chunk_chars as usize,
            fingerprint,
        }))
    }

    pub fn require(conn: &Connection) -> Result<Self> {
        Self::load(conn)?.context("尚未配置向量接口，请在 AI 设置中选择接口和向量模型")
    }

    pub fn client(&self, conn: &Connection) -> Result<EmbeddingClient> {
        let resolved = profiles::resolve(conn, Some(&self.profile_id))?;
        // A saved profile may have changed between reading the plan and resolving its key.
        if resolved.mode != self.mode || resolved.api_url.as_deref() != Some(self.api_url.as_str())
        {
            bail!("向量接口配置已变化，请重试");
        }
        EmbeddingClient::new(self, resolved.api_key.as_deref())
    }
}

pub struct EmbeddingClient {
    client: reqwest::Client,
    url: String,
    model: String,
    mode: String,
    key: Option<String>,
}

impl EmbeddingClient {
    pub fn new(plan: &EmbeddingPlan, key: Option<&str>) -> Result<Self> {
        let url = reqwest::Url::parse(&plan.api_url)?;
        let loopback = url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30));
        if loopback {
            builder = builder.no_proxy();
        }
        Ok(Self {
            client: builder.build()?,
            url: format!(
                "{}{}",
                plan.api_url.trim_end_matches('/'),
                if plan.mode == "ollama" {
                    "/api/embed"
                } else {
                    "/embeddings"
                }
            ),
            mode: plan.mode.clone(),
            model: plan.model.clone(),
            key: key.filter(|key| !key.is_empty()).map(str::to_owned),
        })
    }

    pub async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() || inputs.len() > 8 {
            bail!("向量请求每批须为 1 至 8 段");
        }
        let mut body = serde_json::json!({"model": self.model, "input": inputs});
        if self.mode == "ollama" {
            body["truncate"] = false.into();
        } else {
            body["encoding_format"] = "float".into();
        }
        let mut request = self.client.post(&self.url).json(&body);
        if let Some(key) = &self.key {
            request = request.bearer_auth(key);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("向量接口连接失败或超时"))?;
        if !response.status().is_success() {
            bail!("向量接口返回 HTTP {}", response.status().as_u16());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| anyhow::anyhow!("向量接口响应读取失败"))?
        {
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                bail!("向量接口响应超过 8 MiB");
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).context("向量接口未返回有效 JSON")?;
        parse_response(&value, &self.mode, inputs.len())
    }
}

fn parse_response(value: &serde_json::Value, mode: &str, count: usize) -> Result<Vec<Vec<f32>>> {
    let mut ordered = vec![None; count];
    let entries = value[if mode == "ollama" {
        "embeddings"
    } else {
        "data"
    }]
    .as_array()
    .context("向量响应缺少数组")?;
    if entries.len() != count {
        bail!("向量响应数量与请求不一致");
    }
    let mut dimension = None;
    for (position, entry) in entries.iter().enumerate() {
        let (index, raw) = if mode == "ollama" {
            (position, entry)
        } else {
            let index = entry["index"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .context("向量响应缺少有效索引")?;
            (index, &entry["embedding"])
        };
        if index >= count || ordered[index].is_some() {
            bail!("向量响应索引重复或越界");
        }
        let raw = raw.as_array().context("向量数据格式无效")?;
        if raw.is_empty() || raw.len() > 8192 || dimension.is_some_and(|d| d != raw.len()) {
            bail!("向量维数无效或不一致");
        }
        dimension = Some(raw.len());
        let vector: Vec<f32> = raw
            .iter()
            .map(|n| {
                n.as_f64()
                    .map(|n| n as f32)
                    .filter(|n| n.is_finite())
                    .context("向量含无效数值")
            })
            .collect::<Result<_>>()?;
        if vector.iter().all(|n| *n == 0.0) {
            bail!("向量接口返回零向量");
        }
        ordered[index] = Some(vector);
    }
    ordered
        .into_iter()
        .map(|v| v.context("向量响应缺失输入段落"))
        .collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut aa = 0.0f64;
    let mut bb = 0.0f64;
    for (&x, &y) in a.iter().zip(b) {
        if !x.is_finite() || !y.is_finite() {
            return 0.0;
        }
        dot += x as f64 * y as f64;
        aa += (x as f64).powi(2);
        bb += (y as f64).powi(2);
    }
    if aa == 0.0 || bb == 0.0 {
        0.0
    } else {
        dot / (aa * bb).sqrt()
    }
}

#[tauri::command]
pub async fn test_embedding_connection() -> Result<String, String> {
    let (plan, client) = crate::commands::run_blocking(|| {
        let conn = crate::db::open_db()?;
        let plan = EmbeddingPlan::require(&conn)?;
        let client = plan.client(&conn)?;
        Ok((plan, client))
    })
    .await?;
    let vectors = client
        .embed(&["向量接口连接测试".into()])
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!("{}：{} 维，连接成功", plan.model, vectors[0].len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn validates_indexes_dimensions_and_numbers() {
        let result = parse_response(
            &json!({"data":[{"index":1,"embedding":[0,1]},{"index":0,"embedding":[1,0]}]}),
            "openai",
            2,
        )
        .unwrap();
        assert_eq!(result, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        for data in [
            json!({"data":[]}),
            json!({"data":[{"index":0,"embedding":[0,0]}]}),
            json!({"data":[{"index":0,"embedding":["bad",1]}]}),
            json!({"data":[{"index":2,"embedding":[1,1]}]}),
        ] {
            assert!(parse_response(&data, "openai", 1).is_err());
        }
        assert!(cosine(&[f32::MAX, f32::MAX], &[f32::MAX, f32::MAX]) > 0.99);
    }
}

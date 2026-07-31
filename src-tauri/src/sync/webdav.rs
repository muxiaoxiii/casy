use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

pub struct WebDavClient {
    base_url: String,
    username: String,
    password: String,
    client: Client,
}

#[allow(dead_code)]
impl WebDavClient {
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(300))
            .build()?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    /// HEAD 请求，返回 ETag
    pub async fn head(&self, path: &str) -> Result<Option<String>> {
        let resp = self
            .client
            .head(self.url(path))
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        if resp.status() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            anyhow::bail!("HEAD {} failed: {}", path, resp.status());
        }

        Ok(resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()))
    }

    /// PUT 上传
    pub async fn put(&self, path: &str, data: &[u8]) -> Result<String> {
        let resp = self
            .client
            .put(self.url(path))
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/octet-stream")
            .body(data.to_vec())
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("PUT {} failed: {}", path, resp.status());
        }

        Ok(resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string())
    }

    /// GET 下载
    pub async fn get(&self, path: &str) -> Result<(Vec<u8>, String)> {
        let resp = self
            .client
            .get(self.url(path))
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("GET {} failed: {}", path, resp.status());
        }

        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.bytes().await?.to_vec();

        Ok((body, etag))
    }

    /// MKCOL 创建目录
    pub async fn mkcol(&self, path: &str) -> Result<()> {
        let resp = self
            .client
            .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), self.url(path))
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        // 405 = 已存在，也算成功
        if !resp.status().is_success() && resp.status() != 405 {
            anyhow::bail!("MKCOL {} failed: {}", path, resp.status());
        }
        Ok(())
    }

    /// MOVE 原子操作（用于临时文件 → 正式文件）
    pub async fn move_resource(&self, from_path: &str, to_path: &str) -> Result<()> {
        let from_url = self.url(from_path);
        let to_url = self.url(to_path);

        let resp = self
            .client
            .request(reqwest::Method::from_bytes(b"MOVE").unwrap(), &from_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Destination", &to_url)
            .header("Overwrite", "T")
            .send()
            .await?;

        if !resp.status().is_success() && resp.status() != 201 && resp.status() != 204 {
            anyhow::bail!("MOVE {} -> {} failed: {}", from_path, to_path, resp.status());
        }
        Ok(())
    }

    /// 带 If-Match 条件的 PUT（冲突检测）
    pub async fn put_if_match(&self, path: &str, data: &[u8], etag: &str) -> Result<String> {
        let resp = self
            .client
            .put(self.url(path))
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/octet-stream")
            .header("If-Match", etag)
            .body(data.to_vec())
            .send()
            .await?;

        if resp.status() == 412 {
            anyhow::bail!("ETag conflict: remote file has been modified");
        }
        if !resp.status().is_success() {
            anyhow::bail!("PUT {} failed: {}", path, resp.status());
        }

        Ok(resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string())
    }
}

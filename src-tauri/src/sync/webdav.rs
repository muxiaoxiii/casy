use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

pub struct WebDavClient {
    base_url: String,
    username: String,
    password: String,
    client: Client,
}

// A fixed request deadline also counts time spent uploading. Reset the idle
// budget as the transport consumes each bounded file chunk instead.
async fn upload_with_idle_timeout<F, T>(
    request: F,
    mut progress: tokio::sync::watch::Receiver<tokio::time::Instant>,
    idle: Duration,
) -> Result<T>
where F: std::future::Future<Output = reqwest::Result<T>> {
    tokio::pin!(request);
    let mut deadline = *progress.borrow_and_update() + idle;
    let mut streaming = true;
    loop {
        tokio::select! {
            result = &mut request => return Ok(result?),
            change = progress.changed(), if streaming => {
                if change.is_ok() { deadline = *progress.borrow_and_update() + idle; }
                else { streaming = false; }
            },
            _ = tokio::time::sleep_until(deadline) => anyhow::bail!("上传长时间无进展，请检查网络后重试"),
        }
    }
}

#[allow(dead_code)]
impl WebDavClient {
    /// Full archives use file streams, with no total transfer deadline.
    pub fn for_archive(base_url: &str, username: &str, password: &str) -> Result<Self> {
        let mut client = Self::new(base_url, username, password)?;
        client.client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .build()?;
        Ok(client)
    }

    pub async fn put_file(&self, path: &str, source: &std::path::Path) -> Result<()> {
        use tokio::io::AsyncReadExt;
        let file = tokio::fs::File::open(source).await?;
        let size = file.metadata().await?.len();
        let (progress, receiver) = tokio::sync::watch::channel(tokio::time::Instant::now());
        let stream = futures_util::stream::try_unfold((file, progress), |(mut file, progress)| async move {
            let mut chunk = vec![0; 64 * 1024];
            let count = file.read(&mut chunk).await?;
            if count == 0 { return Ok::<_, std::io::Error>(None); }
            chunk.truncate(count);
            let _ = progress.send(tokio::time::Instant::now());
            Ok(Some((chunk, (file, progress))))
        });
        let request = self.client.put(self.url(path))
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/octet-stream")
            .header("Content-Length", size)
            .body(reqwest::Body::wrap_stream(stream)).send();
        let response = upload_with_idle_timeout(request, receiver, Duration::from_secs(300)).await?;
        anyhow::ensure!(response.status().is_success(), "备份上传失败：{}", response.status());
        Ok(())
    }

    pub async fn get_file(&self, path: &str, destination: &std::path::Path) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        let mut response = tokio::time::timeout(Duration::from_secs(300), self.client.get(self.url(path))
            .basic_auth(&self.username, Some(&self.password)).send()).await??;
        anyhow::ensure!(response.status() != 404, "此 WebDAV 目录尚无完整备份，请先备份全部数据");
        anyhow::ensure!(response.status().is_success(), "备份下载失败：{}", response.status());
        let mut file = tokio::fs::OpenOptions::new().write(true).create_new(true)
            .open(destination).await?;
        while let Some(chunk) = tokio::time::timeout(Duration::from_secs(300), response.chunk()).await?? {
            file.write_all(&chunk).await?;
        }
        file.sync_all().await?;
        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let response = self.client.delete(self.url(path))
            .timeout(Duration::from_secs(30))
            .basic_auth(&self.username, Some(&self.password)).send().await?;
        anyhow::ensure!(response.status().is_success() || response.status() == 404, "临时上传清理失败");
        Ok(())
    }

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
            .request(
                reqwest::Method::from_bytes(b"MKCOL").unwrap(),
                self.url(path),
            )
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
            .timeout(Duration::from_secs(300))
            .send()
            .await?;

        if !resp.status().is_success() && resp.status() != 201 && resp.status() != 204 {
            anyhow::bail!(
                "MOVE {} -> {} failed: {}",
                from_path,
                to_path,
                resp.status()
            );
        }
        Ok(())
    }

    /// 带 If-Match 条件的 PUT（冲突检测，S-4）
    ///
    /// 412 = 远程文件在比对之后已被其他设备修改：明确报冲突，
    /// 由调用方提示用户重新比对，绝不无条件覆盖远程数据。
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
            anyhow::bail!("{}", crate::sync::WEBDAV_CONFLICT_MESSAGE);
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

#[cfg(test)]
mod archive_transfer_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn upload_deadline_tracks_progress_instead_of_total_duration() {
        let (sender, receiver) = tokio::sync::watch::channel(tokio::time::Instant::now());
        let progress = tokio::spawn(async move {
            for _ in 0..4 {
                tokio::time::sleep(Duration::from_millis(400)).await;
                sender.send(tokio::time::Instant::now()).unwrap();
            }
        });
        let request = async {
            tokio::time::sleep(Duration::from_millis(1800)).await;
            Ok::<_, reqwest::Error>(())
        };
        upload_with_idle_timeout(request, receiver, Duration::from_secs(1)).await.unwrap();
        progress.await.unwrap();
        let (_sender, receiver) = tokio::sync::watch::channel(tokio::time::Instant::now());
        let result = upload_with_idle_timeout(
            std::future::pending::<reqwest::Result<()>>(), receiver, Duration::from_millis(30),
        ).await;
        assert!(result.unwrap_err().to_string().contains("长时间无进展"));
    }

    // A local protocol fixture; no user WebDAV credentials or remote data involved.
    async fn server(request_count: usize) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let mut files = std::collections::HashMap::<String, Vec<u8>>::new();
            for _ in 0..request_count {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    header.push(socket.read_u8().await.unwrap());
                }
                let header = String::from_utf8(header).unwrap();
                assert!(header.to_ascii_lowercase().contains("authorization: basic dXNlcjpwYXNz".to_ascii_lowercase().as_str()));
                let mut first = header.lines().next().unwrap().split_whitespace();
                let method = first.next().unwrap();
                let path = first.next().unwrap();
                let value = |key: &str| header.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case(key).then(|| value.trim().to_owned())
                });
                let len: usize = value("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
                let mut body = vec![0; len];
                socket.read_exact(&mut body).await.unwrap();
                let (status, output) = match method {
                    "PUT" => { files.insert(path.into(), body); (201, vec![]) },
                    "MOVE" => {
                        let destination = reqwest::Url::parse(&value("destination").unwrap()).unwrap();
                        let body = files.remove(path).unwrap();
                        files.insert(destination.path().into(), body);
                        (204, vec![])
                    },
                    "GET" if path == "/truncated" => {
                        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nshort").await.unwrap();
                        continue;
                    },
                    "GET" => files.get(path).map(|v| (200, v.clone())).unwrap_or((404, vec![])),
                    "DELETE" => { files.remove(path); (204, vec![]) },
                    _ => panic!("unexpected method: {method}"),
                };
                socket.write_all(format!("HTTP/1.1 {status} Result\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", output.len()).as_bytes()).await.unwrap();
                for chunk in output.chunks(32768) { socket.write_all(chunk).await.unwrap(); }
            }
        });
        (url, handle)
    }

    #[tokio::test]
    async fn archive_stream_roundtrip_and_failure_do_not_clobber_local_files() {
        let (url, server) = server(6).await;
        let client = WebDavClient::for_archive(&url, "user", "pass").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.casy");
        let target = dir.path().join("target.casy");
        let content: Vec<u8> = (0..2_000_000).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &content).unwrap();
        client.put_file(".upload", &source).await.unwrap();
        client.move_resource(".upload", "casy-full-backup.casy").await.unwrap();
        client.get_file("casy-full-backup.casy", &target).await.unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), content);
        assert!(client.get_file("missing.casy", &target).await.unwrap_err().to_string().contains("尚无完整备份"));
        assert_eq!(std::fs::read(&target).unwrap(), content);
        assert!(client.get_file("truncated", &dir.path().join("incomplete.casy")).await.is_err());
        client.delete(".upload").await.unwrap();
        server.await.unwrap();
    }
}

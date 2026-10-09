use anyhow::{bail, Context, Result};
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    sync::Mutex,
};

pub const PROFILE: &str = "builtin-e5-small";
pub const MODEL: &str = "multilingual-e5-small-int8";
struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
static WORKER: Mutex<Option<Worker>> = Mutex::const_new(None);

pub async fn embed(inputs: &[String], query: bool) -> Result<Vec<Vec<f32>>> {
    let mut worker = WORKER.lock().await;
    if worker.is_none() {
        let executable =
            crate::document_pipeline::engine_executable().context("本地文档引擎未安装")?;
        // 默认 multilingual-e5-small-int8（384 维，约 135MB，日/韩/法/德/中/英同家族覆盖）；
        // 未安装时回退 e5-base（768 维，约 281MB）。维度变化由向量索引指纹触发重建。
        let model = ["models/embedding-e5-small", "models/embedding-e5-base"]
            .iter()
            .find_map(|relative| {
                crate::runtime_paths::runtime_asset("CASY_EMBEDDING_MODEL_DIR", relative)
                    .filter(|p| p.join("model_int8.onnx").is_file() && p.join("tokenizer.json").is_file())
            })
            .context("本地向量模型缺失，请安装完整 beta 包")?;
        let mut child = tokio::process::Command::new(executable)
            .arg("embed")
            .env("CASY_EMBEDDING_MODEL_DIR", model)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let input = child.stdin.take().context("无法连接本地向量引擎")?;
        let output = BufReader::new(child.stdout.take().context("无法读取本地向量引擎")?);
        *worker = Some(Worker {
            child,
            input,
            output,
        });
    }
    // Ownership stays in this future so cancellation kills the child and cannot leave a stale reply.
    let mut active = worker.take().unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        let mut payload = serde_json::to_vec(&serde_json::json!({"inputs":inputs,"query":query}))?;
        payload.push(b'\n');
        active.input.write_all(&payload).await?;
        active.input.flush().await?;
        let mut line = String::new();
        (&mut active.output)
            .take(256000)
            .read_line(&mut line)
            .await?;
        if line.is_empty() || line.len() >= 256000 {
            bail!("本地向量引擎退出或响应无效");
        }
        let value: serde_json::Value = serde_json::from_str(&line)?;
        if let Some(error) = value["error"].as_str() {
            bail!("{error}");
        }
        super::embeddings::parse_response(&value, "ollama", inputs.len())
    })
    .await
    .unwrap_or_else(|_| Err(anyhow::anyhow!("本地向量计算超时")));
    if result.is_err() {
        let _ = active.child.kill().await;
    } else {
        *worker = Some(active);
    }
    result
}

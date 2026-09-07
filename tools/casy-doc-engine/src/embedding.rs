use anyhow::{bail, Context, Result};
use ort::{session::Session, value::Tensor};
use serde::Deserialize;
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
};
use tokenizers::Tokenizer;

#[derive(Deserialize)]
struct Request {
    inputs: Vec<String>,
    #[serde(default)]
    query: bool,
}

struct Model {
    session: Session,
    tokenizer: Tokenizer,
}
impl Model {
    fn load() -> Result<Self> {
        let root = std::env::var_os("CASY_EMBEDDING_MODEL_DIR")
            .map(PathBuf::from)
            .context("本地向量模型目录未配置")?;
        let mut tokenizer = Tokenizer::from_file(root.join("tokenizer.json"))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        tokenizer
            .with_truncation(None)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        tokenizer.with_padding(None);
        let session = Session::builder()?
            .with_intra_threads(2)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            .commit_from_file(root.join("model_int8.onnx"))?;
        Ok(Self { session, tokenizer })
    }

    fn embed(&mut self, request: Request) -> Result<Vec<Vec<f32>>> {
        if request.inputs.is_empty()
            || request.inputs.len() > 8
            || request.inputs.iter().any(|s| s.len() > 64000)
        {
            bail!("向量输入须为 1 至 8 段，每段不超过 64 KB");
        }
        let label = if request.query { "query:" } else { "passage:" };
        let prefix = self
            .tokenizer
            .encode(label, false)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let bos = self
            .tokenizer
            .token_to_id("<s>")
            .context("模型缺少起始标记")?;
        let eos = self
            .tokenizer
            .token_to_id("</s>")
            .context("模型缺少结束标记")?;
        let capacity = 512 - prefix.len() - 2;
        let mut vectors = Vec::new();
        for input in request.inputs {
            if input.trim().is_empty() {
                bail!("不能索引空文本");
            }
            let encoding = self
                .tokenizer
                .encode(format!("{label} {input}"), false)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let tokens = encoding
                .get_ids()
                .get(prefix.len()..)
                .filter(|ids| !ids.is_empty())
                .context("无有效文本标记")?;
            let mut start = 0;
            let mut pooled = Vec::<f32>::new();
            // Encode every token, including the tail of unusually long multilingual input.
            loop {
                let end = (start + capacity).min(tokens.len());
                let ids: Vec<i64> = std::iter::once(bos)
                    .chain(prefix.get_ids().iter().copied())
                    .chain(tokens[start..end].iter().copied())
                    .chain(std::iter::once(eos))
                    .map(i64::from)
                    .collect();
                let length = ids.len();
                let mut tensors = ort::inputs![
                    "input_ids" => Tensor::from_array(([1usize, length], ids))?,
                    "attention_mask" => Tensor::from_array(([1usize, length], vec![1i64; length]))?,
                ];
                if self
                    .session
                    .inputs()
                    .iter()
                    .any(|input| input.name() == "token_type_ids")
                {
                    tensors.push((
                        "token_type_ids".into(),
                        Tensor::from_array(([1usize, length], vec![0i64; length]))?.into(),
                    ));
                }
                let outputs = self.session.run(tensors)?;
                let (shape, hidden) = outputs[0].try_extract_tensor::<f32>()?;
                if shape.len() != 3
                    || shape[0] != 1
                    || shape[1] != length as i64
                    || ![384, 768].contains(&shape[2])
                {
                    bail!("本地向量模型输出维数无效");
                }
                let dimension = shape[2] as usize;
                if pooled.is_empty() {
                    pooled.resize(dimension, 0.0);
                }
                if pooled.len() != dimension {
                    bail!("本地模型维数发生变化");
                }
                let weight = (end - start) as f32 / length as f32;
                for token in hidden.chunks_exact(dimension) {
                    for (sum, value) in pooled.iter_mut().zip(token) {
                        *sum += value * weight;
                    }
                }
                if end == tokens.len() {
                    break;
                }
                start = end - 32;
            }
            let norm = pooled
                .iter()
                .map(|n| (*n as f64).powi(2))
                .sum::<f64>()
                .sqrt();
            if !norm.is_finite() || norm == 0.0 {
                bail!("本地模型返回无效向量");
            }
            for value in &mut pooled {
                *value = (*value as f64 / norm) as f32;
            }
            vectors.push(pooled);
        }
        Ok(vectors)
    }
}

pub fn serve() -> Result<()> {
    let mut model = Model::load()?;
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = std::io::stdout().lock();
    loop {
        let mut line = String::new();
        if (&mut reader).take(600000).read_line(&mut line)? == 0 {
            break;
        }
        if line.len() >= 600000 {
            bail!("向量请求过大");
        }
        let result = serde_json::from_str(&line)
            .map_err(anyhow::Error::from)
            .and_then(|r| model.embed(r));
        let response = match result {
            Ok(vectors) => serde_json::json!({"embeddings":vectors}),
            Err(e) => serde_json::json!({"error":e.to_string()}),
        };
        writeln!(stdout, "{}", response)?;
        stdout.flush()?;
    }
    Ok(())
}

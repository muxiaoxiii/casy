use crate::document_pipeline;
use anyhow::{bail, Context, Result};
use std::{io::Write, path::PathBuf};

#[tauri::command]
pub async fn convert_file_to_markdown(
    source_path: String,
    output_dir: String,
) -> Result<serde_json::Value, String> {
    convert(source_path, output_dir)
        .await
        .map_err(|e| e.to_string())
}

async fn convert(source_path: String, output_dir: String) -> Result<serde_json::Value> {
    let (source, destination, hash) = super::run_blocking(move || {
        let source = PathBuf::from(source_path);
        let destination = PathBuf::from(output_dir);
        if !source.is_absolute() || !destination.is_absolute() {
            bail!("请选择源文件和输出目录");
        }
        let source = source.canonicalize()?;
        let destination = destination.canonicalize()?;
        if !source.is_file() || !destination.is_dir() {
            bail!("源文件或输出目录不可用");
        }
        if !document_pipeline::supports_path(&source) {
            bail!("暂不支持此文件格式");
        }
        let hash = document_pipeline::sha256_file(&source)?;
        Ok((source, destination, hash))
    })
    .await
    .map_err(anyhow::Error::msg)?;
    let temporary = tempfile::tempdir()?;
    let request = document_pipeline::process_request(
        &crate::db::new_id(),
        &source.to_string_lossy(),
        &hash,
        temporary.path(),
    );
    let result = document_pipeline::run_engine(request).await?;
    let count = result.pages.len();
    super::run_blocking(move || {
        if document_pipeline::sha256_file(&source)? != hash { bail!("源文件在转换期间发生变化，请重试"); }
        let markdown = std::fs::read(&result.markdown_path).context("无法读取转换结果")?;
        let stem = crate::files::sanitize_filename(source.file_stem().and_then(|s| s.to_str()).unwrap_or("document"));
        let stem: String = stem.chars().take(80).collect();
        let mut output = tempfile::NamedTempFile::new_in(&destination)?;
        output.write_all(&markdown)?;
        output.as_file().sync_all()?;
        for suffix in 0..10000 {
            let name = if suffix == 0 { format!("{stem}.md") } else { format!("{stem} ({suffix}).md") };
            let path = destination.join(name);
            match output.persist_noclobber(&path) {
                Ok(_) => return Ok(serde_json::json!({"outputPath":path.to_string_lossy(),"pages":count,"bytes":markdown.len()})),
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => output = error.file,
                Err(error) => return Err(error.error.into()),
            }
        }
        bail!("同名文件过多，请选择其他输出目录")
    }).await.map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn conversion_preserves_source_and_existing_output() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("证据.md");
        let text = "# Prüfung français 日本語\n\n| 项目 | 金额 |\n|---|---|\n| 赔偿 | 100 |";
        std::fs::write(&source, text).unwrap();
        let result = convert(
            source.display().to_string(),
            root.path().display().to_string(),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read_to_string(&source).unwrap(), text);
        assert!(result["outputPath"]
            .as_str()
            .unwrap()
            .ends_with("证据 (1).md"));
        assert_eq!(
            std::fs::read_to_string(result["outputPath"].as_str().unwrap()).unwrap(),
            text
        );
        assert!(convert(source.display().to_string(), "relative".into())
            .await
            .is_err());
    }
}

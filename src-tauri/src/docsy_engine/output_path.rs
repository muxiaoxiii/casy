use anyhow::Result;
use std::path::{Path, PathBuf};

/// 解析并校验「显式指定」的导出路径。
///
/// 安全（审查 P0-4 / P0-5）：`export_docx`、`export_edited_docx`、
/// `export_knowledge_markdown` 三个命令原本都把前端传入的 `output_path`
/// 直接 `PathBuf::from` 后交给 `File::create` / `fs::write`，且
/// `export.rs` 还会 `create_dir_all(parent)`——构成任意文件写入原语：
/// 一旦 webview 取得脚本执行权，即可在任意位置创建目录并写入文件。
///
/// 此处统一收敛三项最小校验：
///   1. 必须是绝对路径（系统保存对话框返回的必然是绝对路径）
///   2. 扩展名必须在允许清单内（阻断投递 `.command` / `.sh` / `.bashrc` 等）
///   3. 父目录先 `canonicalize` 再拼回文件名，消除 `..` 与符号链接穿越；
///      且要求父目录已存在，不静默创建任意目录树
///
/// 注意：本函数**不限制**导出位置——用户通过系统保存对话框选择桌面、
/// 案件目录或网络盘属正常用法。若后续需要进一步收敛到应用托管目录，
/// 只需在此处追加 `starts_with(export_root())` 判定，三个命令同时生效。
pub fn resolve_explicit_output_path(path: &str, allowed_extensions: &[&str]) -> Result<PathBuf> {
    let requested = Path::new(path);

    if !requested.is_absolute() {
        anyhow::bail!("导出路径必须是绝对路径: {path}");
    }

    let extension = requested
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !allowed_extensions
        .iter()
        .any(|allowed| extension == *allowed)
    {
        anyhow::bail!(
            "导出路径扩展名必须是 {} 之一: {path}",
            allowed_extensions.join(" / ")
        );
    }

    let file_name = requested
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("导出路径缺少文件名: {path}"))?;

    let parent = requested
        .parent()
        .ok_or_else(|| anyhow::anyhow!("导出路径缺少父目录: {path}"))?;
    let parent = std::fs::canonicalize(parent)
        .map_err(|_| anyhow::anyhow!("导出目录不存在: {}", parent.display()))?;

    let output = parent.join(file_name);

    // 安全（审查 P0-4 / P0-5 硬化）：若目标已存在且是符号链接，拒绝跟随写入。
    // 父目录已 canonicalize，消除了目录级穿越；但目标文件名本身可能是
    // 一条指向受管目录之外的已有符号链接，而 `fs::write` / `File::create`
    // 会跟随它。用 `symlink_metadata`（不跟随）探测目标自身，命中即拒绝。
    // 目标不存在（常规新建）或为普通文件时放行。
    if let Ok(meta) = std::fs::symlink_metadata(&output) {
        if meta.file_type().is_symlink() {
            anyhow::bail!("导出目标已是符号链接，拒绝覆盖: {}", output.display());
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_path() {
        assert!(resolve_explicit_output_path("out.docx", &["docx"]).is_err());
    }

    #[test]
    fn rejects_extension_outside_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.command");
        assert!(resolve_explicit_output_path(path.to_str().unwrap(), &["docx"]).is_err());
    }

    #[test]
    fn normalizes_traversal_in_parent() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let path = nested
            .join("..")
            .join("nested")
            .join("..")
            .join("report.docx");
        let resolved = resolve_explicit_output_path(path.to_str().unwrap(), &["docx"]).unwrap();
        // `..` 已被消除：解析结果应落回临时目录自身，且不带任何 `..` 成分。
        // 注意比对前先 canonicalize 期望值——macOS 上 tempdir 位于 /var，
        // 而 canonicalize 会解到 /private/var。
        let expected = std::fs::canonicalize(dir.path()).unwrap();
        assert!(!resolved.components().any(|c| c.as_os_str() == ".."));
        assert_eq!(resolved, expected.join("report.docx"));
    }

    #[test]
    fn allows_existing_regular_target() {
        // 目标已存在但为普通文件 → 放行（避免把「覆盖既有输出」误判为攻击）
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("report.docx");
        std::fs::write(&target, b"existing").unwrap();
        let resolved = resolve_explicit_output_path(target.to_str().unwrap(), &["docx"]).unwrap();
        assert!(resolved.ends_with("report.docx"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_pre_existing_symlink_target() {
        // 安全（审查 P0-4/P0-5 硬化）：目标名是一条指向外部文件的符号链接时，
        // 若放行，`fs::write` 会跟随链接把内容写到别处。必须拒绝。
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, b"boom").unwrap();
        let link = nested.join("report.docx");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let err = resolve_explicit_output_path(link.to_str().unwrap(), &["docx"]).unwrap_err();
        assert!(err.to_string().contains("符号链接"));
    }
}

use super::{backup::calculate_file_sha256, backup::verify_db_file_integrity, run_blocking};
use crate::{db, runtime_paths};
use age::secrecy::SecretString;
use anyhow::{anyhow, bail, Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use zip::{write::FileOptions, ZipArchive, ZipWriter};

#[derive(Serialize, Deserialize)]
struct Root {
    original: PathBuf,
    archived: String,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    database_key: String,
    roots: Vec<Root>,
    entries: Vec<Entry>,
}

fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn open_keyed(path: &Path, key: &str) -> Result<Connection> {
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("备份数据库密钥格式无效");
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(&format!(
        "PRAGMA key = \"x'{key}'\"; PRAGMA busy_timeout=5000;"
    ))?;
    Ok(conn)
}

fn tables(conn: &Connection) -> Result<Vec<(String, Vec<String>)>> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_list WHERE schema='main' AND type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
    let names = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut result = Vec::new();
    for name in names {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", ident(&name)))?;
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        result.push((name, cols));
    }
    Ok(result)
}

fn ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn local_reference(destination: &str) -> Option<PathBuf> {
    if destination.starts_with("file:") {
        reqwest::Url::parse(destination).ok()?.to_file_path().ok()
    } else {
        let path = PathBuf::from(destination);
        path.is_absolute().then_some(path)
    }
}

fn reference_source(destination: &str) -> Option<String> {
    if !destination.starts_with("file:") {
        return portable_absolute_path(destination).then(|| destination.to_owned());
    }
    let url = reqwest::Url::parse(destination).ok()?;
    let bytes = url.path().as_bytes();
    let mut decoded = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let path = String::from_utf8(decoded).ok()?;
    if let Some(host) = url.host_str().filter(|host| *host != "localhost") {
        return Some(format!("//{host}{path}"));
    }
    if path.len() >= 4 && path.as_bytes()[2] == b':' {
        Some(path[1..].to_owned())
    } else {
        Some(path)
    }
}

fn markdown_destination(path: &str) -> Option<String> {
    // File URLs avoid Markdown escapes, spaces and parentheses in native paths.
    let path = portable_path(path);
    let mut url = reqwest::Url::parse("file:///").ok()?;
    if let Some(unc) = path.strip_prefix("//") {
        let (host, tail) = unc.split_once('/')?;
        url.set_host(Some(host)).ok()?;
        url.set_path(&format!("/{tail}"));
    } else {
        url.set_path(&path);
    }
    Some(url.to_string().replace('(', "%28").replace(')', "%29"))
}

fn markdown_references(value: &str) -> Vec<(std::ops::Range<usize>, String)> {
    use pulldown_cmark::{Event, LinkType, Parser, Tag};
    let parser = Parser::new(value);
    let mut references: Vec<_> = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| (definition.span.clone(), definition.dest.to_string()))
        .collect();
    for (event, range) in parser.into_offset_iter() {
        if let Event::Start(
            Tag::Link {
                link_type: LinkType::Inline,
                dest_url,
                ..
            }
            | Tag::Image {
                link_type: LinkType::Inline,
                dest_url,
                ..
            },
        ) = event
        {
            // Limit the search to the destination part, excluding the visible label.
            if let Some(offset) = value[range.clone()].find("](") {
                references.push((range.start + offset + 2..range.end, dest_url.to_string()));
            }
        }
    }
    references
}

pub(crate) fn relocate_markdown(value: &str, mappings: &[(PathBuf, PathBuf)]) -> String {
    let mut replacements = Vec::new();
    for (range, destination) in markdown_references(value) {
        let Some(path) = reference_source(&destination) else {
            continue;
        };
        let relocated = relocate_string(&path, mappings);
        if relocated == path {
            continue;
        }
        let replacement = if destination.starts_with("file:")
            || relocated.contains(['\\', ' ', '(', ')', '#', '%'])
        {
            markdown_destination(&relocated)
        } else {
            Some(relocated)
        };
        if let (Some(replacement), Some(offset)) =
            (replacement, value[range.clone()].find(&destination))
        {
            replacements.push((
                range.start + offset..range.start + offset + destination.len(),
                replacement,
            ));
        }
    }
    replacements.sort_by_key(|a| std::cmp::Reverse(a.0.start));
    let mut result = value.to_string();
    for (range, replacement) in replacements {
        result.replace_range(range, &replacement);
    }
    result
        .split_inclusive('\n')
        .map(|original| {
            let (line, ending) = if let Some(line) = original.strip_suffix("\r\n") {
                (line, "\r\n")
            } else if let Some(line) = original.strip_suffix('\n') {
                (line, "\n")
            } else {
                (original, "")
            };
            if let Some(path) = line.strip_prefix("> 可搜索 PDF：") {
                format!("> 可搜索 PDF：{}{ending}", relocate_string(path, mappings))
            } else {
                original.to_string()
            }
        })
        .collect()
}

fn collect_roots(conn: &Connection) -> Result<Vec<PathBuf>> {
    let running: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM document_processing_jobs WHERE status='running')",
        [],
        |r| r.get(0),
    )?;
    if running {
        bail!("文档处理进行中，请完成或取消后再备份、恢复");
    }
    let mut candidates = BTreeSet::new();
    let documents = runtime_paths::documents_root();
    if documents.exists() {
        candidates.insert(documents);
    }
    let data = runtime_paths::data_root();
    // Include user-created data directories, including earlier restored material, but no runtimes or backups.
    for item in fs::read_dir(&data)? {
        let item = item?;
        let name = item.file_name();
        if ["backups", "bin", "models", "logs", "indexes"]
            .iter()
            .any(|s| name == *s)
            || name.to_string_lossy().starts_with('.')
        {
            continue;
        }
        if item.file_type()?.is_dir() {
            candidates.insert(item.path());
        }
    }
    for (table, cols) in tables(conn)? {
        for col in cols
            .iter()
            .filter(|c| matches!(c.as_str(), "content" | "markdown"))
        {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM {} WHERE typeof({})='text'",
                ident(col),
                ident(&table),
                ident(col)
            ))?;
            for row in stmt.query_map([], |r| r.get::<_, String>(0))? {
                for (_, destination) in markdown_references(&row?) {
                    if let Some(path) = local_reference(&destination) {
                        if path.is_file() {
                            candidates.insert(path);
                        } else if destination.starts_with("file:") {
                            bail!("笔记附件不存在，完整备份已中止：{}", path.display());
                        }
                    }
                }
            }
        }
        for col in cols.iter().filter(|c| c.ends_with("_path")) {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM {} WHERE {} IS NOT NULL",
                ident(col),
                ident(&table),
                ident(col)
            ))?;
            for row in stmt.query_map([], |r| r.get::<_, String>(0))? {
                let raw = row?;
                let path = PathBuf::from(&raw);
                if path.is_absolute() && path.exists() {
                    candidates.insert(path);
                }
            }
        }
    }
    // Missing originals must not produce a misleading successful full backup.
    let mut stmt = conn.prepare("SELECT file_path FROM case_files WHERE deleted_at IS NULL")?;
    for row in stmt.query_map([], |r| r.get::<_, String>(0))? {
        let path = row?;
        if !Path::new(&path).is_file() {
            bail!("卷宗原件不存在，完整备份已中止：{path}");
        }
    }
    let all: Vec<_> = candidates
        .into_iter()
        .filter(|p| !p.starts_with(data.join("backups")))
        .collect();
    Ok(all
        .iter()
        .filter(|p| {
            !all.iter()
                .any(|parent| parent != *p && p.starts_with(parent))
        })
        .cloned()
        .collect())
}

fn add_file(
    zip: &mut ZipWriter<File>,
    path: &Path,
    name: &str,
    entries: &mut Vec<Entry>,
) -> Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        bail!("备份不支持符号链接，请先将原件归档：{}", path.display());
    }
    if path.is_dir() {
        zip.add_directory(
            format!("{name}/"),
            FileOptions::default().unix_permissions(0o700),
        )?;
        let mut children = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
        children.sort_by_key(|c| c.file_name());
        for child in children {
            let filename = child
                .file_name()
                .into_string()
                .map_err(|_| anyhow!("文件名不是有效 Unicode"))?;
            add_file(zip, &child.path(), &format!("{name}/{filename}"), entries)?;
        }
    } else {
        if !path.is_file() {
            bail!("不能备份特殊文件：{}", path.display());
        }
        let before = calculate_file_sha256(path)?;
        zip.start_file(
            name,
            FileOptions::default()
                .large_file(true)
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(0o600),
        )?;
        let size = std::io::copy(&mut File::open(path)?, zip)?;
        if calculate_file_sha256(path)? != before {
            bail!(
                "备份时文件发生变化，请关闭外部编辑器后重试：{}",
                path.display()
            );
        }
        entries.push(Entry {
            path: name.to_owned(),
            sha256: before,
            size,
        });
    }
    Ok(())
}

pub fn write_archive(
    conn: &Connection,
    key: &str,
    roots: Vec<PathBuf>,
    dest: &Path,
    password: SecretString,
) -> Result<()> {
    if dest.exists() {
        bail!("目标文件已存在，请选择新文件名");
    }
    let parent = dest.parent().ok_or_else(|| anyhow!("备份目标目录不可用"))?;
    let temp = tempfile::Builder::new()
        .prefix(".casy-backup-")
        .tempdir_in(parent)?;
    private_dir(temp.path())?;
    let snapshot = temp.path().join("database.db");
    conn.execute("VACUUM INTO ?1", [snapshot.to_string_lossy().as_ref()])?;
    verify_db_file_integrity(&snapshot, key)?;
    let mut zip = ZipWriter::new(File::create(temp.path().join("payload.zip"))?);
    let mut manifest = Manifest {
        version: 1,
        database_key: key.to_owned(),
        roots: vec![],
        entries: vec![],
    };
    add_file(&mut zip, &snapshot, "database.db", &mut manifest.entries)?;
    for (index, path) in roots.into_iter().enumerate() {
        if dest.starts_with(&path) || temp.path().starts_with(&path) {
            bail!("请将备份保存到案件和应用数据目录以外的位置");
        }
        let archived = format!("files/{index}");
        add_file(&mut zip, &path, &archived, &mut manifest.entries)?;
        let canonical = path.canonicalize()?;
        if canonical != path {
            manifest.roots.push(Root {
                original: canonical,
                archived: archived.clone(),
            });
        }
        manifest.roots.push(Root {
            original: path,
            archived,
        });
    }
    zip.start_file(
        "manifest.json",
        FileOptions::default().unix_permissions(0o600),
    )?;
    serde_json::to_writer(&mut zip, &manifest)?;
    zip.finish()?.sync_all()?;
    let mut encrypted = tempfile::NamedTempFile::new_in(parent)?;
    let mut writer =
        age::Encryptor::with_user_passphrase(password).wrap_output(encrypted.as_file_mut())?;
    std::io::copy(
        &mut File::open(temp.path().join("payload.zip"))?,
        &mut writer,
    )?;
    writer.finish()?;
    encrypted.as_file().sync_all()?;
    encrypted
        .persist_noclobber(dest)
        .map_err(|e| anyhow!("备份保存失败：{}", e.error))?;
    Ok(())
}

fn decrypt_archive(source: &Path, password: SecretString, target: &Path) -> Result<Manifest> {
    private_dir(target)?;
    let identity = age::scrypt::Identity::new(password);
    let decryptor = age::Decryptor::new(File::open(source)?).context("不是有效的 Casy 加密备份")?;
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| anyhow!("备份密码错误或文件损坏"))?;
    let mut payload = tempfile::tempfile()?;
    std::io::copy(&mut reader, &mut payload).context("备份认证失败，文件可能被截断或损坏")?;
    let mut zip = ZipArchive::new(payload)?;
    let mut json = String::new();
    zip.by_name("manifest.json")?
        .take(32 * 1024 * 1024 + 1)
        .read_to_string(&mut json)?;
    if json.len() > 32 * 1024 * 1024 {
        bail!("备份清单过大");
    }
    let manifest: Manifest = serde_json::from_str(&json)?;
    if manifest.version != 1 {
        bail!("不支持此备份版本");
    }
    let mut seen = BTreeSet::new();
    let mut extracted_files = BTreeSet::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| anyhow!("备份包含越界路径"))?
            .to_path_buf();
        if !seen.insert(relative.clone()) {
            bail!("备份包含重复路径");
        }
        if entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            bail!("备份包含符号链接");
        }
        if relative == Path::new("manifest.json") {
            continue;
        }
        if relative != Path::new("database.db") && !relative.starts_with("files") {
            bail!("备份包含未声明的路径");
        }
        let path = target.join(&relative);
        if entry.is_dir() {
            private_dir(&path)?;
        } else {
            extracted_files.insert(relative.to_string_lossy().replace('\\', "/"));
            private_dir(path.parent().unwrap())?;
            let mut file = File::options().write(true).create_new(true).open(&path)?;
            std::io::copy(&mut entry, &mut file)?;
            file.sync_all()?;
        }
    }
    let mut declared = BTreeSet::new();
    for entry in &manifest.entries {
        let path = Path::new(&entry.path);
        if path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || !declared.insert(entry.path.clone())
        {
            bail!("备份文件清单无效");
        }
        let restored = target.join(path);
        if fs::metadata(&restored)?.len() != entry.size
            || calculate_file_sha256(&restored)? != entry.sha256
        {
            bail!("备份文件校验失败：{}", entry.path);
        }
    }
    if declared != extracted_files || !declared.contains("database.db") {
        bail!("备份清单与实际文件不一致");
    }
    for root in &manifest.roots {
        if !portable_absolute_path(&root.original.to_string_lossy())
            || !root.archived.starts_with("files/")
            || Path::new(&root.archived)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || !target.join(&root.archived).exists()
        {
            bail!("备份目录映射无效");
        }
    }
    verify_db_file_integrity(&target.join("database.db"), &manifest.database_key)?;
    Ok(manifest)
}

// Source roots in an archive belong to the exporting OS, not the restoring OS.
fn portable_absolute_path(value: &str) -> bool {
    let value = value.replace('\\', "/");
    value.starts_with('/')
        || (value.len() >= 3
            && value.as_bytes()[0].is_ascii_alphabetic()
            && &value.as_bytes()[1..3] == b":/")
}

fn portable_path(value: &str) -> String {
    let windows = cfg!(windows) || value.starts_with("\\\\")
        || (value.len() >= 2 && value.as_bytes()[1] == b':');
    let value = if windows { value.replace('\\', "/") } else { value.to_owned() };
    let value = value.strip_prefix("//?/UNC/")
        .map(|tail| format!("//{tail}"))
        .unwrap_or_else(|| value.strip_prefix("//?/").unwrap_or(&value).to_owned());
    value.trim_end_matches('/').to_owned()
}

fn relocate_string(value: &str, mappings: &[(PathBuf, PathBuf)]) -> String {
    let source = portable_path(value);
    let mut roots: Vec<_> = mappings.iter().collect();
    // Nested attachments must not accidentally resolve through a parent mapping.
    roots.sort_by_key(|(old, _)| std::cmp::Reverse(old.as_os_str().len()));
    for (old, new) in roots {
        let old = portable_path(&old.to_string_lossy());
        let prefix = format!("{old}/");
        let suffix = if source == old {
            ""
        } else if let Some(suffix) = source.strip_prefix(&prefix) {
            suffix
        } else {
            continue;
        };
        if suffix.split('/').any(|part| matches!(part, "." | "..")) {
            continue;
        }
        let mut target = new.clone();
        for part in suffix.split('/').filter(|part| !part.is_empty()) {
            target.push(part);
        }
        return target.to_string_lossy().into_owned();
    }
    value.to_owned()
}

fn relocate_json(value: &mut serde_json::Value, mappings: &[(PathBuf, PathBuf)]) {
    match value {
        serde_json::Value::String(s) => *s = relocate_string(s, mappings),
        serde_json::Value::Array(values) => {
            values.iter_mut().for_each(|v| relocate_json(v, mappings))
        }
        serde_json::Value::Object(values) => {
            values.values_mut().for_each(|v| relocate_json(v, mappings))
        }
        _ => {}
    }
}

fn relocate_database(conn: &mut Connection, mappings: &[(PathBuf, PathBuf)]) -> Result<()> {
    let tx = conn.transaction()?;
    for (table, cols) in tables(&tx)? {
        for col in cols {
            let mut stmt = tx.prepare(&format!(
                "SELECT rowid, {} FROM {} WHERE typeof({})='text'",
                ident(&col),
                ident(&table),
                ident(&col)
            ))?;
            let values = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(stmt);
            for (id, old) in values {
                let mut new = relocate_string(&old, mappings);
                if new == old && matches!(col.as_str(), "content" | "markdown") {
                    new = relocate_markdown(&old, mappings);
                }
                if new == old && (old.starts_with('{') || old.starts_with('[')) {
                    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&old) {
                        let before = json.clone();
                        relocate_json(&mut json, mappings);
                        if json != before {
                            new = serde_json::to_string(&json)?;
                        }
                    }
                }
                if new != old {
                    tx.execute(
                        &format!(
                            "UPDATE {} SET {}=?1 WHERE rowid=?2",
                            ident(&table),
                            ident(&col)
                        ),
                        rusqlite::params![new, id],
                    )?;
                }
            }
        }
    }
    tx.execute("UPDATE document_processing_jobs SET status='queued',started_at=NULL WHERE status='running'", [])?;
    tx.commit()?;
    let foreign_keys: bool = conn
        .prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_none();
    if !foreign_keys {
        bail!("恢复数据库存在无效引用");
    }
    Ok(())
}

pub fn prepare_restore(
    source: &Path,
    password: SecretString,
    target: &Path,
    local_key: &str,
) -> Result<PathBuf> {
    let manifest = decrypt_archive(source, password, target)?;
    let mappings: Vec<_> = manifest
        .roots
        .iter()
        .map(|r| (r.original.clone(), target.join(&r.archived)))
        .collect();
    let database = target.join("database.db");
    let mut conn = open_keyed(&database, &manifest.database_key)?;
    relocate_database(&mut conn, &mappings)?;
    if local_key.len() != 64 || !local_key.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("本地数据库密钥无效");
    }
    conn.execute_batch(&format!(
        "PRAGMA journal_mode=DELETE; PRAGMA rekey = \"x'{local_key}'\";"
    ))?;
    drop(conn);
    verify_db_file_integrity(&database, local_key)?;
    Ok(database)
}

/// Complete an interrupted atomic database replacement before any connection is opened.
pub fn recover_interrupted_restore() -> Result<()> {
    let live = db::get_db_path();
    let previous = live.with_extension("restore-previous");
    if !live.exists() && previous.exists() {
        fs::rename(&previous, &live)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn export_full_backup(destination: String, password: String) -> Result<bool, String> {
    run_blocking(move || {
        if password.chars().count() < 10 {
            bail!("备份密码至少需要 10 个字符");
        }
        let password = SecretString::from(password);
        let _guard = db::enter_maintenance()?;
        let key = db::get_or_create_encryption_key()?;
        let conn = db::open_db_encrypted()?;
        let roots = collect_roots(&conn)?;
        write_archive(&conn, &key, roots, Path::new(&destination), password)?;
        Ok(true)
    })
    .await
}

#[tauri::command]
pub async fn import_full_backup(source: String, password: String) -> Result<bool, String> {
    run_blocking(move || {
        let password = SecretString::from(password);
        let root = runtime_paths::data_root().join("restored");
        private_dir(&root)?;
        let staging = tempfile::Builder::new()
            .prefix("restore-")
            .tempdir_in(&root)?;
        let key = db::get_or_create_encryption_key()?;
        let restored = prepare_restore(Path::new(&source), password.clone(), staging.path(), &key)?;
        let _guard = db::enter_maintenance()?;
        let conn = db::open_db_encrypted()?;
        let backups = super::backup::backups_dir();
        private_dir(&backups)?;
        let protection = backups.join(format!("pre-restore-{}.casy", uuid::Uuid::new_v4()));
        let mut roots = collect_roots(&conn)?;
        // The incoming material is not part of the outgoing database's protection snapshot.
        roots.retain(|p| !p.starts_with(staging.path()) && p != &root);
        if root.exists() {
            for entry in fs::read_dir(&root)? {
                let path = entry?.path();
                if path != staging.path() {
                    roots.push(path);
                }
            }
        }
        write_archive(&conn, &key, roots, &protection, password)?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        drop(conn);
        let live = db::get_db_path();
        let next = live.with_extension("restore-next");
        fs::copy(&restored, &next)?;
        File::open(&next)?.sync_all()?;
        let previous = live.with_extension("restore-previous");
        if previous.exists() {
            fs::remove_file(&previous)?;
        }
        // Material is durable before the database can refer to it.
        let _ = staging.keep();
        fs::rename(&live, &previous)?;
        if let Err(error) = fs::rename(&next, &live) {
            fs::rename(&previous, &live)?;
            return Err(error.into());
        }
        Ok(true)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const NEW_KEY: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    #[test]
    fn portable_backup_restores_files_relations_and_paths_under_a_new_key() {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("original");
        fs::create_dir_all(original.join("empty-folder")).unwrap();
        let file = original.join("evidence.md");
        fs::write(&file, "第三人、Schadensersatz、français、日本語").unwrap();
        let conn = open_keyed(&temp.path().join("live.db"), KEY).unwrap();
        db::schema::run_migrations(&conn, 0).unwrap();
        conn.execute("INSERT INTO cases(id,case_name,client_name,folder_path) VALUES('c','Restore test','Client',?1)", [original.to_str().unwrap()]).unwrap();
        conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f','c','evidence.md',?1,'evidence')", [file.to_str().unwrap()]).unwrap();
        conn.execute(
            "INSERT INTO settings(key,value) VALUES('restore-test',?1)",
            [serde_json::json!({"nested": [{"path": file}]}).to_string()],
        )
        .unwrap();
        let dest = temp.path().join("backup.casy");
        write_archive(
            &conn,
            KEY,
            vec![original.clone()],
            &dest,
            SecretString::from("test-passphrase-123".to_owned()),
        )
        .unwrap();
        assert!(fs::read(&dest)
            .unwrap()
            .starts_with(b"age-encryption.org/v1"));
        fs::remove_dir_all(&original).unwrap();
        let restored = temp.path().join("fresh-machine");
        let database = prepare_restore(
            &dest,
            SecretString::from("test-passphrase-123".to_owned()),
            &restored,
            NEW_KEY,
        )
        .unwrap();
        assert!(verify_db_file_integrity(&database, KEY).is_err());
        let recovered = open_keyed(&database, NEW_KEY).unwrap();
        let path: String = recovered
            .query_row("SELECT file_path FROM case_files WHERE id='f'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(Path::new(&path).starts_with(&restored));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "第三人、Schadensersatz、français、日本語"
        );
        let case_folder: String = recovered
            .query_row("SELECT folder_path FROM cases WHERE id='c'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(Path::new(&case_folder).join("empty-folder").is_dir());
        let json: String = recovered
            .query_row(
                "SELECT value FROM settings WHERE key='restore-test'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap()["nested"][0]["path"],
            path
        );
        assert!(recovered
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query([])
            .unwrap()
            .next()
            .unwrap()
            .is_none());
        assert!(prepare_restore(
            &dest,
            SecretString::from("wrong-password".to_owned()),
            &temp.path().join("wrong"),
            NEW_KEY
        )
        .is_err());
        let mut bytes = fs::read(&dest).unwrap();
        bytes.truncate(bytes.len() - 20);
        let truncated = temp.path().join("truncated.casy");
        fs::write(&truncated, bytes).unwrap();
        assert!(prepare_restore(
            &truncated,
            SecretString::from("test-passphrase-123".to_owned()),
            &temp.path().join("truncated"),
            NEW_KEY
        )
        .is_err());
    }

    #[test]
    fn foreign_archive_restores_database_and_attachment_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("source.db");
        let conn = open_keyed(&database, KEY).unwrap();
        db::schema::run_migrations(&conn, 0).unwrap();
        conn.execute_batch(r"INSERT INTO cases(id,case_name,client_name,folder_path)
            VALUES('c','Case','Client','C:\source');
            INSERT INTO case_files(id,case_id,file_name,file_path,category)
            VALUES('f','c','file.txt','C:\source\file.txt','evidence');
            INSERT INTO knowledge_items(id,title,content,category)
            VALUES('k','Note','[file](file:///C:/source/file.txt)','reference');").unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;").unwrap();
        drop(conn);
        let files = temp.path().join("files");
        fs::create_dir_all(&files).unwrap();
        fs::write(files.join("file.txt"), b"original evidence").unwrap();
        let payload = temp.path().join("payload.zip");
        let mut zip = ZipWriter::new(File::create(&payload).unwrap());
        let mut manifest = Manifest {
            version: 1, database_key: KEY.into(),
            roots: vec![Root { original: PathBuf::from(r"C:\source"), archived: "files/0".into() }],
            entries: vec![],
        };
        add_file(&mut zip, &database, "database.db", &mut manifest.entries).unwrap();
        add_file(&mut zip, &files, "files/0", &mut manifest.entries).unwrap();
        zip.start_file("manifest.json", FileOptions::default()).unwrap();
        serde_json::to_writer(&mut zip, &manifest).unwrap();
        zip.finish().unwrap();
        let archive = temp.path().join("foreign.casy");
        let password = SecretString::from("foreign-archive-test".to_owned());
        let mut writer = age::Encryptor::with_user_passphrase(password.clone())
            .wrap_output(File::create(&archive).unwrap()).unwrap();
        std::io::copy(&mut File::open(payload).unwrap(), &mut writer).unwrap();
        writer.finish().unwrap();
        let target = temp.path().join("restored (new)");
        let restored = prepare_restore(&archive, password, &target, NEW_KEY).unwrap();
        let conn = open_keyed(&restored, NEW_KEY).unwrap();
        let path: String = conn.query_row("SELECT file_path FROM case_files WHERE id='f'", [], |r| r.get(0)).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"original evidence");
        assert!(Path::new(&path).starts_with(&target));
        let note: String = conn.query_row("SELECT content FROM knowledge_items WHERE id='k'", [], |r| r.get(0)).unwrap();
        let references = markdown_references(&note);
        assert_eq!(local_reference(&references[0].1).unwrap(), PathBuf::from(path));
    }

    #[test]
    fn foreign_source_paths_relocate_to_native_destinations() {
        let temp = tempfile::tempdir().unwrap();
        for original in [r"C:\Users\lawyer\case", r"\\?\C:\Users\lawyer\case", "/Users/lawyer/case", r"\\server\share\case"] {
            assert!(portable_absolute_path(original));
            let mappings = vec![(PathBuf::from(original), temp.path().to_owned())];
            let source = format!("{original}/evidence/file.pdf");
            assert_eq!(relocate_string(&source, &mappings), temp.path().join("evidence").join("file.pdf").to_string_lossy());
            let escape = format!("{original}/../outside.pdf");
            assert_eq!(relocate_string(&escape, &mappings), escape);
            let sibling = format!("{original}-other/file.pdf");
            assert_eq!(relocate_string(&sibling, &mappings), sibling);
        }
        assert!(!portable_absolute_path("C:relative"));
        assert!(!portable_absolute_path("relative/path"));
    }

    #[test]
    fn foreign_markdown_urls_keep_attachments_and_text() {
        let temp = tempfile::tempdir().unwrap();
        let mappings = vec![(PathBuf::from(r"C:\case"), temp.path().join("new (case)"))];
        let original = "[file](file:///C:/case/a%20b.pdf)\n`C:\\case\\a b.pdf`\n";
        let moved = relocate_markdown(original, &mappings);
        let references = markdown_references(&moved);
        assert_eq!(references.len(), 1);
        assert_eq!(local_reference(&references[0].1).unwrap(), temp.path().join("new (case)").join("a b.pdf"));
        assert!(moved.contains("`C:\\case\\a b.pdf`"));
    }

    #[test]
    fn relocation_respects_path_boundaries() {
        let mappings = vec![(PathBuf::from("/old/case"), PathBuf::from("/new/case"))];
        assert_eq!(
            relocate_string("/old/case/file.pdf", &mappings),
            Path::new("/new/case").join("file.pdf").to_string_lossy()
        );
        assert_eq!(
            relocate_string("/old/case-other/file.pdf", &mappings),
            "/old/case-other/file.pdf"
        );
        assert_eq!(
            relocate_string("ordinary case notes", &mappings),
            "ordinary case notes"
        );
    }

    #[test]
    fn markdown_attachments_move_without_rewriting_other_text() {
        let mappings = vec![(PathBuf::from("/old/case"), PathBuf::from("/new/case"))];
        let original="[original](/old/case/evidence.pdf)\n![image](file:///old/case/scan%20one.png)\n[reference][ref]\n\n[ref]: /old/case/citation.pdf\n\n`/old/case/plain text`\n";
        let moved = relocate_markdown(original, &mappings);
        let destinations: Vec<_> = markdown_references(&moved).into_iter()
            .map(|(_, destination)| reference_source(&destination).unwrap()).collect();
        assert!(destinations.iter().any(|path| portable_path(path) == "/new/case/evidence.pdf"));
        assert!(moved.contains("file:///new/case/scan%20one.png"));
        assert!(destinations.iter().any(|path| portable_path(path) == "/new/case/citation.pdf"));
        assert!(moved.contains("`/old/case/plain text`\n"));
        assert_eq!(
            relocate_markdown("unchanged\n\n", &mappings),
            "unchanged\n\n"
        );
        assert_eq!(
            relocate_markdown("unchanged\r\n\r\n", &mappings),
            "unchanged\r\n\r\n"
        );
    }
}

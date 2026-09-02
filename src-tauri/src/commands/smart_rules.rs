//! Smart Rules（W5 · DEVONthink 式本地自动化）—— CRUD + 规则引擎
//!
//! 匹配语义：对 match_field（filename / ocr_text）做大小写不敏感的子串匹配。
//! 动作：
//!   - set_category: action_payload 写入 case_files.category（须为合法枚举）
//!   - mark_urgent:  给案件写一条 urgent 通知（通知中心）
//!   - add_keyword:  追加到 case_files.knowledge_keywords
//! OCR 执行：系统存在 tesseract 时调用（诚实降级：不存在则标记 failed 并给出原因）。
use rusqlite::params;
use serde::Serialize;

use super::run_blocking;
use crate::db;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SmartRuleDto {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub match_field: String,
    pub match_pattern: String,
    pub action_type: String,
    pub action_payload: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SmartRuleApplyResult {
    pub file_id: String,
    pub matched_rules: Vec<String>,
    pub actions_applied: Vec<String>,
}

fn row_to_rule(row: &rusqlite::Row) -> rusqlite::Result<SmartRuleDto> {
    Ok(SmartRuleDto {
        id: row.get("id")?,
        name: row.get("name")?,
        enabled: row.get::<_, i32>("enabled")? != 0,
        match_field: row.get("match_field")?,
        match_pattern: row.get("match_pattern")?,
        action_type: row.get("action_type")?,
        action_payload: row.get("action_payload")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

const RULE_COLS: &str =
    "id, name, enabled, match_field, match_pattern, action_type, action_payload, created_at, updated_at";

const VALID_CATEGORIES: [&str; 7] = [
    "summons",
    "evidence",
    "submitted",
    "received",
    "internal",
    "correspondence",
    "other",
];

#[tauri::command]
pub async fn list_smart_rules() -> Result<Vec<SmartRuleDto>, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {RULE_COLS} FROM smart_rules ORDER BY created_at ASC"
        ))?;
        let rows = stmt.query_map([], row_to_rule)?;
        Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .await
}

#[tauri::command]
pub async fn upsert_smart_rule(
    id: Option<String>,
    name: String,
    enabled: bool,
    match_field: String,
    match_pattern: String,
    action_type: String,
    action_payload: String,
) -> Result<String, String> {
    run_blocking(move || {
        if !["filename", "ocr_text"].contains(&match_field.as_str()) {
            return Err(anyhow::anyhow!("无效的匹配字段: {match_field}"));
        }
        if !["set_category", "mark_urgent", "add_keyword"].contains(&action_type.as_str()) {
            return Err(anyhow::anyhow!("无效的动作类型: {action_type}"));
        }
        if action_type == "set_category" && !VALID_CATEGORIES.contains(&action_payload.as_str()) {
            return Err(anyhow::anyhow!(
                "set_category 的载荷必须是合法分类: {}",
                VALID_CATEGORIES.join("/")
            ));
        }
        if match_pattern.trim().is_empty() {
            return Err(anyhow::anyhow!("匹配模式不能为空"));
        }
        let conn = db::open_db()?;
        match &id {
            Some(rid) => {
                let n = conn.execute(
                    "UPDATE smart_rules SET name=?2, enabled=?3, match_field=?4, match_pattern=?5,
                     action_type=?6, action_payload=?7, updated_at=datetime('now','localtime')
                     WHERE id=?1",
                    params![rid, name, if enabled { 1 } else { 0 }, match_field,
                            match_pattern, action_type, action_payload],
                )?;
                if n == 0 {
                    return Err(anyhow::anyhow!("规则不存在: {rid}"));
                }
                Ok(rid.clone())
            }
            None => {
                let new_id = db::new_id();
                conn.execute(
                    "INSERT INTO smart_rules (id, name, enabled, match_field, match_pattern, action_type, action_payload)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![new_id, name, if enabled { 1 } else { 0 }, match_field,
                            match_pattern, action_type, action_payload],
                )?;
                Ok(new_id)
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn delete_smart_rule(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM smart_rules WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}

/// 对单个文件执行全部启用规则（文件登记 / OCR 完成时自动调用，也可手动触发）
#[tauri::command]
pub async fn apply_smart_rules(file_id: String) -> Result<SmartRuleApplyResult, String> {
    run_blocking(move || apply_rules_inner(&file_id)).await
}

/// 对全部已登记文件批量执行规则
#[tauri::command]
pub async fn run_smart_rules_for_all() -> Result<i64, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare("SELECT id FROM case_files")?;
        let ids: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        drop(conn);
        let mut applied = 0i64;
        for id in ids {
            // 单文件失败不中止整批（与 ocr_all_pending 容错语义一致）
            match apply_rules_inner(&id) {
                Ok(res) => {
                    if !res.matched_rules.is_empty() {
                        applied += 1;
                    }
                }
                Err(e) => log::warn!("smart rules 应用失败 (file {}): {}", id, e),
            }
        }
        Ok(applied)
    })
    .await
}

pub(crate) fn apply_rules_inner(file_id: &str) -> Result<SmartRuleApplyResult, anyhow::Error> {
    let conn = db::open_db()?;
    let (file_name, ocr_text, case_id): (String, Option<String>, String) = conn.query_row(
        "SELECT file_name, ocr_text, case_id FROM case_files WHERE id = ?1",
        params![file_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {RULE_COLS} FROM smart_rules WHERE enabled = 1 ORDER BY created_at ASC"
    ))?;
    let rules: Vec<SmartRuleDto> = stmt
        .query_map([], row_to_rule)?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut matched = Vec::new();
    let mut applied = Vec::new();
    for rule in rules {
        let haystack = match rule.match_field.as_str() {
            "filename" => file_name.to_lowercase(),
            "ocr_text" => ocr_text.clone().unwrap_or_default().to_lowercase(),
            _ => continue,
        };
        if haystack.contains(&rule.match_pattern.to_lowercase()) {
            matched.push(rule.name.clone());
            match rule.action_type.as_str() {
                "set_category" => {
                    conn.execute(
                        "UPDATE case_files SET category = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
                        params![file_id, rule.action_payload],
                    )?;
                    applied.push(format!("分类 → {}", rule.action_payload));
                }
                "mark_urgent" => {
                    // 幂等：同文件同规则存在未处理通知则跳过（dismiss 后允许再次提醒）
                    let payload =
                        serde_json::json!({"caseId": case_id, "fileId": file_id, "rule": rule.name}).to_string();
                    let existing: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM notifications
                         WHERE dismissed_at IS NULL AND type = 'smart_rule'
                           AND payload_json LIKE ?1",
                        params![format!("%\"fileId\":\"{file_id}\"%")],
                        |r| r.get(0),
                    )?;
                    if existing == 0 {
                        conn.execute(
                            "INSERT INTO notifications (id, type, title, body, payload_json)
                             VALUES (?1, 'smart_rule', ?2, ?3, ?4)",
                            params![
                                db::new_id(),
                                format!("⚡ 紧急文件命中规则「{}」", rule.name),
                                format!("文件 {} 匹配「{}」", file_name, rule.match_pattern),
                                payload
                            ],
                        )?;
                        applied.push("已推送紧急通知".to_string());
                    }
                }
                "add_keyword" => {
                    let existing: Option<String> = conn
                        .query_row(
                            "SELECT knowledge_keywords FROM case_files WHERE id = ?1",
                            params![file_id],
                            |r| r.get(0),
                        )
                        .ok()
                        .flatten();
                    let mut kws: Vec<String> = existing
                        .map(|s| {
                            s.split(',')
                                .map(|x| x.trim().to_string())
                                .filter(|x| !x.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();
                    if !kws.contains(&rule.action_payload) {
                        kws.push(rule.action_payload.clone());
                        conn.execute(
                            "UPDATE case_files SET knowledge_keywords = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
                            params![file_id, kws.join(",")],
                        )?;
                    }
                    applied.push(format!("关键词 +{}", rule.action_payload));
                }
                _ => {}
            }
        }
    }
    Ok(SmartRuleApplyResult {
        file_id: file_id.to_string(),
        matched_rules: matched,
        actions_applied: applied,
    })
}

/// 列出待 OCR 的扫描件（pdf_extractor 标记 NeedsOCR / ocr_status = 'pending'）
#[tauri::command]
pub async fn list_pending_ocr_files() -> Result<Vec<(String, String, String)>, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, file_name, case_id FROM case_files
             WHERE ocr_status = 'pending' AND (file_type LIKE '%pdf%' OR file_name LIKE '%.pdf'
                   OR file_name LIKE '%.png' OR file_name LIKE '%.jpg' OR file_name LIKE '%.jpeg')
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .await
}

// ============================================================
// OCR 执行器（W5 · DEVONthink 式本地静默 OCR · 诚实降级）
//
// 外部依赖（全部本机探测，缺失时显式失败，绝不假成功）：
//   - tesseract        : OCR 引擎本体（brew install tesseract）
//                        中文识别另需 chi_sim 语言包（brew install tesseract-lang）
//   - pdftoppm(poppler): 仅扫描件 PDF 需要，先转图片再识别（brew install poppler）
// 图片文件（png/jpg/jpeg/tif/tiff/bmp/webp/gif）直接喂给 tesseract，无需 poppler。
// ============================================================

/// 探测外部命令是否可用（执行 --version / -v，退出码为 0 视为可用）
fn tool_available(tool: &str, version_arg: &str) -> bool {
    std::process::Command::new(tool)
        .arg(version_arg)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 读取 tesseract 已安装语言包列表
fn tesseract_langs() -> Vec<String> {
    let Ok(out) = std::process::Command::new("tesseract")
        .arg("--list-langs")
        .output()
    else {
        return Vec::new();
    };
    // 语言列表在 stdout，部分版本打到 stderr，两处都解析
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with("List of available languages"))
        .collect()
}

/// 选择识别语言：优先 chi_sim+eng，缺中文包则诚实回退 eng（返回值附带是否含中文）
fn pick_tess_lang() -> Option<(String, bool)> {
    let langs = tesseract_langs();
    if langs.is_empty() {
        return None;
    }
    let has_chi = langs.iter().any(|l| l == "chi_sim");
    let has_eng = langs.iter().any(|l| l == "eng");
    let lang = match (has_chi, has_eng) {
        (true, true) => "chi_sim+eng".to_string(),
        (true, false) => "chi_sim".to_string(),
        (false, true) => "eng".to_string(),
        (false, false) => langs[0].clone(),
    };
    Some((lang, has_chi))
}

/// 带超时的子进程执行（防卡死：轮询 try_wait + 超时 kill；
/// stdout/stderr 重定向到临时文件，避免管道缓冲写满导致的双向死锁）
fn run_with_timeout(
    cmd: &mut std::process::Command,
    timeout_secs: u64,
    tag: &str,
) -> Result<(std::process::ExitStatus, String, String), String> {
    let tmp = std::env::temp_dir();
    let uniq = format!("casy_cmd_{}_{}", tag, db::new_id());
    let out_path = tmp.join(format!("{uniq}.out"));
    let err_path = tmp.join(format!("{uniq}.err"));
    let out_file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
    let err_file = std::fs::File::create(&err_path).map_err(|e| e.to_string())?;
    let mut child = cmd
        .stdout(std::process::Stdio::from(out_file))
        .stderr(std::process::Stdio::from(err_file))
        .spawn()
        .map_err(|e| format!("子进程启动失败: {e}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&out_path);
                    let _ = std::fs::remove_file(&err_path);
                    return Err(format!("执行超过 {timeout_secs}s 超时，已终止"));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&out_path);
                let _ = std::fs::remove_file(&err_path);
                return Err(format!("子进程等待失败: {e}"));
            }
        }
    };
    let stdout = std::fs::read_to_string(&out_path).unwrap_or_default();
    let stderr = std::fs::read_to_string(&err_path).unwrap_or_default();
    let _ = std::fs::remove_file(&out_path);
    let _ = std::fs::remove_file(&err_path);
    Ok((status, stdout, stderr))
}

/// 对单张图片执行 tesseract，成功返回识别文本（120s 超时保护）
fn run_tesseract(image: &std::path::Path, lang: &str) -> Result<String, String> {
    let (status, stdout, stderr) = run_with_timeout(
        std::process::Command::new("tesseract")
            .arg(image.as_os_str())
            .arg("stdout")
            .arg("-l")
            .arg(lang),
        120,
        "tess",
    )?;
    if !status.success() {
        return Err(format!("tesseract 识别失败: {}", stderr.trim()));
    }
    Ok(stdout)
}

fn set_ocr_status(file_id: &str, status: &str) -> anyhow::Result<()> {
    let conn = db::open_db()?;
    conn.execute(
        "UPDATE case_files SET ocr_status = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
        params![file_id, status],
    )?;
    Ok(())
}

/// 是否 OCR 候选文件（pdf / 常见图片）
fn is_ocr_candidate(file_name: &str, file_type: Option<&str>) -> bool {
    let ext = file_type
        .map(|s| s.trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::path::Path::new(file_name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
        });
    matches!(
        ext.as_deref(),
        Some("pdf" | "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" | "gif")
    )
}

/// 登记自动接线（供 commands/files.rs 调用）：
/// pdf/图片登记成功后立即套用 Smart Rules（filename 类规则即时生效）。
/// 失败仅记录日志，绝不影响文件登记主流程。
pub fn auto_rules_on_register(file_id: &str, file_name: &str, file_type: Option<&str>) {
    if !is_ocr_candidate(file_name, file_type) {
        return;
    }
    if let Err(e) = apply_rules_inner(file_id) {
        log::warn!("smart rules on register failed for {file_id}: {e}");
    }
}

/// 单文件 OCR 主流程（run_blocking 内执行；返回中文状态说明，失败路径同步落库 failed）
fn ocr_case_file_inner(file_id: &str) -> anyhow::Result<String> {
    let (file_name, file_path, file_type): (String, String, Option<String>) = {
        let conn = db::open_db()?;
        conn.query_row(
            "SELECT file_name, file_path, file_type FROM case_files WHERE id = ?1",
            params![file_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| anyhow::anyhow!("文件不存在: {file_id}"))?
    };

    if !is_ocr_candidate(&file_name, file_type.as_deref()) {
        set_ocr_status(file_id, "failed")?;
        return Ok(format!(
            "「{file_name}」类型不支持 OCR（仅支持 PDF 与常见图片）"
        ));
    }

    // 诚实降级第一关：tesseract 本体
    if !tool_available("tesseract", "--version") {
        set_ocr_status(file_id, "failed")?;
        return Ok("未检测到 tesseract，可通过 brew install tesseract 安装".to_string());
    }

    set_ocr_status(file_id, "processing")?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.is_file() {
        set_ocr_status(file_id, "failed")?;
        return Ok(format!(
            "源文件不存在或不可读: {file_name}（请检查卷宗目录）"
        ));
    }

    let (lang, has_chi) =
        pick_tess_lang().ok_or_else(|| anyhow::anyhow!("tesseract 无可用语言包"))?;
    let lang_note = if has_chi {
        String::new()
    } else {
        "（未安装 chi_sim 中文语言包，已回退英文识别；可 brew install tesseract-lang 补齐）"
            .to_string()
    };

    let is_pdf = file_type
        .as_deref()
        .map(|t| t.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
        || path
            .extension()
            .map(|e| e.to_string_lossy().eq_ignore_ascii_case("pdf"))
            .unwrap_or(false);

    let text_result: Result<String, String> = if is_pdf {
        // 诚实降级第二关：pdftoppm（poppler）
        if !tool_available("pdftoppm", "-v") {
            Err("未检测到 pdftoppm（poppler），可通过 brew install poppler 安装".to_string())
        } else {
            let tmp_dir = std::env::temp_dir().join(format!("casy_ocr_{file_id}"));
            let r = (|| -> Result<String, String> {
                std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("临时目录创建失败: {e}"))?;
                let prefix = tmp_dir.join("page");
                let (conv_status, _conv_out, conv_err) = run_with_timeout(
                    std::process::Command::new("pdftoppm")
                        .arg("-png")
                        .arg("-r")
                        .arg("200")
                        .arg(path.as_os_str())
                        .arg(prefix.as_os_str()),
                    180,
                    "pdftoppm",
                )?;
                if !conv_status.success() {
                    return Err(format!("PDF 转图片失败: {}", conv_err.trim()));
                }
                let mut pages: Vec<std::path::PathBuf> = std::fs::read_dir(&tmp_dir)
                    .map_err(|e| e.to_string())?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension().map(|x| x == "png").unwrap_or(false)
                            && p.file_name()
                                .map(|n| n.to_string_lossy().starts_with("page"))
                                .unwrap_or(false)
                    })
                    .collect();
                pages.sort();
                if pages.is_empty() {
                    return Err("PDF 转图片未产生任何页面".to_string());
                }
                let mut buf = String::new();
                for page in &pages {
                    match run_tesseract(page, &lang) {
                        Ok(t) => {
                            buf.push_str(&t);
                            buf.push('\n');
                        }
                        Err(e) => return Err(format!("第 {:?} 页识别失败: {e}", page.file_name())),
                    }
                }
                Ok(buf)
            })();
            let _ = std::fs::remove_dir_all(&tmp_dir); // 清理临时图片
            r
        }
    } else {
        run_tesseract(&path, &lang)
    };

    match text_result {
        Ok(text) => {
            let trimmed = text.trim().to_string();
            let conn = db::open_db()?;
            conn.execute(
                "UPDATE case_files SET ocr_text = ?2, ocr_status = 'completed',
                 updated_at = datetime('now','localtime') WHERE id = ?1",
                params![file_id, trimmed],
            )?;
            drop(conn);
            // OCR 完成后让 ocr_text 类规则生效
            let rule_note = match apply_rules_inner(file_id) {
                Ok(res) if !res.matched_rules.is_empty() => {
                    format!("，命中 {} 条规则并已执行", res.matched_rules.len())
                }
                Ok(_) => String::new(),
                Err(e) => {
                    log::warn!("apply rules after ocr failed for {file_id}: {e}");
                    "（规则执行失败，详见日志）".to_string()
                }
            };
            if trimmed.is_empty() {
                Ok(format!(
                    "OCR 完成，但未识别出文字（可能为空白或低清扫描件）{lang_note}{rule_note}"
                ))
            } else {
                Ok(format!(
                    "OCR 完成，识别 {} 个字符{lang_note}{rule_note}",
                    trimmed.chars().count()
                ))
            }
        }
        Err(msg) => {
            set_ocr_status(file_id, "failed")?;
            Ok(msg)
        }
    }
}

/// 对单个文件执行本地 OCR（PDF 走 pdftoppm 转图，图片直接识别）
#[tauri::command]
pub async fn ocr_case_file(file_id: String) -> Result<String, String> {
    run_blocking(move || ocr_case_file_inner(&file_id)).await
}

/// 批量 OCR 全部待识别文件（单文件失败不影响其他；返回实际处理数）
#[tauri::command]
pub async fn ocr_all_pending() -> Result<i64, String> {
    run_blocking(|| {
        let ids: Vec<String> = {
            let conn = db::open_db()?;
            let mut stmt = conn.prepare(
                "SELECT id FROM case_files
                 WHERE ocr_status = 'pending' AND (file_type LIKE '%pdf%' OR file_name LIKE '%.pdf'
                       OR file_name LIKE '%.png' OR file_name LIKE '%.jpg' OR file_name LIKE '%.jpeg')
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };
        let mut processed = 0i64;
        for id in ids {
            match ocr_case_file_inner(&id) {
                Ok(_) => processed += 1,
                Err(e) => {
                    log::warn!("ocr_all_pending: {id} failed: {e}");
                    let _ = set_ocr_status(&id, "failed");
                }
            }
        }
        Ok(processed)
    })
    .await
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileOcrStateDto {
    pub file_id: String,
    pub ocr_status: String,
    pub has_text: bool,
}

/// 查询案件全部文件的 OCR 状态（文件列表徽标用；不搬运 ocr_text 正文）
#[tauri::command]
pub async fn list_case_ocr_states(case_id: String) -> Result<Vec<FileOcrStateDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, ocr_status, (ocr_text IS NOT NULL AND LENGTH(ocr_text) > 0)
             FROM case_files WHERE case_id = ?1",
        )?;
        let rows = stmt.query_map(params![case_id], |r| {
            Ok(FileOcrStateDto {
                file_id: r.get(0)?,
                ocr_status: r
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| "pending".to_string()),
                has_text: r.get::<_, i32>(2)? != 0,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .await
}

/// 读取单文件的 OCR 文本（「查看文本」弹窗用）
#[tauri::command]
pub async fn get_file_ocr_text(file_id: String) -> Result<Option<String>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let text = conn
            .query_row(
                "SELECT ocr_text FROM case_files WHERE id = ?1",
                params![file_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        Ok(text)
    })
    .await
}

//! 文书语义抽取（UPDATE_PLAN 0.2.0 Phase 1）
//!
//! 把 OCR 产物（或收件箱原文）变成结构化事实：文书类型、法院、案号、当事人、
//! **开庭日期（带页码出处）**、法定期限。每条开庭事实都能回溯到原件页码——
//! 这是自动建档敢让用户信的前提。
//!
//! 语义字段复用 `parse::classify_document` 的规则库（不重复造正则）；
//! 页码出处在本模块内计算（OCR markdown 用 `<!-- page N -->` 标记分页）。

use regex::Regex;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use specta::Type;

/// 单条开庭事实。`page` 为 1-based 页码；无法定位时为 None。
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HearingFact {
    /// "YYYY-MM-DD HH:MM"
    pub date: String,
    pub venue: Option<String>,
    pub page: Option<u32>,
    /// 0–1；来自 parse 规则库对该文书类型的置信度
    pub confidence: f64,
}

/// 单条法定期限事实（举证期限 / 上诉期 / 答辩期）。
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineFact {
    pub name: String,
    /// "YYYY-MM-DD"；只抽到相对期间（如"判决送达之日起十五日"）时为 None
    pub due_date: Option<String>,
    pub legal_basis: Option<String>,
    pub page: Option<u32>,
}

/// 一份文书抽取出的全部事实。
#[derive(Debug, Clone, Serialize, Type, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFacts {
    /// summons / hearing_notice / judgment / ...（与 parse 的 doc_type 一致）
    pub doc_type: String,
    pub court: Option<String>,
    pub case_no: Option<String>,
    pub parties: Vec<PartyFact>,
    pub hearings: Vec<HearingFact>,
    pub deadlines: Vec<DeadlineFact>,
    pub overall_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PartyFact {
    pub name: String,
    pub role: String,
}

/// 抽取器版本：规则变化时 bump，便于识别旧数据需重跑。
pub const EXTRACTOR_VERSION: &str = "facts-1";

/// 文本分页：返回 `(起始偏移, 结束偏移, 页码)`。
///
/// 没有页码标记时整段算第 1 页（收件箱原文、纯文本）。
fn page_sections(text: &str) -> Vec<(usize, usize, u32)> {
    let marker = match Regex::new(r"<!-- page (\d+) -->") {
        Ok(re) => re,
        Err(_) => return vec![(0, text.len(), 1)],
    };
    let mut sections = Vec::new();
    let mut page = 1u32;
    let mut start = 0usize;
    for m in marker.find_iter(text) {
        if let Some(cap) = marker.captures(m.as_str()) {
            if let Ok(n) = cap[1].parse::<u32>() {
                sections.push((start, m.start(), page));
                page = n;
                start = m.end();
            }
        }
    }
    sections.push((start, text.len(), page));
    sections
}

/// 偏移落在哪一页。
fn page_of(sections: &[(usize, usize, u32)], offset: usize) -> Option<u32> {
    sections
        .iter()
        .find(|(start, end, _)| offset >= *start && offset < *end)
        .map(|(_, _, page)| *page)
}

/// 中文数字（法律期间常用 1–99）：十五日、十日、三十日、三个月。
fn parse_cn_number(text: &str) -> Option<i64> {
    const DIGITS: [(char, i64); 10] = [
        ('一', 1), ('二', 2), ('两', 2), ('三', 3), ('四', 4),
        ('五', 5), ('六', 6), ('七', 7), ('八', 8), ('九', 9),
    ];
    let mut total = 0i64;
    let mut current = 0i64;
    let mut seen = false;
    for c in text.chars() {
        if c == '十' {
            total += if current == 0 { 10 } else { current * 10 };
            current = 0;
            seen = true;
        } else if let Some((_, value)) = DIGITS.iter().find(|(ch, _)| *ch == c) {
            current = *value;
            seen = true;
        } else if let Ok(value) = c.to_string().parse::<i64>() {
            current = value;
            seen = true;
        } else {
            return None;
        }
    }
    total += current;
    if seen && total > 0 { Some(total) } else { None }
}

/// 期间名称归一化："上诉" → "上诉期"、"答辩" → "答辩期"。
fn normalize_period_name(name: &str) -> String {
    match name {
        "上诉" => "上诉期".to_string(),
        "答辩" => "答辩期".to_string(),
        other => other.to_string(),
    }
}

/// 法定期限：判决/裁定送达后的上诉期、答辩期、举证期限。
///
/// 中文法律文本两种语序都常见，各用一个正则覆盖：
///   1. 关键词在前：「上诉期为十五日」「答辩期三十日」
///   2. 数字在前  ：「十五日内提起上诉」「三十日内申请再审」
/// 基准日取全文**最后一个** `X年X月X日`（判决/裁定的日期在文末）。
fn extract_deadlines(text: &str, doc_type: &str, sections: &[(usize, usize, u32)]) -> Vec<DeadlineFact> {
    if doc_type != "judgment" && doc_type != "ruling" {
        return Vec::new();
    }
    let num = "[0-9一二三四五六七八九十两]+";
    // 语序 1：上诉期…十五日
    let kw_first = match Regex::new(&format!(
        r"(上诉期?|答辩期?|举证期限)[^。；\n]{{0,24}}?({num})\s*(日|个月|月)"
    )) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    // 语序 2：十五日…（提起）上诉
    let num_first = match Regex::new(&format!(
        r"({num})\s*(日|个月|月)[^。；\n]{{0,12}}?(?:提起|申请|向)?\s*(上诉期?|答辩期?|举证期限)"
    )) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    let date_re = match Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日") {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    let base = date_re
        .find_iter(text)
        .last()
        .and_then(|m| date_re.captures(m.as_str()))
        .and_then(|c| {
            let y: i32 = c[1].parse().ok()?;
            let m: u32 = c[2].parse().ok()?;
            let d: u32 = c[3].parse().ok()?;
            chrono::NaiveDate::from_ymd_opt(y, m, d)
        });

    // (span 起点, 期间名, 数量, 单位)——按起点去重，避免两种语序重复计同一条
    let mut found: Vec<(usize, String, i64, String)> = Vec::new();
    for cap in kw_first.captures_iter(text) {
        let whole = cap.get(0).unwrap();
        if let Some(amount) = parse_cn_number(&cap[2]) {
            found.push((whole.start(), cap[1].to_string(), amount, cap[3].to_string()));
        }
    }
    for cap in num_first.captures_iter(text) {
        let whole = cap.get(0).unwrap();
        if let Some(amount) = parse_cn_number(&cap[1]) {
            let start = whole.start();
            if found.iter().any(|(existing, _, _, _)| *existing == start) {
                continue;
            }
            found.push((start, cap[3].to_string(), amount, cap[2].to_string()));
        }
    }
    found.sort_by_key(|(start, _, _, _)| *start);

    let mut out = Vec::new();
    for (start, name, amount, unit) in found {
        let due = base.and_then(|base| {
            if unit.contains('月') {
                base.checked_add_months(chrono::Months::new(amount as u32))
            } else {
                base.checked_add_signed(chrono::Duration::days(amount))
            }
        });
        out.push(DeadlineFact {
            name: normalize_period_name(&name),
            due_date: due.map(|d| d.format("%Y-%m-%d").to_string()),
            legal_basis: None,
            page: page_of(sections, start),
        });
    }
    out
}

/// 从文本抽取事实。
///
/// `text` 可以是 OCR markdown（含 `<!-- page N -->`）或收件箱原文。
/// 语义字段来自 `parse::classify_document`，页码在本模块计算。
pub fn extract_facts(text: &str) -> DocumentFacts {
    let parsed = crate::parse::classify_document(text);
    let sections = page_sections(text);

    let mut hearings = Vec::new();
    if let Some(date) = parsed.hearing_date.clone() {
        // 页码出处：在原文里找第一个"X年X月X日N时"的日期时间，取其所在页。
        // 与 parse 的日期正则保持同一形态（parse 只回传格式化结果、不回传偏移）。
        let page = Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日\s*(\d{1,2})\s*时")
            .ok()
            .and_then(|re| re.find(text))
            .and_then(|m| page_of(&sections, m.start()));
        hearings.push(HearingFact {
            date,
            venue: parsed.venue.clone(),
            page,
            confidence: parsed.confidence,
        });
    }

    let deadlines = extract_deadlines(text, &parsed.doc_type, &sections);
    let parties = parsed
        .parties
        .iter()
        .map(|p| PartyFact { name: p.name.clone(), role: p.role.clone() })
        .collect();

    let mut confidence = parsed.confidence;
    if hearings.iter().all(|h| h.page.is_some()) && !hearings.is_empty() {
        // 定位到页码的事实更可信
        confidence = (confidence + 0.05).min(1.0);
    }

    DocumentFacts {
        doc_type: parsed.doc_type,
        court: parsed.court,
        case_no: parsed.case_no,
        parties,
        hearings,
        deadlines,
        overall_confidence: confidence,
    }
}

/// 取待抽取的文本。
///
/// - `case_file`：最新**已完成** job 的 markdown（含页码标记，出处最准）；
///   没有则回退 `case_files.ocr_text`（纯文本，出处只能算第 1 页）。
/// - `inbox_item`：收件箱原文（短信/粘贴，本来就没有页码）。
fn load_source_text(
    conn: &rusqlite::Connection,
    ref_type: &str,
    ref_id: &str,
) -> anyhow::Result<String> {
    match ref_type {
        "case_file" => {
            let markdown_path: Option<String> = conn
                .query_row(
                    "SELECT markdown_path FROM document_processing_jobs
                     WHERE file_id=?1 AND status='completed' AND markdown_path IS NOT NULL
                     ORDER BY pinned DESC, rowid DESC LIMIT 1",
                    [ref_id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(path) = markdown_path {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| anyhow::anyhow!("无法读取 OCR 产物 {path}: {e}"))?;
                if !text.trim().is_empty() {
                    return Ok(text);
                }
            }
            let ocr_text: Option<String> = conn
                .query_row(
                    "SELECT ocr_text FROM case_files WHERE id=?1 AND deleted_at IS NULL",
                    [ref_id],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            ocr_text.ok_or_else(|| anyhow::anyhow!("该文件尚未完成 OCR，无法抽取事实"))
        }
        "inbox_item" => {
            let text: String = conn
                .query_row(
                    "SELECT COALESCE(content_text,'') FROM inbox_items WHERE id=?1",
                    [ref_id],
                    |row| row.get(0),
                )
                .map_err(|_| anyhow::anyhow!("收件箱项不存在"))?;
            anyhow::ensure!(!text.trim().is_empty(), "收件箱项没有正文可抽取");
            Ok(text)
        }
        other => anyhow::bail!("不支持的引用类型: {other}"),
    }
}

/// 把事实写库（UNIQUE(ref_type, ref_id) → 幂等重跑覆盖）。
pub(crate) fn upsert_facts(
    conn: &rusqlite::Connection,
    ref_type: &str,
    ref_id: &str,
    facts: &DocumentFacts,
) -> anyhow::Result<()> {
    let parties = serde_json::to_string(&facts.parties)?;
    let hearings = serde_json::to_string(&facts.hearings)?;
    let deadlines = serde_json::to_string(&facts.deadlines)?;
    conn.execute(
        "INSERT INTO document_facts
           (id, ref_type, ref_id, doc_type, court, case_no, parties_json, hearings_json,
            deadlines_json, overall_confidence, extractor_version, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now','localtime'))
         ON CONFLICT(ref_type, ref_id) DO UPDATE SET
           doc_type=excluded.doc_type, court=excluded.court, case_no=excluded.case_no,
           parties_json=excluded.parties_json, hearings_json=excluded.hearings_json,
           deadlines_json=excluded.deadlines_json,
           overall_confidence=excluded.overall_confidence,
           extractor_version=excluded.extractor_version,
           created_at=excluded.created_at",
        rusqlite::params![
            crate::db::new_id(),
            ref_type,
            ref_id,
            facts.doc_type,
            facts.court,
            facts.case_no,
            parties,
            hearings,
            deadlines,
            facts.overall_confidence,
            EXTRACTOR_VERSION,
        ],
    )?;
    Ok(())
}

pub(crate) fn read_facts_row(conn: &rusqlite::Connection, ref_type: &str, ref_id: &str) -> anyhow::Result<DocumentFacts> {
    let row = conn.query_row(
        "SELECT doc_type, court, case_no, parties_json, hearings_json, deadlines_json, overall_confidence
         FROM document_facts WHERE ref_type=?1 AND ref_id=?2",
        rusqlite::params![ref_type, ref_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                // 三列 JSON 均可空（旧数据/部分写入）：空串而非报错
                row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                row.get::<_, f64>(6)?,
            ))
        },
    )?;
    // 三列 JSON 均可空/空串（旧数据或部分写入）：空串按空数组处理，
    // 不能让 serde_json::from_str("") 的 EOF 错误冒出去掩盖真正的问题。
    let parse_list = |text: &str| -> anyhow::Result<Vec<serde_json::Value>> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_str(text)?)
    };
    Ok(DocumentFacts {
        doc_type: row.0,
        court: row.1,
        case_no: row.2,
        parties: serde_json::from_value(serde_json::Value::Array(parse_list(&row.3)?))?,
        hearings: serde_json::from_value(serde_json::Value::Array(parse_list(&row.4)?))?,
        deadlines: serde_json::from_value(serde_json::Value::Array(parse_list(&row.5)?))?,
        overall_confidence: row.6,
    })
}

/// 抽取（或重跑）一份文书的事实。OCR 未完成时明确报错，不返回空事实。
#[tauri::command]
pub async fn extract_document_facts(ref_type: String, ref_id: String) -> Result<DocumentFacts, String> {
    super::run_blocking(move || {
        let conn = crate::db::open_db()?;
        let text = load_source_text(&conn, &ref_type, &ref_id)?;
        let facts = extract_facts(&text);
        upsert_facts(&conn, &ref_type, &ref_id, &facts)?;
        Ok(facts)
    })
    .await
}

/// 读取已抽取的事实（没有则报错，由前端决定是否触发抽取）。
#[tauri::command]
pub async fn get_document_facts(ref_type: String, ref_id: String) -> Result<DocumentFacts, String> {
    super::run_blocking(move || {
        let conn = crate::db::open_db()?;
        read_facts_row(&conn, &ref_type, &ref_id).map_err(|e| anyhow::anyhow!("尚未抽取事实: {e}"))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_summons_hearing_with_page_provenance() {
        let text = "<!-- page 1 -->\n传票\n北京市第一中级人民法院\n案号（2026）京01民初12号\n\n<!-- page 2 -->\n开庭时间：2026年10月26日14时30分\n地点：第五法庭\n";
        let facts = extract_facts(text);
        assert_eq!(facts.doc_type, "summons");
        assert_eq!(facts.hearings.len(), 1);
        let hearing = &facts.hearings[0];
        assert_eq!(hearing.date, "2026-10-26 14:30");
        assert_eq!(hearing.page, Some(2), "开庭日期在第 2 页");
        assert_eq!(hearing.venue.as_deref(), Some("第五法庭"));
        assert!(facts.overall_confidence > 0.9);
    }

    #[test]
    fn single_page_text_reports_page_one() {
        let text = "传票\n开庭时间：2026年3月5日9时\n";
        let facts = extract_facts(text);
        assert_eq!(facts.hearings.len(), 1);
        assert_eq!(facts.hearings[0].page, Some(1));
        assert_eq!(facts.hearings[0].date, "2026-03-05 09:00");
    }

    #[test]
    fn document_without_hearing_has_no_hearing_facts() {
        let text = "<!-- page 1 -->\n判决书\n案号（2026）京01民初12号\n";
        let facts = extract_facts(text);
        assert_eq!(facts.doc_type, "judgment");
        assert!(facts.hearings.is_empty(), "判决书不应抽离开庭事实");
    }

    #[test]
    fn judgment_appeal_deadline_is_computed_from_service_date() {
        let text = "<!-- page 1 -->\n判决书\n本判决送达之日起十五日内提起上诉。\n2026年1月10日作出判决。\n";
        let facts = extract_facts(text);
        assert_eq!(facts.doc_type, "judgment");
        assert_eq!(facts.deadlines.len(), 1);
        let deadline = &facts.deadlines[0];
        assert_eq!(deadline.name, "上诉期");
        assert_eq!(deadline.due_date.as_deref(), Some("2026-01-25"));
        assert_eq!(deadline.page, Some(1));
    }

    #[test]
    fn relative_deadline_without_base_date_keeps_no_due_date() {
        let text = "判决书\n答辩期为收到起诉状副本之日起十五日。\n";
        let facts = extract_facts(text);
        assert_eq!(facts.deadlines.len(), 1);
        assert_eq!(facts.deadlines[0].due_date, None, "没有基准日就不编日子");
    }

    #[test]
    fn non_judgment_documents_have_no_deadlines() {
        let text = "传票\n上诉期为十五日。\n";
        let facts = extract_facts(text);
        assert!(facts.deadlines.is_empty(), "只有判决/裁定才抽期限");
    }
}

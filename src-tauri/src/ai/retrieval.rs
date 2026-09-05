use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::{collections::HashSet, sync::OnceLock};

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPassage {
    pub file_id: String,
    pub job_id: String,
    pub file_name: String,
    pub source_path: String,
    pub number: u32,
    pub location_kind: String,
    pub content: String,
    pub citation: String,
}

pub(crate) fn query_terms(query: &str) -> Vec<String> {
    static JIEBA: OnceLock<jieba_rs::Jieba> = OnceLock::new();
    let tokenizer = JIEBA.get_or_init(jieba_rs::Jieba::new);
    let mut seen = HashSet::new();
    tokenizer
        .cut_for_search(query, true)
        .into_iter()
        .filter(|s| s.chars().count() >= 2 && s.chars().any(char::is_alphanumeric))
        .filter(|s| {
            ![
                "什么", "哪些", "如何", "是否", "多少", "我们", "这个", "相关", "请问", "可以",
                "进行", "以及", "the", "and", "what",
            ]
            .contains(s)
        })
        .map(str::to_owned)
        .filter(|s| seen.insert(s.clone()))
        .take(16)
        .collect()
}

pub fn search(
    conn: &Connection,
    query: &str,
    scope: &[String],
    limit: usize,
) -> Result<Vec<DocumentPassage>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }
    if query.chars().count() > 500 {
        bail!("检索问题不能超过 500 字");
    }
    if scope.is_empty() || scope.len() > 200 {
        bail!("请选择 1 至 200 份文档");
    }
    let terms = query_terms(query);
    let mut phrases = vec![query.to_owned()];
    phrases.extend(terms.iter().cloned());
    let fts = phrases
        .iter()
        .filter(|s| s.chars().count() >= 3)
        .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ");
    // FTS narrows the usual path; only two-character terms need scoped LIKE scans.
    let short: Vec<String> = phrases
        .iter()
        .filter(|s| s.chars().count() < 3)
        .map(|s| {
            format!(
                "%{}%",
                s.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        })
        .collect();
    let sql = r#"
      WITH latest AS (
        SELECT j.id FROM case_files f JOIN document_processing_jobs j ON j.id=(
          SELECT id FROM document_processing_jobs WHERE file_id=f.id AND status='completed' ORDER BY rowid DESC LIMIT 1
        ) WHERE f.id IN (SELECT value FROM json_each(?1))
      ), hits AS (
        SELECT p.rowid FROM document_pages_fts ft JOIN document_pages p ON p.rowid=ft.rowid
          WHERE document_pages_fts MATCH ?2 AND p.job_id IN (SELECT id FROM latest)
        UNION
        SELECT p.rowid FROM document_pages p WHERE p.job_id IN (SELECT id FROM latest)
          AND EXISTS(SELECT 1 FROM json_each(?3) term WHERE p.plain_text LIKE term.value ESCAPE '\' OR p.markdown LIKE term.value ESCAPE '\')
      )
      SELECT f.id,p.job_id,f.file_name,f.file_path,p.page_number,j.engine,p.markdown,p.plain_text
        FROM hits JOIN document_pages p ON p.rowid=hits.rowid JOIN case_files f ON f.id=p.file_id
        JOIN document_processing_jobs j ON j.id=p.job_id
        ORDER BY (CASE WHEN instr(lower(p.markdown || p.plain_text),lower(?4))>0 THEN 100 ELSE 0 END
          + 10 * (SELECT count(*) FROM json_each(?5) term WHERE instr(lower(p.markdown || p.plain_text),lower(term.value))>0)) DESC,
          f.id,p.page_number LIMIT ?6
    "#;
    let mut stmt = conn.prepare(sql)?;
    let candidates = stmt
        .query_map(
            params![
                serde_json::to_string(scope)?,
                if fts.is_empty() { "\"\"" } else { &fts },
                serde_json::to_string(&short)?,
                query,
                serde_json::to_string(&terms)?,
                limit.min(50) as i64
            ],
            |row| {
                let file_id: String = row.get(0)?;
                let number: u32 = row.get(4)?;
                let engine: String = row.get(5)?;
                let markdown: String = row.get(6)?;
                let plain: String = row.get(7)?;
                let kind = if engine == "text-document" { "s" } else { "p" };
                Ok(DocumentPassage {
                    citation: format!("file {file_id} {kind}{number}"),
                    file_id,
                    job_id: row.get(1)?,
                    file_name: row.get(2)?,
                    source_path: row.get(3)?,
                    number,
                    location_kind: if kind == "s" { "segment" } else { "page" }.into(),
                    content: if markdown.trim().is_empty() {
                        plain
                    } else {
                        markdown
                    },
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut ranked: Vec<_> = candidates
        .into_iter()
        .map(|passage| {
            let text = passage.content.to_lowercase();
            let score = usize::from(text.contains(&query.to_lowercase())) * 100
                + terms
                    .iter()
                    .filter(|term| text.contains(&term.to_lowercase()))
                    .count()
                    * 10;
            (score, passage)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(ranked
        .into_iter()
        .take(limit.min(50))
        .map(|(_, mut p)| {
            // Excerpts remain anchored near the first query term; no full-library content crosses IPC.
            if p.content.chars().count() > 6000 {
                let start = terms
                    .iter()
                    .filter_map(|term| p.content.find(term))
                    .min()
                    .unwrap_or(0);
                let mut boundary = start.saturating_sub(600);
                while !p.content.is_char_boundary(boundary) {
                    boundary += 1;
                }
                p.content = p.content[boundary..].chars().take(6000).collect();
            }
            p
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn searches_chinese_questions_and_never_leaks_other_files_or_old_jobs() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::run_migrations(&conn, 0).unwrap();
        conn.execute_batch("INSERT INTO cases(id,case_name,client_name) VALUES('c','合成案件','测试客户');
          INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f','c','答辩.docx','/tmp/a.docx','evidence'),('outside','c','其他.pdf','/tmp/b.pdf','evidence');
          INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,engine) VALUES('old','f','h','completed','text-document'),('new','f','h','completed','text-document'),('out','outside','h','completed','ocr');
          INSERT INTO document_pages(job_id,file_id,page_number,markdown) VALUES('old','f',1,'第三人旧名称丙公司'),('new','f',1,'本案第三人为乙公司，应当参加诉讼。'),('new','f',2,'请求金额为125000.25元。'),('out','outside',1,'第三人无关保密资料');").unwrap();
        let hits = search(&conn, "第三人是哪家公司", &["f".into()], 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].citation, "file f s1");
        assert_eq!(hits[0].job_id, "new");
        assert!(hits[0].content.contains("乙公司"));
        assert_eq!(
            search(&conn, "金额", &["f".into()], 10).unwrap()[0].number,
            2
        );
        assert!(search(&conn, "不存在的专利技术", &["f".into()], 10)
            .unwrap()
            .is_empty());
    }
}

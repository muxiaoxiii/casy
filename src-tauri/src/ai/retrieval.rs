use super::document_match::{PageText, SourceLocation, SourceText};
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
    pub locations: Vec<SourceLocation>,
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
        ) WHERE f.deleted_at IS NULL AND f.id IN (SELECT value FROM json_each(?1))
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
                    locations: vec![],
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
    let mut candidates = candidates;
    // FTS indexes individual pages. Scan adjacent pages in the selected, latest
    // document versions too, so a word split at a page break is still found.
    let mut stmt = conn.prepare(r#"
        SELECT f.id,j.id,f.file_name,f.file_path,j.engine,p.page_number,p.width,p.height,
               CASE WHEN p.plain_text='' THEN p.markdown ELSE p.plain_text END,p.regions_json
        FROM case_files f JOIN document_processing_jobs j ON j.id=(
            SELECT id FROM document_processing_jobs WHERE file_id=f.id AND status='completed' ORDER BY rowid DESC LIMIT 1)
        JOIN document_pages p ON p.job_id=j.id
        WHERE f.deleted_at IS NULL AND f.id IN (SELECT value FROM json_each(?1))
        ORDER BY f.id,p.page_number
    "#)?;
    let mut rows = stmt.query([serde_json::to_string(scope)?])?;
    let mut window = std::collections::VecDeque::<PageText>::new();
    let mut window_job = String::new();
    while let Some(row) = rows.next()? {
        let file_id: String = row.get(0)?;
        let job_id: String = row.get(1)?;
        let engine: String = row.get(4)?;
        let page = PageText {
            number: row.get(5)?,
            width: row.get(6)?,
            height: row.get(7)?,
            text: row.get(8)?,
            regions: serde_json::from_str(&row.get::<_, String>(9)?)?,
        };
        let source = SourceText::new(&[&page]);
        let matched = source
            .find(query)
            .or_else(|| terms.iter().find_map(|term| source.find(term)));
        if let Some((content, locations)) = matched {
            if let Some(hit) = candidates
                .iter_mut()
                .find(|hit| hit.job_id == job_id && hit.number == page.number)
            {
                hit.locations = locations;
                hit.content = content;
            } else {
                let kind = if engine == "text-document" { "s" } else { "p" };
                candidates.push(DocumentPassage {
                    file_id: file_id.clone(),
                    job_id: job_id.clone(),
                    file_name: row.get(2)?,
                    source_path: row.get(3)?,
                    number: page.number,
                    location_kind: if kind == "s" { "segment" } else { "page" }.into(),
                    content,
                    citation: format!("file {file_id} {kind}{}", page.number),
                    locations,
                });
            }
        }
        if window_job != job_id || window.back().is_some_and(|old|old.number + 1 != page.number) { window.clear(); }
        window_job = job_id.clone();
        window.push_back(page);
        while window.len() > 2 && (window.len() > 100 || window.iter().skip(1).map(|p|p.text.chars().count()).sum::<usize>() > query.chars().count() + 2000) { window.pop_front(); }
        if window.len() > 1 {
                let refs: Vec<_> = window.iter().collect();
                let pair = SourceText::continuous(&refs);
                if let Some((content, locations)) = pair.find_cross_page(query) {
                    let first = locations.first().unwrap().page_number;
                    let last = locations.last().unwrap().page_number;
                    if last == window.back().unwrap().number
                    {
                        let kind = if engine == "text-document" { "s" } else { "p" };
                        candidates.push(DocumentPassage {
                            file_id: file_id.clone(),
                            job_id: job_id.clone(),
                            file_name: row.get(2)?,
                            source_path: row.get(3)?,
                            number: first,
                            location_kind: if kind == "s" { "segment" } else { "page" }.into(),
                            content,
                            citation: format!(
                                "file {file_id} {kind}{}-{kind}{}",
                                first, last
                            ),
                            locations,
                        });
                    }
                }
        }
        // Bound retained candidates even for a very common term in a large library.
        if candidates.len() > 200 {
            rank_candidates(&mut candidates, query, &terms);
            candidates.truncate(100);
        }
    }
    let mut ranked: Vec<_> = candidates
        .into_iter()
        .map(|passage| {
            let text = passage.content.to_lowercase();
            let score = usize::from(
                passage
                    .locations
                    .iter()
                    .any(|l| l.page_number != passage.number),
            ) * 200
                + usize::from(text.contains(&query.to_lowercase())) * 100
                + terms
                    .iter()
                    .filter(|term| text.contains(&term.to_lowercase()))
                    .count()
                    * 10;
            (score, passage)
        })
        .collect();
    ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
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

fn rank_candidates(candidates: &mut [DocumentPassage], query: &str, terms: &[String]) {
    let score = |p: &DocumentPassage| {
        let text = p.content.to_lowercase();
        usize::from(p.locations.iter().any(|l| l.page_number != p.number)) * 200
            + usize::from(text.contains(&query.to_lowercase())) * 100
            + terms
                .iter()
                .filter(|t| text.contains(&t.to_lowercase()))
                .count()
                * 10
    };
    candidates.sort_by_key(|p| std::cmp::Reverse(score(p)));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cross_page_word_is_searchable_with_both_region_references() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::run_migrations(&conn, 0).unwrap();
        conn.execute_batch("INSERT INTO cases(id,case_name,client_name) VALUES('c','Synthetic','Test');
          INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f','c','claim.pdf','/tmp/claim.pdf','evidence');
          INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,engine) VALUES('j','f','hash','completed','paddle-onnx-visual');").unwrap();
        for (number, text) in [(1, "Schadens-"), (2, "ersatz Prüfung")] {
            let regions =
                serde_json::json!([{"text":text,"bbox":[10,20,200,40],"confidence":0.99}])
                    .to_string();
            conn.execute("INSERT INTO document_pages(job_id,file_id,page_number,width,height,plain_text,markdown,regions_json) VALUES('j','f',?1,400,600,?2,?2,?3)",params![number,text,regions]).unwrap();
        }
        let hits = search(&conn, "Schadensersatz", &["f".into()], 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].citation, "file f p1-p2");
        assert_eq!(
            hits[0]
                .locations
                .iter()
                .map(|l| l.page_number)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(hits[0].content.contains("Schadens-\nersatz"));
        conn.execute(
            "UPDATE case_files SET deleted_at='2026-09-07' WHERE id='f'",
            [],
        )
        .unwrap();
        assert!(search(&conn, "Schadensersatz", &["f".into()], 10)
            .unwrap()
            .is_empty());
    }
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

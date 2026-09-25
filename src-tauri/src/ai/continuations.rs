//! Candidate screening adapted from MinerU-Popo's merge_rules/filter_contd.
//! Copyright (c) 2026 MinerU-Popo Project Authors. MIT; see docs/compliance/MinerU-Popo-LICENSE.txt.
//! These links are suggestions, never permission to rewrite source text or tables.
use super::document_match::{PageText, SourceLocation};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Continuation {
    pub from_page: u32,
    pub to_page: u32,
    pub locations: Vec<SourceLocation>,
}

fn list_or_heading(text: &str) -> bool {
    static PREFIX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PREFIX.get_or_init(|| regex::Regex::new(r"^(?:[#>|]|[-*+]\s|\d+[.)）、．]|[（(\[【]|[A-Za-z][.)]\s|[一二三四五六七八九十百]+、|第[一二三四五六七八九十百\d]+[条节章]|[①-⑳])").unwrap()).is_match(text.trim())
}

fn may_continue(left: &str, right: &str) -> bool {
    let left = left
        .trim()
        .trim_end_matches(['’', '”', '\'', '"', '）', '」', '】', ']', ')']);
    left.chars().count() >= 10
        && !right.trim().is_empty()
        && !left.ends_with(['.', '。', '?', '!', '？', '！', ':', '：', ';', '；', '…'])
        && !list_or_heading(right)
        && !list_or_heading(left)
        && !left.contains('\t')
        && !right.contains('\t')
}

pub fn detect(pages: &[PageText]) -> Vec<Continuation> {
    let mut margins = std::collections::HashMap::<String, std::collections::HashSet<u32>>::new();
    let signature = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace() && !c.is_numeric())
            .collect::<String>()
    };
    let is_margin = |p: &PageText, bbox: [f32; 4]| {
        p.height
            .is_some_and(|h| bbox[3] < h * 0.08 || bbox[1] > h * 0.92)
    };
    for p in pages {
        for r in &p.regions {
            if is_margin(p, r.bbox) {
                margins
                    .entry(signature(&r.text))
                    .or_default()
                    .insert(p.number);
            }
        }
    }
    let edge = |p: &PageText, last: bool| {
        let candidates: Vec<_> = p
            .regions
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                !(r.text.trim().is_empty()
                    || (is_margin(p, r.bbox)
                        && margins
                            .get(&signature(&r.text))
                            .is_some_and(|s| s.len() > 1)))
            })
            .collect();
        let &(index, r) = if last {
            candidates.last()?
        } else {
            candidates.first()?
        };
        Some((
            r.text.clone(),
            SourceLocation {
                page_number: p.number,
                region_index: Some(index as u32),
                bbox: Some(r.bbox),
                width: p.width,
                height: p.height,
            },
        ))
    };
    pages
        .windows(2)
        .filter_map(|pair| {
            let (left, right) = (&pair[0], &pair[1]);
            if right.number != left.number + 1 {
                return None;
            }
            let ((tail, a), (head, b)) = (edge(left, true)?, edge(right, false)?);
            // Require physical page-edge evidence; OCR text documents have no invented boxes.
            if a.bbox?[3] < left.height? * 0.65
                || b.bbox?[1] > right.height? * 0.35
                || !may_continue(&tail, &head)
            {
                return None;
            }
            Some(Continuation {
                from_page: left.number,
                to_page: right.number,
                locations: vec![a, b],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidates_exclude_new_lists_headings_and_finished_sentences() {
        assert!(may_continue(
            "Die vorliegende Erfindung betrifft ein Ver-",
            "fahren zur Prüfung"
        ));
        assert!(may_continue(
            "根据双方约定本案应当承担的赔偿",
            "责任包括直接损失"
        ));
        for right in [
            "# 新章节",
            "1. 新请求",
            "第一条 适用范围",
            "（一）独立证据",
            "| 表格 |",
        ] {
            assert!(!may_continue("根据双方约定本案应当承担的赔偿", right));
        }
        assert!(!may_continue(
            "This paragraph is complete.”",
            "Next paragraph"
        ));
    }
    #[test]
    fn preserves_both_page_locations_and_rejects_nonadjacent_pages() {
        let page = |number, text: &str, bbox| PageText {
            number,
            width: Some(600.),
            height: Some(800.),
            text: text.into(),
            regions: vec![crate::document_pipeline::DocumentRegion {
                text: text.into(),
                bbox,
                confidence: Some(0.9),
            }],
        };
        let mut pages = vec![
            page(
                1,
                "Der Anspruch umfasst ein Verfahren zur",
                [30., 680., 560., 720.],
            ),
            page(2, "Prüfung der Beweise", [30., 80., 560., 120.]),
        ];
        let links = detect(&pages);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].locations[1].page_number, 2);
        pages[1].number = 3;
        assert!(detect(&pages).is_empty());
    }
}

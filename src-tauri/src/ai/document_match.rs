use crate::document_pipeline::DocumentRegion;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub page_number: u32,
    pub region_index: Option<u32>,
    pub bbox: Option<[f32; 4]>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub struct PageText {
    pub number: u32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub text: String,
    pub regions: Vec<DocumentRegion>,
}

pub struct SourceText {
    pub raw: String,
    spans: Vec<(usize, usize, SourceLocation)>,
}

impl SourceText {
    pub fn new(pages: &[&PageText]) -> Self {
        Self::build(pages, false)
    }

    pub fn continuous(pages: &[&PageText]) -> Self {
        Self::build(pages, true)
    }

    fn build(pages: &[&PageText], exclude_repeated_margins: bool) -> Self {
        let margin = |page: &PageText, region: &DocumentRegion| {
            page.height.is_some_and(|h| region.bbox[3] < h * 0.08 || region.bbox[1] > h * 0.92)
        };
        let signature = |text: &str| text.chars().filter(|c| !c.is_whitespace() && !c.is_numeric()).collect::<String>();
        let mut margins = std::collections::HashMap::<String, std::collections::HashSet<u32>>::new();
        if exclude_repeated_margins {
            for page in pages {
                for region in &page.regions {
                    if margin(page,region) { margins.entry(signature(&region.text)).or_default().insert(page.number); }
                }
            }
        }
        let mut result = Self {
            raw: String::new(),
            spans: vec![],
        };
        for page in pages {
            if !result.raw.is_empty() {
                result.raw.push('\n');
            }
            let location = SourceLocation {
                page_number: page.number,
                region_index: None,
                bbox: None,
                width: page.width,
                height: page.height,
            };
            if page.regions.is_empty() {
                let start = result.raw.len();
                result.raw.push_str(&page.text);
                result.spans.push((start, result.raw.len(), location));
            } else {
                for (index, region) in page.regions.iter().enumerate() {
                    if exclude_repeated_margins && margin(page, region) && margins.get(&signature(&region.text)).is_some_and(|pages| pages.len() >= 2) { continue; }
                    if index > 0 {
                        result.raw.push('\n');
                    }
                    let start = result.raw.len();
                    result.raw.push_str(&region.text);
                    result.spans.push((
                        start,
                        result.raw.len(),
                        SourceLocation {
                            region_index: Some(index as u32),
                            bbox: Some(region.bbox),
                            ..location.clone()
                        },
                    ));
                }
            }
        }
        result
    }

    pub fn find(&self, query: &str) -> Option<(String, Vec<SourceLocation>)> {
        self.find_match(query,false)
    }

    pub fn find_cross_page(&self, query: &str) -> Option<(String, Vec<SourceLocation>)> {
        self.find_match(query,true)
    }

    fn find_match(&self, query: &str, cross_page:bool) -> Option<(String, Vec<SourceLocation>)> {
        let needle = normalize(query).0;
        if needle.is_empty() {
            return None;
        }
        let (text, offsets) = normalize(&self.raw);
        for (start,_) in text.match_indices(&needle) {
        let end = start + needle.len();
        let raw_start = offsets[start].0;
        let raw_end = offsets[end - 1].1;
        let locations:Vec<SourceLocation> = self
            .spans
            .iter()
            .filter(|(a, b, _)| *a < raw_end && *b > raw_start)
            .map(|(_, _, location)| location.clone())
            .collect();
        if cross_page && !locations.first().is_some_and(|first|locations.iter().any(|l|l.page_number!=first.page_number)) {continue;}
        let before = self.raw[..raw_start]
            .char_indices()
            .rev()
            .nth(180)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let after = self.raw[raw_end..]
            .char_indices()
            .nth(600)
            .map(|(i, _)| raw_end + i)
            .unwrap_or(self.raw.len());
        return Some((self.raw[before..after].into(), locations));
        }
        None
    }
}

// Search normalization only: original text and byte provenance remain unchanged.
// A printed line-end hyphen is joined only when alphabetic text continues below.
fn normalize(raw: &str) -> (String, Vec<(usize, usize)>) {
    let mut normalized = String::new();
    let mut offsets = vec![];
    let mut previous = None;
    for (index, character) in raw.char_indices() {
        let tail = &raw[index + character.len_utf8()..];
        let line_hyphen = character == '-'
            && previous.is_some_and(char::is_alphabetic)
            && tail.starts_with(['\r', '\n'])
            && tail
                .chars()
                .find(|c| !c.is_whitespace())
                .is_some_and(char::is_alphabetic);
        if !character.is_whitespace() && character != '\u{00ad}' && !line_hyphen {
            for lowered in character.to_lowercase() {
                normalized.push(lowered);
                offsets.extend(std::iter::repeat_n(
                    (index, index + character.len_utf8()),
                    lowered.len_utf8(),
                ));
            }
        }
        if !character.is_whitespace() {
            previous = Some(character);
        }
    }
    (normalized, offsets)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(number: u32, text: &str) -> PageText {
        PageText {
            number,
            width: Some(400.0),
            height: Some(600.0),
            text: text.into(),
            regions: vec![DocumentRegion {
                text: text.into(),
                bbox: [10.0, 20.0, 200.0, 40.0],
                confidence: None,
            }],
        }
    }
    #[test]
    fn cross_page_matches_preserve_multilingual_regions_and_original_hyphen() {
        for (left, right, query) in [
            ("第三人赔偿", "金额约定", "赔偿金额"),
            ("Schadens-", "ersatz Prüfung", "Schadensersatz"),
            ("responsabilité", " française", "responsabilité française"),
            ("損害賠償", "請求", "賠償請求"),
        ] {
            let a = page(1, left);
            let b = page(2, right);
            let source = SourceText::new(&[&a, &b]);
            let (excerpt, locations) = source.find(query).unwrap();
            assert!(excerpt.contains(left));
            assert_eq!(
                locations.iter().map(|l| l.page_number).collect::<Vec<_>>(),
                vec![1, 2]
            );
            assert!(locations.iter().all(|l| l.bbox.is_some()));
        }
        assert_ne!(normalize("123-456").0, normalize("123456").0);
    }

    #[test]
    fn repeated_margins_do_not_interrupt_a_three_page_match() {
        let mut pages = vec![page(1,"赔偿"),page(2,"责任"),page(3,"承担")];
        for page in &mut pages {
            page.regions[0].bbox = [10.,100.,200.,140.];
            page.regions.insert(0,DocumentRegion {text:format!("Evidence {}",page.number),bbox:[10.,5.,200.,20.],confidence:None});
            page.regions.push(DocumentRegion {text:page.number.to_string(),bbox:[10.,575.,40.,595.],confidence:None});
        }
        let refs: Vec<_> = pages.iter().collect();
        let source=SourceText::continuous(&refs);
        let (_,locations)=source.find_cross_page("赔偿责任承担").unwrap();
        assert_eq!(locations.iter().map(|l|(l.page_number,l.region_index)).collect::<Vec<_>>(),vec![(1,Some(1)),(2,Some(1)),(3,Some(1))]);
        assert!(SourceText::new(&[&pages[0]]).find("Evidence 1").is_some());
    }
}

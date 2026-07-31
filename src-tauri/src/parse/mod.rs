use regex::Regex;
use serde::Serialize;

/// 解析后的文档信息
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDocument {
    pub doc_type: String,
    pub case_no: Option<String>,
    pub parties: Vec<PartyInfo>,
    pub date: Option<String>,
    pub court: Option<String>,
    pub judge: Option<String>,
    pub clerk: Option<String>,
    pub patent_no: Option<String>,
    pub patent_name: Option<String>,
    pub hearing_date: Option<String>,
    pub venue: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyInfo {
    pub name: String,
    pub role: String,
}

/// 基于规则的文档分类
pub fn classify_document(text: &str) -> ParsedDocument {
    let mut doc = ParsedDocument::default();

    // 传票
    if text.contains("传票") || (text.contains("传唤") && text.contains("开庭")) {
        doc.doc_type = "summons".into();
        doc.confidence = 0.85;
        parse_summons(text, &mut doc);
        return doc;
    }

    // 口审通知书
    if text.contains("口头审理通知书") || text.contains("口审") {
        doc.doc_type = "hearing_notice".into();
        doc.confidence = 0.85;
        parse_hearing_notice(text, &mut doc);
        return doc;
    }

    // 判决/裁定/决定
    if text.contains("判决书") || text.contains("裁定书") || text.contains("无效决定") {
        doc.doc_type = "judgment".into();
        doc.confidence = 0.8;
        parse_judgment(text, &mut doc);
        return doc;
    }

    // 起诉状
    if text.contains("起诉状") || text.contains("行政起诉") {
        doc.doc_type = "complaint".into();
        doc.confidence = 0.8;
        return doc;
    }

    // 答辩状
    if text.contains("答辩状") || text.contains("答辩意见") {
        doc.doc_type = "defense".into();
        doc.confidence = 0.8;
        return doc;
    }

    doc.doc_type = "other".into();
    doc.confidence = 0.3;
    doc
}

fn parse_summons(text: &str, doc: &mut ParsedDocument) {
    // 案号
    let case_no_re = Regex::new(r"[（(]\s*\d{4}\s*[）)].*?号").unwrap();
    if let Some(m) = case_no_re.find(text) {
        doc.case_no = Some(m.as_str().to_string());
    }

    // 日期时间
    let datetime_re =
        Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日\s*(\d{1,2})\s*时\s*(\d{1,2})\s*分?")
            .unwrap();
    if let Some(caps) = datetime_re.captures(text) {
        doc.hearing_date = Some(format!(
            "{}-{:02}-{:02} {:02}:{:02}",
            &caps[1],
            caps[2].parse::<u32>().unwrap_or(1),
            caps[3].parse::<u32>().unwrap_or(1),
            caps[4].parse::<u32>().unwrap_or(0),
            caps[5].parse::<u32>().unwrap_or(0),
        ));
    }

    // 法院
    let court_re = Regex::new(r"([一-龥]+(?:人民法院|知识产权法院|仲裁委员会))").unwrap();
    if let Some(caps) = court_re.captures(text) {
        doc.court = Some(caps[1].to_string());
    }

    // 审判长
    let judge_re = Regex::new(r"(?:审判长|审判员|法官)\s*[：:]\s*([一-龥]{2,4})").unwrap();
    if let Some(caps) = judge_re.captures(text) {
        doc.judge = Some(caps[1].to_string());
    }

    // 书记员
    let clerk_re = Regex::new(r"书记员\s*[：:]\s*([一-龥]{2,4})").unwrap();
    if let Some(caps) = clerk_re.captures(text) {
        doc.clerk = Some(caps[1].to_string());
    }
}

fn parse_hearing_notice(text: &str, doc: &mut ParsedDocument) {
    // 案件编号（国知局格式: 4W123456）
    let cnipa_re = Regex::new(r"(\d+W\d+)").unwrap();
    if let Some(caps) = cnipa_re.captures(text) {
        doc.case_no = Some(caps[1].to_string());
    }

    // 专利号
    let patent_re = Regex::new(r"(?:专利号|ZL)\s*[：:]?\s*(\d{9,13}\.?\d?)").unwrap();
    if let Some(caps) = patent_re.captures(text) {
        doc.patent_no = Some(caps[1].to_string());
    }

    // 请求人/专利权人
    let party_re = Regex::new(r"(?:请求人|专利权人)\s*[：:]\s*([一-龥\w\(\)（）]+)").unwrap();
    for caps in party_re.captures_iter(text) {
        let name = caps[1].to_string();
        let role = if caps[0].contains("请求人") {
            "请求人"
        } else {
            "专利权人"
        };
        doc.parties.push(PartyInfo {
            name,
            role: role.into(),
        });
    }

    // 合议组
    let panel_re = Regex::new(r"(?:合议组组长|组长)\s*[：:]\s*([一-龥]{2,4})").unwrap();
    if let Some(caps) = panel_re.captures(text) {
        doc.judge = Some(caps[1].to_string());
    }
}

fn parse_judgment(text: &str, doc: &mut ParsedDocument) {
    // 案号
    let case_no_re = Regex::new(r"[（(]\s*\d{4}\s*[）)].*?号").unwrap();
    if let Some(m) = case_no_re.find(text) {
        doc.case_no = Some(m.as_str().to_string());
    }

    // 当事人
    let party_re =
        Regex::new(r"(?:原告|被告|上诉人|被上诉人|请求人|专利权人)\s*[：:]*\s*([一-龥\w\(\)（）]+)")
            .unwrap();
    for caps in party_re.captures_iter(text) {
        let full = caps[0].to_string();
        let role = full
            .split(|c: char| c == '：' || c == ':')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        doc.parties.push(PartyInfo {
            name: caps[1].to_string(),
            role,
        });
    }
}

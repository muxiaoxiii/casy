pub mod pdf_extractor;
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

/// 基于本地超大规则库的文档精准分类（§2.2 & §10）
pub fn classify_document(text: &str) -> ParsedDocument {
    let mut doc = ParsedDocument::default();

    // 1. 传票 / 出庭通知 / 传唤
    if text.contains("传票")
        || (text.contains("传唤") && text.contains("开庭"))
        || text.contains("出庭通知")
    {
        doc.doc_type = "summons".into();
        doc.confidence = 0.90;
        parse_summons(text, &mut doc);
        return doc;
    }

    // 2. 口审通知书 / 专利无效审理
    if text.contains("口头审理通知书")
        || text.contains("口审")
        || text.contains("无效宣告请求口头审理")
    {
        doc.doc_type = "hearing_notice".into();
        doc.confidence = 0.90;
        parse_hearing_notice(text, &mut doc);
        return doc;
    }

    // 3. 判决/裁定/决定/调解书
    if text.contains("判决书")
        || text.contains("裁定书")
        || text.contains("无效决定")
        || text.contains("无效宣告请求审查决定")
        || text.contains("调解书")
    {
        doc.doc_type = "judgment".into();
        doc.confidence = 0.88;
        parse_judgment(text, &mut doc);
        return doc;
    }

    // 4. 起诉状 / 申请书 / 申诉书
    if text.contains("起诉状")
        || text.contains("行政起诉")
        || text.contains("民事起诉")
        || text.contains("无效宣告请求书")
    {
        doc.doc_type = "complaint".into();
        doc.confidence = 0.85;
        parse_complaint_or_defense(text, &mut doc);
        return doc;
    }

    // 5. 答辩状 / 答辩意见
    if text.contains("答辩状") || text.contains("答辩意见") || text.contains("意见陈述书")
    {
        doc.doc_type = "defense".into();
        doc.confidence = 0.85;
        parse_complaint_or_defense(text, &mut doc);
        return doc;
    }

    // 6. 证据目录 / 证据清单 / 证据材料
    if text.contains("证据目录")
        || text.contains("证据清单")
        || text.contains("证据材料")
        || text.contains("证据附卷")
    {
        doc.doc_type = "evidence".into();
        doc.confidence = 0.85;
        parse_evidence_doc(text, &mut doc);
        return doc;
    }

    // 7. 授权委托书 / 律所函件
    if text.contains("授权委托书") || text.contains("法定代表人身份证明") || text.contains("律师函")
    {
        doc.doc_type = "power_of_attorney".into();
        doc.confidence = 0.85;
        parse_attorney_doc(text, &mut doc);
        return doc;
    }

    // 8. 代理词 / 质证意见 / 庭审笔录
    if text.contains("代理词")
        || text.contains("质证意见")
        || text.contains("庭审笔录")
        || text.contains("辩护词")
    {
        doc.doc_type = "brief".into();
        doc.confidence = 0.82;
        parse_complaint_or_defense(text, &mut doc);
        return doc;
    }

    // 9. 法律意见书 / 备忘录 / 尽职调查
    if text.contains("法律意见书")
        || text.contains("咨询备忘录")
        || text.contains("尽职调查")
        || text.contains("分析报告")
    {
        doc.doc_type = "legal_opinion".into();
        doc.confidence = 0.80;
        return doc;
    }

    doc.doc_type = "other".into();
    doc.confidence = 0.30;
    doc
}

fn parse_summons(text: &str, doc: &mut ParsedDocument) {
    // 案号提取：支持各类中院、高院、知产法院与最高法格式
    extract_comprehensive_case_no(text, doc);

    // 日期时间
    let datetime_re = Regex::new(
        r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日\s*(\d{1,2})\s*时(?:\s*(\d{1,2})\s*分)?",
    )
    .unwrap();
    if let Some(caps) = datetime_re.captures(text) {
        doc.hearing_date = Some(format!(
            "{}-{:02}-{:02} {:02}:{:02}",
            &caps[1],
            caps[2].parse::<u32>().unwrap_or(1),
            caps[3].parse::<u32>().unwrap_or(1),
            caps[4].parse::<u32>().unwrap_or(0),
            caps.get(5)
                .and_then(|m| m.as_str().parse::<u32>().ok())
                .unwrap_or(0),
        ));
    }

    // 法院
    let court_re =
        Regex::new(r"([一-龥]+(?:人民法院|知识产权法院|海事法院|仲裁委员会|国家知识产权局))")
            .unwrap();
    if let Some(caps) = court_re.captures(text) {
        doc.court = Some(caps[1].to_string());
    }

    // 审判长/法官
    let judge_re =
        Regex::new(r"(?:审判长|审判员|主审法官|承办法官|法官)\s*[：:]\s*([一-龥]{2,4})").unwrap();
    if let Some(caps) = judge_re.captures(text) {
        doc.judge = Some(caps[1].to_string());
    }

    // 书记员
    let clerk_re = Regex::new(r"书记员\s*[：:]\s*([一-龥]{2,4})").unwrap();
    if let Some(caps) = clerk_re.captures(text) {
        doc.clerk = Some(caps[1].to_string());
    }

    // 地点 / 法庭
    let venue_re =
        Regex::new(r"(?:地点|法庭|审理地点|场所)\s*[：:]\s*([一-龥\w\d第（）()号楼层室]+)")
            .unwrap();
    if let Some(caps) = venue_re.captures(text) {
        doc.venue = Some(caps[1].to_string());
    }
}

fn parse_hearing_notice(text: &str, doc: &mut ParsedDocument) {
    // 案件编号（国知局格式: 4W123456 / 5W123456 / 6W123456 / 5F123456 / 6F123456）
    let cnipa_re = Regex::new(r"(\d+[WF]\d+)").unwrap();
    if let Some(caps) = cnipa_re.captures(text) {
        doc.case_no = Some(caps[1].to_string());
    } else {
        extract_comprehensive_case_no(text, doc);
    }

    // 专利号 (ZLxxxxxxxx.x 或 申请号)
    let patent_re =
        Regex::new(r"(?:专利号|申请号|ZL)\s*[：:]?\s*([0-9]{8,13}\.?[0-9xX]?)").unwrap();
    if let Some(caps) = patent_re.captures(text) {
        doc.patent_no = Some(caps[1].to_string());
    }

    // 发明/实用新型/外观设计名称
    let patent_name_re = Regex::new(
        r"(?:实用新型名称|发明名称|外观设计名称|专利名称)\s*[：:]\s*([一-龥\w\d\-—（）()、]+)",
    )
    .unwrap();
    if let Some(caps) = patent_name_re.captures(text) {
        doc.patent_name = Some(caps[1].to_string());
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

    // 合议组组长 / 主审员
    let panel_re = Regex::new(r"(?:合议组组长|组长|主审员)\s*[：:]\s*([一-龥]{2,4})").unwrap();
    if let Some(caps) = panel_re.captures(text) {
        doc.judge = Some(caps[1].to_string());
    }

    // 口审时间
    let datetime_re = Regex::new(
        r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日\s*(\d{1,2})\s*时(?:\s*(\d{1,2})\s*分)?",
    )
    .unwrap();
    if let Some(caps) = datetime_re.captures(text) {
        doc.hearing_date = Some(format!(
            "{}-{:02}-{:02} {:02}:{:02}",
            &caps[1],
            caps[2].parse::<u32>().unwrap_or(1),
            caps[3].parse::<u32>().unwrap_or(1),
            caps[4].parse::<u32>().unwrap_or(0),
            caps.get(5)
                .and_then(|m| m.as_str().parse::<u32>().ok())
                .unwrap_or(0),
        ));
    }
}

fn parse_judgment(text: &str, doc: &mut ParsedDocument) {
    extract_comprehensive_case_no(text, doc);

    // 当事人
    let party_re =
        Regex::new(r"(?:原告|被告|第三人|上诉人|被上诉人|申请人|被申请人|请求人|专利权人)\s*[：:]*\s*([一-龥\w\(\)（）]+)")
            .unwrap();
    for caps in party_re.captures_iter(text) {
        let full = caps[0].to_string();
        let role = full
            .split(['：', ':'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        doc.parties.push(PartyInfo {
            name: caps[1].to_string(),
            role,
        });
    }

    // 审判组织
    let judge_re = Regex::new(r"(?:审判长|审判员|代理审判员)\s+([一-龥]{2,4})").unwrap();
    if let Some(caps) = judge_re.captures(text) {
        doc.judge = Some(caps[1].to_string());
    }
}

fn parse_complaint_or_defense(text: &str, doc: &mut ParsedDocument) {
    extract_comprehensive_case_no(text, doc);

    let party_re = Regex::new(
        r"(?:原告|被告|起诉人|被起诉人|申请人|被申请人|答辩人)\s*[：:]\s*([一-龥\w\(\)（）]+)",
    )
    .unwrap();
    for caps in party_re.captures_iter(text) {
        let role = caps[0]
            .split(['：', ':'])
            .next()
            .unwrap_or("当事人")
            .trim()
            .to_string();
        doc.parties.push(PartyInfo {
            name: caps[1].to_string(),
            role,
        });
    }
}

fn parse_evidence_doc(text: &str, doc: &mut ParsedDocument) {
    extract_comprehensive_case_no(text, doc);
    let party_re =
        Regex::new(r"(?:举证人|提交人|原告|被告)\s*[：:]\s*([一-龥\w\(\)（）]+)").unwrap();
    if let Some(caps) = party_re.captures(text) {
        doc.parties.push(PartyInfo {
            name: caps[1].to_string(),
            role: "举证人".into(),
        });
    }
}

fn parse_attorney_doc(text: &str, doc: &mut ParsedDocument) {
    extract_comprehensive_case_no(text, doc);
    let party_re = Regex::new(r"(?:委托人|委托单位)\s*[：:]\s*([一-龥\w\(\)（）]+)").unwrap();
    if let Some(caps) = party_re.captures(text) {
        doc.parties.push(PartyInfo {
            name: caps[1].to_string(),
            role: "委托人".into(),
        });
    }
}

/// 通用案号/决定号提取规则（覆盖全国法院、最高法知产法庭与国知局）
fn extract_comprehensive_case_no(text: &str, doc: &mut ParsedDocument) {
    // 1. 标准法院案号：如 (2023)最高法知行终123号 / （2024）京73行初456号
    let standard_case_no_re =
        Regex::new(r"[（(]\s*\d{4}\s*[）)][\u{4e00}-\u{9fff}\w\d\-_]+?\d+号").unwrap();
    if let Some(m) = standard_case_no_re.find(text) {
        doc.case_no = Some(m.as_str().to_string());
        return;
    }

    // 2. 国知局审查决定号：如 第56123号 / 第123456号无效宣告请求审查决定
    let decision_no_re =
        Regex::new(r"第\s*\d{4,7}\s*号(?:无效宣告请求审查决定|复审请求审查决定)?").unwrap();
    if let Some(m) = decision_no_re.find(text) {
        doc.case_no = Some(m.as_str().to_string());
        return;
    }

    // 3. 国知局案件编号：4W123456 / 5W123456 / 5F123456
    let cnipa_code_re = Regex::new(r"\b(\d+[WF]\d+)\b").unwrap();
    if let Some(caps) = cnipa_code_re.captures(text) {
        doc.case_no = Some(caps[1].to_string());
    }
}

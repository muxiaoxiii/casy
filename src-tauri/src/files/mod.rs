use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::db;

/// 案件文件夹根目录
pub fn case_folder_base() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Casy")
        .join("cases")
}

/// 从模板 JSON 解析子目录列表
fn parse_template_directories(json_str: &str) -> Vec<(String, String)> {
    serde_json::from_str::<Vec<serde_json::Value>>(json_str)
        .unwrap_or_default()
        .iter()
        .filter_map(|v| {
            let id = v.get("id")?.as_str()?;
            let name = v.get("name")?.as_str()?;
            Some((format!("{}_{}", id, name), name.to_string()))
        })
        .collect()
}

/// 根据模板 ID 从数据库加载子目录结构
fn load_template_dirs(template_id: &str) -> Option<Vec<(String, String)>> {
    let conn = db::open_db().ok()?;
    let json: String = conn
        .query_row(
            "SELECT directories_json FROM case_folder_templates WHERE id = ?1",
            rusqlite::params![template_id],
            |row| row.get(0),
        )
        .ok()?;
    let dirs = parse_template_directories(&json);
    if dirs.is_empty() {
        None
    } else {
        Some(dirs)
    }
}

/// 根据 case_type 推断内置模板 ID
fn builtin_template_id_for_case_type(case_type: Option<&str>) -> &'static str {
    match case_type {
        Some(t)
            if t.contains("侵权")
                || t.contains("民事")
                || t.contains("刑事")
                || t.contains("行政")
                || t.contains("合同")
                || t.contains("纠纷") =>
        {
            "tpl-litigation"
        }
        Some(t)
            if t.contains("专利")
                || t.contains("发明")
                || t.contains("实用新型")
                || t.contains("外观")
                || t.contains("无效") =>
        {
            "tpl-patent"
        }
        Some(t) if t.contains("商标") => "tpl-trademark",
        _ => "tpl-consultation",
    }
}

/// 案件类型 → 子目录结构
///
/// 优先使用模板 ID，否则按案件类型查找内置模板，最后回退到硬编码默认值。
pub fn subdirectories_for_case_type(
    case_type: Option<&str>,
    template_id: Option<&str>,
) -> Vec<(String, String)> {
    // 1) 如果有指定模板 ID，从数据库加载
    if let Some(tid) = template_id {
        if let Some(dirs) = load_template_dirs(tid) {
            return dirs;
        }
    }
    // 2) 按案件类型查找内置模板
    let builtin_id = builtin_template_id_for_case_type(case_type);
    if let Some(dirs) = load_template_dirs(builtin_id) {
        return dirs;
    }
    // 3) 回退硬编码默认值（数据库未初始化时）
    hardcoded_fallback(case_type)
}

/// 硬编码回退（数据库未就绪时使用）
fn hardcoded_fallback(case_type: Option<&str>) -> Vec<(String, String)> {
    match case_type {
        Some(t)
            if t.contains("侵权")
                || t.contains("民事")
                || t.contains("刑事")
                || t.contains("行政")
                || t.contains("合同")
                || t.contains("纠纷") =>
        {
            vec![
                ("01_委托材料".into(), "委托合同、授权书、身份证明".into()),
                ("02_案件分析".into(), "案件分析、争议焦点、诉讼策略".into()),
                ("03_法律研究".into(), "法条检索、判例研究".into()),
                ("04_客户提供".into(), "客户提供的所有材料".into()),
                ("05_证据材料".into(), "证据清单、质证意见".into()),
                ("06_法律文书".into(), "起诉状、答辩状、代理词".into()),
                ("07_对方提交".into(), "对方当事人提交的材料".into()),
                (
                    "08_法院文书".into(),
                    "传票、判决书、裁定书、送达文书".into(),
                ),
                ("09_庭审材料".into(), "庭审笔录、庭后分析".into()),
                ("10_综合报告".into(), "进展报告、客户汇报".into()),
                ("11_其他".into(), "辅助性参考材料".into()),
            ]
        }
        Some(t)
            if t.contains("专利")
                || t.contains("发明")
                || t.contains("实用新型")
                || t.contains("外观")
                || t.contains("无效") =>
        {
            vec![
                ("01_委托材料".into(), "代理委托书、合同、工作记录".into()),
                ("02_申请清单".into(), "拟申请专利清单".into()),
                (
                    "03_客户提供".into(),
                    "技术交底书、现有技术资料".into(),
                ),
                ("04_律师工作".into(), "检索报告、分析、申请规划".into()),
                (
                    "05_申请文件".into(),
                    "请求书、说明书、权利要求书".into(),
                ),
                (
                    "06_国知局文件".into(),
                    "受理通知书、审查意见、授权通知".into(),
                ),
                ("07_对方提交".into(), "对方意见、无效请求".into()),
                ("08_证据材料".into(), "证据清单、对比文件".into()),
                ("09_财务".into(), "代理费发票、官费凭证".into()),
            ]
        }
        Some(t) if t.contains("商标") => vec![
            ("01_委托材料".into(), "委托书、合同、工作记录".into()),
            ("02_商标图样".into(), "商标图样、设计稿".into()),
            ("03_申请文件".into(), "申请书、商品清单".into()),
            ("04_律师工作".into(), "检索报告、分析、策略".into()),
            ("05_官方文书".into(), "受理通知书、驳回决定".into()),
            ("06_商标注册证".into(), "注册证、续展证明".into()),
            ("07_证据材料".into(), "异议/无效证据".into()),
            ("08_对方提交".into(), "对方意见、答辩".into()),
            ("09_财务".into(), "代理费发票、官费凭证".into()),
        ],
        _ => vec![
            ("01_客户材料".into(), "客户提供的所有材料".into()),
            ("02_工作文件".into(), "律师工作产出".into()),
            ("03_其他".into(), "辅助性材料".into()),
        ],
    }
}

/// 确保案件文件夹存在，按案件类型创建标准化子目录
pub fn ensure_case_folder(case: &db::cases::Case) -> Result<PathBuf> {
    let base = case_folder_base();
    let case_no = case.case_no.as_deref().unwrap_or("无案号");
    let short_id = &case.id[..8.min(case.id.len())];
    let folder_name = format!("{}_{}", sanitize_filename(case_no), short_id);
    let folder = base.join(&folder_name);

    std::fs::create_dir_all(&folder)?;

    // 按模板或案件类型创建子目录
    let subs = subdirectories_for_case_type(
        case.cause_action.as_deref(),
        case.folder_template_id.as_deref(),
    );
    for (name, _desc) in &subs {
        std::fs::create_dir_all(folder.join(name))?;
    }

    Ok(folder)
}

/// 文件路由：根据文档类型确定目标子目录
///
/// 收件箱归档、文件导入时调用，自动放到正确的子目录。
/// 返回子目录名（如 "08_法院文书"）。
pub fn route_file_to_subdir(
    category: &str,
    case_type: Option<&str>,
    template_id: Option<&str>,
) -> String {
    let subs = subdirectories_for_case_type(case_type, template_id);

    // category → 优先匹配的子目录关键词
    let preferred = match category {
        "summons" | "传票" => vec!["法院文书", "传票"],
        "judgment" | "判决" | "裁定" | "决定" => vec!["法院文书", "判决"],
        "service" | "送达" => vec!["法院文书", "送达"],
        "evidence" | "证据" => vec!["证据材料", "证据"],
        "complaint" | "起诉状" => vec!["法律文书", "起诉状", "申请文件"],
        "defence" | "答辩状" | "代理词" => vec!["法律文书", "对方提交", "答辩"],
        "correspondence" | "函件" | "通信" => vec!["客户提供", "委托材料", "通信"],
        "official_notice" | "通知书" => vec!["法院文书", "国知局文件", "官方文书"],
        "contract" | "合同" => vec!["委托材料", "合同"],
        "invoice" | "发票" => vec!["财务", "发票"],
        "patent_doc" | "专利文件" => vec!["国知局文件", "申请文件"],
        "trademark_doc" | "商标文件" => vec!["官方文书", "商标注册证"],
        _ => vec!["其他", "工作文件"],
    };

    // 在案件类型的子目录中查找匹配
    for keyword in &preferred {
        for (name, _) in &subs {
            if name.contains(keyword) {
                return name.clone();
            }
        }
    }

    // 兜底：最后一个子目录（通常是"其他"）
    subs.last()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "其他".to_string())
}

/// 清理文件名中的非法字符
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// 智能命名（含 SHA-256 防覆盖）
#[allow(dead_code)]
pub fn smart_rename(
    original: &str,
    case: &db::cases::Case,
    category: &str,
    doc_date: Option<&str>,
) -> String {
    let date_str = doc_date
        .map(|d| d.to_string())
        .unwrap_or_else(|| chrono::Local::now().naive_local().date().format("%Y-%m-%d").to_string());

    let category_cn = match category {
        "summons" => "传票",
        "evidence" => "证据",
        "submitted" => "交文",
        "received" => "收文",
        "internal" => "内部",
        "correspondence" => "通信",
        _ => "其他",
    };

    let ext = Path::new(original)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("pdf");

    let case_no = case.case_no.as_deref().unwrap_or("无案号");
    let clean_no: String = case_no
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    // SHA-256 防覆盖
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(original.as_bytes());
    hasher.update(case.id.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let hash_suffix = &hash[..8];

    format!("{date_str}_{category_cn}_{clean_no}_{hash_suffix}.{ext}")
}

/// 自动分类文件（增强版，参照 legal-skills 的分类规则）
pub fn auto_classify(file_name: &str) -> &'static str {
    let name_lower = file_name.to_lowercase();

    // 法院文书
    if name_lower.contains("传票") || name_lower.contains("应诉通知书")
        || name_lower.contains("受理通知书") || name_lower.contains("举证通知书") {
        return "summons";
    }
    if name_lower.contains("判决") || name_lower.contains("裁定")
        || name_lower.contains("决定") || name_lower.contains("调解书") {
        return "judgment";
    }
    if name_lower.contains("送达") || name_lower.contains("回证") {
        return "service";
    }

    // 法律文书
    if name_lower.contains("起诉状") || name_lower.contains("起诉书") {
        return "complaint";
    }
    if name_lower.contains("答辩状") || name_lower.contains("代理词")
        || name_lower.contains("辩护词") || name_lower.contains("上诉状") {
        return "defence";
    }

    // 证据
    if name_lower.contains("证据") || name_lower.contains("质证") {
        return "evidence";
    }

    // 国知局/商标局
    if name_lower.contains("审查意见") || name_lower.contains("驳回决定")
        || name_lower.contains("授权通知") || name_lower.contains("缴费通知") {
        return "official_notice";
    }

    // 专利文件
    if name_lower.contains("权利要求") || name_lower.contains("说明书")
        || name_lower.contains("技术交底") || name_lower.contains("检索报告") {
        return "patent_doc";
    }

    // 商标文件
    if name_lower.contains("商标注册证") || name_lower.contains("商标图样") {
        return "trademark_doc";
    }

    // 合同/委托
    if name_lower.contains("委托") || name_lower.contains("合同")
        || name_lower.contains("授权书") || name_lower.contains("协议") {
        return "contract";
    }

    // 发票
    if name_lower.contains("发票") || name_lower.contains("收据")
        || name_lower.contains("缴费") {
        return "invoice";
    }

    // 函件
    if name_lower.contains("函") || name_lower.contains("邮件") {
        return "correspondence";
    }

    "other"
}

/// 归档收件箱文件到案件目录
///
/// 1. 确保案件目录存在
/// 2. 根据文件类型路由到正确子目录
/// 3. 复制文件（保留原文件在收件箱）
/// 4. 返回目标路径
pub fn file_to_case(source_path: &Path, case: &db::cases::Case, category: &str) -> Result<PathBuf> {
    let case_folder = ensure_case_folder(case)?;
    let subdir = route_file_to_subdir(
        category,
        case.cause_action.as_deref(),
        case.folder_template_id.as_deref(),
    );
    let target_dir = case_folder.join(subdir);
    std::fs::create_dir_all(&target_dir)?;

    let file_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf");

    // 重命名：日期_类型_案号_hash.ext
    let new_name = smart_rename(file_name, case, category, None);
    let target_path = target_dir.join(&new_name);

    // 复制文件（保留原文件在收件箱供复查）
    std::fs::copy(source_path, &target_path)?;

    Ok(target_path)
}

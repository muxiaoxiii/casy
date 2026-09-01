use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use calamine::{open_workbook_auto, Data, DataType, Reader};
use chrono::{Duration, NaiveDate};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::run_blocking;
use crate::db;

/// 工作表摘要信息
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SheetInfo {
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
}

/// 列映射推荐
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMappingRecommendation {
    pub column_index: usize,
    pub excel_header: String,
    pub sample_values: Vec<String>,
    pub suggested_field: Option<String>,
    pub confidence: f32,
}

/// 工作表探测与预览结果
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExcelInspectResult {
    pub sheet_name: String,
    pub total_rows: usize,
    pub detected_header_row: usize,
    pub columns: Vec<ColumnMappingRecommendation>,
    pub preview_rows: Vec<HashMap<String, serde_json::Value>>,
}

/// 案件导入配置
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaseImportConfig {
    pub header_row: usize,
    pub column_mappings: HashMap<usize, String>,
    pub forward_fill_columns: Vec<usize>,
    pub conflict_strategy: String, // "skip" | "update" | "duplicate"
    pub default_track: Option<String>,
}

/// 案件导入报告
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaseImportReport {
    pub total_rows_processed: usize,
    pub created_count: usize,
    pub updated_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
    pub imported_case_ids: Vec<String>,
}

/// 分表导入配置 (支持关联任务、庭审、办案日志分表)
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SubtableImportConfig {
    pub target_entity: String, // "cases" | "tasks" | "hearings" | "case_logs"
    pub header_row: usize,
    pub column_mappings: HashMap<usize, String>,
    pub forward_fill_columns: Vec<usize>,
}

/// 分表导入报告
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SubtableImportReport {
    pub target_entity: String,
    pub total_rows_processed: usize,
    pub created_count: usize,
    pub linked_cases_count: usize,
    pub unlinked_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════
// 底层数据清洗与转换算法 (Smart Value Cleaner)
// ═══════════════════════════════════════════════════════════════════

/// 将 calamine 单元格数据转为清晰的字符串表示
pub fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => {
            // 如果是浮点整数，去掉 .0
            if (f.fract()).abs() < f64::EPSILON && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::DateTime(dt) => {
            excel_serial_date_to_str(dt.as_f64()).unwrap_or_else(|| format!("{}", dt.as_f64()))
        }
        Data::DateTimeIso(iso) => iso.clone(),
        Data::DurationIso(iso) => iso.clone(),
        Data::Error(e) => format!("#ERR:{:?}", e),
    }
}

/// Excel Serial Date 转 YYYY-MM-DD 字符串
/// Excel 将 1900-01-01 记为 1，且存在历史 Lotus 1900 闰年 bug (将 1900 视为闰年，serial 60 = 1900-02-29)
pub fn excel_serial_date_to_str(serial: f64) -> Option<String> {
    let int_part = serial.floor() as i64;
    // 基础范围保护 (1900年 ~ 2100年，对应序列号 1 ~ 73050)
    if !(1..=100_000).contains(&int_part) {
        return None;
    }

    let base_date = if int_part >= 61 {
        // 超过 1900-02-28，基准为 1899-12-30
        NaiveDate::from_ymd_opt(1899, 12, 30)?
    } else {
        // 1900-02-28 之前，基准为 1899-12-31
        NaiveDate::from_ymd_opt(1899, 12, 31)?
    };

    let target_date = base_date + Duration::days(int_part);
    Some(target_date.format("%Y-%m-%d").to_string())
}

/// 智能提取字符串中包含的所有有效日期（支持多行、带时间、中文年月日、斜杠等）
pub fn extract_dates_list(raw: &str) -> Vec<String> {
    let mut dates = Vec::new();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return dates;
    }

    // 先按行/分号拆分
    let lines: Vec<&str> = trimmed
        .split(['\n', '\r', ';', '；', '、'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let re = Regex::new(r"(\d{4})[-/.\s年](\d{1,2})[-/.\s月](\d{1,2})").unwrap();
    let re8 = Regex::new(r"(20\d{2}|19\d{2})(\d{2})(\d{2})").unwrap();

    for line in lines {
        if let Some(d) = clean_single_date(line) {
            if !dates.contains(&d) {
                dates.push(d);
            }
        } else {
            // 在行内进行子串正则提取
            for cap in re.captures_iter(line) {
                if let (Ok(y), Ok(m), Ok(d)) = (
                    cap[1].parse::<i32>(),
                    cap[2].parse::<u32>(),
                    cap[3].parse::<u32>(),
                ) {
                    if let Some(nd) = NaiveDate::from_ymd_opt(y, m, d) {
                        let ds = nd.format("%Y-%m-%d").to_string();
                        if !dates.contains(&ds) {
                            dates.push(ds);
                        }
                    }
                }
            }

            // 紧凑 8 位日期：20250113
            for cap in re8.captures_iter(line) {
                if let (Ok(y), Ok(m), Ok(d)) = (
                    cap[1].parse::<i32>(),
                    cap[2].parse::<u32>(),
                    cap[3].parse::<u32>(),
                ) {
                    if let Some(nd) = NaiveDate::from_ymd_opt(y, m, d) {
                        let ds = nd.format("%Y-%m-%d").to_string();
                        if !dates.contains(&ds) {
                            dates.push(ds);
                        }
                    }
                }
            }
        }
    }

    dates
}

pub fn clean_single_date(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. 尝试作为 Excel Serial Number 解析
    if let Ok(num) = trimmed.parse::<f64>() {
        if (30000.0..=80000.0).contains(&num) {
            if let Some(d) = excel_serial_date_to_str(num) {
                return Some(d);
            }
        }
    }

    // 2. 正则捕获：YYYY[-/.\s年]MM[-/.\s月]DD[日号]? (可能后面跟随时间或文字)
    let ymd_re = Regex::new(
        r"^(\d{4})[-/.\s年](\d{1,2})[-/.\s月](\d{1,2})[日号]?(\s*(\d{1,2}:\d{1,2}(:\d{1,2})?)?.*)?$",
    )
    .unwrap();
    if let Some(caps) = ymd_re.captures(trimmed) {
        if let (Ok(y), Ok(m), Ok(d)) = (
            caps[1].parse::<i32>(),
            caps[2].parse::<u32>(),
            caps[3].parse::<u32>(),
        ) {
            if let Some(nd) = NaiveDate::from_ymd_opt(y, m, d) {
                return Some(nd.format("%Y-%m-%d").to_string());
            }
        }
    }

    // 3. 紧凑 8 位日期：20240501
    let compact_re = Regex::new(r"^(20\d{2}|19\d{2})(\d{2})(\d{2})(.*)$").unwrap();
    if let Some(caps) = compact_re.captures(trimmed) {
        if let (Ok(y), Ok(m), Ok(d)) = (
            caps[1].parse::<i32>(),
            caps[2].parse::<u32>(),
            caps[3].parse::<u32>(),
        ) {
            if let Some(nd) = NaiveDate::from_ymd_opt(y, m, d) {
                return Some(nd.format("%Y-%m-%d").to_string());
            }
        }
    }

    // 4. 2位年份：24-05-01 / 24/5/1 / 24.5.1
    let short_y_re = Regex::new(r"^(\d{2})[-/.](\d{1,2})[-/.](\d{1,2})(.*)$").unwrap();
    if let Some(caps) = short_y_re.captures(trimmed) {
        if let (Ok(y_short), Ok(m), Ok(d)) = (
            caps[1].parse::<i32>(),
            caps[2].parse::<u32>(),
            caps[3].parse::<u32>(),
        ) {
            let y = if y_short < 70 {
                2000 + y_short
            } else {
                1900 + y_short
            };
            if let Some(nd) = NaiveDate::from_ymd_opt(y, m, d) {
                return Some(nd.format("%Y-%m-%d").to_string());
            }
        }
    }

    None
}

/// 智能日期清洗引擎：支持 Excel 序列号、中文年月日、斜杠点分、2位年份等
pub fn clean_date_str(raw: &str) -> Option<String> {
    let dates = extract_dates_list(raw);
    dates.into_iter().next()
}

/// 智能金额清洗引擎：去除货币符号、识别“万/亿/k”单位折算为标准数值字符串
pub fn clean_amount_str(raw: &str) -> Option<String> {
    let mut s = raw
        .trim()
        .replace(['￥', '¥', '$', '€', ',', '，', ' '], "");
    if s.is_empty() {
        return None;
    }

    // 去掉尾部的“元”
    if s.ends_with('元') {
        s.pop();
    }

    // 提取倍数
    let (multiplier, s_num) = if s.ends_with("万") {
        (10_000.0, s.trim_end_matches("万"))
    } else if s.ends_with("亿") {
        (100_000_000.0, s.trim_end_matches("亿"))
    } else if s.ends_with('k') || s.ends_with('K') || s.ends_with("千") {
        (1_000.0, s.trim_end_matches(['k', 'K', '千']))
    } else if s.ends_with('w') || s.ends_with('W') {
        (10_000.0, s.trim_end_matches(['w', 'W']))
    } else {
        (1.0, s.as_str())
    };

    if let Ok(num) = s_num.trim().parse::<f64>() {
        let total = num * multiplier;
        return Some(format!("{:.2}", total));
    }

    None
}

/// 智能人员/数组清洗：支持顿号、逗号、斜杠、分号、换行拆分与去重
pub fn clean_array_to_json(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<String> = trimmed
        .split(|c| {
            c == '、'
                || c == '/'
                || c == ','
                || c == '，'
                || c == ';'
                || c == '；'
                || c == '\n'
                || c == '\r'
                || c == '|'
        })
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();

    if parts.is_empty() {
        return None;
    }

    // 去重但保留顺序
    let mut unique_parts = Vec::new();
    for p in parts {
        if !unique_parts.contains(&p) {
            unique_parts.push(p);
        }
    }

    serde_json::to_string(&unique_parts).ok()
}

/// 智能推断赛道 (Track) 与 案件路径 (Case Route)
/// 返回值: (safe_track, raw_track, case_route)
pub fn infer_track_and_route(
    default_track_opt: Option<&str>,
    row_track_opt: Option<&str>,
    row_route_opt: Option<&str>,
    case_name: &str,
    cause_action: Option<&str>,
) -> (String, String, String) {
    let map_track_to_route = |t: &str| -> String {
        match t {
            "patent_invalidation" => "专利无效".to_string(),
            "admin_litigation" => "行政诉讼".to_string(),
            _ => "民事诉讼".to_string(),
        }
    };

    let validate_route = |r: &str| -> String {
        let valid = ["民事诉讼", "专利无效", "行政诉讼", "民事诉讼+专利无效", "专利无效+行政诉讼", "三轨并行"];
        if valid.contains(&r) {
            r.to_string()
        } else {
            "民事诉讼".to_string() // Fallback to avoid constraint error if it ever mattered
        }
    };

    let get_tracks = |t: &str| -> (String, String) {
        // (safe_track, raw_track)
        if t.contains("无效") || t.contains("patent invalidation") {
            ("patent_invalidation".to_string(), "patent_invalidation".to_string())
        } else if t.contains("复审") {
            ("other".to_string(), "patent_reexam".to_string())
        } else if t.contains("民事") || t.contains("侵权") || t.contains("合同") || t.contains("civil") {
            ("civil_tort".to_string(), "civil_first_instance".to_string())
        } else if t.contains("行政") || t.contains("admin") {
            ("admin_litigation".to_string(), "admin_first_instance".to_string())
        } else if t.contains("刑事") || t.contains("criminal") {
            ("other".to_string(), "criminal_investigation".to_string())
        } else if t.contains("仲裁") || t.contains("arbitration") {
            ("other".to_string(), "commercial_arbitration".to_string())
        } else if t.contains("非诉") || t.contains("顾问") || t.contains("合规") {
            ("other".to_string(), "non_litigation".to_string())
        } else {
            // Default fallback
            ("other".to_string(), "other".to_string())
        }
    };

    if let Some(rt) = row_track_opt {
        if !rt.trim().is_empty() {
            let t = rt.to_lowercase();
            let (mut safe_track, mut raw_track) = get_tracks(&t);
            
            // If the row explicitly matches nothing special, try the default track
            if safe_track == "other" && raw_track == "other" {
                if let Some(dt) = default_track_opt {
                    if !dt.trim().is_empty() {
                        let (dt_safe, dt_raw) = get_tracks(&dt.to_lowercase());
                        safe_track = dt_safe;
                        raw_track = dt_raw;
                    }
                }
            }
            
            let final_route = if let Some(rr) = row_route_opt {
                if !rr.trim().is_empty() {
                    validate_route(rr)
                } else {
                    map_track_to_route(&safe_track)
                }
            } else {
                map_track_to_route(&safe_track)
            };
            
            return (safe_track, raw_track, final_route);
        }
    }

    if let (Some(t), Some(r)) = (default_track_opt, row_route_opt) {
        if !t.is_empty() && !r.is_empty() {
            let (safe_t, raw_t) = get_tracks(&t.to_lowercase());
            return (safe_t, raw_t, validate_route(r));
        }
    }

    if let Some(t) = default_track_opt {
        if !t.is_empty() {
            let (safe_t, raw_t) = get_tracks(&t.to_lowercase());
            let route = map_track_to_route(&safe_t);
            return (safe_t, raw_t, route);
        }
    }

    let combined = format!("{} {}", case_name, cause_action.unwrap_or_default());
    let (fallback_safe, fallback_raw) = get_tracks(&combined);
    (fallback_safe.clone(), fallback_raw.clone(), map_track_to_route(&fallback_safe))
}

// ═══════════════════════════════════════════════════════════════════
// 智能表头与字段映射匹配 (Header Detection & Fuzzy Matcher)
// ═══════════════════════════════════════════════════════════════════

struct FieldMatcher {
    field_name: &'static str,
    keywords: &'static [&'static str],
}

const FIELD_MATCHERS: &[FieldMatcher] = &[
    FieldMatcher {
        field_name: "caseNo",
        keywords: &[
            "案号",
            "法院案号",
            "审判案号",
            "字号",
            "执行案号",
            "诉讼案号",
            "caseno",
            "case no",
        ],
    },
    FieldMatcher {
        field_name: "caseName",
        keywords: &[
            "案件信息",
            "案件名称",
            "案名",
            "案件全称",
            "案件",
            "纠纷名称",
            "项目名称",
            "casename",
            "case name",
        ],
    },
    FieldMatcher {
        field_name: "clientName",
        keywords: &[
            "我方-名称",
            "我方名称",
            "我方",
            "委托方",
            "委托人",
            "委托单位",
            "受托人",
            "客户",
            "客户名称",
            "原告",
            "申请人",
            "申请方",
            "甲方",
            "client",
            "client name",
        ],
    },
    FieldMatcher {
        field_name: "opponentName",
        keywords: &[
            "对方-名称",
            "对方名称",
            "对方",
            "相对方",
            "对方当事人",
            "被告",
            "相对人",
            "被申请人",
            "被申请方",
            "被执行人",
            "乙方",
            "第三人",
            "opponent",
            "opponent name",
        ],
    },
    FieldMatcher {
        field_name: "ourRole",
        keywords: &[
            "我方-诉讼地位",
            "我方诉讼地位",
            "我方地位",
            "诉讼地位",
            "诉讼身份",
            "我方角色",
            "代理地位",
            "代理身份",
            "我方主体",
            "our role",
        ],
    },
    FieldMatcher {
        field_name: "opponentRole",
        keywords: &[
            "对方-诉讼地位",
            "对方诉讼地位",
            "对方地位",
            "对方角色",
            "相对方地位",
            "opponent role",
        ],
    },
    FieldMatcher {
        field_name: "causeAction",
        keywords: &["案由", "诉由", "纠纷类型", "纠纷", "争议事由", "cause of action"],
    },
    FieldMatcher {
        field_name: "court",
        keywords: &[
            "管辖-审理机关",
            "管辖审理机关",
            "审理机关",
            "审理法院",
            "受诉法院",
            "管辖法院",
            "法院",
            "国知局",
            "仲裁委",
            "仲裁委员会",
            "court",
        ],
    },
    FieldMatcher {
        field_name: "judgePanel",
        keywords: &[
            "管辖-合议庭",
            "管辖合议庭",
            "合议庭",
            "承办法官",
            "审判长",
            "法官",
            "主审法官",
            "主审人",
            "承办人(法院)",
            "judge",
        ],
    },
    FieldMatcher {
        field_name: "clerk",
        keywords: &[
            "管辖-书记员/助理",
            "管辖书记员",
            "书记员/助理",
            "书记员助理",
            "法院电话",
            "法官电话",
            "书记员",
            "法官助理",
            "书记员电话",
            "联系电话",
            "法院联系方式",
            "clerk",
        ],
    },
    FieldMatcher {
        field_name: "attorneys",
        keywords: &[
            "负责律师",
            "承办律师",
            "主办律师",
            "承办人",
            "主办人",
            "代理人",
            "经办律师",
            "协办律师",
            "主办团队",
            "律师",
            "attorney",
            "lawyer",
        ],
    },
    FieldMatcher {
        field_name: "filingDate",
        keywords: &[
            "案件进度-立案",
            "案件进度立案",
            "立案",
            "接案日期",
            "接案时间",
            "立案日期",
            "立案时间",
            "收案日期",
            "收案时间",
            "签约日期",
            "受案日期",
            "受理时间",
            "委托日期",
            "委派时间",
            "filing date",
        ],
    },
    FieldMatcher {
        field_name: "trial3Date",
        keywords: &[
            "三次开庭丨口审",
            "三次开庭/口审",
            "三次开庭",
            "第三次开庭",
            "三审开庭",
            "3次开庭",
            "第三期开庭",
            "三次口审",
            "第三次口审",
            "trial3",
        ],
    },
    FieldMatcher {
        field_name: "trial2Date",
        keywords: &[
            "二次开庭丨口审",
            "二次开庭/口审",
            "二次开庭",
            "第二次开庭",
            "二审开庭",
            "2次开庭",
            "第二期开庭",
            "二次口审",
            "第二次口审",
            "trial2",
        ],
    },
    FieldMatcher {
        field_name: "trialDate",
        keywords: &[
            "一次开庭丨口审",
            "一次开庭/口审",
            "一次开庭",
            "第一次开庭",
            "案件进度-开庭/口审",
            "案件进度开庭",
            "开庭/口审",
            "开庭丨口审",
            "开庭口审",
            "未来开庭",
            "最近已开庭",
            "开庭日期",
            "开庭时间",
            "口审日期",
            "口审时间",
            "开庭",
            "口审",
            "trial date",
            "trial",
        ],
    },
    FieldMatcher {
        field_name: "verdictDate",
        keywords: &[
            "案件进度-裁判",
            "裁判日期",
            "判决日期",
            "判决时间",
            "裁定时间",
            "裁判时间",
            "verdict date",
        ],
    },
    FieldMatcher {
        field_name: "completedText",
        keywords: &[
            "重要节点-已完成",
            "重要节点已完成",
            "已完成节点",
            "已完成事项",
            "已完成",
            "completed text",
        ],
    },
    FieldMatcher {
        field_name: "caseLevel",
        keywords: &[
            "管辖-审级",
            "管辖审级",
            "案件阶段",
            "审级",
            "案件审级",
            "阶段",
            "诉讼阶段",
            "程序阶段",
            "case level",
        ],
    },
    FieldMatcher {
        field_name: "caseProgress",
        keywords: &[
            "进展",
            "案件进度",
            "当前进展",
            "办理情况",
            "案情进展",
            "执行阶段",
            "处理进度",
            "progress",
        ],
    },
    FieldMatcher {
        field_name: "caseResult",
        keywords: &[
            "案件进度-裁判",
            "裁判",
            "案件结果",
            "判决结果",
            "裁判结果",
            "结案方式",
            "裁判文书",
            "处理结果",
            "result",
        ],
    },
    FieldMatcher {
        field_name: "stayDate",
        keywords: &[
            "保全结束时间",
            "保全开始时间",
            "保全时间",
            "保全期限",
            "财产保全",
            "中止时间",
            "stay date",
        ],
    },
    FieldMatcher {
        field_name: "internalNo",
        keywords: &[
            "案件编号",
            "内部编号",
            "内部案号",
            "内部流水号",
            "档案编号",
            "系统编号",
            "流水号",
            "卷号",
            "卷宗号",
            "internal no",
        ],
    },
    FieldMatcher {
        field_name: "patentName",
        keywords: &[
            "涉案知识产权-名",
            "涉案知识产权名",
            "涉案知识产权名称",
            "涉案知识产权",
            "专利名称",
            "涉案专利",
            "发明名称",
            "patent name",
        ],
    },
    FieldMatcher {
        field_name: "patentAppNo",
        keywords: &[
            "涉案知识产权-号",
            "涉案知识产权号",
            "专利号",
            "申请号",
            "patent no",
            "app no",
        ],
    },
    FieldMatcher {
        field_name: "taskName",
        keywords: &[
            "任务名称",
            "待办事项",
            "任务内容",
            "待办内容",
            "待办工作",
            "工作任务",
            "task name",
            "task",
        ],
    },
    FieldMatcher {
        field_name: "deadline",
        keywords: &[
            "截止日期",
            "截止时间",
            "到期日",
            "应完成时间",
            "办理期限",
            "限期",
            "due date",
            "deadline",
        ],
    },
    FieldMatcher {
        field_name: "priority",
        keywords: &[
            "优先级",
            "重要程度",
            "紧急程度",
            "重要紧急程度",
            "priority",
        ],
    },
    FieldMatcher {
        field_name: "assignee",
        keywords: &[
            "执行人",
            "责任人",
            "指派人",
            "主办人",
            "承办人",
            "assignee",
        ],
    },
    FieldMatcher {
        field_name: "description",
        keywords: &[
            "任务描述",
            "详细说明",
            "具体要求",
            "任务详情",
            "description",
        ],
    },
    FieldMatcher {
        field_name: "completed",
        keywords: &[
            "完成状态",
            "是否完成",
            "完成情况",
            "完成与否",
            "completed",
            "status",
        ],
    },
    FieldMatcher {
        field_name: "eventDate",
        keywords: &[
            "事件日期",
            "记录时间",
            "日志时间",
            "发生日期",
            "活动时间",
            "event date",
        ],
    },
    FieldMatcher {
        field_name: "eventType",
        keywords: &[
            "事件类型",
            "日志类型",
            "活动类型",
            "节点类型",
            "event type",
        ],
    },
    FieldMatcher {
        field_name: "content",
        keywords: &[
            "日志内容",
            "事件内容",
            "记录详情",
            "沟通记录",
            "工作内容",
            "content",
        ],
    },
    FieldMatcher {
        field_name: "operator",
        keywords: &[
            "经办人",
            "记录人",
            "填写人",
            "登记人",
            "operator",
        ],
    },
    FieldMatcher {
        field_name: "hearingName",
        keywords: &[
            "庭审名称",
            "庭审轮次",
            "口审轮次",
            "开庭说明",
            "hearing name",
        ],
    },
    FieldMatcher {
        field_name: "notes",
        keywords: &[
            "对方-代理律所",
            "对方代理律所",
            "对方-代理人",
            "对方代理人",
            "案件进度-管辖异议",
            "管辖异议",
            "重要节点-已完成",
            "重要节点已完成",
            "已完成",
            "重要节点-待办",
            "重要节点待办",
            "待办",
            "办案日志",
            "诉讼请求",
            "律师费用",
            "律师费到账情况",
            "重要紧急程度",
            "提交续保书面申请",
            "备注",
            "说明",
            "备注说明",
            "工作日志",
            "标的额",
            "诉请金额",
            "律师费",
            "代理费",
            "紧急程度",
            "notes",
            "remark",
        ],
    },
];

/// 构建融合表头（自动检测并融合复合双层/多层表头与合并单元格）
pub fn resolve_effective_headers(raw_rows: &[Vec<Data>], header_row_idx: usize) -> Vec<String> {
    let current_row = match raw_rows.get(header_row_idx) {
        Some(r) => r,
        None => return Vec::new(),
    };

    let col_count = current_row.len();
    if col_count == 0 {
        return Vec::new();
    }

    // 检查是否有上一行作为父级表头
    let parent_row = if header_row_idx > 0 {
        raw_rows.get(header_row_idx - 1)
    } else {
        None
    };

    let mut parent_forward_filled = Vec::with_capacity(col_count);
    if let Some(p_row) = parent_row {
        let mut last_parent_text = String::new();
        for cell in p_row {
            let s = cell_to_string(cell).trim().to_string();
            if !s.is_empty() {
                last_parent_text = s.clone();
                parent_forward_filled.push(s);
            } else {
                parent_forward_filled.push(last_parent_text.clone());
            }
        }
    }

    let mut result = Vec::with_capacity(col_count);
    for (col_idx, cell) in current_row.iter().enumerate() {
        let cur_text = cell_to_string(cell).trim().to_string();
        let parent_text = parent_forward_filled.get(col_idx).map(|s| s.trim()).unwrap_or("");

        let effective = if cur_text.is_empty() {
            // 当前行为空，直接取上一层表头（例如“案号”、“案由”、“序号”、“备注”）
            if !parent_text.is_empty() {
                parent_text.to_string()
            } else {
                format!("第 {} 列", col_idx + 1)
            }
        } else if !parent_text.is_empty() && parent_text != cur_text {
            // 两层都有内容（如父级“我方” + 子级“诉讼地位”，或父级“对方” + 子级“名称”）
            if cur_text == "名称" || cur_text == "名" {
                format!("{}-名称", parent_text)
            } else if cur_text == "号" {
                format!("{}-号", parent_text)
            } else if cur_text == "诉讼地位" || cur_text == "地位" {
                format!("{}-诉讼地位", parent_text)
            } else {
                format!("{}-{}", parent_text, cur_text)
            }
        } else {
            cur_text
        };

        result.push(effective);
    }

    result
}

/// 评估列名与系统字段的匹配度 (0.0 ~ 1.0)
pub fn match_field_confidence(header: &str) -> (Option<String>, f32) {
    let clean_header = header
        .trim()
        .to_lowercase()
        .replace([' ', '_', '-', ':', '：', '(', ')', '（', '）', '【', '】', '[', ']'], "");

    if clean_header.is_empty() {
        return (None, 0.0);
    }

    // 1. 先进行完全精确匹配 (Exact Match 优先，防止子串抢先拦截)
    for matcher in FIELD_MATCHERS {
        for &kw in matcher.keywords {
            let clean_kw = kw
                .to_lowercase()
                .replace([' ', '_', '-', ':', '：', '(', ')', '（', '）', '【', '】', '[', ']'], "");
            if clean_header == clean_kw {
                return (Some(matcher.field_name.to_string()), 1.0);
            }
        }
    }

    // 2. 再进行包含匹配 (Substring Match)
    for matcher in FIELD_MATCHERS {
        for &kw in matcher.keywords {
            let clean_kw = kw
                .to_lowercase()
                .replace([' ', '_', '-', ':', '：', '(', ')', '（', '）', '【', '】', '[', ']'], "");
            if clean_header.contains(&clean_kw) || clean_kw.contains(&clean_header) {
                return (Some(matcher.field_name.to_string()), 0.8);
            }
        }
    }

    (None, 0.0)
}

/// 智能探测表头所在行索引（0-indexed）
pub fn detect_header_row(rows: &[Vec<Data>]) -> usize {
    let mut best_row = 0;
    let mut max_score = 0.0f32;

    let scan_limit = rows.len().min(10);
    for (row_idx, row) in rows.iter().enumerate().take(scan_limit) {
        let non_empty_cells: Vec<&Data> = row.iter().filter(|c| !c.is_empty()).collect();
        if non_empty_cells.is_empty() {
            continue;
        }

        let mut row_score = 0.0f32;
        for cell in &non_empty_cells {
            let text = cell_to_string(cell);
            let (_, conf) = match_field_confidence(&text);
            row_score += conf;
        }

        let density_bonus = (non_empty_cells.len() as f32) / (row.len().max(1) as f32);
        let total_score = row_score * density_bonus;

        if total_score > max_score {
            max_score = total_score;
            best_row = row_idx;
        }
    }

    best_row
}

// ═══════════════════════════════════════════════════════════════════
// Tauri IPC 命令实现
// ═══════════════════════════════════════════════════════════════════

/// 获取 Excel 文件包含的工作表列表
#[tauri::command]
pub async fn excel_get_sheets(file_path: String) -> Result<Vec<SheetInfo>, String> {
    run_blocking(move || {
        let path = Path::new(&file_path);
        if !path.exists() {
            return Err(anyhow!("Excel 文件不存在: {}", file_path));
        }

        let mut workbook =
            open_workbook_auto(path).map_err(|e| anyhow!("打开 Excel 工作簿失败: {}", e))?;

        let sheet_names = workbook.sheet_names().to_vec();
        let mut list = Vec::new();

        for name in sheet_names {
            if let Ok(range) = workbook.worksheet_range(&name) {
                list.push(SheetInfo {
                    name,
                    row_count: range.height(),
                    column_count: range.width(),
                });
            }
        }

        Ok(list)
    })
    .await
}

/// 探测工作表结构、表头与预览数据
#[tauri::command]
pub async fn excel_inspect_sheet(
    file_path: String,
    sheet_name: String,
    header_row_override: Option<usize>,
) -> Result<ExcelInspectResult, String> {
    run_blocking(move || {
        let path = Path::new(&file_path);
        let mut workbook =
            open_workbook_auto(path).map_err(|e| anyhow!("打开 Excel 失败: {}", e))?;

        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|e| anyhow!("读取工作表 [{}] 失败: {}", sheet_name, e))?;

        let raw_rows: Vec<Vec<Data>> = range.rows().map(|r| r.to_vec()).collect();
        let total_rows = raw_rows.len();

        if total_rows == 0 {
            return Ok(ExcelInspectResult {
                sheet_name,
                total_rows: 0,
                detected_header_row: 0,
                columns: Vec::new(),
                preview_rows: Vec::new(),
            });
        }

        let header_row_idx = header_row_override.unwrap_or_else(|| detect_header_row(&raw_rows));
        let effective_headers = resolve_effective_headers(&raw_rows, header_row_idx);

        // 收集列推荐与样本
        let mut columns = Vec::new();
        let sample_limit = 5;

        for (col_idx, header_text) in effective_headers.into_iter().enumerate() {
            let (suggested_field, confidence) = match_field_confidence(&header_text);

            let mut sample_values = Vec::new();
            for r in raw_rows.iter().skip(header_row_idx + 1).take(sample_limit) {
                if let Some(c) = r.get(col_idx) {
                    let s = cell_to_string(c);
                    if !s.is_empty() && !sample_values.contains(&s) {
                        sample_values.push(s);
                    }
                }
            }

            columns.push(ColumnMappingRecommendation {
                column_index: col_idx,
                excel_header: if header_text.is_empty() {
                    format!("第 {} 列", col_idx + 1)
                } else {
                    header_text
                },
                sample_values,
                suggested_field,
                confidence,
            });
        }

        // 生成前 10 行预览数据
        let mut preview_rows = Vec::new();
        for r in raw_rows.iter().skip(header_row_idx + 1).take(10) {
            let mut row_map = HashMap::new();
            let mut has_data = false;
            for (col_idx, cell) in r.iter().enumerate() {
                let col_name = columns
                    .get(col_idx)
                    .map(|c| c.excel_header.clone())
                    .unwrap_or_else(|| format!("col_{}", col_idx));
                let val_str = cell_to_string(cell);
                if !val_str.is_empty() {
                    has_data = true;
                }
                row_map.insert(col_name, serde_json::Value::String(val_str));
            }
            if has_data {
                preview_rows.push(row_map);
            }
        }

        Ok(ExcelInspectResult {
            sheet_name,
            total_rows,
            detected_header_row: header_row_idx,
            columns,
            preview_rows,
        })
    })
    .await
}

/// 执行案件批量导入
#[tauri::command]
pub async fn excel_import_cases(
    file_path: String,
    sheet_name: String,
    config: CaseImportConfig,
) -> Result<CaseImportReport, String> {
    run_blocking(move || {
        let path = Path::new(&file_path);
        let mut workbook = open_workbook_auto(path)
            .map_err(|e| anyhow!("打开 Excel 失败: {}", e))?;

        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|e| anyhow!("读取工作表 [{}] 失败: {}", sheet_name, e))?;

        let raw_rows: Vec<Vec<Data>> = range.rows().map(|r| r.to_vec()).collect();
        let total_rows = raw_rows.len();

        let mut report = CaseImportReport {
            total_rows_processed: 0,
            created_count: 0,
            updated_count: 0,
            skipped_count: 0,
            failed_count: 0,
            errors: Vec::new(),
            imported_case_ids: Vec::new(),
        };

        if config.header_row >= total_rows {
            return Ok(report);
        }

        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;

        let mut last_forward_fill_vals: HashMap<usize, String> = HashMap::new();
        let effective_headers = resolve_effective_headers(&raw_rows, config.header_row);

        for (rel_idx, row) in raw_rows.iter().enumerate().skip(config.header_row + 1) {
            let row_line = rel_idx + 1; // 1-indexed 供报错提示

            // 1. 抽取映射字段并处理 Forward-Fill 与多列 Notes/开庭 追加
            let mut extracted: HashMap<String, String> = HashMap::new();
            let mut notes_collected: Vec<String> = Vec::new();
            let mut raw_trial_dates_collected: Vec<String> = Vec::new();
            let mut row_is_empty = true;

            for (col_idx, field_name) in &config.column_mappings {
                let raw_val = row.get(*col_idx).map(cell_to_string).unwrap_or_default();
                let final_val = if raw_val.is_empty() {
                    if config.forward_fill_columns.contains(col_idx) {
                        last_forward_fill_vals.get(col_idx).cloned().unwrap_or_default()
                    } else {
                        String::new()
                    }
                } else {
                    if config.forward_fill_columns.contains(col_idx) {
                        last_forward_fill_vals.insert(*col_idx, raw_val.clone());
                    }
                    raw_val
                };

                if !final_val.is_empty() {
                    row_is_empty = false;
                }

                if field_name == "notes" {
                    if !final_val.is_empty() {
                        let header_name = effective_headers
                            .get(*col_idx)
                            .cloned()
                            .unwrap_or_else(|| "附注".to_string());
                        notes_collected.push(format!("[{}] {}", header_name, final_val));
                    }
                } else if field_name == "trialDate" || field_name == "trial2Date" || field_name == "trial3Date" {
                    if !final_val.is_empty() {
                        for d in extract_dates_list(&final_val) {
                            if !raw_trial_dates_collected.contains(&d) {
                                raw_trial_dates_collected.push(d);
                            }
                        }
                    }
                } else {
                    extracted.insert(field_name.clone(), final_val);
                }
            }

            // 跳过完全空白行
            if row_is_empty {
                continue;
            }

            report.total_rows_processed += 1;

            // 2. 核心字段校验与案名智能生成
            let case_name = extracted.get("caseName").map(|s| s.trim().to_string()).unwrap_or_default();
            let case_no = extracted.get("caseNo").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            let client_name = extracted.get("clientName").map(|s| s.trim().to_string()).unwrap_or_default();
            let opponent_name = extracted.get("opponentName").map(|s| s.trim().to_string()).unwrap_or_default();
            let cause_action = extracted.get("causeAction").map(|s| s.trim().to_string()).unwrap_or_default();
            let patent_name = extracted.get("patentName").map(|s| s.trim().to_string()).unwrap_or_default();

            // 如果没有案名、案号、专利名，且委托人/相对方为空，视为无效数据行或尾部附注
            if case_name.is_empty()
                && case_no.is_none()
                && patent_name.is_empty()
                && (client_name.is_empty() || opponent_name.is_empty())
            {
                report.skipped_count += 1;
                continue;
            }

            let final_case_name = if !case_name.is_empty() {
                case_name
            } else if !client_name.is_empty() && !opponent_name.is_empty() && !cause_action.is_empty() {
                let c_lead = client_name.lines().next().unwrap_or(&client_name).trim();
                let o_lead = opponent_name.lines().next().unwrap_or(&opponent_name).trim();
                format!("{}诉{}{}", c_lead, o_lead, cause_action)
            } else if !patent_name.is_empty() {
                let p_lead = patent_name.lines().next().unwrap_or(&patent_name).trim();
                if !cause_action.is_empty() {
                    format!("{}「{}」案", cause_action, p_lead)
                } else {
                    p_lead.to_string()
                }
            } else {
                case_no.clone().unwrap_or_else(|| format!("未命名案件-{}", row_line))
            };

            let final_client_name = if client_name.is_empty() {
                "待补充委托人".to_string()
            } else {
                client_name
            };

            // 过滤汇总统计行或表尾备注行
            let is_summary_row = |text: &str| {
                let t = text.trim();
                t.starts_with("合计")
                    || t.starts_with("总计")
                    || t.starts_with("共计")
                    || t.starts_with("统计")
                    || t.starts_with("汇总")
                    || t.starts_with("注：")
                    || t.starts_with("注:")
                    || t.starts_with("说明")
                    || t.starts_with("制表")
            };

            if is_summary_row(&final_case_name)
                || case_no.as_deref().map(is_summary_row).unwrap_or(false)
            {
                report.skipped_count += 1;
                continue;
            }

            // 3. 值清洗与多节点日期解析 (按顺序分配首期/二次/三次开庭，超出顺延至附注)
            let filing_date = extracted.get("filingDate").and_then(|s| clean_date_str(s));
            let trial_date = raw_trial_dates_collected.first().cloned();
            let trial2_date = raw_trial_dates_collected.get(1).cloned();
            let trial3_date = raw_trial_dates_collected.get(2).cloned();
            for (extra_idx, extra_d) in raw_trial_dates_collected.iter().skip(3).enumerate() {
                notes_collected.push(format!("[第 {} 次开庭] {}", extra_idx + 4, extra_d));
            }
            let verdict_date = extracted
                .get("verdictDate")
                .and_then(|s| clean_date_str(s))
                .or_else(|| {
                    extracted
                        .get("caseResult")
                        .and_then(|s| clean_date_str(s))
                });
            let completed_text = extracted.get("completedText").cloned();
            let stay_date = extracted.get("stayDate").and_then(|s| clean_date_str(s));
            let relief_deadline = extracted.get("reliefDeadline").and_then(|s| clean_date_str(s));
            let attorneys = extracted.get("attorneys").and_then(|s| clean_array_to_json(s));
            let final_notes = if !notes_collected.is_empty() {
                Some(notes_collected.join("\n"))
            } else {
                extracted.get("notes").cloned().filter(|s| !s.is_empty())
            };
            let (track, raw_track, case_route) = infer_track_and_route(
                config.default_track.as_deref(),
                extracted.get("track").map(|s| s.as_str()),
                None,
                &final_case_name,
                extracted.get("causeAction").map(|s| s.as_str()),
            );

            // 4. 查重处理 (Deduplication & Conflict Strategy)
            let existing_case_id: Option<String> = if let Some(no) = &case_no {
                tx.query_row(
                    "SELECT id FROM cases WHERE case_no = ?1 LIMIT 1",
                    rusqlite::params![no],
                    |r| r.get(0),
                )
                .ok()
            } else {
                tx.query_row(
                    "SELECT id FROM cases WHERE case_name = ?1 LIMIT 1",
                    rusqlite::params![final_case_name],
                    |r| r.get(0),
                )
                .ok()
            };

            match existing_case_id {
                Some(_exist_id) if config.conflict_strategy == "skip" => {
                    report.skipped_count += 1;
                    continue;
                }
                Some(exist_id) if config.conflict_strategy == "update" => {
                    // 更新已有案件
                    let update_res = tx.execute(
                        "UPDATE cases SET
                            case_name = COALESCE(NULLIF(?1, ''), case_name),
                            client_name = COALESCE(NULLIF(?2, ''), client_name),
                            court = COALESCE(NULLIF(?3, ''), court),
                            cause_action = COALESCE(NULLIF(?4, ''), cause_action),
                            filing_date = COALESCE(NULLIF(?5, ''), filing_date),
                            trial_date = COALESCE(NULLIF(?6, ''), trial_date),
                            trial2_date = COALESCE(NULLIF(?7, ''), trial2_date),
                            trial3_date = COALESCE(NULLIF(?8, ''), trial3_date),
                            verdict_date = COALESCE(NULLIF(?9, ''), verdict_date),
                            completed_text = COALESCE(NULLIF(?10, ''), completed_text),
                            attorneys = COALESCE(NULLIF(?11, ''), attorneys),
                            judge_panel = COALESCE(NULLIF(?12, ''), judge_panel),
                            clerk = COALESCE(NULLIF(?13, ''), clerk),
                            stay_date = COALESCE(NULLIF(?14, ''), stay_date),
                            notes = CASE
                                WHEN notes IS NULL OR notes = '' THEN ?15
                                WHEN ?15 IS NULL OR ?15 = '' THEN notes
                                WHEN notes = ?15 THEN notes
                                ELSE notes || char(10) || ?15
                            END,
                            updated_at = datetime('now', 'localtime')
                         WHERE id = ?16",
                        rusqlite::params![
                            final_case_name,
                            final_client_name,
                            extracted.get("court"),
                            extracted.get("causeAction"),
                            filing_date,
                            trial_date,
                            trial2_date,
                            trial3_date,
                            verdict_date,
                            completed_text,
                            attorneys,
                            extracted.get("judgePanel"),
                            extracted.get("clerk"),
                            stay_date,
                            final_notes,
                            exist_id
                        ],
                    );

                    match update_res {
                        Ok(_) => {
                            // 自动同步至 hearings 庭审分表 (支持无限次开庭)
                            for (h_idx, h_date) in raw_trial_dates_collected.iter().enumerate() {
                                let exists: bool = tx
                                    .query_row(
                                        "SELECT COUNT(*) > 0 FROM hearings WHERE case_id = ?1 AND hearing_date = ?2",
                                        rusqlite::params![&exist_id, h_date],
                                        |r| r.get(0),
                                    )
                                    .unwrap_or(false);

                                if !exists {
                                    let h_id = db::new_id();
                                    let h_name = format!("第 {} 次开庭/口审", h_idx + 1);
                                    let judges_json = extracted.get("judgePanel").and_then(|s| clean_array_to_json(s));
                                    let _ = tx.execute(
                                        "INSERT INTO hearings (
                                            id, case_id, hearing_record, hearing_name, hearing_date,
                                            court, case_level, judges, contact_info, actual_status, created_at
                                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '未开', datetime('now', 'localtime'))",
                                        rusqlite::params![
                                            h_id,
                                            &exist_id,
                                            final_case_name,
                                            h_name,
                                            h_date,
                                            extracted.get("court"),
                                            extracted.get("caseLevel"),
                                            judges_json,
                                            extracted.get("clerk"),
                                        ],
                                    );
                                }
                            }

                            report.updated_count += 1;
                            report.imported_case_ids.push(exist_id);
                        }
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 行更新失败: {}", row_line, e));
                        }
                    }
                }
                _ => {
                    // 新建案件 (包含 duplicate 策略或不存在已有案件)
                    let new_case_id = db::new_id();
                    let opponent_name = extracted.get("opponentName").cloned().unwrap_or_default();
                    let our_role = extracted.get("ourRole").cloned();
                    let opponent_role = extracted.get("opponentRole").cloned();
                    let case_level = extracted.get("caseLevel").cloned();
                    let case_progress = extracted.get("caseProgress").cloned();
                    let case_result = extracted.get("caseResult").cloned();
                    let cause_action = extracted.get("causeAction").cloned();
                    let court = extracted.get("court").cloned();
                    let judge_panel = extracted.get("judgePanel").cloned();
                    let clerk = extracted.get("clerk").cloned();
                    let internal_no = extracted.get("internalNo").cloned();
                    let patent_name = extracted.get("patentName").cloned();
                    let patent_app_no = extracted.get("patentAppNo").cloned();

                    let insert_res = tx.execute(
                        "INSERT INTO cases (
                            id, track, raw_track, case_name, case_no, internal_no, cause_action,
                            client_name, our_role, opponent_name, opponent_role, court, judge_panel, clerk, attorneys,
                            case_level, case_progress, case_result,
                            patent_name, patent_app_no,
                            filing_date, trial_date, trial2_date, trial3_date, verdict_date, completed_text,
                            stay_date, relief_deadline, case_route, notes, created_at, updated_at
                        ) VALUES (
                            ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                            ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                            ?16, ?17, ?18,
                            ?19, ?20,
                            ?21, ?22, ?23, ?24, ?25, ?26,
                            ?27, ?28, ?29, ?30, datetime('now', 'localtime'), datetime('now', 'localtime')
                        )",
                        rusqlite::params![
                            new_case_id,
                            track,
                            raw_track,
                            final_case_name,
                            case_no,
                            internal_no,
                            cause_action,
                            final_client_name,
                            our_role,
                            opponent_name,
                            opponent_role,
                            court,
                            judge_panel,
                            clerk,
                            attorneys,
                            case_level,
                            case_progress,
                            case_result,
                            patent_name,
                            patent_app_no,
                            filing_date,
                            trial_date,
                            trial2_date,
                            trial3_date,
                            verdict_date,
                            completed_text,
                            stay_date,
                            relief_deadline,
                            case_route,
                            final_notes,
                        ],
                    );

                    match insert_res {
                        Ok(_) => {
                            // 自动同步至 hearings 庭审分表 (支持无限次开庭)
                            for (h_idx, h_date) in raw_trial_dates_collected.iter().enumerate() {
                                let h_id = db::new_id();
                                let h_name = format!("第 {} 次开庭/口审", h_idx + 1);
                                let judges_json = extracted.get("judgePanel").and_then(|s| clean_array_to_json(s));
                                let _ = tx.execute(
                                    "INSERT INTO hearings (
                                        id, case_id, hearing_record, hearing_name, hearing_date,
                                        court, case_level, judges, contact_info, actual_status, created_at
                                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '未开', datetime('now', 'localtime'))",
                                    rusqlite::params![
                                        h_id,
                                        &new_case_id,
                                        final_case_name,
                                        h_name,
                                        h_date,
                                        extracted.get("court"),
                                        extracted.get("caseLevel"),
                                        judges_json,
                                        extracted.get("clerk"),
                                    ],
                                );
                            }

                            // 自动创建对应客户记录 (如果不存在)
                            if !final_client_name.is_empty() && final_client_name != "待补充委托人" {
                                let _ = tx.execute(
                                    "INSERT OR IGNORE INTO clients (id, name) VALUES (?1, ?2)",
                                    rusqlite::params![db::new_id(), final_client_name],
                                );
                            }
                            report.created_count += 1;
                            report.imported_case_ids.push(new_case_id);
                        }
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 行插入失败: {}", row_line, e));
                        }
                    }
                }
            }
        }

        tx.commit()?;

        Ok(report)
    })
    .await
}

/// Excel 关联分表批量导入 (任务分表 / 庭审分表 / 办案日志分表)
#[tauri::command]
pub async fn excel_import_subtable(
    file_path: String,
    sheet_name: String,
    config: SubtableImportConfig,
) -> Result<SubtableImportReport, String> {
    run_blocking(move || {
        let path = Path::new(&file_path);
        let mut workbook = open_workbook_auto(path)
            .map_err(|e| anyhow!("打开 Excel 工作簿失败: {}", e))?;

        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|e| anyhow!("读取工作表 [{}] 失败: {}", sheet_name, e))?;

        let raw_rows: Vec<Vec<Data>> = range.rows().map(|r| r.to_vec()).collect();
        let total_rows = raw_rows.len();

        let mut report = SubtableImportReport {
            target_entity: config.target_entity.clone(),
            total_rows_processed: 0,
            created_count: 0,
            linked_cases_count: 0,
            unlinked_count: 0,
            failed_count: 0,
            errors: Vec::new(),
        };

        if config.header_row >= total_rows {
            return Ok(report);
        }

        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;

        let mut last_forward_fill_vals: HashMap<usize, String> = HashMap::new();

        for (rel_idx, row) in raw_rows.iter().enumerate().skip(config.header_row + 1) {
            let row_line = rel_idx + 1;
            let mut extracted: HashMap<String, String> = HashMap::new();
            let mut row_is_empty = true;

            for (col_idx, field_name) in &config.column_mappings {
                let raw_val = row.get(*col_idx).map(cell_to_string).unwrap_or_default();
                let final_val = if raw_val.is_empty() {
                    if config.forward_fill_columns.contains(col_idx) {
                        last_forward_fill_vals.get(col_idx).cloned().unwrap_or_default()
                    } else {
                        String::new()
                    }
                } else {
                    if config.forward_fill_columns.contains(col_idx) {
                        last_forward_fill_vals.insert(*col_idx, raw_val.clone());
                    }
                    raw_val
                };

                if !final_val.is_empty() {
                    row_is_empty = false;
                }
                extracted.insert(field_name.clone(), final_val);
            }

            if row_is_empty {
                continue;
            }

            report.total_rows_processed += 1;

            // 解析关联案件 (通过 caseNo 或 caseName)
            let case_no = extracted.get("caseNo").map(|s| s.trim()).filter(|s| !s.is_empty());
            let case_name = extracted.get("caseName").map(|s| s.trim()).filter(|s| !s.is_empty());

            let matched_case_id: Option<String> = if let Some(no) = case_no {
                tx.query_row("SELECT id FROM cases WHERE case_no = ?1 LIMIT 1", rusqlite::params![no], |r| r.get(0)).ok()
            } else if let Some(name) = case_name {
                tx.query_row("SELECT id FROM cases WHERE case_name = ?1 LIMIT 1", rusqlite::params![name], |r| r.get(0)).ok()
            } else {
                None
            };

            if matched_case_id.is_some() {
                report.linked_cases_count += 1;
            } else {
                report.unlinked_count += 1;
            }

            match config.target_entity.as_str() {
                "tasks" => {
                    let task_name = extracted.get("taskName")
                        .or_else(|| extracted.get("name"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    if task_name.is_empty() {
                        continue;
                    }

                    let t_id = db::new_id();
                    let deadline = extracted.get("deadline").and_then(|s| clean_date_str(s));
                    let description = extracted.get("description").cloned();
                    let clean_priority = match extracted.get("priority").map(|s| s.trim()) {
                        Some("高") | Some("紧急") | Some("high") | Some("urgent_important") => "urgent_important",
                        Some("重要") | Some("important") => "important",
                        Some("次要") | Some("低") | Some("low") => "normal",
                        _ => "normal",
                    };
                    let created_date = chrono::Local::now().format("%Y-%m-%d").to_string();
                    let assignee = extracted.get("assignee").cloned();
                    let completed = match extracted.get("completed").map(|s| s.as_str()) {
                        Some("已完成") | Some("1") | Some("true") | Some("是") | Some("完成") => 1,
                        _ => 0,
                    };

                    let res = tx.execute(
                        "INSERT INTO tasks (
                            id, case_id, task_name, description, created_date, deadline, due_date, priority, completed, assignee, created_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8, ?9, datetime('now', 'localtime'))",
                        rusqlite::params![
                            t_id,
                            matched_case_id,
                            task_name,
                            description,
                            created_date,
                            deadline,
                            clean_priority,
                            completed,
                            assignee
                        ],
                    );

                    match res {
                        Ok(_) => report.created_count += 1,
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 行任务插入失败: {}", row_line, e));
                        }
                    }
                }
                "hearings" => {
                    let hearing_date = extracted.get("hearingDate")
                        .or_else(|| extracted.get("trialDate"))
                        .and_then(|s| clean_date_str(s));

                    if hearing_date.is_none() {
                        continue;
                    }

                    let h_id = db::new_id();
                    let hearing_name = extracted.get("hearingName").cloned().unwrap_or_else(|| "开庭/口审".to_string());
                    let court = extracted.get("court").cloned();
                    let case_level = extracted.get("caseLevel").cloned();
                    let judges_json = extracted.get("judgePanel").or_else(|| extracted.get("judges")).and_then(|s| clean_array_to_json(s));
                    let clerk = extracted.get("clerk").or_else(|| extracted.get("contactInfo")).cloned();
                    let actual_status = extracted.get("actualStatus").cloned();

                    let res = tx.execute(
                        "INSERT INTO hearings (
                            id, case_id, hearing_record, hearing_name, hearing_date,
                            court, case_level, judges, contact_info, actual_status, created_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now', 'localtime'))",
                        rusqlite::params![
                            h_id,
                            matched_case_id.unwrap_or_default(),
                            case_name.unwrap_or_default(),
                            hearing_name,
                            hearing_date,
                            court,
                            case_level,
                            judges_json,
                            clerk,
                            actual_status,
                        ],
                    );

                    match res {
                        Ok(_) => report.created_count += 1,
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 行开庭插入失败: {}", row_line, e));
                        }
                    }
                }
                "case_logs" => {
                    let content = extracted.get("content")
                        .or_else(|| extracted.get("notes"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    if content.is_empty() {
                        continue;
                    }

                    let l_id = db::new_id();
                    let event_date = extracted.get("eventDate").and_then(|s| clean_date_str(s)).unwrap_or_else(|| "2026-08-31".to_string());
                    let event_type = extracted.get("eventType").cloned().unwrap_or_else(|| "办案日志".to_string());
                    let operator = extracted.get("operator").cloned();

                    let res = tx.execute(
                        "INSERT INTO case_logs (
                            id, case_id, event_date, event_type, content, operator, created_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now', 'localtime'))",
                        rusqlite::params![
                            l_id,
                            matched_case_id.unwrap_or_default(),
                            event_date,
                            event_type,
                            content,
                            operator,
                        ],
                    );

                    match res {
                        Ok(_) => report.created_count += 1,
                        Err(e) => {
                            report.failed_count += 1;
                            report.errors.push(format!("第 {} 行日志插入失败: {}", row_line, e));
                        }
                    }
                }
                _ => {}
            }
        }

        tx.commit()?;
        Ok(report)
    })
    .await
}

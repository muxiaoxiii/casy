use super::run_blocking;
use crate::{db, parse};
use chrono::Datelike;

/// 收件箱列表项（B1 类型化：返回侧 Value → 强类型）
///
/// 空值口径：title/contentText/aiCategory/aiConfidence/sourcePath/userCategory/createdAt
/// 在 SQL 层 COALESCE 归一为 '' / 0（前端手写契约按非空建模；null 与 ''
/// 在展示层真值判断等价），其余可空字段保持 Option。
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemDto {
    pub id: String,
    pub source_type: String,
    pub source_path: String,
    pub source_url: Option<String>,
    pub source_time: Option<String>,
    pub title: String,
    pub content_text: String,
    pub ai_category: String,
    pub ai_confidence: f64,
    /// AI 抽取的原始 JSON（结构随文档类型变化）
    pub ai_extracted: Option<serde_json::Value>,
    pub ai_suggested_case_id: Option<String>,
    pub status: String,
    pub user_category: String,
    pub linked_case_id: Option<String>,
    pub created_at: String,
    pub processed_at: Option<String>,
}

/// 批处理进度
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InboxProgress {
    pub total: i64,
    pub processed: i64,
    pub pending: i64,
}

/// AI 处理结果（分类 + 置信度 + 抽取 + 自动路由动作）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedInboxResult {
    pub category: String,
    pub confidence: f64,
    pub suggested_case_id: Option<String>,
    pub case_no: Option<String>,
    pub extracted: Option<serde_json::Value>,
    pub route_actions: Vec<serde_json::Value>,
}

/// 节假日解析结果（B1 类型化）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HolidayNotice {
    pub year: i32,
    pub holidays: Vec<String>,
    pub workdays: Vec<String>,
}

#[tauri::command]
pub async fn add_inbox_item(
    source_type: String,
    title: Option<String>,
    content_text: Option<String>,
    source_path: Option<String>,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let text = content_text.clone().unwrap_or_default();

        let norm_source_type = match source_type.to_lowercase().as_str() {
            "text" | "note" | "manual" | "voice" => "note",
            "paste" | "clipboard" => "paste",
            "file" | "attachment" => "file",
            "email" | "mail" => "email",
            "sms" | "message" => "sms",
            "imap" => "imap",
            _ => "note",
        };

        // 尝试 AI 分类（使用 prompt 增强）
        let ai_config = crate::ai::load_ai_config();
        let (category, confidence, ai_extracted, suggested_case_id) = if ai_config.mode != "noop" {
            match tauri::async_runtime::block_on(crate::ai::process_inbox_with_ai(&text)) {
                Ok((result, routing)) => {
                    let suggested_id = match &routing {
                        crate::ai::RoutingDecision::AutoLinked { case_id, .. } => {
                            Some(case_id.clone())
                        }
                        _ => None,
                    };
                    let extracted = result
                        .extracted_info
                        .as_ref()
                        .map(|v| serde_json::to_string(v).unwrap_or_default());
                    (result.category, result.confidence, extracted, suggested_id)
                }
                Err(e) => {
                    log::warn!("AI 分类失败，回退到规则匹配: {}", e);
                    let parsed = parse::classify_document(&text);
                    let extracted = serde_json::to_string(&parsed).ok();
                    (parsed.doc_type, parsed.confidence, extracted, None)
                }
            }
        } else {
            let parsed = parse::classify_document(&text);
            let extracted = serde_json::to_string(&parsed).ok();
            (parsed.doc_type, parsed.confidence, extracted, None)
        };

        conn.execute(
            "INSERT INTO inbox_items (id, source_type, title, content_text, source_path,
             ai_category, ai_confidence, ai_extracted, ai_suggested_case_id, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', ?10)",
            rusqlite::params![
                id,
                norm_source_type,
                title.unwrap_or_else(|| category.clone()),
                text,
                source_path,
                category,
                confidence,
                ai_extracted,
                suggested_case_id,
                db::now_local(),
            ],
        )?;

        Ok(id)
    })
    .await
}

#[tauri::command]
pub async fn list_inbox_items(status: Option<String>) -> Result<Vec<InboxItemDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        // COALESCE 归一口径见 InboxItemDto 文档注释
        let mut sql = String::from(
            "SELECT id, source_type, COALESCE(source_path,'') AS source_path, \
             source_url, source_time, COALESCE(title,'') AS title, \
             COALESCE(content_text,'') AS content_text, COALESCE(ai_category,'') AS ai_category, \
             COALESCE(ai_confidence,0) AS ai_confidence, ai_extracted, ai_suggested_case_id, \
             status, COALESCE(user_category,'') AS user_category, linked_case_id, \
             COALESCE(created_at,'') AS created_at, processed_at \
             FROM inbox_items WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = &status {
            if !s.is_empty() {
                sql.push_str(" AND status = ?1");
                params.push(Box::new(s.clone()));
            }
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT 100");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let items: Vec<InboxItemDto> = stmt
            .query_map(param_refs.as_slice(), |row| {
                let ai_extracted: Option<String> = row.get("ai_extracted")?;
                let parsed_extracted: Option<serde_json::Value> =
                    ai_extracted.and_then(|s| serde_json::from_str(&s).ok());

                Ok(InboxItemDto {
                    id: row.get::<_, String>("id")?,
                    source_type: row.get::<_, String>("source_type")?,
                    source_path: row.get::<_, String>("source_path")?,
                    source_url: row.get::<_, Option<String>>("source_url")?,
                    source_time: row.get::<_, Option<String>>("source_time")?,
                    title: row.get::<_, String>("title")?,
                    content_text: row.get::<_, String>("content_text")?,
                    ai_category: row.get::<_, String>("ai_category")?,
                    ai_confidence: row.get::<_, f64>("ai_confidence")?,
                    ai_extracted: parsed_extracted,
                    ai_suggested_case_id: row.get::<_, Option<String>>("ai_suggested_case_id")?,
                    status: row.get::<_, String>("status")?,
                    user_category: row.get::<_, String>("user_category")?,
                    linked_case_id: row.get::<_, Option<String>>("linked_case_id")?,
                    created_at: row.get::<_, String>("created_at")?,
                    processed_at: row.get::<_, Option<String>>("processed_at")?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(items)
    })
    .await
}

#[tauri::command]
pub async fn process_inbox_item(id: String) -> Result<ProcessedInboxResult, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 获取收件项
        let content_text: String = conn
            .query_row(
                "SELECT content_text FROM inbox_items WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .map_err(|e| {
                anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::INBOX_NOT_FOUND,
                    format!("收件项不存在: {id} ({e})"),
                ))
            })?;

        // 尝试 AI 分类（使用 prompt 增强）
        let ai_config = crate::ai::load_ai_config();
        let (category, confidence, extracted, routing) = if ai_config.mode != "noop" {
            match tauri::async_runtime::block_on(crate::ai::process_inbox_with_ai(&content_text)) {
                Ok((result, routing)) => {
                    let extracted = result.extracted_info.clone();
                    (result.category, result.confidence, extracted, Some(routing))
                }
                Err(e) => {
                    log::warn!("AI 分类失败，回退到规则匹配: {}", e);
                    let parsed = parse::classify_document(&content_text);
                    let extracted = serde_json::to_value(&parsed).ok();
                    (parsed.doc_type, parsed.confidence, extracted, None)
                }
            }
        } else {
            let parsed = parse::classify_document(&content_text);
            let extracted = serde_json::to_value(&parsed).ok();
            (parsed.doc_type, parsed.confidence, extracted, None)
        };

        // 从提取的信息中获取案号
        let case_no = extracted
            .as_ref()
            .and_then(|e| e.get("case_no").and_then(|v| v.as_str()))
            .map(|s| s.to_string());

        // 根据路由决策确定 suggested_case_id
        let suggested_case_id = match routing {
            Some(crate::ai::RoutingDecision::AutoLinked { case_id, .. }) => Some(case_id),
            _ => {
                // 回退到原有匹配逻辑
                if let Some(ref cn) = case_no {
                    conn.query_row(
                        "SELECT id FROM cases WHERE case_no LIKE ?1 LIMIT 1",
                        rusqlite::params![format!("%{}%", cn)],
                        |r| r.get::<_, String>(0),
                    )
                    .ok()
                } else {
                    let party_name = extracted
                        .as_ref()
                        .and_then(|e| {
                            e.get("parties")
                                .and_then(|p| p.as_array())
                                .and_then(|arr| arr.first())
                                .and_then(|p| p.get("name"))
                                .and_then(|v| v.as_str())
                        })
                        .map(|s| s.to_string());

                    if let Some(name) = party_name {
                        conn.query_row(
                            "SELECT id FROM cases WHERE client_name LIKE ?1 OR opponent_name LIKE ?1 LIMIT 1",
                            rusqlite::params![format!("%{}%", name)],
                            |r| r.get::<_, String>(0),
                        )
                        .ok()
                    } else {
                        None
                    }
                }
            }
        };

        // 更新收件项
        conn.execute(
            "UPDATE inbox_items SET ai_category = ?1, ai_confidence = ?2,
             ai_extracted = ?3, ai_suggested_case_id = ?4, status = 'pending', processed_at = ?5
             WHERE id = ?6",
            rusqlite::params![
                category,
                confidence,
                extracted.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default()),
                suggested_case_id,
                db::now_local(),
                id,
            ],
        )?;

        // ── 自动路由：根据分类执行后续动作 ──────────────────────
        let route_actions = execute_auto_routes(
            &conn,
            &id,
            &category,
            confidence,
            extracted.as_ref(),
            suggested_case_id.as_deref(),
            &content_text,
        );
        if let Err(e) = &route_actions {
            log::warn!("自动路由部分失败: {}", e);
        }

        Ok(ProcessedInboxResult {
            category,
            confidence,
            suggested_case_id,
            case_no,
            extracted,
            route_actions: route_actions.unwrap_or_default(),
        })
    })
    .await
}

/// 根据分类结果执行自动路由动作
fn execute_auto_routes(
    conn: &rusqlite::Connection,
    inbox_id: &str,
    category: &str,
    confidence: f64,
    extracted: Option<&serde_json::Value>,
    suggested_case_id: Option<&str>,
    content_text: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut actions = Vec::new();

    // 低置信度不自动路由
    if confidence < 0.5 {
        actions.push(serde_json::json!({"action": "skip", "reason": "置信度过低，等待人工确认"}));
        return Ok(actions);
    }

    match category {
        // ── 法条/法规 → 自动入库知识库 ──
        "legal_provision" => match auto_import_legal_provisions(conn, content_text, inbox_id) {
            Ok(count) => {
                actions.push(serde_json::json!({
                    "action": "knowledge_import",
                    "type": "legal_provision",
                    "count": count,
                    "message": format!("已自动导入 {} 条法条到知识库", count)
                }));
            }
            Err(e) => {
                actions.push(serde_json::json!({
                    "action": "knowledge_import_failed",
                    "error": e
                }));
            }
        },

        // ── 节假日通知 → 解析并提示确认 ──
        "holiday_notice" => match parse_holiday_dates(content_text) {
            Ok(parsed) => {
                actions.push(serde_json::json!({
                    "action": "holiday_parsed",
                    "data": parsed,
                    "message": "已解析节假日数据，请确认后更新日历"
                }));
            }
            Err(e) => {
                actions.push(serde_json::json!({
                    "action": "holiday_parse_failed",
                    "error": e
                }));
            }
        },

        // ── 传票/口审通知 → 自动创建准备任务 ──
        "summons" | "hearing_notice" => {
            if let Some(case_id) = suggested_case_id {
                let hearing_date = extract_date_from_extracted(extracted, "hearing_date")
                    .or_else(|| extract_date_from_extracted(extracted, "trial_date"));
                match auto_create_hearing_tasks(conn, case_id, &hearing_date, category) {
                    Ok(task_count) => {
                        actions.push(serde_json::json!({
                            "action": "tasks_created",
                            "type": "hearing_prep",
                            "count": task_count,
                            "message": format!("已自动创建 {} 个庭审准备任务", task_count)
                        }));
                    }
                    Err(e) => {
                        actions.push(serde_json::json!({
                            "action": "task_creation_failed",
                            "error": e
                        }));
                    }
                }
            } else {
                actions.push(serde_json::json!({
                    "action": "needs_manual_case_link",
                    "message": "未能自动匹配案件，请手动关联后将自动创建任务"
                }));
            }
        }

        // ── 判决/裁定 → 更新案件结果 + 触发期限重算 ──
        "judgment" => {
            if let Some(case_id) = suggested_case_id {
                match auto_update_case_from_judgment(conn, case_id, extracted) {
                    Ok(update_fields) => {
                        actions.push(serde_json::json!({
                            "action": "case_updated",
                            "type": "judgment",
                            "fields": update_fields,
                            "message": "已更新案件判决信息，期限已自动重算"
                        }));
                    }
                    Err(e) => {
                        actions.push(serde_json::json!({
                            "action": "case_update_failed",
                            "error": e
                        }));
                    }
                }
            }
        }

        // ── 起诉状 → 更新案件起诉信息 + 触发答辩期限 ──
        "complaint" => {
            if let Some(case_id) = suggested_case_id {
                match auto_update_case_from_complaint(conn, case_id, extracted) {
                    Ok(update_fields) => {
                        actions.push(serde_json::json!({
                            "action": "case_updated",
                            "type": "complaint",
                            "fields": update_fields,
                            "message": "已更新起诉信息，答辩期限已触发计算"
                        }));
                    }
                    Err(e) => {
                        actions.push(serde_json::json!({
                            "action": "case_update_failed",
                            "error": e
                        }));
                    }
                }
            }
        }

        // ── 审查意见 → 更新专利期限 ──
        "examination_opinion" => {
            if let Some(case_id) = suggested_case_id {
                match auto_update_case_from_examination(conn, case_id, extracted) {
                    Ok(update_fields) => {
                        actions.push(serde_json::json!({
                            "action": "case_updated",
                            "type": "examination_opinion",
                            "fields": update_fields,
                            "message": "已更新审查意见信息"
                        }));
                    }
                    Err(e) => {
                        actions.push(serde_json::json!({
                            "action": "case_update_failed",
                            "error": e
                        }));
                    }
                }
            }
        }

        // ── 案由更新 → 更新案由数据库 ──
        "cause_action_update" => {
            actions.push(serde_json::json!({
                "action": "cause_action_update",
                "message": "检测到案由规定更新，请在知识库中查看解析结果"
            }));
            // 写入知识库
            let _ = insert_knowledge_item(
                conn,
                "案由规定更新",
                "cause_action",
                content_text,
                inbox_id,
                suggested_case_id,
            );
        }

        // ── 笔记/其他 → 写入知识库 ──
        "note" | "client_instruction" | "correspondence" => {
            match insert_knowledge_item(
                conn,
                &format!(
                    "收件箱笔记: {}",
                    &content_text[..content_text.len().min(50)]
                ),
                "case_note",
                content_text,
                inbox_id,
                suggested_case_id,
            ) {
                Ok(_) => {
                    actions.push(serde_json::json!({
                        "action": "knowledge_saved",
                        "type": "note",
                        "message": "已自动保存到知识库"
                    }));
                }
                Err(e) => {
                    actions.push(serde_json::json!({
                        "action": "knowledge_save_failed",
                        "error": e
                    }));
                }
            }
        }

        _ => {
            actions.push(serde_json::json!({
                "action": "waiting_for_review",
                "message": "待人工处理"
            }));
        }
    }

    Ok(actions)
}

// ── 辅助函数 ──────────────────────────────────────────────────────

/// 自动导入法条到知识库
fn auto_import_legal_provisions(
    conn: &rusqlite::Connection,
    content: &str,
    inbox_id: &str,
) -> Result<usize, String> {
    // 按"第X条"拆分
    let article_re = regex::Regex::new(r"第[一二三四五六七八九十百千\d]+条").unwrap();
    let mut articles = Vec::new();
    let mut current_start = 0;
    let mut current_article_no = String::new();

    for mat in article_re.find_iter(content) {
        if !current_article_no.is_empty() {
            let text = content[current_start..mat.start()].trim();
            if !text.is_empty() {
                articles.push((current_article_no.clone(), text.to_string()));
            }
        }
        current_article_no = mat.as_str().to_string();
        current_start = mat.end();
    }
    // 最后一条
    if !current_article_no.is_empty() {
        let text = content[current_start..].trim();
        if !text.is_empty() {
            articles.push((current_article_no, text.to_string()));
        }
    }

    // 提取法律名称（取第一个"第X条"之前的文本）
    let first_article_pos = article_re.find(content).map(|m| m.start());
    let law_name = first_article_pos
        .map(|pos| content[..pos].trim())
        .unwrap_or("未知法律")
        .lines()
        .last()
        .unwrap_or("未知法律")
        .trim()
        .to_string();

    let mut count = 0;
    for (article_no, article_text) in &articles {
        let title = format!("{}{}", law_name, article_no);
        let id = db::new_id();
        let now = db::now_local();
        let tags = serde_json::to_string(&serde_json::json!([law_name, article_no, "法条"]))
            .unwrap_or_default();

        if conn
            .execute(
                "INSERT OR IGNORE INTO knowledge_items
                 (id, title, category, content, tags, source_type, source_id, law_name, article_no, status, created_at, updated_at)
                 VALUES (?1, ?2, 'legal_provision', ?3, ?4, 'inbox', ?5, ?6, ?7, 'current', ?8, ?8)",
                rusqlite::params![
                    id, title, article_text, tags, inbox_id, law_name, article_no, now,
                ],
            )
            .is_ok()
        {
            count += 1;
        }
    }

    if articles.is_empty() {
        // 没有按条拆分成功，整体存为一条
        let id = db::new_id();
        let now = db::now_local();
        let _ = conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content, source_type, source_id, status, created_at, updated_at)
             VALUES (?1, ?2, 'legal_provision', ?3, 'inbox', ?4, 'current', ?5, ?5)",
            rusqlite::params![id, law_name, content, inbox_id, now],
        );
        count = 1;
    }

    Ok(count)
}

/// 解析节假日日期（B1 类型化）
fn parse_holiday_dates(content: &str) -> Result<HolidayNotice, String> {
    use chrono::{Duration, NaiveDate};
    use std::collections::BTreeSet;

    let year_re = regex::Regex::new(r"(\d{4})\s*年").unwrap();
    let year = year_re
        .captures(content)
        .and_then(|c| c[1].parse::<i32>().ok())
        .unwrap_or(chrono::Local::now().year());

    let range_re = regex::Regex::new(
        r"(\d{1,2})\s*月\s*(\d{1,2})\s*日(?:（[^）]*）|\([^)]*\))?\s*(?:至|到|[-—~])\s*(?:(\d{1,2})\s*月\s*)?(\d{1,2})\s*日",
    )
    .unwrap();
    let date_re = regex::Regex::new(r"(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap();
    let mut holidays = BTreeSet::new();
    let mut workdays = BTreeSet::new();

    for segment in content.split(['。', '；', ';', '\n']) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let is_workday_segment = segment.contains("上班") || segment.contains("补班");
        let is_holiday_segment = segment.contains("放假")
            || segment.contains("休假")
            || segment.contains("节假日")
            || segment.contains("调休");
        if !is_workday_segment && !is_holiday_segment {
            continue;
        }

        let mut parsed_dates = BTreeSet::new();
        let mut range_spans = Vec::new();
        for captures in range_re.captures_iter(segment) {
            let whole = captures.get(0).unwrap();
            range_spans.push(whole.start()..whole.end());
            let start_month = captures[1].parse::<u32>().map_err(|_| "无效月份")?;
            let start_day = captures[2].parse::<u32>().map_err(|_| "无效日期")?;
            let end_month = captures
                .get(3)
                .and_then(|value| value.as_str().parse::<u32>().ok())
                .unwrap_or(start_month);
            let end_day = captures[4].parse::<u32>().map_err(|_| "无效日期")?;
            let mut date = NaiveDate::from_ymd_opt(year, start_month, start_day)
                .ok_or_else(|| "节假日通知包含无效起始日期".to_string())?;
            let end = NaiveDate::from_ymd_opt(year, end_month, end_day)
                .ok_or_else(|| "节假日通知包含无效结束日期".to_string())?;
            if end < date || (end - date).num_days() > 31 {
                return Err("节假日日期范围异常，请检查原文".to_string());
            }
            while date <= end {
                parsed_dates.insert(date);
                date += Duration::days(1);
            }
        }
        for captures in date_re.captures_iter(segment) {
            let whole = captures.get(0).unwrap();
            if range_spans
                .iter()
                .any(|span| whole.start() >= span.start && whole.end() <= span.end)
            {
                continue;
            }
            let month = captures[1].parse::<u32>().map_err(|_| "无效月份")?;
            let day = captures[2].parse::<u32>().map_err(|_| "无效日期")?;
            let date = NaiveDate::from_ymd_opt(year, month, day)
                .ok_or_else(|| "节假日通知包含无效日期".to_string())?;
            parsed_dates.insert(date);
        }

        let target = if is_workday_segment {
            &mut workdays
        } else {
            &mut holidays
        };
        target.extend(parsed_dates);
    }

    for workday in &workdays {
        holidays.remove(workday);
    }
    if holidays.is_empty() && workdays.is_empty() {
        return Err("未从通知中识别到放假或调休上班日期".to_string());
    }

    Ok(HolidayNotice {
        year,
        holidays: holidays.into_iter().map(|date| date.to_string()).collect(),
        workdays: workdays.into_iter().map(|date| date.to_string()).collect(),
    })
}

/// 从 extracted 中提取日期字段
fn extract_date_from_extracted(
    extracted: Option<&serde_json::Value>,
    field: &str,
) -> Option<String> {
    extracted
        .and_then(|e| e.get(field))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            extracted
                .and_then(|e| e.get("dates"))
                .and_then(|d| d.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .find(|d| {
                            d.get("description")
                                .and_then(|desc| desc.as_str())
                                .map(|s| {
                                    s.contains(field) || s.contains("开庭") || s.contains("口审")
                                })
                                .unwrap_or(false)
                        })
                        .and_then(|d| d.get("date"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
        })
}

/// 自动创建庭审准备任务
fn auto_create_hearing_tasks(
    conn: &rusqlite::Connection,
    case_id: &str,
    hearing_date: &Option<String>,
    doc_type: &str,
) -> Result<usize, String> {
    let task_templates: Vec<(&str, &str, i64)> = match doc_type {
        "summons" => vec![
            ("准备证据材料", "整理并提交证据清单", 7),
            ("准备代理词", "撰写代理词/答辩意见", 5),
            ("确认出庭人员", "确认出庭律师和当事人", 3),
            ("检查材料完整性", "核对全部提交材料", 2),
            ("准备庭审提纲", "准备庭审发言提纲", 1),
        ],
        "hearing_notice" => vec![
            ("准备无效宣告意见", "整理无效宣告理由和证据", 7),
            ("准备口审提纲", "准备口头审理发言提纲", 5),
            ("确认出庭人员", "确认合议组口审出庭安排", 3),
            ("准备技术比对", "整理权利要求技术比对表", 2),
        ],
        _ => vec![],
    };

    let today = chrono::Local::now().naive_local().date();
    let mut count = 0;

    for (title, desc, days_before) in &task_templates {
        let deadline = hearing_date
            .as_ref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .map(|hd| hd - chrono::Duration::days(*days_before))
            .map(|d| d.format("%Y-%m-%d").to_string());

        let task_id = db::new_id();
        let _ = conn.execute(
            "INSERT INTO tasks (id, case_id, title, description, priority, deadline, completed, created_at)
             VALUES (?1, ?2, ?3, ?4, 'important', ?5, 0, ?6)",
            rusqlite::params![
                task_id, case_id, title, desc, deadline, today.format("%Y-%m-%d").to_string(),
            ],
        );
        count += 1;
    }

    Ok(count)
}

/// 从判决信息更新案件
fn auto_update_case_from_judgment(
    conn: &rusqlite::Connection,
    case_id: &str,
    extracted: Option<&serde_json::Value>,
) -> Result<Vec<String>, String> {
    let mut updated = Vec::new();

    // 更新判决日期
    if let Some(date) = extract_date_from_extracted(extracted, "verdict_date")
        .or_else(|| extract_date_from_extracted(extracted, "判决日期"))
    {
        conn.execute(
            "UPDATE cases SET verdict_date = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![date, db::now_local(), case_id],
        )
        .map_err(|e| e.to_string())?;
        updated.push("verdict_date".to_string());
    }

    // 更新判决类型
    if let Some(extracted) = extracted {
        if let Some(result) = extracted.get("result").and_then(|v| v.as_str()) {
            let verdict_type = if result.contains("判决") {
                "判决"
            } else if result.contains("裁定") {
                "裁定"
            } else if result.contains("决定") {
                "决定"
            } else {
                ""
            };
            if !verdict_type.is_empty() {
                conn.execute(
                    "UPDATE cases SET verdict_type = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![verdict_type, db::now_local(), case_id],
                )
                .map_err(|e| e.to_string())?;
                updated.push("verdict_type".to_string());
            }
        }

        // 更新案件结果
        if let Some(result) = extracted.get("result").and_then(|v| v.as_str()) {
            conn.execute(
                "UPDATE cases SET case_result = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![result, db::now_local(), case_id],
            )
            .map_err(|e| e.to_string())?;
            updated.push("case_result".to_string());
        }
    }

    Ok(updated)
}

/// 从起诉状更新案件
fn auto_update_case_from_complaint(
    conn: &rusqlite::Connection,
    case_id: &str,
    extracted: Option<&serde_json::Value>,
) -> Result<Vec<String>, String> {
    let mut updated = Vec::new();
    let now = db::now_local();

    // 更新收到起诉状时间
    let today = chrono::Local::now()
        .naive_local()
        .date()
        .format("%Y-%m-%d")
        .to_string();
    conn.execute(
        "UPDATE cases SET complaint_received_date = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![today, now, case_id],
    )
    .map_err(|e| e.to_string())?;
    updated.push("complaint_received_date".to_string());

    // 更新法院
    if let Some(extracted) = extracted {
        if let Some(court) = extracted.get("court").and_then(|v| v.as_str()) {
            conn.execute(
                "UPDATE cases SET court = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![court, now, case_id],
            )
            .map_err(|e| e.to_string())?;
            updated.push("court".to_string());
        }
    }

    Ok(updated)
}

/// 从审查意见更新案件
fn auto_update_case_from_examination(
    conn: &rusqlite::Connection,
    case_id: &str,
    extracted: Option<&serde_json::Value>,
) -> Result<Vec<String>, String> {
    let mut updated = Vec::new();
    let now = db::now_local();

    if let Some(extracted) = extracted {
        // 更新答复期限
        if let Some(deadline) = extracted.get("deadline").and_then(|v| v.as_str()) {
            conn.execute(
                "UPDATE cases SET relief_deadline = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![deadline, now, case_id],
            )
            .map_err(|e| e.to_string())?;
            updated.push("relief_deadline".to_string());
        }
    }

    Ok(updated)
}

/// 插入知识条目
fn insert_knowledge_item(
    conn: &rusqlite::Connection,
    title: &str,
    category: &str,
    content: &str,
    source_id: &str,
    linked_case_id: Option<&str>,
) -> Result<String, String> {
    let id = db::new_id();
    let now = db::now_local();
    conn.execute(
        "INSERT INTO knowledge_items (id, title, category, content, source_type, source_id, linked_case_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'inbox', ?5, ?6, ?7, ?7)",
        rusqlite::params![id, title, category, content, source_id, linked_case_id, now],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn file_inbox_item(
    item_id: String,
    case_id: String,
    category: String,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 获取收件项信息
        let (title, source_path): (String, Option<String>) = conn
            .query_row(
                "SELECT COALESCE(title, ''), source_path FROM inbox_items WHERE id = ?1",
                rusqlite::params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| {
                anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::INBOX_NOT_FOUND,
                    format!("收件项不存在: {item_id} ({e})"),
                ))
            })?;

        // 获取案件信息
        let case = db::cases::get_case(&conn, &case_id).map_err(|e| {
            anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::CASE_NOT_FOUND,
                format!("案件不存在: {case_id} ({e})"),
            ))
        })?;

        // 如果有源文件，归档到案件目录
        let filed_path = if let Some(ref path_str) = source_path {
            let source = std::path::Path::new(path_str);
            if source.exists() {
                match crate::files::file_to_case(source, &case, &category) {
                    Ok(target) => Some(target.to_string_lossy().to_string()),
                    Err(e) => {
                        log::warn!("文件归档失败（不影响状态更新）: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // 更新收件项状态
        conn.execute(
            "UPDATE inbox_items SET status = 'filed', linked_case_id = ?1,
             filed_as = ?2, processed_at = ?3 WHERE id = ?4",
            rusqlite::params![case_id, category, db::now_local(), item_id],
        )?;

        // 记录办案日志
        let log_detail = match &filed_path {
            Some(path) => format!("归档收件: {} → {}", title, path),
            None => format!("归档收件: {}", title),
        };
        let _ = conn.execute(
            "INSERT INTO case_logs (id, case_id, event_summary, event_type, event_date, created_at)
             VALUES (?1, ?2, ?3, 'record', ?4, ?4)",
            rusqlite::params![db::new_id(), case_id, log_detail, db::today()],
        );

        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn dismiss_inbox_item(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let affected = conn.execute(
            "UPDATE inbox_items SET status = 'dismissed', processed_at = ?1 WHERE id = ?2",
            rusqlite::params![db::now_local(), id],
        )?;
        if affected == 0 {
            return Err(anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::INBOX_NOT_FOUND,
                format!("收件项不存在: {id}"),
            )));
        }
        Ok(())
    })
    .await
}

/// 解析节假日通知并更新日历（B1 类型化）
#[tauri::command]
pub async fn parse_holiday_notice(content: String) -> Result<HolidayNotice, String> {
    run_blocking(move || parse_holiday_dates(&content).map_err(|e| anyhow::anyhow!(e))).await
}

// ── v2.1: 即时判断 + 安全拷贝 + AI 缓存 ──────────────────────

/// 即时判断结果
#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuickJudgeResult {
    pub category: String,
    pub confidence: f32,
    pub strength: String, // "strong" / "candidate" / "fallback"
    pub recommendations: Vec<QuickRecommendation>,
    pub ai_available: bool,
    pub ai_analyzed: bool,
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuickRecommendation {
    /// 动作类型：file_to_case | create_task | create_deadline | create_event | save_knowledge | create_case | create_project | update_holidays | set_reminder
    pub action: String,
    pub target_case_id: Option<String>,
    pub target_case_name: Option<String>,
    pub target_folder: Option<String>,
    /// 动作参数（如 create_task 的 taskName/dueDate；file_to_case 为 null）
    pub intent: Option<serde_json::Value>,
    pub reason: String,
}

/// 送达文书检测信息
#[derive(Debug, Clone)]
struct ServiceDeliveryInfo {
    pub case_no: String,
    pub service_url: String,
    pub recipient_name: String,
}

/// 检测文本中的法院送达链接。只接受 court.gov.cn 子域，避免把任意网址送入下载器。
fn detect_service_delivery(text: &str) -> Option<ServiceDeliveryInfo> {
    let url_re = regex::Regex::new(r#"https?://[^\s<>\"']+"#).ok()?;
    let service_url = url_re
        .find_iter(text)
        .map(|m| m.as_str().trim_end_matches(['。', '，', ',', '.']))
        .find(|url| {
            reqwest::Url::parse(url)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_string))
                .is_some_and(|host| host == "court.gov.cn" || host.ends_with(".court.gov.cn"))
        })?
        .to_string();
    let case_no_re = regex::Regex::new(r"[（(]\s*\d{4}\s*[）)].{2,30}?号").ok()?;
    let case_no = case_no_re
        .find(text)
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    let recipient_re =
        regex::Regex::new(r"(?:受送达人|收件人)[：:]?\s*([\u{4e00}-\u{9fff}]{2,8})").ok()?;
    let recipient_name = recipient_re
        .captures(text)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
        .unwrap_or_default();

    Some(ServiceDeliveryInfo {
        case_no,
        service_url,
        recipient_name,
    })
}

/// 尝试下载法院送达链接中的直接文书响应。登录页、验证码页和其他 HTML 页面不会伪装成成功。
async fn download_service_delivery_url(
    inbox_item_id: &str,
    service_url: &str,
) -> Result<String, String> {
    let parsed =
        reqwest::Url::parse(service_url).map_err(|_| "法院送达链接格式无效".to_string())?;
    let host = parsed.host_str().unwrap_or_default();
    if host != "court.gov.cn" && !host.ends_with(".court.gov.cn") {
        return Err("仅允许下载 court.gov.cn 官方域名的送达链接".to_string());
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("无法初始化送达下载器: {e}"))?;
    let response = client
        .get(parsed)
        .header(reqwest::header::USER_AGENT, "Casy/0.1 court-delivery")
        .send()
        .await
        .map_err(|e| format!("法院送达链接访问失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("法院送达链接返回错误: {e}"))?;

    if response
        .content_length()
        .is_some_and(|size| size > 50 * 1024 * 1024)
    {
        return Err("送达文件超过 50MB 安全限制".to_string());
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if content_type.contains("text/html") {
        return Err("法院页面需要登录或验证码，已保留短信原文，请在收件箱中人工继续".to_string());
    }
    let extension = if content_type.contains("pdf") {
        "pdf"
    } else if content_type.contains("zip") {
        "zip"
    } else {
        "bin"
    };
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取送达文件失败: {e}"))?;
    if bytes.is_empty() {
        return Err("法院送达链接没有返回文件内容".to_string());
    }

    let directory = crate::runtime_paths::documents_root().join("inbox");
    std::fs::create_dir_all(&directory).map_err(|e| format!("无法创建收件目录: {e}"))?;
    let path = directory.join(format!("法院送达-{}.{}", inbox_item_id, extension));
    std::fs::write(&path, &bytes).map_err(|e| format!("无法保存送达文件: {e}"))?;

    let stored_path = path.to_string_lossy().to_string();
    let update_id = inbox_item_id.to_string();
    let update_path = stored_path.clone();
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "UPDATE inbox_items SET source_type = 'file', source_path = ?1, title = ?2, quick_category = NULL, quick_confidence = NULL WHERE id = ?3",
            rusqlite::params![update_path, path.file_name().and_then(|name| name.to_str()).unwrap_or("法院送达文件"), update_id],
        )?;
        Ok(())
    })
    .await?;
    Ok(stored_path)
}

/// 即时判断命令（纯本地，0ms）
#[tauri::command]
pub async fn quick_judge_inbox_item(id: String) -> Result<QuickJudgeResult, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 读取收件项（含内容文本，用于文本意图判断）
        let (title, source_path, content_text, ai_analyzed, ai_extracted): (Option<String>, Option<String>, String, i32, Option<String>) = conn
            .query_row(
                "SELECT title, source_path, COALESCE(content_text, ''), ai_analyzed, ai_extracted FROM inbox_items WHERE id = ?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .map_err(|e| {
                anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::INBOX_NOT_FOUND,
                    format!("收件项不存在: {id} ({e})"),
                ))
            })?;

        // 有源文件 → 文件归档意图；纯文本 → 文本意图判断（设计哲学 §10）
        let result = if source_path.is_some() {
            let file_name = title.as_deref().unwrap_or("");
            let file_size: u64 = source_path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len())
                .unwrap_or(0);
            quick_judge(&conn, file_name, file_size, ai_analyzed != 0, ai_extracted.as_deref())?
        } else {
            let text = if content_text.trim().is_empty() { title.as_deref().unwrap_or("") } else { &content_text };
            quick_judge_text(&conn, text)?
        };

        // 缓存快速判断结果到 inbox_items
        conn.execute(
            "UPDATE inbox_items SET quick_category = ?1, quick_confidence = ?2 WHERE id = ?3",
            rusqlite::params![result.category, result.confidence as f64, id],
        )?;

        Ok(result)
    })
    .await
}

/// 纯本地即时判断逻辑（§2.2）
fn quick_judge(
    conn: &rusqlite::Connection,
    file_name: &str,
    file_size: u64,
    already_analyzed: bool,
    _cached_ai_extracted: Option<&str>,
) -> anyhow::Result<QuickJudgeResult> {
    let category = crate::files::auto_classify(file_name).to_string();

    // 匹配到的案件按 case_id 去重
    let mut matches: std::collections::HashMap<String, (String, Vec<String>)> =
        std::collections::HashMap::new();

    // 1. 案号提取（最高权重信号）
    if let Some(cn) = extract_case_no_from_name(file_name) {
        if let Ok(case_id) = conn.query_row(
            "SELECT id FROM cases WHERE case_no LIKE ?1 LIMIT 1",
            rusqlite::params![format!("%{}%", cn)],
            |r| r.get::<_, String>(0),
        ) {
            let case_name: String = conn
                .query_row(
                    "SELECT COALESCE(display_name, case_name) FROM cases WHERE id = ?1",
                    rusqlite::params![case_id],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            matches
                .entry(case_id.clone())
                .or_insert_with(|| (case_name, vec![]))
                .1
                .push(format!("文件名包含案号 {}", cn));
        }
    }

    // 2. 当事人匹配
    for party in extract_parties_from_name(file_name) {
        let mut stmt = conn.prepare(
            "SELECT id, COALESCE(display_name, case_name) FROM cases WHERE client_name LIKE ?1 OR opponent_name LIKE ?1"
        )?;
        let case_iter = stmt.query_map(rusqlite::params![format!("%{}%", party)], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for (case_id, case_name) in case_iter.flatten() {
            matches
                .entry(case_id)
                .or_insert_with(|| (case_name, vec![]))
                .1
                .push(format!("文件名包含当事人 {}", party));
        }
    }

    // 3. 置信度 = 信号加权，封顶 0.95
    let mut confidence: f32 = 0.0;
    if category != "other" {
        confidence += 0.3;
    }
    if matches
        .values()
        .any(|(_, r)| r.iter().any(|s| s.contains("案号")))
    {
        confidence += 0.4;
    }
    if matches
        .values()
        .any(|(_, r)| r.iter().any(|s| s.contains("当事人")))
    {
        confidence += 0.2;
    }
    confidence = confidence.min(0.95);

    // 4. 推荐强度分级
    let strength = if confidence >= 0.7 {
        "strong"
    } else if confidence >= 0.3 {
        "candidate"
    } else {
        "fallback"
    };

    // 5. 构建推荐列表
    let recommendations: Vec<QuickRecommendation> = matches
        .into_iter()
        .map(|(case_id, (case_name, reasons))| QuickRecommendation {
            action: "file_to_case".to_string(),
            target_case_id: Some(case_id),
            target_case_name: Some(case_name),
            target_folder: Some(category_to_folder(&category)),
            intent: None,
            reason: reasons.join("；"),
        })
        .collect();

    // 6. AI 可用性：文件 < 5MB 可调 AI
    let ai_available = file_size < 5 * 1024 * 1024;

    Ok(QuickJudgeResult {
        category,
        confidence,
        strength: strength.to_string(),
        recommendations,
        ai_available,
        ai_analyzed: already_analyzed,
    })
}
/// 文本意图判断（设计哲学 §10：捕获任意信息 → 判断意图 → 推荐按钮 → 自行推送）
///
/// 意图优先级：期限 > 日程 > 任务 > 提醒 > 知识 > 新案件 > 兜底
/// 推荐动作：update_holidays / create_deadline / create_event / create_task / set_reminder / save_knowledge / create_case
fn quick_judge_text(conn: &rusqlite::Connection, text: &str) -> anyhow::Result<QuickJudgeResult> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(QuickJudgeResult {
            category: "note".to_string(),
            confidence: 0.0,
            strength: "fallback".to_string(),
            recommendations: vec![],
            ai_available: false,
            ai_analyzed: false,
        });
    }

    let mut recommendations: Vec<QuickRecommendation> = Vec::new();

    // 0) 关联案件：案号 / 当事人命中本地案件
    let mut matched_case: Option<(String, String)> = None;
    if let Some(cn) = extract_case_no_from_name(text) {
        if let Ok(case_id) = conn.query_row(
            "SELECT id FROM cases WHERE case_no LIKE ?1 LIMIT 1",
            rusqlite::params![format!("%{}%", cn)],
            |r| r.get::<_, String>(0),
        ) {
            let case_name: String = conn
                .query_row(
                    "SELECT COALESCE(display_name, case_name) FROM cases WHERE id = ?1",
                    rusqlite::params![case_id],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            matched_case = Some((case_id, case_name));
        }
    }
    if matched_case.is_none() {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, COALESCE(display_name, case_name) AS cname, client_name, opponent_name FROM cases"
        ) {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                ))
            }) {
                for r in rows.flatten() {
                    let (cid, cname, client, opponent) = r;
                    if (!client.is_empty() && client.chars().count() >= 2 && text.contains(&client))
                        || (!opponent.is_empty() && opponent.chars().count() >= 2 && text.contains(&opponent))
                        || (!cname.is_empty() && cname.chars().count() >= 2 && text.contains(&cname))
                    {
                        matched_case = Some((cid, cname));
                        break;
                    }
                }
            }
        }
    }
    if matched_case.is_none() {
        for party in extract_parties_from_name(text) {
            if let Ok((case_id, case_name)) = conn.query_row(
                "SELECT id, COALESCE(display_name, case_name) FROM cases WHERE client_name LIKE ?1 OR opponent_name LIKE ?1 OR case_name LIKE ?1 OR display_name LIKE ?1 LIMIT 1",
                rusqlite::params![format!("%{}%", party)],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            ) {
                matched_case = Some((case_id, case_name));
                break;
            }
        }
    }

    let due = extract_date_hint(text);
    let intent_base = |action: &str, _name: &str, reason: &str| -> QuickRecommendation {
        let mut intent = serde_json::json!({ "name": truncate_text(text, 60) });
        if let Some(d) = &due {
            intent["dueDate"] = serde_json::Value::String(d.clone());
        }
        if let Some(c) = &matched_case {
            intent["caseId"] = serde_json::Value::String(c.0.clone());
        }
        QuickRecommendation {
            action: action.to_string(),
            target_case_id: matched_case.as_ref().map(|c| c.0.clone()),
            target_case_name: matched_case.as_ref().map(|c| c.1.clone()),
            target_folder: None,
            intent: Some(intent),
            reason: reason.to_string(),
        }
    };

    // 0.5) 法定节假日通知：先解析，确认后才覆盖本地日历数据。
    if ["节假日", "放假安排", "放假调休", "调休上班", "国务院办公厅"]
        .iter()
        .any(|word| text.contains(word))
    {
        if let Ok(notice) = parse_holiday_dates(text) {
            recommendations.push(QuickRecommendation {
                action: "update_holidays".to_string(),
                target_case_id: None,
                target_case_name: None,
                target_folder: None,
                intent: Some(serde_json::json!({
                    "year": notice.year,
                    "holidays": notice.holidays,
                    "workdays": notice.workdays,
                })),
                reason: "检测到法定节假日或调休通知，确认后更新日历".to_string(),
            });
        }
    }

    // 1) 期限意图
    if ["截止", "到期", "期限", "届满", "失效", "最后一天", "提交日"]
        .iter()
        .any(|w| text.contains(w))
    {
        recommendations.push(intent_base(
            "create_deadline",
            "",
            "文本含期限词（截止/到期/期限）",
        ));
    }

    // 2) 日程意图
    if [
        "会议", "开会", "开庭", "日程", "预约", "拜访", "会面", "庭审",
    ]
    .iter()
    .any(|w| text.contains(w))
    {
        let mut rec = intent_base("create_event", "", "文本含日程词（会议/开庭/预约）");
        if let Some(intent) = rec.intent.as_mut() {
            intent["title"] = serde_json::Value::String(truncate_text(text, 60));
            if let Some(d) = &due {
                intent["eventDate"] = serde_json::Value::String(d.clone());
            }
        }
        recommendations.push(rec);
    }

    // 3) 任务意图
    if [
        "需要",
        "要做",
        "尽快",
        "别忘了",
        "安排",
        "完成",
        "准备",
        "交",
        "提交",
        "递交",
        "办理",
        "出具",
        "草拟",
        "签署",
        "寄送",
        "发送",
        "委托书",
        "授权委托书",
    ]
    .iter()
    .any(|w| text.contains(w))
    {
        let mut rec = intent_base("create_task", "", "文本含行动词（交/提交/办理/委托书）");
        if let Some(intent) = rec.intent.as_mut() {
            intent["taskName"] = serde_json::Value::String(truncate_text(text, 60));
        }
        recommendations.push(rec);
    }

    // 4) 提醒意图
    if ["提醒", "记得"].iter().any(|w| text.contains(w)) {
        let mut rec = intent_base("set_reminder", "", "文本含提醒词（提醒/记得）");
        if let Some(intent) = rec.intent.as_mut() {
            intent["title"] = serde_json::Value::String(truncate_text(text, 60));
            if let Some(d) = &due {
                intent["remindAt"] = serde_json::Value::String(d.clone());
            }
        }
        recommendations.push(rec);
    }

    // 4.5) 法院送达短信意图（court.gov.cn 官方链接 → 抓取送达文书）
    if let Some(delivery) = detect_service_delivery(text) {
        recommendations.push(QuickRecommendation {
            action: "service_delivery".to_string(),
            target_case_id: matched_case.as_ref().map(|c| c.0.clone()),
            target_case_name: matched_case.as_ref().map(|c| c.1.clone()),
            target_folder: None,
            intent: Some(serde_json::json!({
                "caseNo": delivery.case_no,
                "serviceUrl": delivery.service_url,
                "recipientName": delivery.recipient_name,
            })),
            reason: "检测到法院送达短信链接（zxfw.court.gov.cn）".to_string(),
        });
    }

    // 5) 知识意图
    if [
        "笔记", "参考", "资料", "心得", "总结", "整理", "备忘", "要点",
    ]
    .iter()
    .any(|w| text.contains(w))
    {
        recommendations.push(QuickRecommendation {
            action: "save_knowledge".to_string(),
            target_case_id: matched_case.as_ref().map(|c| c.0.clone()),
            target_case_name: matched_case.as_ref().map(|c| c.1.clone()),
            target_folder: None,
            intent: Some(serde_json::json!({
                "title": truncate_text(text, 60),
                "content": text,
                "category": "reference",
            })),
            reason: "文本含知识词（笔记/参考/资料）".to_string(),
        });
    }

    // 6) 新案件意图
    if ["收案", "委托", "新案件", "代理", "接案"]
        .iter()
        .any(|w| text.contains(w))
    {
        recommendations.push(QuickRecommendation {
            action: "create_case".to_string(),
            target_case_id: None,
            target_case_name: None,
            target_folder: None,
            intent: Some(serde_json::json!({
                "caseName": truncate_text(text, 60),
            })),
            reason: "文本含收案词（收案/委托/新案件）".to_string(),
        });
    }

    // 6.5) 非案件项目意图
    if [
        "新项目",
        "创建项目",
        "立项",
        "顾问项目",
        "尽调项目",
        "研究项目",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        recommendations.push(QuickRecommendation {
            action: "create_project".to_string(),
            target_case_id: None,
            target_case_name: None,
            target_folder: None,
            intent: Some(serde_json::json!({
                "name": truncate_text(text, 60),
                "description": text,
            })),
            reason: "文本含非案件项目词（新项目/顾问项目/尽调项目）".to_string(),
        });
    }

    // 7) 兜底：关联案件 → 转任务；否则 → 存知识
    if recommendations.is_empty() {
        if let Some((case_id, case_name)) = &matched_case {
            recommendations.push(QuickRecommendation {
                action: "create_task".to_string(),
                target_case_id: Some(case_id.clone()),
                target_case_name: Some(case_name.clone()),
                target_folder: None,
                intent: Some(serde_json::json!({
                    "taskName": truncate_text(text, 60),
                    "caseId": case_id,
                })),
                reason: "未识别明确意图，默认转为任务（关联案件）".to_string(),
            });
        } else {
            recommendations.push(QuickRecommendation {
                action: "save_knowledge".to_string(),
                target_case_id: None,
                target_case_name: None,
                target_folder: None,
                intent: Some(serde_json::json!({
                    "title": truncate_text(text, 40),
                    "content": text,
                    "category": "reference",
                })),
                reason: "未识别明确意图，默认存入知识库".to_string(),
            });
        }
    }

    // 强度：有关联案件或 2+ 意图 → strong；有明确意图词 → candidate；兜底 → fallback
    let primary = recommendations[0].action.clone();
    let explicit = matches!(
        primary.as_str(),
        "update_holidays"
            | "create_deadline"
            | "create_event"
            | "create_task"
            | "set_reminder"
            | "create_case"
            | "create_project"
            | "service_delivery"
    );
    let confidence: f32 = if matched_case.is_some() || recommendations.len() >= 2 {
        0.85
    } else if explicit {
        0.72
    } else {
        0.4
    };
    let strength = if confidence >= 0.7 {
        "strong"
    } else {
        "candidate"
    };

    Ok(QuickJudgeResult {
        category: primary,
        confidence,
        strength: strength.to_string(),
        recommendations,
        ai_available: true,
        ai_analyzed: false,
    })
}

/// 中文数字与常规数字统一解析 (1-100)
fn parse_zh_num(s: &str) -> Option<u32> {
    if let Ok(n) = s.parse::<u32>() {
        return Some(n);
    }
    match s {
        "一" | "壹" | "1" => Some(1),
        "二" | "两" | "贰" | "2" => Some(2),
        "三" | "叁" | "3" => Some(3),
        "四" | "肆" | "4" => Some(4),
        "五" | "伍" | "5" => Some(5),
        "六" | "陆" | "6" => Some(6),
        "七" | "柒" | "7" => Some(7),
        "八" | "捌" | "8" => Some(8),
        "九" | "玖" | "9" => Some(9),
        "十" | "拾" | "10" => Some(10),
        "十一" | "11" => Some(11),
        "十二" | "12" => Some(12),
        "十三" | "13" => Some(13),
        "十四" | "14" => Some(14),
        "十五" | "15" => Some(15),
        "十六" | "16" => Some(16),
        "十七" | "17" => Some(17),
        "十八" | "18" => Some(18),
        "十九" | "19" => Some(19),
        "二十" | "廿" | "20" => Some(20),
        "二十一" | "21" => Some(21),
        "二十二" | "22" => Some(22),
        "二十三" | "23" => Some(23),
        "二十四" | "24" => Some(24),
        "二十五" | "25" => Some(25),
        "二十六" | "26" => Some(26),
        "二十七" | "27" => Some(27),
        "二十八" | "28" => Some(28),
        "二十九" | "29" => Some(29),
        "三十" | "卅" | "30" => Some(30),
        "三十一" | "31" => Some(31),
        _ => None,
    }
}

/// 计算月份加减（自动对齐月末有效天数）
fn add_months_clamped(date: chrono::NaiveDate, months: i32) -> Option<chrono::NaiveDate> {
    use chrono::Datelike;
    let total_m = date.year() * 12 + (date.month() as i32 - 1) + months;
    let new_y = total_m / 12;
    let new_m = (total_m % 12 + 1) as u32;
    let max_day = last_day_of_month(new_y, new_m).day();
    let new_d = std::cmp::min(date.day(), max_day);
    chrono::NaiveDate::from_ymd_opt(new_y, new_m, new_d)
}

/// 获取指定年月最后一天
fn last_day_of_month(year: i32, month: u32) -> chrono::NaiveDate {
    let (next_y, next_m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    chrono::NaiveDate::from_ymd_opt(next_y, next_m, 1)
        .unwrap()
        .pred_opt()
        .unwrap()
}

/// 解析周几与周期相对日期 (本周五 / 下周一 / 星期三 / 这周天 / 下下周二等)
fn parse_weekday_match(text: &str, today: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
    use chrono::{Datelike, Duration};

    let re = regex::Regex::new(
        r"(本周|这周|这个星期|这个礼拜|下周|下个星期|下礼拜|下下周|下两个星期|周|星期|礼拜)\s*([一二三四五六日天七1-7])",
    )
    .ok()?;
    let caps = re.captures(text)?;
    let prefix = &caps[1];
    let day_str = &caps[2];

    let target_weekday_num = match day_str {
        "一" | "1" => 1,
        "二" | "2" => 2,
        "三" | "3" => 3,
        "四" | "4" => 4,
        "五" | "5" => 5,
        "六" | "6" => 6,
        "日" | "天" | "七" | "7" => 7,
        _ => return None,
    };

    let cur_weekday_num = today.weekday().number_from_monday(); // 1 = Mon, 7 = Sun

    if prefix.starts_with("下下周") || prefix.starts_with("下两个星期") {
        let days_until_next_next_mon = (7 - cur_weekday_num + 1) + 7;
        let target_mon = today + Duration::days(days_until_next_next_mon as i64);
        Some(target_mon + Duration::days((target_weekday_num - 1) as i64))
    } else if prefix.starts_with("下周")
        || prefix.starts_with("下个星期")
        || prefix.starts_with("下礼拜")
    {
        let days_until_next_mon = 7 - cur_weekday_num + 1;
        let next_mon = today + Duration::days(days_until_next_mon as i64);
        Some(next_mon + Duration::days((target_weekday_num - 1) as i64))
    } else if prefix.starts_with("本周")
        || prefix.starts_with("这周")
        || prefix.starts_with("这个星期")
        || prefix.starts_with("这个礼拜")
    {
        let mon = today - Duration::days((cur_weekday_num - 1) as i64);
        Some(mon + Duration::days((target_weekday_num - 1) as i64))
    } else {
        // 单纯出现 "周五" / "星期三"：若是今天及之后的周几算本周，否则算下周
        if target_weekday_num >= cur_weekday_num {
            let diff = target_weekday_num - cur_weekday_num;
            Some(today + Duration::days(diff as i64))
        } else {
            let diff = 7 - (cur_weekday_num - target_weekday_num);
            Some(today + Duration::days(diff as i64))
        }
    }
}

/// 解析相对日期 (今天/明天/后天/大后天/N天后/N周后/N月后/月初/月末等)
fn parse_relative_date(text: &str, today: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
    use chrono::{Datelike, Duration};

    // 今日 / 今天 / 今早 / 今晚 / 今晨
    if text.contains("今天")
        || text.contains("今日")
        || text.contains("今早")
        || text.contains("今晚")
        || text.contains("今晨")
    {
        return Some(today);
    }
    // 大后天
    if text.contains("大后天") {
        return Some(today + Duration::days(3));
    }
    // 后天 / 后日
    if text.contains("后天") || text.contains("后日") {
        return Some(today + Duration::days(2));
    }
    // 明天 / 明日 / 明早 / 明晚 / 明晨
    if text.contains("明天")
        || text.contains("明日")
        || text.contains("明早")
        || text.contains("明晚")
        || text.contains("明晨")
    {
        return Some(today + Duration::days(1));
    }
    // 昨天 / 昨晚
    if text.contains("昨天") || text.contains("昨日") || text.contains("昨晚") {
        return Some(today - Duration::days(1));
    }
    // 前天
    if text.contains("前天") || text.contains("前日") {
        return Some(today - Duration::days(2));
    }
    // 半个月后 / 半月后 / 半个月内
    if text.contains("半个月后")
        || text.contains("半个月内")
        || text.contains("半月后")
        || text.contains("半月内")
    {
        return Some(today + Duration::days(15));
    }

    // N天后 / N日后 / N天内 / N日内 / N天之后 / N天以内
    if let Ok(re_days) = regex::Regex::new(
        r"(\d+|[一二两三四五六七八九十百]+)\s*(?:个)?(?:工作)?(?:天|日)\s*(?:之|以)?(?:后|内|后交|后截止|后到期|之后|以内)",
    ) {
        if let Some(caps) = re_days.captures(text) {
            if let Some(n) = parse_zh_num(&caps[1]) {
                return Some(today + Duration::days(n as i64));
            }
        }
    }

    // N周后 / N个星期后 / N礼拜后 / N周内
    if let Ok(re_weeks) = regex::Regex::new(
        r"(\d+|[一二两三四五六七八九十]+)\s*(?:个)?(?:周|星期|礼拜)\s*(?:之|以)?(?:后|内|之后|以内)",
    ) {
        if let Some(caps) = re_weeks.captures(text) {
            if let Some(n) = parse_zh_num(&caps[1]) {
                return Some(today + Duration::days((n * 7) as i64));
            }
        }
    }

    // N个月后 / N月后 / N个月内
    if let Ok(re_months) = regex::Regex::new(
        r"(\d+|[一二两三四五六七八九十]+)\s*(?:个)?月\s*(?:之|以)?(?:后|内|之后|以内)",
    ) {
        if let Some(caps) = re_months.captures(text) {
            if let Some(n) = parse_zh_num(&caps[1]) {
                return add_months_clamped(today, n as i32);
            }
        }
    }

    // N年后 / N年内
    if let Ok(re_years) =
        regex::Regex::new(r"(\d+|[一二两三四五六七八九十]+)\s*年\s*(?:之|以)?(?:后|内|之后|以内)")
    {
        if let Some(caps) = re_years.captures(text) {
            if let Some(n) = parse_zh_num(&caps[1]) {
                return chrono::NaiveDate::from_ymd_opt(
                    today.year() + n as i32,
                    today.month(),
                    today.day(),
                )
                .or_else(|| {
                    chrono::NaiveDate::from_ymd_opt(today.year() + n as i32, today.month(), 28)
                });
            }
        }
    }

    // 周几解析：本周X / 下周X / 这周X / 星期X / 礼拜X
    if let Some(d) = parse_weekday_match(text, today) {
        return Some(d);
    }

    // 月初 / 月末 / 月底解析
    if text.contains("本月底")
        || text.contains("当月底")
        || text.contains("这个月底")
        || text.contains("月末")
        || text.contains("月底")
    {
        return Some(last_day_of_month(today.year(), today.month()));
    }
    if text.contains("下月初") || text.contains("下月头") {
        let (y, m) = if today.month() == 12 {
            (today.year() + 1, 1)
        } else {
            (today.year(), today.month() + 1)
        };
        return chrono::NaiveDate::from_ymd_opt(y, m, 1);
    }
    if text.contains("下月底") || text.contains("下月末") {
        let (y, m) = if today.month() == 12 {
            (today.year() + 1, 1)
        } else {
            (today.year(), today.month() + 1)
        };
        return Some(last_day_of_month(y, m));
    }

    None
}

/// 解析绝对日期 (YYYY-MM-DD / YYYY年M月D日 / MM-DD / M月D日 / 中文月份日期)
fn parse_absolute_date(text: &str, today: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
    use chrono::Datelike;

    // 1. YYYY-MM-DD / YYYY/M/D / YYYY.M.D / YYYY年M月D日(号)
    if let Ok(re) =
        regex::Regex::new(r"(\d{4})\s*[\-/\.年]\s*(\d{1,2})\s*[\-/\.月]\s*(\d{1,2})\s*[日号]?")
    {
        if let Some(caps) = re.captures(text) {
            if let (Ok(y), Ok(m), Ok(d)) = (
                caps[1].parse::<i32>(),
                caps[2].parse::<u32>(),
                caps[3].parse::<u32>(),
            ) {
                if let Some(dt) = chrono::NaiveDate::from_ymd_opt(y, m, d) {
                    return Some(dt);
                }
            }
        }
    }

    // 2. MM-DD / M/D / M.D / M月D日 / M月D号
    if let Ok(re) =
        regex::Regex::new(r"(?:^|[^\d])(\d{1,2})\s*[\-/\.月]\s*(\d{1,2})(?:[日号]|\b|$)")
    {
        if let Some(caps) = re.captures(text) {
            if let (Ok(m), Ok(d)) = (caps[1].parse::<u32>(), caps[2].parse::<u32>()) {
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    if let Some(dt) = chrono::NaiveDate::from_ymd_opt(today.year(), m, d) {
                        if dt >= today {
                            return Some(dt);
                        } else if let Some(next_year_dt) =
                            chrono::NaiveDate::from_ymd_opt(today.year() + 1, m, d)
                        {
                            return Some(next_year_dt);
                        }
                    }
                }
            }
        }
    }

    // 3. 中文数字月份与日期：如 "九月十五日" / "十月一号" / "五月二十"
    if let Ok(re) =
        regex::Regex::new(r"([一二三四五六七八九十]+)月\s*([一二三四五六七八九十廿卅]+)[日号]?")
    {
        if let Some(caps) = re.captures(text) {
            if let (Some(m), Some(d)) = (parse_zh_num(&caps[1]), parse_zh_num(&caps[2])) {
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    if let Some(dt) = chrono::NaiveDate::from_ymd_opt(today.year(), m, d) {
                        if dt >= today {
                            return Some(dt);
                        } else if let Some(next_year_dt) =
                            chrono::NaiveDate::from_ymd_opt(today.year() + 1, m, d)
                        {
                            return Some(next_year_dt);
                        }
                    }
                }
            }
        }
    }

    None
}

/// 从文本提取完整自然语言日期与时间（支持所有相对日期、中文表达、周期、年月日及 N 天/周/月/年后）
fn extract_date_hint(text: &str) -> Option<String> {
    let today = chrono::Local::now().date_naive();

    // 1. 优先绝对日期
    if let Some(d) = parse_absolute_date(text, today) {
        return Some(d.format("%Y-%m-%d").to_string());
    }

    // 2. 相对日期推断
    if let Some(d) = parse_relative_date(text, today) {
        return Some(d.format("%Y-%m-%d").to_string());
    }

    None
}

/// 截断文本（按字符，保留省略号）
fn truncate_text(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count > max {
        format!("{}…", s.chars().take(max).collect::<String>())
    } else {
        s.to_string()
    }
}

/// 从文件名或文本中提取案号/决定号（支持全国法院案号、最高法知产案号、国知局4W编号及决定号）
fn extract_case_no_from_name(file_name: &str) -> Option<String> {
    // 1. 标准法院案号：如 (2023)最高法知行终123号 / （2024）京73行初456号
    if let Ok(re) = regex::Regex::new(r"[（(]\s*\d{4}\s*[）)][\u{4e00}-\u{9fff}\w\d\-_]+?\d+号")
    {
        if let Some(m) = re.find(file_name) {
            return Some(m.as_str().to_string());
        }
    }
    // 2. 国知局无效/复审案件编号：4W123456 / 5W123456 / 5F123456
    if let Ok(re) = regex::Regex::new(r"\b(\d+[WF]\d+)\b") {
        if let Some(caps) = re.captures(file_name) {
            return Some(caps[1].to_string());
        }
    }
    // 3. 决定号：第56123号
    if let Ok(re) = regex::Regex::new(r"第\s*\d{4,7}\s*号") {
        if let Some(m) = re.find(file_name) {
            return Some(m.as_str().to_string());
        }
    }
    // 4. 宽松兜底案号格式：[(（]202x[)）]...号
    if let Ok(re) = regex::Regex::new(r"[（(]\s*\d{4}\s*[）)].*?号") {
        if let Some(m) = re.find(file_name) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

/// 从文件名或文本中提取当事人/案件关键词
fn extract_parties_from_name(text: &str) -> Vec<String> {
    let mut parties = Vec::new();

    // 1. 匹配 "...案" (如 "李四案", "张三诉李四案", "华为中兴案")
    if let Ok(re_case) = regex::Regex::new(r"([\u{4e00}-\u{9fff}\w]{2,12})案") {
        for cap in re_case.captures_iter(text) {
            let p = cap[1].to_string();
            let trimmed = p
                .trim_start_matches(|c| "交办写发关于对看查与".contains(c))
                .to_string();
            if trimmed.chars().count() >= 2 {
                parties.push(trimmed);
            }
        }
    }

    // 2. 匹配 "X诉Y"
    if let Ok(re_vs) =
        regex::Regex::new(r"([\u{4e00}-\u{9fff}]{2,10})诉([\u{4e00}-\u{9fff}]{2,10})")
    {
        for cap in re_vs.captures_iter(text) {
            parties.push(cap[1].to_string());
            parties.push(cap[2].to_string());
        }
    }

    // 3. 过滤掉常见非当事人词
    let stop_words: std::collections::HashSet<&str> = [
        "传票",
        "判决",
        "裁定",
        "决定",
        "起诉",
        "答辩",
        "证据",
        "通知书",
        "口审",
        "函件",
        "文件",
        "扫描",
        "复印件",
        "原件",
        "副本",
        "明天",
        "后天",
        "今天",
        "委托书",
        "授权委托书",
        "起诉状",
        "答辩状",
        "代理词",
    ]
    .iter()
    .copied()
    .collect();

    let re = regex::Regex::new(r"[\u{4e00}-\u{9fff}]{2,6}").unwrap();
    for m in re.find_iter(text) {
        let w = m.as_str();
        if !stop_words.contains(w) && !parties.iter().any(|p| p == w) {
            parties.push(w.to_string());
        }
    }

    parties
}

/// 分类 → 标准子目录映射
fn category_to_folder(category: &str) -> String {
    match category {
        "summons" => "01_传票".to_string(),
        "evidence" => "02_证据".to_string(),
        "complaint" | "defence" | "submitted" => "03_交文".to_string(),
        "judgment" | "official_notice" | "hearing_notice" => "04_收文".to_string(),
        "correspondence" => "06_通信".to_string(),
        _ => "07_其他".to_string(),
    }
}

/// 拒绝推荐反馈（设计哲学 §10：推荐拒绝 → 学习信号）
///
/// 记录用户拒绝推荐的原因到 inbox_feedback 表，供推荐系统学习改进。
/// 字段：inbox_item_id, action, reason, intent_json, accepted, rejected_at
#[tauri::command]
pub async fn reject_inbox_recommendation(
    inbox_item_id: String,
    action: String,
    reason: Option<String>,
    intent: Option<serde_json::Value>,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let intent_json = intent.map(|v| serde_json::to_string(&v).unwrap_or_default());

        conn.execute(
            "INSERT INTO inbox_feedback (id, inbox_item_id, action, reason, intent_json, accepted, rejected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            rusqlite::params![id, inbox_item_id, action, reason, intent_json, now],
        )?;

        log::info!(
            "收件箱推荐拒绝反馈已记录: item={}, action={}, reason={:?}",
            inbox_item_id,
            action,
            reason
        );

        Ok(())
    })
    .await
}

/// 确认收件箱推荐动作（设计哲学 §10：捕获→厘清→执行闭环）
#[tauri::command]
pub async fn confirm_inbox_action(
    inbox_item_id: String,
    action: String,
    target_case_id: Option<String>,
    target_category: Option<String>,
    intent: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let lookup_id = inbox_item_id.clone();
    let content_text = run_blocking(move || {
        let conn = db::open_db()?;
        conn.query_row(
            "SELECT COALESCE(content_text, title, '') FROM inbox_items WHERE id = ?1",
            rusqlite::params![lookup_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|_| anyhow::anyhow!("收件箱项不存在"))
    })
    .await?;

    let field = |name: &str| {
        intent
            .as_ref()
            .and_then(|value| value.get(name))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    };

    let result = match action.as_str() {
        "file_to_case" => {
            let case_id = target_case_id
                .clone()
                .ok_or_else(|| "请选择目标案件".to_string())?;
            let category = target_category
                .clone()
                .unwrap_or_else(|| "07_其他".to_string());
            file_inbox_item(inbox_item_id.clone(), case_id.clone(), category.clone()).await?;
            serde_json::json!({"success": true, "action": "filed", "caseId": case_id, "category": category})
        }
        "create_task" | "create_deadline" | "set_reminder" => {
            let task_name = field("taskName")
                .or_else(|| field("name"))
                .unwrap_or_else(|| content_text.clone());
            let due_date = field("dueDate").or_else(|| field("remindAt"));
            let data = serde_json::json!({
                "taskName": task_name,
                "caseId": target_case_id,
                "taskType": if action == "create_deadline" { "deadline" } else { "action" },
                "startBucket": if due_date.is_some() { "upcoming" } else { "inbox" },
                "startDate": due_date,
                "dueDate": due_date,
                "dueTime": field("dueTime"),
                "inboxSourceId": inbox_item_id,
            });
            let created = super::tasks::create_task(data).await?;
            serde_json::json!({"success": true, "action": "task_created", "task": created})
        }
        "update_holidays" => {
            let notice = parse_holiday_dates(&content_text)?;
            let notice_year = notice.year;
            let holidays_count = notice.holidays.len();
            let workdays_count = notice.workdays.len();
            run_blocking(move || {
                let conn = db::open_db()?;
                let mut calendar = db::get_setting(&conn, "holidays_json")
                    .ok()
                    .flatten()
                    .and_then(|value| {
                        crate::deadline::holidays::HolidayCalendar::from_json_str(&value).ok()
                    })
                    .unwrap_or_else(crate::deadline::holidays::HolidayCalendar::builtin);
                calendar
                    .merge_dates(&notice.holidays, &notice.workdays)
                    .map_err(anyhow::Error::msg)?;
                conn.execute(
                    "INSERT INTO settings (key, value) VALUES ('holidays_json', ?1)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    rusqlite::params![calendar.to_json()],
                )?;
                Ok(())
            })
            .await?;
            serde_json::json!({
                "success": true,
                "action": "holidays_updated",
                "year": notice_year,
                "holidaysCount": holidays_count,
                "workdaysCount": workdays_count,
            })
        }
        "create_event" => {
            let title = field("title")
                .or_else(|| field("name"))
                .unwrap_or_else(|| content_text.clone());
            let event_date = field("eventDate")
                .or_else(|| field("dueDate"))
                .unwrap_or_else(db::today);
            let created = super::calendar_events::create_calendar_event(serde_json::json!({
                "title": title,
                "eventDate": event_date,
                "startTime": field("startTime"),
                "endTime": field("endTime"),
                "allDay": field("startTime").is_none(),
                "caseId": target_case_id,
                "notes": content_text,
            }))
            .await?;
            serde_json::json!({"success": true, "action": "event_created", "event": created})
        }
        "save_knowledge" => {
            let title = field("title").unwrap_or_else(|| truncate_text(&content_text, 60));
            let input = super::knowledge::CreateKnowledgeInput {
                title,
                content: field("content").unwrap_or_else(|| content_text.clone()),
                category: "other".to_string(),
                source_type: Some("inbox".to_string()),
                source_id: Some(inbox_item_id.clone()),
                linked_case_id: target_case_id.clone(),
                ..Default::default()
            };
            let id = super::knowledge::create_knowledge(input).await?;
            serde_json::json!({"success": true, "action": "knowledge_saved", "knowledgeId": id})
        }
        "create_case" => {
            let case_name = field("caseName")
                .or_else(|| field("name"))
                .unwrap_or_else(|| content_text.clone());
            let created = super::cases::create_case(serde_json::json!({
                "track": field("track").unwrap_or_else(|| "patent_invalidation".to_string()),
                "caseName": case_name,
                "clientName": field("clientName").unwrap_or_default(),
                "opponentName": field("opponentName").unwrap_or_default(),
                "caseNo": field("caseNo"),
                "court": field("court"),
                "causeAction": field("causeAction"),
            }))
            .await?;
            serde_json::json!({"success": true, "action": "case_created", "case": created})
        }
        "create_project" => {
            let name = field("name")
                .or_else(|| field("title"))
                .unwrap_or_else(|| content_text.clone());
            let created = super::projects::create_personal_project(serde_json::json!({
                "name": name,
                "description": field("description").unwrap_or_else(|| content_text.clone()),
            }))
            .await?;
            serde_json::json!({"success": true, "action": "project_created", "project": created})
        }
        "service_delivery" => {
            let service_url =
                field("serviceUrl").ok_or_else(|| "法院送达短信中缺少有效链接".to_string())?;
            let path = download_service_delivery_url(&inbox_item_id, &service_url).await?;
            serde_json::json!({"success": true, "action": "service_downloaded", "path": path})
        }
        "ignore" | "dismiss" => {
            dismiss_inbox_item(inbox_item_id.clone()).await?;
            serde_json::json!({"success": true, "action": "dismissed"})
        }
        _ => return Err(format!("未知收件箱动作: {action}")),
    };

    if !matches!(
        action.as_str(),
        "file_to_case" | "service_delivery" | "ignore" | "dismiss"
    ) {
        let finalize_id = inbox_item_id.clone();
        let feedback_action = action.clone();
        let feedback_intent = intent.clone();
        run_blocking(move || {
            let conn = db::open_db()?;
            let now = db::now_local();
            conn.execute(
                "UPDATE inbox_items SET status = 'filed', processed_at = ?1 WHERE id = ?2",
                rusqlite::params![&now, &finalize_id],
            )?;
            let feedback_id = db::new_id();
            conn.execute(
                "INSERT INTO inbox_feedback (id, inbox_item_id, action, intent_json, accepted, rejected_at) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
                rusqlite::params![feedback_id, finalize_id, feedback_action, feedback_intent.map(|v| v.to_string()), now],
            )?;
            Ok(())
        })
        .await?;
    }

    Ok(result)
}

/// 深度分析收件箱项。复用统一处理链：AI 可用时调用 AI，失败或关闭时回退本地规则。
#[tauri::command]
pub async fn ai_analyze_inbox_item(id: String) -> Result<serde_json::Value, String> {
    let result = process_inbox_item(id).await?;
    serde_json::to_value(result).map_err(|e| format!("无法序列化分析结果: {e}"))
}

/// 兼容旧入口：旧命令没有 URL 参数，提示用户改走可审计的 Inbox 完整短信流程。
#[tauri::command]
pub async fn download_service_delivery(
    _case_no: String,
    _recipient_name: String,
) -> Result<String, String> {
    Err("请把包含法院链接的完整短信粘贴到统一捕获，Casy 会先校验官方域名再尝试下载".to_string())
}

/// 处理已进入收件箱的法院送达短信。
#[tauri::command]
pub async fn process_service_delivery(inbox_item_id: String) -> Result<(), String> {
    let lookup_id = inbox_item_id.clone();
    let content = run_blocking(move || {
        let conn = db::open_db()?;
        conn.query_row(
            "SELECT COALESCE(content_text, '') FROM inbox_items WHERE id = ?1",
            rusqlite::params![lookup_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|_| anyhow::anyhow!("收件箱项不存在"))
    })
    .await?;
    let delivery = detect_service_delivery(&content)
        .ok_or_else(|| "未检测到 court.gov.cn 法院送达链接".to_string())?;
    download_service_delivery_url(&inbox_item_id, &delivery.service_url).await?;
    Ok(())
}

/// 捕获屏幕截图到收件箱（占位）
#[tauri::command]
pub async fn capture_screenshot() -> Result<String, String> {
    Err("截图捕获功能开发中，敬请期待".into())
}

/// 捕获剪贴板内容到收件箱（占位）
#[tauri::command]
pub async fn capture_clipboard() -> Result<String, String> {
    Err("剪贴板捕获功能开发中，敬请期待".into())
}

/// 启动剪贴板监听（占位）
#[tauri::command]
pub async fn start_clipboard_monitor() -> Result<(), String> {
    Err("剪贴板监听功能开发中，敬请期待".into())
}

/// 保存语音速记（占位）
#[tauri::command]
pub async fn save_voice_note(
    _audio_data: Vec<u8>,
    _duration_seconds: i32,
) -> Result<String, String> {
    Err("语音速记功能开发中，敬请期待".into())
}

/// 语音转写（占位）
#[tauri::command]
pub async fn transcribe_voice_note(_voice_note_id: String) -> Result<String, String> {
    Err("语音转写功能开发中，敬请期待".into())
}

/// 启动收件箱批量处理（占位）
#[tauri::command]
pub async fn start_inbox_batch() -> Result<(), String> {
    Err("批量处理功能开发中，敬请期待".into())
}

/// 暂停收件箱批量处理（占位）
#[tauri::command]
pub async fn pause_inbox_batch() -> Result<(), String> {
    Ok(())
}

/// 恢复收件箱批量处理（占位）
#[tauri::command]
pub async fn resume_inbox_batch() -> Result<(), String> {
    Ok(())
}

/// 取消收件箱批量处理（占位）
#[tauri::command]
pub async fn cancel_inbox_batch() -> Result<(), String> {
    Ok(())
}

/// 获取收件箱处理进度（占位）
#[tauri::command]
pub async fn get_inbox_progress() -> Result<InboxProgress, String> {
    Ok(InboxProgress {
        total: 0,
        processed: 0,
        pending: 0,
    })
}

/// 重试收件箱项（占位）
#[tauri::command]
pub async fn retry_inbox_item(_id: String) -> Result<(), String> {
    Ok(())
}

/// 重试收件箱案件（占位）
#[tauri::command]
pub async fn retry_inbox_case(_case_id: String) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        detect_service_delivery, extract_date_hint, parse_holiday_dates, quick_judge_text,
    };

    #[test]
    fn detects_official_court_delivery_link_and_fields() {
        let text = "受送达人：张三，案号（2026）京73民初12号，请访问 https://zxfw.court.gov.cn/sd/abc123 查看文书";
        let delivery = detect_service_delivery(text).expect("应识别法院送达短信");
        assert_eq!(delivery.service_url, "https://zxfw.court.gov.cn/sd/abc123");
        assert_eq!(delivery.recipient_name, "张三");
        assert!(delivery.case_no.contains("2026"));
    }

    #[test]
    fn rejects_non_court_link() {
        let text = "请访问 https://example.com/zxfw.court.gov.cn/fake 下载文书";
        assert!(detect_service_delivery(text).is_none());
    }

    #[test]
    fn recognizes_short_date_event_intent() {
        let conn = rusqlite::Connection::open_in_memory().expect("内存数据库应可用");
        let result = quick_judge_text(&conn, "9-10 下午开庭").expect("规则判断应成功");
        assert!(result
            .recommendations
            .iter()
            .any(|item| item.action == "create_event"));
        assert_eq!(
            result.recommendations[0]
                .intent
                .as_ref()
                .and_then(|intent| intent.get("eventDate"))
                .and_then(|value| value.as_str())
                .map(|value| &value[value.len() - 5..]),
            Some("09-10")
        );
    }

    #[test]
    fn parses_short_date_without_control_characters() {
        let date = extract_date_hint("请安排 10/8 会面").expect("应识别短日期");
        assert!(date.ends_with("-10-08"));
    }

    #[test]
    fn parses_official_holiday_ranges_and_makeup_days() {
        let text = "2027年元旦：1月1日至3日放假调休，共3天。1月4日上班。春节：2月5日至11日放假，共7天。2月4日、2月20日上班。";
        let notice = parse_holiday_dates(text).expect("应解析节假日通知");
        assert_eq!(notice.year, 2027);
        assert_eq!(notice.holidays.len(), 10);
        assert_eq!(
            notice.workdays,
            vec!["2027-01-04", "2027-02-04", "2027-02-20"]
        );
    }

    #[test]
    fn recommends_holiday_update_before_generic_task() {
        let conn = rusqlite::Connection::open_in_memory().expect("内存数据库应可用");
        let result = quick_judge_text(
            &conn,
            "2027年节假日放假安排：1月1日至3日放假调休。1月4日上班。",
        )
        .expect("规则判断应成功");
        assert_eq!(result.recommendations[0].action, "update_holidays");
    }

    #[test]
    fn test_quick_judge_tomorrow_submit_poa_with_or_without_case() {
        let conn = rusqlite::Connection::open_in_memory().expect("内存数据库应可用");
        conn.execute_batch(
            "CREATE TABLE cases (id TEXT PRIMARY KEY, case_name TEXT, display_name TEXT, client_name TEXT, opponent_name TEXT, case_no TEXT);
             INSERT INTO cases (id, case_name, client_name) VALUES ('case-101', '李四专利侵权纠纷案', '李四');"
        ).expect("初始化cases表应成功");

        // 1. 命中「李四案」时
        let result_matched =
            quick_judge_text(&conn, "明天交李四案的授权委托书").expect("规则判断应成功");
        assert!(result_matched
            .recommendations
            .iter()
            .any(|r| r.action == "create_task"));
        let task_rec = result_matched
            .recommendations
            .iter()
            .find(|r| r.action == "create_task")
            .unwrap();
        assert_eq!(task_rec.target_case_id, Some("case-101".to_string()));
        assert!(task_rec.intent.as_ref().unwrap().get("dueDate").is_some());

        // 2. 未命中任何案件时（例如数据库中无王五）
        let result_unmatched =
            quick_judge_text(&conn, "明天交王五案的授权委托书").expect("规则判断应成功");
        assert!(result_unmatched
            .recommendations
            .iter()
            .any(|r| r.action == "create_task"));
        let unlinked_rec = result_unmatched
            .recommendations
            .iter()
            .find(|r| r.action == "create_task")
            .unwrap();
        assert_eq!(unlinked_rec.target_case_id, None);
        assert!(unlinked_rec
            .intent
            .as_ref()
            .unwrap()
            .get("dueDate")
            .is_some());
    }

    #[test]
    fn test_all_natural_language_dates_recognition() {
        use chrono::{Datelike, Duration, Local};
        let today = Local::now().date_naive();

        // 1. 相对今天/明天/后天/大后天
        assert_eq!(
            extract_date_hint("今天下午开会"),
            Some(today.format("%Y-%m-%d").to_string())
        );
        assert_eq!(
            extract_date_hint("明天提交答辩状"),
            Some((today + Duration::days(1)).format("%Y-%m-%d").to_string())
        );
        assert_eq!(
            extract_date_hint("后天上午开庭"),
            Some((today + Duration::days(2)).format("%Y-%m-%d").to_string())
        );
        assert_eq!(
            extract_date_hint("大后天截止"),
            Some((today + Duration::days(3)).format("%Y-%m-%d").to_string())
        );

        // 2. N天/周/月/年后
        assert_eq!(
            extract_date_hint("3天后交证据"),
            Some((today + Duration::days(3)).format("%Y-%m-%d").to_string())
        );
        assert_eq!(
            extract_date_hint("三天内提交"),
            Some((today + Duration::days(3)).format("%Y-%m-%d").to_string())
        );
        assert_eq!(
            extract_date_hint("15日内提出上诉"),
            Some((today + Duration::days(15)).format("%Y-%m-%d").to_string())
        );
        assert_eq!(
            extract_date_hint("2周后交代理词"),
            Some((today + Duration::days(14)).format("%Y-%m-%d").to_string())
        );
        assert_eq!(
            extract_date_hint("半个月后截止"),
            Some((today + Duration::days(15)).format("%Y-%m-%d").to_string())
        );
        assert!(extract_date_hint("1个月后").is_some());
        assert!(extract_date_hint("3个月内").is_some());

        // 3. 周几 (下周五 / 本周三 / 星期一)
        assert!(extract_date_hint("下周五开庭").is_some());
        assert!(extract_date_hint("下个星期二交材料").is_some());
        assert!(extract_date_hint("本周五必须完成").is_some());

        // 4. 月底 / 月初
        assert!(extract_date_hint("本月底之前完成").is_some());
        assert!(extract_date_hint("下月初开会").is_some());

        // 5. 绝对中文/点号/斜杠/横杠日期
        assert_eq!(
            extract_date_hint("2026年9月15日交公证书"),
            Some("2026-09-15".to_string())
        );
        assert_eq!(
            extract_date_hint("2026.09.15 开庭"),
            Some("2026-09-15".to_string())
        );
        assert_eq!(
            extract_date_hint("2026/9/15"),
            Some("2026-09-15".to_string())
        );
        assert_eq!(
            extract_date_hint("2026-9-15"),
            Some("2026-09-15".to_string())
        );
        assert_eq!(
            extract_date_hint("九月十五日截止"),
            Some(format!("{}-09-15", today.year()))
        );
        assert_eq!(
            extract_date_hint("十月一日放假"),
            Some(format!("{}-10-01", today.year()))
        );
    }
}

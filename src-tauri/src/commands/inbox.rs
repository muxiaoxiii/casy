use super::run_blocking;
use crate::{db, parse};
use chrono::Datelike;

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

        // 尝试 AI 分类（使用 prompt 增强）
        let ai_config = crate::ai::load_ai_config();
        let (category, confidence, ai_extracted, suggested_case_id) = if ai_config.mode != "noop"
        {
            let rt = tokio::runtime::Runtime::new().unwrap();
            match rt.block_on(crate::ai::process_inbox_with_ai(&text)) {
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
                source_type,
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
pub async fn list_inbox_items(status: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut sql = String::from("SELECT * FROM inbox_items WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = &status {
            if !s.is_empty() {
                sql.push_str(" AND status = ?1");
                params.push(Box::new(s.clone()));
            }
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT 100");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let items: Vec<serde_json::Value> = stmt
            .query_map(param_refs.as_slice(), |row| {
                let ai_extracted: Option<String> = row.get("ai_extracted")?;
                let parsed_extracted: Option<serde_json::Value> = ai_extracted
                    .and_then(|s| serde_json::from_str(&s).ok());

                Ok(serde_json::json!({
                    "id": row.get::<_, String>("id")?,
                    "sourceType": row.get::<_, String>("source_type")?,
                    "title": row.get::<_, Option<String>>("title")?,
                    "contentText": row.get::<_, Option<String>>("content_text")?,
                    "aiCategory": row.get::<_, Option<String>>("ai_category")?,
                    "aiConfidence": row.get::<_, Option<f64>>("ai_confidence")?,
                    "aiExtracted": parsed_extracted,
                    "aiSuggestedCaseId": row.get::<_, Option<String>>("ai_suggested_case_id")?,
                    "status": row.get::<_, String>("status")?,
                    "linkedCaseId": row.get::<_, Option<String>>("linked_case_id")?,
                    "createdAt": row.get::<_, Option<String>>("created_at")?,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(items)
    })
    .await
}

#[tauri::command]
pub async fn process_inbox_item(id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 获取收件项
        let content_text: String = conn
            .query_row(
                "SELECT content_text FROM inbox_items WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .map_err(|_| anyhow::anyhow!("收件项不存在"))?;

        // 尝试 AI 分类（使用 prompt 增强）
        let ai_config = crate::ai::load_ai_config();
        let (category, confidence, extracted, routing) = if ai_config.mode != "noop" {
            let rt = tokio::runtime::Runtime::new().unwrap();
            match rt.block_on(crate::ai::process_inbox_with_ai(&content_text)) {
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

        Ok(serde_json::json!({
            "category": category,
            "confidence": confidence,
            "suggestedCaseId": suggested_case_id,
            "caseNo": case_no,
            "extracted": extracted,
            "routeActions": route_actions.unwrap_or_default(),
        }))
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
        "legal_provision" => {
            match auto_import_legal_provisions(conn, content_text, inbox_id) {
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
            }
        }

        // ── 节假日通知 → 解析并提示确认 ──
        "holiday_notice" => {
            match parse_holiday_dates(content_text) {
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
            }
        }

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
                conn, "案由规定更新", "cause_action", content_text,
                inbox_id, suggested_case_id,
            );
        }

        // ── 笔记/其他 → 写入知识库 ──
        "note" | "client_instruction" | "correspondence" => {
            match insert_knowledge_item(
                conn,
                &format!("收件箱笔记: {}", &content_text[..content_text.len().min(50)]),
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
        let tags = serde_json::to_string(&serde_json::json!([
            law_name, article_no, "法条"
        ]))
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

/// 解析节假日日期
fn parse_holiday_dates(content: &str) -> Result<serde_json::Value, String> {
    let year_re = regex::Regex::new(r"(\d{4})\s*年").unwrap();
    let year = year_re
        .captures(content)
        .and_then(|c| c[1].parse::<i32>().ok())
        .unwrap_or(chrono::Local::now().year());

    let date_re = regex::Regex::new(r"(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap();
    let mut holidays = Vec::new();
    for caps in date_re.captures_iter(content) {
        let month = caps[1].parse::<u32>().unwrap_or(1);
        let day = caps[2].parse::<u32>().unwrap_or(1);
        holidays.push(format!("{:04}-{:02}-{:02}", year, month, day));
    }

    let workday_re = regex::Regex::new(r"(\d{1,2})\s*月\s*(\d{1,2})\s*日[^，。]*上班").unwrap();
    let mut workdays = Vec::new();
    for caps in workday_re.captures_iter(content) {
        let month = caps[1].parse::<u32>().unwrap_or(1);
        let day = caps[2].parse::<u32>().unwrap_or(1);
        workdays.push(format!("{:04}-{:02}-{:02}", year, month, day));
    }

    Ok(serde_json::json!({
        "year": year,
        "holidays": holidays,
        "workdays": workdays,
    }))
}

/// 从 extracted 中提取日期字段
fn extract_date_from_extracted(extracted: Option<&serde_json::Value>, field: &str) -> Option<String> {
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
                                .map(|s| s.contains(field) || s.contains("开庭") || s.contains("口审"))
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
        ).map_err(|e| e.to_string())?;
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
                ).map_err(|e| e.to_string())?;
                updated.push("verdict_type".to_string());
            }
        }

        // 更新案件结果
        if let Some(result) = extracted.get("result").and_then(|v| v.as_str()) {
            conn.execute(
                "UPDATE cases SET case_result = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![result, db::now_local(), case_id],
            ).map_err(|e| e.to_string())?;
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
    let today = chrono::Local::now().naive_local().date().format("%Y-%m-%d").to_string();
    conn.execute(
        "UPDATE cases SET complaint_received_date = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![today, now, case_id],
    ).map_err(|e| e.to_string())?;
    updated.push("complaint_received_date".to_string());

    // 更新法院
    if let Some(extracted) = extracted {
        if let Some(court) = extracted.get("court").and_then(|v| v.as_str()) {
            conn.execute(
                "UPDATE cases SET court = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![court, now, case_id],
            ).map_err(|e| e.to_string())?;
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
            ).map_err(|e| e.to_string())?;
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

        // 更新收件项状态
        conn.execute(
            "UPDATE inbox_items SET status = 'filed', linked_case_id = ?1,
             filed_as = ?2, processed_at = ?3 WHERE id = ?4",
            rusqlite::params![case_id, category, db::now_local(), item_id],
        )?;

        // 获取收件项信息
        let title: String = conn
            .query_row(
                "SELECT COALESCE(title, '') FROM inbox_items WHERE id = ?1",
                rusqlite::params![item_id],
                |r| r.get(0),
            )
            .unwrap_or_default();

        // 记录办案日志
        let _ = conn.execute(
            "INSERT INTO case_logs (id, case_id, event_summary, event_type, event_date, created_at)
             VALUES (?1, ?2, ?3, 'record', ?4, ?4)",
            rusqlite::params![
                db::new_id(),
                case_id,
                format!("归档收件: {}", title),
                db::today(),
            ],
        );

        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn dismiss_inbox_item(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "UPDATE inbox_items SET status = 'dismissed', processed_at = ?1 WHERE id = ?2",
            rusqlite::params![db::now_local(), id],
        )?;
        Ok(())
    })
    .await
}

/// 解析节假日通知并更新日历
#[tauri::command]
pub async fn parse_holiday_notice(content: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        parse_holiday_dates(&content).map_err(|e| anyhow::anyhow!(e))
    })
    .await
}

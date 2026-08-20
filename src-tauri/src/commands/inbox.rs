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

        // 获取收件项信息
        let (title, source_path): (String, Option<String>) = conn
            .query_row(
                "SELECT COALESCE(title, ''), source_path FROM inbox_items WHERE id = ?1",
                rusqlite::params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| anyhow::anyhow!("收件项不存在: {}", e))?;

        // 获取案件信息
        let case = db::cases::get_case(&conn, &case_id)
            .map_err(|e| anyhow::anyhow!("案件不存在: {}", e))?;

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

// ── v2.1: 即时判断 + 安全拷贝 + AI 缓存 ──────────────────────

/// 即时判断结果
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickJudgeResult {
    pub category: String,
    pub confidence: f32,
    pub strength: String,          // "strong" / "candidate" / "fallback"
    pub recommendations: Vec<QuickRecommendation>,
    pub ai_available: bool,
    pub ai_analyzed: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickRecommendation {
    /// 动作类型：file_to_case | create_task | create_deadline | save_knowledge | create_case | set_reminder
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
    pub matched_case_id: Option<String>,
    pub matched_case_name: Option<String>,
}

/// 检测文本中是否包含法院送达链接（占位实现）
fn detect_service_delivery(text: &str) -> Option<ServiceDeliveryInfo> {
    // 检测 zxfw.court.gov.cn 链接
    if !text.contains("zxfw.court.gov.cn") {
        return None;
    }
    // 提取案号（简单正则）
    let case_no_re = regex::Regex::new(r"[(（]d{4}[）)].{2,20}号").ok()?;
    let case_no = case_no_re.find(text).map(|m| m.as_str().to_string()).unwrap_or_default();
    
    Some(ServiceDeliveryInfo {
        case_no,
        service_url: "https://zxfw.court.gov.cn".to_string(),
        recipient_name: "".to_string(),
        matched_case_id: None,
        matched_case_name: None,
    })
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
            .map_err(|_| anyhow::anyhow!("收件项不存在"))?;

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
    let mut matches: std::collections::HashMap<String, (String, Vec<String>)> = std::collections::HashMap::new();

    // 1. 案号提取（最高权重信号）
    if let Some(cn) = extract_case_no_from_name(file_name) {
        if let Ok(case_id) = conn.query_row(
            "SELECT id FROM cases WHERE case_no LIKE ?1 LIMIT 1",
            rusqlite::params![format!("%{}%", cn)],
            |r| r.get::<_, String>(0),
        ) {
            let case_name: String = conn.query_row(
                "SELECT COALESCE(display_name, case_name) FROM cases WHERE id = ?1",
                rusqlite::params![case_id],
                |r| r.get(0),
            ).unwrap_or_default();
            matches.entry(case_id.clone())
                .or_insert_with(|| (case_name, vec![]))
                .1.push(format!("文件名包含案号 {}", cn));
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
        for case_result in case_iter {
            if let Ok((case_id, case_name)) = case_result {
                matches.entry(case_id)
                    .or_insert_with(|| (case_name, vec![]))
                    .1.push(format!("文件名包含当事人 {}", party));
            }
        }
    }

    // 3. 置信度 = 信号加权，封顶 0.95
    let mut confidence: f32 = 0.0;
    if category != "other" { confidence += 0.3; }
    if matches.values().any(|(_, r)| r.iter().any(|s| s.contains("案号"))) { confidence += 0.4; }
    if matches.values().any(|(_, r)| r.iter().any(|s| s.contains("当事人"))) { confidence += 0.2; }
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
/// 意图优先级：期限 > 任务 > 提醒 > 知识 > 新案件 > 兜底
/// 推荐动作：create_deadline / create_task / set_reminder / save_knowledge / create_case
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
            let case_name: String = conn.query_row(
                "SELECT COALESCE(display_name, case_name) FROM cases WHERE id = ?1",
                rusqlite::params![case_id],
                |r| r.get(0),
            ).unwrap_or_default();
            matched_case = Some((case_id, case_name));
        }
    }
    if matched_case.is_none() {
        for party in extract_parties_from_name(text) {
            if let Ok((case_id, case_name)) = conn.query_row(
                "SELECT id, COALESCE(display_name, case_name) FROM cases WHERE client_name LIKE ?1 OR opponent_name LIKE ?1 LIMIT 1",
                rusqlite::params![format!("%{}%", party)],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            ) {
                matched_case = Some((case_id, case_name));
                break;
            }
        }
    }

    let due = extract_date_hint(text);
    let mut intent_base = |action: &str, name: &str, reason: &str| -> QuickRecommendation {
        let mut intent = serde_json::json!({ "name": truncate_text(text, 60) });
        if let Some(d) = &due { intent["dueDate"] = serde_json::Value::String(d.clone()); }
        if let Some(c) = &matched_case { intent["caseId"] = serde_json::Value::String(c.0.clone()); }
        QuickRecommendation {
            action: action.to_string(),
            target_case_id: matched_case.as_ref().map(|c| c.0.clone()),
            target_case_name: matched_case.as_ref().map(|c| c.1.clone()),
            target_folder: None,
            intent: Some(intent),
            reason: reason.to_string(),
        }
    };

    // 1) 期限意图
    if ["截止", "到期", "期限", "届满", "失效", "最后一天", "提交日"].iter().any(|w| text.contains(w)) {
        recommendations.push(intent_base("create_deadline", "", "文本含期限词（截止/到期/期限）"));
    }

    // 2) 任务意图
    if ["需要", "要做", "尽快", "别忘了", "安排", "完成", "准备"].iter().any(|w| text.contains(w)) {
        let mut rec = intent_base("create_task", "", "文本含行动词（需要/要做/尽快）");
        if let Some(intent) = rec.intent.as_mut() {
            intent["taskName"] = serde_json::Value::String(truncate_text(text, 60));
        }
        recommendations.push(rec);
    }

    // 3) 提醒意图
    if ["提醒", "记得"].iter().any(|w| text.contains(w)) {
        let mut rec = intent_base("set_reminder", "", "文本含提醒词（提醒/记得）");
        if let Some(intent) = rec.intent.as_mut() {
            intent["title"] = serde_json::Value::String(truncate_text(text, 60));
            if let Some(d) = &due { intent["remindAt"] = serde_json::Value::String(d.clone()); }
        }
        recommendations.push(rec);
    }

    // 3.5) 法院送达短信意图（zxfw.court.gov.cn 链接 → 抓取送达文书）
    if let Some(delivery) = detect_service_delivery(text) {
        recommendations.push(QuickRecommendation {
            action: "service_delivery".to_string(),
            target_case_id: delivery.matched_case_id.clone(),
            target_case_name: delivery.matched_case_name.clone(),
            target_folder: None,
            intent: Some(serde_json::json!({
                "caseNo": delivery.case_no,
                "serviceUrl": delivery.service_url,
                "recipientName": delivery.recipient_name,
            })),
            reason: "检测到法院送达短信链接（zxfw.court.gov.cn）".to_string(),
        });
    }

    // 4) 知识意图
    if ["笔记", "参考", "资料", "心得", "总结", "整理", "备忘", "要点"].iter().any(|w| text.contains(w)) {
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

    // 5) 新案件意图
    if ["收案", "委托", "新案件", "代理", "接案"].iter().any(|w| text.contains(w)) {
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

    // 6) 兜底：关联案件 → 转任务；否则 → 存知识
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
    let explicit = matches!(primary.as_str(), "create_deadline" | "create_task" | "set_reminder" | "create_case");
    let confidence: f32 = if matched_case.is_some() || recommendations.len() >= 2 { 0.85 } else if explicit { 0.72 } else { 0.4 };
    let strength = if confidence >= 0.7 { "strong" } else { "candidate" };

    Ok(QuickJudgeResult {
        category: primary,
        confidence,
        strength: strength.to_string(),
        recommendations,
        ai_available: true,
        ai_analyzed: false,
    })
}

/// 从文本提取日期提示（YYYY-MM-DD / MM-DD / 明天 / 后天）
fn extract_date_hint(text: &str) -> Option<String> {
    use chrono::{Duration, Local};
    // YYYY-MM-DD / YYYY/M/D
    if let Ok(full) = regex::Regex::new(r"\d{4}[-/]\d{1,2}[-/]\d{1,2}") {
        if let Some(m) = full.find(text) {
            return normalize_date(m.as_str());
        }
    }
    // MM-DD
    if let Ok(md) = regex::Regex::new(r"\d{1,2}[-/]\d{1,2}") {
        if let Some(m) = md.find(text) {
            let parts: Vec<&str> = m.as_str().split(['-', '/']).collect();
            if parts.len() == 2 {
                if let (Ok(mm), Ok(dd)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    let now = Local::now();
                    let this_year = chrono::NaiveDate::from_ymd_opt(now.year(), mm, dd);
                    if let Some(date) = this_year {
                        if date >= now.date_naive() {
                            return Some(date.format("%Y-%m-%d").to_string());
                        }
                        if let Some(next) = chrono::NaiveDate::from_ymd_opt(now.year() + 1, mm, dd) {
                            return Some(next.format("%Y-%m-%d").to_string());
                        }
                    }
                }
            }
        }
    }
    if text.contains("明天") {
        return Some((Local::now().date_naive() + Duration::days(1)).format("%Y-%m-%d").to_string());
    }
    if text.contains("后天") {
        return Some((Local::now().date_naive() + Duration::days(2)).format("%Y-%m-%d").to_string());
    }
    None
}

/// 规范化日期字符串为 YYYY-MM-DD
fn normalize_date(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split(['-', '/']).collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
            if let Some(dt) = chrono::NaiveDate::from_ymd_opt(y, m, d) {
                return Some(dt.format("%Y-%m-%d").to_string());
            }
        }
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


/// 从文件名提取案号
fn extract_case_no_from_name(file_name: &str) -> Option<String> {
    let re = regex::Regex::new(r"[（(]\s*\d{4}\s*[）)].*?号").ok()?;
    re.find(file_name).map(|m| m.as_str().to_string())
}

/// 从文件名提取当事人名（简单规则：中文字符连续出现 > 2 字）
fn extract_parties_from_name(file_name: &str) -> Vec<String> {
    let re = regex::Regex::new(r"[\u{4e00}-\u{9fff}]{2,8}").unwrap();
    // 过滤掉常见非当事人词
    let stop_words: std::collections::HashSet<&str> = [
        "传票", "判决", "裁定", "决定", "起诉", "答辩", "证据", "通知书",
        "口审", "函件", "文件", "扫描", "复印件", "原件", "副本",
    ].iter().copied().collect();

    re.find_iter(file_name)
        .map(|m| m.as_str().to_string())
        .filter(|w| !stop_words.contains(w.as_str()))
        .collect()
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

/// 安全拷贝：按文件大小分流（§3.5）
#[tauri::command]
pub async fn copy_file_with_progress(
    source_path: String,
    target_case_id: String,
    target_category: String,
    _app: tauri::AppHandle,
) -> Result<String, String> {
    run_blocking(move || {
        use sha2::Digest;

        let source = std::path::Path::new(&source_path);
        let meta = std::fs::metadata(source)
            .map_err(|e| anyhow::anyhow!("无法读取文件: {}", e))?;
        let file_size = meta.len();

        // 读取案件 folder_name，回退到 case_name
        let conn = db::open_db()?;
        let folder_name: String = conn.query_row(
            "SELECT COALESCE(folder_name, case_name, id) FROM cases WHERE id = ?1",
            rusqlite::params![target_case_id],
            |r| r.get(0),
        ).map_err(|_| anyhow::anyhow!("案件不存在"))?;

        let cases_root = dirs::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Casy")
            .join("cases")
            .join(&folder_name)
            .join(&target_category);

        std::fs::create_dir_all(&cases_root)?;

        // 生成目标文件名（处理已存在）
        let file_stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mut target_name = if ext.is_empty() {
            file_stem.to_string()
        } else {
            format!("{}.{}", file_stem, ext)
        };
        let mut target = cases_root.join(&target_name);
        let mut counter = 1u32;
        while target.exists() {
            target_name = if ext.is_empty() {
                format!("{}_{}", file_stem, counter)
            } else {
                format!("{}_{}.{}", file_stem, counter, ext)
            };
            target = cases_root.join(&target_name);
            counter += 1;
        }

        // ── 快速路径：< 10MB，直接 OS 拷贝 + 大小校验 ──
        if file_size < 10 * 1024 * 1024 {
            std::fs::copy(source, &target)?;
            let copied_size = std::fs::metadata(&target)?.len();
            if copied_size != file_size {
                return Err(anyhow::anyhow!("拷贝后大小不一致"));
            }
        } else {
            // 大文件：分块拷贝
            use std::io::{Read, Write};
            let mut src = std::fs::File::open(source)?;
            let mut dst = std::fs::File::create(&target)?;
            let mut buf = vec![0u8; 8192];
            loop {
                let n = src.read(&mut buf)?;
                if n == 0 { break; }
                dst.write_all(&buf[..n])?;
            }
        }

        // 计算 SHA256
        let mut file = std::fs::File::open(&target)?;
        let mut hasher = sha2::Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let hash = format!("{:x}", hasher.finalize());

        Ok(target.to_string_lossy().to_string())
    })
    .await
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

// ═══════════════════════════════════════════════════════════
// 以下函数为占位实现（设计哲学路线图功能，待完整实现）
// ═══════════════════════════════════════════════════════════

/// 确认收件箱推荐动作（设计哲学 §10：捕获→厘清→执行闭环）
#[tauri::command]
pub async fn confirm_inbox_action(
    inbox_item_id: String,
    action: String,
    target_case_id: Option<String>,
    target_category: Option<String>,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let now = db::now_local();
        
        // 获取收件箱项信息
        let (content_text, source_type): (String, String) = conn.query_row(
            "SELECT content_text, source_type FROM inbox_items WHERE id = ?1",
            rusqlite::params![inbox_item_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|_| anyhow::anyhow!("收件箱项不存在"))?;
        
        // 根据 action 执行对应操作
        let result = match action.as_str() {
            "file_to_case" => {
                // 归档到案件文件夹
                if let (Some(case_id), Some(category)) = (&target_case_id, &target_category) {
                    // 更新 inbox_item 状态
                    conn.execute(
                        "UPDATE inbox_items SET status = 'processed', linked_case_id = ?1, user_category = ?2, processed_at = ?3 WHERE id = ?4",
                        rusqlite::params![case_id, category, &now, &inbox_item_id],
                    )?;
                    serde_json::json!({"success": true, "action": "filed", "caseId": case_id, "category": category})
                } else {
                    serde_json::json!({"success": false, "error": "缺少案件ID或分类"})
                }
            }
            "create_task" => {
                // 创建任务
                let task_id = db::new_id();
                conn.execute(
                    "INSERT INTO tasks (id, task_name, case_id, created_date, task_type, start_bucket, created_at) VALUES (?1, ?2, ?3, ?4, 'action', 'inbox', ?5)",
                    rusqlite::params![&task_id, &content_text, target_case_id.as_ref().unwrap_or(&String::new()), &now, &now],
                )?;
                // 更新 inbox_item
                conn.execute(
                    "UPDATE inbox_items SET status = 'processed', processed_at = ?1 WHERE id = ?2",
                    rusqlite::params![&now, &inbox_item_id],
                )?;
                serde_json::json!({"success": true, "action": "task_created", "taskId": task_id})
            }
            "save_knowledge" => {
                // 保存到知识库
                let knowledge_id = db::new_id();
                conn.execute(
                    "INSERT INTO knowledge_items (id, title, content, category, created_at) VALUES (?1, ?2, ?3, 'reference', ?4)",
                    rusqlite::params![&knowledge_id, &content_text, &content_text, &now],
                )?;
                conn.execute(
                    "UPDATE inbox_items SET status = 'processed', processed_at = ?1 WHERE id = ?2",
                    rusqlite::params![&now, &inbox_item_id],
                )?;
                serde_json::json!({"success": true, "action": "knowledge_saved", "knowledgeId": knowledge_id})
            }
            "set_reminder" => {
                // 设置提醒
                conn.execute(
                    "UPDATE inbox_items SET status = 'processed', processed_at = ?1 WHERE id = ?2",
                    rusqlite::params![&now, &inbox_item_id],
                )?;
                serde_json::json!({"success": true, "action": "reminder_set"})
            }
            "create_case" => {
                // 创建新案件
                let case_id = db::new_id();
                conn.execute(
                    "INSERT INTO cases (id, case_name, client_name, opponent_name, created_at) VALUES (?1, ?2, '', '', ?3)",
                    rusqlite::params![&case_id, &content_text, &now],
                )?;
                conn.execute(
                    "UPDATE inbox_items SET status = 'processed', processed_at = ?1 WHERE id = ?2",
                    rusqlite::params![&now, &inbox_item_id],
                )?;
                serde_json::json!({"success": true, "action": "case_created", "caseId": case_id})
            }
            "ignore" | "dismiss" => {
                conn.execute(
                    "UPDATE inbox_items SET status = 'ignored', processed_at = ?1 WHERE id = ?2",
                    rusqlite::params![&now, &inbox_item_id],
                )?;
                serde_json::json!({"success": true, "action": "ignored"})
            }
            _ => {
                serde_json::json!({"success": false, "error": "未知动作"})
            }
        };
        
        // 记录处理历史到 inbox_feedback（设计哲学 §10：反馈学习）
        let feedback_id = db::new_id();
        conn.execute(
            "INSERT INTO inbox_feedback (id, inbox_item_id, action, intent_json, accepted, rejected_at) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            rusqlite::params![
                &feedback_id,
                &inbox_item_id,
                &action,
                serde_json::json!({"targetCaseId": target_case_id, "targetCategory": target_category}).to_string(),
                &now,
            ],
        )?;
        
        Ok(result)
    })
    .await
}

/// AI 分析收件箱项（占位）
#[tauri::command]
pub async fn ai_analyze_inbox_item(_id: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"cached": true, "intent": {}}))
}

/// 下载送达文书（占位）
#[tauri::command]
pub async fn download_service_delivery(
    _case_no: String,
    _recipient_name: String,
) -> Result<String, String> {
    Err("送达文书抓取功能待实现".to_string())
}

/// 处理送达文书（占位）
#[tauri::command]
pub async fn process_service_delivery(
    _inbox_item_id: String,
) -> Result<(), String> {
    Ok(())
}

/// 捕获屏幕截图到收件箱（占位）
#[tauri::command]
pub async fn capture_screenshot() -> Result<String, String> {
    Err("截图功能待实现".to_string())
}

/// 捕获剪贴板内容到收件箱（占位）
#[tauri::command]
pub async fn capture_clipboard() -> Result<String, String> {
    Err("剪贴板捕获功能待实现".to_string())
}

/// 启动剪贴板监听（占位）
#[tauri::command]
pub async fn start_clipboard_monitor() -> Result<(), String> {
    Ok(())
}

/// 保存语音速记（占位）
#[tauri::command]
pub async fn save_voice_note(
    _audio_data: Vec<u8>,
    _duration_seconds: i32,
) -> Result<String, String> {
    Err("语音速记功能待实现".to_string())
}

/// 语音转写（占位）
#[tauri::command]
pub async fn transcribe_voice_note(_voice_note_id: String) -> Result<String, String> {
    Err("语音转写功能待实现".to_string())
}

/// 启动收件箱批量处理（占位）
#[tauri::command]
pub async fn start_inbox_batch() -> Result<(), String> {
    Ok(())
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
pub async fn get_inbox_progress() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"total": 0, "processed": 0, "pending": 0}))
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

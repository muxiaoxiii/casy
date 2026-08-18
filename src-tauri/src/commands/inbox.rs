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
    pub action: String,
    pub target_case_id: Option<String>,
    pub target_case_name: Option<String>,
    pub target_folder: Option<String>,
    pub reason: String,
}

/// 即时判断命令（纯本地，0ms）
#[tauri::command]
pub async fn quick_judge_inbox_item(id: String) -> Result<QuickJudgeResult, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 读取收件项
        let (title, source_path, ai_analyzed, ai_extracted): (Option<String>, Option<String>, i32, Option<String>) = conn
            .query_row(
                "SELECT title, source_path, ai_analyzed, ai_extracted FROM inbox_items WHERE id = ?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|_| anyhow::anyhow!("收件项不存在"))?;

        let file_name = title.as_deref().unwrap_or("");
        let file_size: u64 = source_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0);

        let result = quick_judge(&conn, file_name, file_size, ai_analyzed != 0, ai_extracted.as_deref())?;

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
    app: tauri::AppHandle,
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
            std::fs::copy(source, &target)
                .map_err(|e| anyhow::anyhow!("拷贝失败: {}", e))?;

            if std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0) != file_size {
                let _ = std::fs::remove_file(&target);
                return Err(anyhow::anyhow!("文件校验失败（大小不一致）").into());
            }

            let file_id = record_file(&conn, &target_case_id, &target, &target_category, source)?;
            return Ok(file_id);
        }

        // ── 标准路径：>= 10MB，流式拷贝 + 目标哈希 + 后台校验 ──
        let tmp_target = target.with_extension(format!(
            "{}.tmp",
            target.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));

        let mut src_file = std::fs::File::open(source)?;
        let mut dst_file = std::fs::File::create(&tmp_target)?;
        let mut hasher = sha2::Sha256::new();
        let mut copied: u64 = 0;
        let mut last_emit = std::time::Instant::now();
        let block_size = 1_048_576usize; // 1MB
        let mut buffer = vec![0u8; block_size];

        loop {
            let bytes_read = std::io::Read::read(&mut src_file, &mut buffer)?;
            if bytes_read == 0 { break; }
            std::io::Write::write_all(&mut dst_file, &buffer[..bytes_read])?;
            hasher.update(&buffer[..bytes_read]);
            copied += bytes_read as u64;

            // 进度节流：每 1% 或每 200ms
            let pct_now = (copied * 100 / file_size) as u32;
            let pct_prev = ((copied - bytes_read as u64) * 100 / file_size) as u32;
            if pct_now > pct_prev || last_emit.elapsed() > std::time::Duration::from_millis(200) {
                let _ = tauri::Emitter::emit(&app, "file-copy-progress", serde_json::json!({
                    "copied": copied,
                    "total": file_size,
                    "percent": pct_now,
                }));
                last_emit = std::time::Instant::now();
            }
        }
        std::io::Write::flush(&mut dst_file)?;
        drop(dst_file);

        std::fs::rename(&tmp_target, &target)?;
        let file_id = record_file(&conn, &target_case_id, &target, &target_category, source)?;

        // 后台异步校验源哈希
        let dst_hash = hasher.finalize();
        let src_path = source.to_path_buf();
        let case_id_clone = target_case_id.clone();
        let file_id_clone = file_id.clone();
        tauri::async_runtime::spawn(async move {
            match compute_sha256(&src_path) {
                Ok(src_hash) if src_hash[..] == dst_hash[..] => { /* 校验通过 */ }
                Ok(_) => {
                    let _ = tauri::Emitter::emit(&app, "file-verify-failed", serde_json::json!({
                        "file_id": file_id_clone, "case_id": case_id_clone,
                        "msg": "文件校验失败，建议重新归档"
                    }));
                }
                Err(_) => {}
            }
        });

        Ok(file_id)
    })
    .await
}

/// 记录文件到 case_files 表
fn record_file(
    conn: &rusqlite::Connection,
    case_id: &str,
    target: &std::path::Path,
    category: &str,
    source: &std::path::Path,
) -> anyhow::Result<String> {
    let file_id = db::new_id();
    let file_name = target.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let file_size = std::fs::metadata(target).map(|m| m.len() as i64).ok();
    let ext = target.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();

    conn.execute(
        "INSERT INTO case_files (id, case_id, file_name, file_path, file_size, file_type, category, source_type, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'inbox', ?8, ?8)",
        rusqlite::params![
            file_id, case_id, file_name,
            target.to_string_lossy(),
            file_size, ext, category,
            db::now_local(),
        ],
    )?;

    // 写办案日志
    conn.execute(
        "INSERT INTO case_logs (id, case_id, event_summary, event_type, event_date, created_at)
         VALUES (?1, ?2, ?3, 'record', ?4, ?4)",
        rusqlite::params![
            db::new_id(), case_id,
            format!("归档文件: {}", source.file_name().and_then(|s| s.to_str()).unwrap_or("")),
            db::today(),
        ],
    )?;

    Ok(file_id)
}

/// 计算文件 SHA-256
fn compute_sha256(path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    use sha2::Digest;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 1_048_576];
    loop {
        let n = std::io::Read::read(&mut file, &mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_vec())
}

/// 用户确认推荐 → 执行归档（§6.2）
#[tauri::command]
pub async fn confirm_inbox_action(
    inbox_item_id: String,
    target_case_id: String,
    target_category: String,
    _app: tauri::AppHandle,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 获取收件项源文件路径
        let source_path: Option<String> = conn.query_row(
            "SELECT source_path FROM inbox_items WHERE id = ?1",
            rusqlite::params![inbox_item_id],
            |r| r.get(0),
        ).map_err(|_| anyhow::anyhow!("收件项不存在"))?;

        let source = source_path.ok_or_else(|| anyhow::anyhow!("收件项无源文件路径"))?;

        // 执行安全拷贝（复用逻辑）
        let source_path_obj = std::path::Path::new(&source);
        let meta = std::fs::metadata(source_path_obj)
            .map_err(|e| anyhow::anyhow!("无法读取文件: {}", e))?;
        let file_size = meta.len();

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

        let file_stem = source_path_obj.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = source_path_obj.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mut target_name = if ext.is_empty() { file_stem.to_string() } else { format!("{}.{}", file_stem, ext) };
        let mut target = cases_root.join(&target_name);
        let mut counter = 1u32;
        while target.exists() {
            target_name = if ext.is_empty() { format!("{}_{}", file_stem, counter) } else { format!("{}_{}.{}", file_stem, counter, ext) };
            target = cases_root.join(&target_name);
            counter += 1;
        }

        // 快速路径
        if file_size < 10 * 1024 * 1024 {
            std::fs::copy(source_path_obj, &target)?;
        } else {
            std::fs::copy(source_path_obj, &target)?;
            // 大文件也先走简单拷贝，后台校验在 copy_file_with_progress 中处理
        }

        let file_id = record_file(&conn, &target_case_id, &target, &target_category, source_path_obj)?;

        // 更新 inbox 状态为 filed
        conn.execute(
            "UPDATE inbox_items SET status = 'filed', linked_case_id = ?1, filed_to = ?2, filed_as = ?3, processed_at = ?4 WHERE id = ?5",
            rusqlite::params![
                target_case_id,
                target.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
                target_name,
                db::now_local(),
                inbox_item_id,
            ],
        )?;

        // 写入推荐记录
        let rec_id = db::new_id();
        conn.execute(
            "INSERT INTO inbox_recommendations (id, inbox_item_id, action, target_case_id, target_folder, reason, confidence, accepted)
             VALUES (?1, ?2, 'file_to_case', ?3, ?4, '用户确认归档', 1.0, 1)",
            rusqlite::params![rec_id, inbox_item_id, target_case_id, target_category],
        )?;

        Ok(file_id)
    })
    .await
}

/// AI 分析（带缓存：ai_analyzed=1 直接返回缓存结果）
#[tauri::command]
pub async fn ai_analyze_inbox_item(id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 检查缓存
        let (ai_analyzed, ai_extracted, content_text): (i32, Option<String>, String) = conn.query_row(
            "SELECT ai_analyzed, ai_extracted, COALESCE(content_text, '') FROM inbox_items WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).map_err(|_| anyhow::anyhow!("收件项不存在"))?;

        // 如果已分析过，直接返回缓存
        if ai_analyzed == 1 && ai_extracted.is_some() {
            let cached: serde_json::Value = serde_json::from_str(&ai_extracted.unwrap_or_default())
                .unwrap_or(serde_json::json!({}));
            return Ok(serde_json::json!({
                "cached": true,
                "result": cached,
                "message": "已分析过，结果如下（缓存）"
            }));
        }

        // 新的 AI 分析
        let ai_config = crate::ai::load_ai_config();
        let (category, confidence, extracted) = if ai_config.mode != "noop" {
            let rt = tokio::runtime::Runtime::new().unwrap();
            match rt.block_on(crate::ai::process_inbox_with_ai(&content_text)) {
                Ok((result, _routing)) => {
                    let extracted = result.extracted_info.clone();
                    (result.category, result.confidence, extracted)
                }
                Err(e) => {
                    log::warn!("AI 分析失败: {}", e);
                    let parsed = parse::classify_document(&content_text);
                    let extracted = serde_json::to_value(&parsed).ok();
                    (parsed.doc_type, parsed.confidence, extracted)
                }
            }
        } else {
            let parsed = parse::classify_document(&content_text);
            let extracted = serde_json::to_value(&parsed).ok();
            (parsed.doc_type, parsed.confidence, extracted)
        };

        let extracted_str = extracted.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default());

        // 缓存结果
        conn.execute(
            "UPDATE inbox_items SET ai_category = ?1, ai_confidence = ?2, ai_extracted = ?3, ai_analyzed = 1, processed_at = ?4 WHERE id = ?5",
            rusqlite::params![category, confidence, extracted_str, db::now_local(), id],
        )?;

        Ok(serde_json::json!({
            "cached": false,
            "category": category,
            "confidence": confidence,
            "extracted": extracted,
        }))
    })
    .await
}

// ═══════════════════════════════════════════════════════════
// 法院送达文书特性
// ═══════════════════════════════════════════════════════════

use regex::Regex;

/// 送达短信检测结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServiceDeliveryInfo {
    /// 从短信中提取的案号
    pub case_no: String,
    /// 送达平台 URL
    pub service_url: String,
    /// 收件人姓名
    pub recipient_name: String,
    /// 匹配到的本地案件 ID
    pub matched_case_id: Option<String>,
    /// 匹配到的案件名称
    pub matched_case_name: Option<String>,
}

/// 检测文本是否为法院送达短信
/// 
/// 典型格式：
/// "吕晗你好，请查收（2026）京73行初6803号案件中你的送达文书
///  点击链接查阅：https://zxfw.court.gov.cn/zxfw/..."
pub fn detect_service_delivery(text: &str) -> Option<ServiceDeliveryInfo> {
    // 检测送达平台链接
    let url_re = Regex::new(r"https?://zxfw\.court\.gov\.cn/zxfw/[^\s]+").ok()?;
    let url_match = url_re.find(text)?;

    // 检测案号（多种格式）
    let case_no_re = Regex::new(r"[（(]\d{4}[）)][\u4e00-\u9fff]+\d+号").ok()?;
    let case_no = case_no_re.find(text)?.as_str().to_string();

    // 检测收件人姓名（"XX你好" 模式）
    let name_re = Regex::new(r"([\u4e00-\u9fff]{2,4})你好").ok()?;
    let recipient_name = name_re
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    // 检测是否包含送达关键词
    let has_delivery_keyword = text.contains("送达") || text.contains("查收") || text.contains("文书");

    if has_delivery_keyword {
        Some(ServiceDeliveryInfo {
            case_no,
            service_url: url_match.as_str().to_string(),
            recipient_name,
            matched_case_id: None,
            matched_case_name: None,
        })
    } else {
        None
    }
}

/// 在数据库中匹配案号，填充 matched_case_id
pub fn match_service_delivery_case(
    conn: &rusqlite::Connection,
    info: &mut ServiceDeliveryInfo,
) {
    // 精确匹配案号
    if let Ok(case_id) = conn.query_row(
        "SELECT id, COALESCE(display_name, case_name) FROM cases WHERE case_no LIKE ?1 LIMIT 1",
        rusqlite::params![format!("%{}%", info.case_no)],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ) {
        info.matched_case_id = Some(case_id.0);
        info.matched_case_name = Some(case_id.1);
    }
}

/// 从送达平台 URL 提取参数
pub fn parse_service_url(url: &str) -> Option<ServiceUrlParams> {
    let qdbh_re = Regex::new(r"qdbh=([a-f0-9]+)").ok()?;
    let sdbh_re = Regex::new(r"sdbh=([a-f0-9]+)").ok()?;
    let sdsin_re = Regex::new(r"sdsin=([a-f0-9]+)").ok()?;

    Some(ServiceUrlParams {
        qdbh: qdbh_re.captures(url)?.get(1)?.as_str().to_string(),
        sdbh: sdbh_re.captures(url)?.get(1)?.as_str().to_string(),
        sdsin: sdsin_re.captures(url)?.get(1)?.as_str().to_string(),
    })
}

#[derive(Debug)]
pub struct ServiceUrlParams {
    pub qdbh: String,
    pub sdbh: String,
    pub sdsin: String,
}

/// 尝试通过送达平台 API 下载文书
/// 
/// 通过送达平台 API 获取文书列表并下载
/// 
/// API: POST https://zxfw.court.gov.cn/yzw/yzw-zxfw-sdfw/api/v1/sdfw/getWsListBySdbhNew
/// 请求体: {"qdbh":"xxx","sdbh":"xxx","sdsin":"xxx"}
/// 返回: data[].c_wsmc（文书名称）、data[].wjlj（OSS 签名下载链接）、data[].c_fymc（法院名称）
/// OSS URL 有效期约 1 小时，获取后应尽快下载
#[tauri::command]
pub async fn download_service_delivery(url: String, _case_id: String) -> Result<serde_json::Value, String> {
    // 1. 从 URL 提取参数
    let params = parse_service_url(&url)
        .ok_or("无法从 URL 提取送达参数（qdbh/sdbh/sdsin）")?;

    // 2. 调用 API 获取文书列表
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let api_resp = client.post("https://zxfw.court.gov.cn/yzw/yzw-zxfw-sdfw/api/v1/sdfw/getWsListBySdbhNew")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "qdbh": params.qdbh,
            "sdbh": params.sdbh,
            "sdsin": params.sdsin,
        }))
        .send()
        .await
        .map_err(|e| format!("请求送达平台 API 失败: {}", e))?;

    let resp_json: serde_json::Value = api_resp.json().await
        .map_err(|e| format!("解析 API 响应失败: {}", e))?;

    let data = resp_json.get("data")
        .and_then(|d| d.as_array())
        .ok_or("API 响应格式异常：缺少 data 字段")?;

    if data.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "message": "送达平台返回空文书列表（可能链接已过期）",
            "documents": [],
        }));
    }

    // 3. 逐个下载 PDF
    let inbox_dir = crate::files::case_folder_base().join("inbox");
    std::fs::create_dir_all(&inbox_dir)
        .map_err(|e| format!("创建收件箱目录失败: {}", e))?;

    let mut downloaded = Vec::new();
    let court_name = data.first()
        .and_then(|d| d.get("c_fymc"))
        .and_then(|v| v.as_str())
        .unwrap_or("未知法院");

    for doc in data {
        let doc_name = doc.get("c_wsmc").and_then(|v| v.as_str()).unwrap_or("未知文书");
        let oss_url = doc.get("wjlj").and_then(|v| v.as_str()).unwrap_or("");
        if oss_url.is_empty() { continue; }

        let safe_name = format!("{}_{}.pdf", doc_name, chrono::Local::now().format("%Y%m%d_%H%M%S"));
        let file_path = inbox_dir.join(&safe_name);

        match client.get(oss_url)
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .header("Referer", "https://zxfw.court.gov.cn/")
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(bytes) = resp.bytes().await {
                    if bytes.len() > 1000 {  // 有效 PDF 至少 1KB
                        let _ = std::fs::write(&file_path, &bytes);
                        downloaded.push(serde_json::json!({
                            "name": doc_name,
                            "fileName": safe_name,
                            "filePath": file_path.to_string_lossy(),
                            "fileSize": bytes.len(),
                        }));
                    }
                }
            }
            Err(_) => continue,
        }
    }

    Ok(serde_json::json!({
        "success": !downloaded.is_empty(),
        "courtName": court_name,
        "totalCount": data.len(),
        "downloadedCount": downloaded.len(),
        "documents": downloaded,
        "message": if downloaded.is_empty() {
            "文书下载失败（OSS 链接可能已过期）".to_string()
        } else {
            format!("已从{}下载 {} 份文书", court_name, downloaded.len())
        },
    }))
}

/// 处理送达短信：识别 → 匹配案件 → 推荐操作
#[tauri::command]
pub async fn process_service_delivery(text: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let mut info = detect_service_delivery(&text)
            .ok_or_else(|| anyhow::anyhow!("未检测到送达文书信息"))?;

        match_service_delivery_case(&conn, &mut info);

        let params = parse_service_url(&info.service_url);

        Ok(serde_json::json!({
            "detected": true,
            "caseNo": info.case_no,
            "recipientName": info.recipient_name,
            "serviceUrl": info.service_url,
            "urlParams": params.as_ref().map(|p| serde_json::json!({
                "qdbh": p.qdbh,
                "sdbh": p.sdbh,
                "sdsin": p.sdsin,
            })),
            "matchedCaseId": info.matched_case_id,
            "matchedCaseName": info.matched_case_name,
            "recommendation": if info.matched_case_id.is_some() {
                serde_json::json!({
                    "action": "download_and_file",
                    "message": format!("检测到送达文书（{}），匹配案件：{}。是否下载并归档？",
                        info.case_no, info.matched_case_name.as_deref().unwrap_or("未知")),
                    "targetCaseId": info.matched_case_id,
                    "targetFolder": "04_收文",
                })
            } else {
                serde_json::json!({
                    "action": "select_case",
                    "message": format!("检测到送达文书（{}），未匹配到本地案件。请手动选择案件。", info.case_no),
                })
            }
        }))
    }).await
}

// ═══════════════════════════════════════════════════════════
// 批量处理队列
// ═══════════════════════════════════════════════════════════

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Semaphore};

/// 处理进度
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingProgress {
    pub total: usize,
    pub processed: usize,
    pub failed: usize,
    pub active: usize,
    pub current_item: Option<String>,
    pub running: bool,
}

/// 队列项
struct QueueItem {
    id: String,
    title: Option<String>,
    source_type: String,
    content_text: Option<String>,
    source_path: Option<String>,
    retry_count: i32,
}

/// 全局处理器实例
static PROCESSOR: std::sync::OnceLock<InboxProcessor> = std::sync::OnceLock::new();

fn get_processor() -> &'static InboxProcessor {
    PROCESSOR.get_or_init(|| InboxProcessor::new())
}

struct InboxProcessor {
    max_concurrency: Arc<AtomicUsize>,
    active_count: Arc<AtomicUsize>,
    processed_count: Arc<AtomicUsize>,
    failed_count: Arc<AtomicUsize>,
    total_count: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    progress_tx: broadcast::Sender<ProcessingProgress>,
}

impl InboxProcessor {
    fn new() -> Self {
        let (progress_tx, _) = broadcast::channel(16);
        Self {
            max_concurrency: Arc::new(AtomicUsize::new(8)),
            active_count: Arc::new(AtomicUsize::new(0)),
            processed_count: Arc::new(AtomicUsize::new(0)),
            failed_count: Arc::new(AtomicUsize::new(0)),
            total_count: Arc::new(AtomicUsize::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            progress_tx,
        }
    }

    fn get_progress(&self) -> ProcessingProgress {
        ProcessingProgress {
            total: self.total_count.load(Ordering::Relaxed),
            processed: self.processed_count.load(Ordering::Relaxed),
            failed: self.failed_count.load(Ordering::Relaxed),
            active: self.active_count.load(Ordering::Relaxed),
            current_item: None,
            running: self.running.load(Ordering::Relaxed),
        }
    }

    fn load_queue(&self) -> anyhow::Result<Vec<QueueItem>> {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, source_type, content_text, source_path, retry_count
             FROM inbox_items
             WHERE status IN ('pending', 'failed')
             ORDER BY
               CASE source_type
                 WHEN 'manual' THEN 1 WHEN 'paste' THEN 2 WHEN 'file' THEN 3
                 WHEN 'email' THEN 4 WHEN 'imap' THEN 5 ELSE 6
               END, created_at ASC"
        )?;
        let items = stmt.query_map([], |row| {
            Ok(QueueItem {
                id: row.get(0)?,
                title: row.get(1)?,
                source_type: row.get(2)?,
                content_text: row.get(3)?,
                source_path: row.get(4)?,
                retry_count: row.get(5)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }

    async fn start_batch(&self) -> anyhow::Result<usize> {
        if self.running.load(Ordering::Relaxed) {
            return Ok(0);
        }
        self.running.store(true, Ordering::Relaxed);
        self.cancel.store(false, Ordering::Relaxed);
        self.paused.store(false, Ordering::Relaxed);
        self.processed_count.store(0, Ordering::Relaxed);
        self.failed_count.store(0, Ordering::Relaxed);

        let queue = self.load_queue()?;
        let total = queue.len();
        self.total_count.store(total, Ordering::Relaxed);

        if total == 0 {
            self.running.store(false, Ordering::Relaxed);
            return Ok(0);
        }

        let semaphore = Arc::new(Semaphore::new(self.max_concurrency.load(Ordering::Relaxed)));
        let mut handles = Vec::new();

        for item in queue {
            if self.cancel.load(Ordering::Relaxed) { break; }
            while self.paused.load(Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                if self.cancel.load(Ordering::Relaxed) { break; }
            }

            let sem = semaphore.clone();
            let active = Arc::new(AtomicBool::new(false)); // placeholder
            let proc_count = self.processed_count.clone();
            let fail_count = self.failed_count.clone();
            let act_count = self.active_count.clone();
            let max_conc = self.max_concurrency.clone();
            let progress_tx = self.progress_tx.clone();
            let total_c = self.total_count.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                act_count.fetch_add(1, Ordering::Relaxed);

                // 标记为处理中
                if let Ok(conn) = db::open_db() {
                    let _ = conn.execute(
                        "UPDATE inbox_items SET status = 'processing', processing_started_at = datetime('now','localtime') WHERE id = ?1",
                        rusqlite::params![item.id],
                    );
                }

                let start = tokio::time::Instant::now();
                let result = process_queue_item(&item).await;
                let elapsed = start.elapsed().as_millis() as u64;

                act_count.fetch_sub(1, Ordering::Relaxed);

                match result {
                    Ok(_) => {
                        proc_count.fetch_add(1, Ordering::Relaxed);
                        if let Ok(conn) = db::open_db() {
                            let _ = conn.execute(
                                "UPDATE inbox_items SET status = 'processed', processed_at = datetime('now','localtime') WHERE id = ?1",
                                rusqlite::params![item.id],
                            );
                        }
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        let retryable = is_retryable(&err_str);
                        if retryable {
                            if let Ok(conn) = db::open_db() {
                                let _ = conn.execute(
                                    "UPDATE inbox_items SET retry_count = retry_count + 1, last_error = ?1, status = 'pending' WHERE id = ?2",
                                    rusqlite::params![err_str, item.id],
                                );
                            }
                        } else {
                            fail_count.fetch_add(1, Ordering::Relaxed);
                            if let Ok(conn) = db::open_db() {
                                let _ = conn.execute(
                                    "UPDATE inbox_items SET status = 'failed', last_error = ?1 WHERE id = ?2",
                                    rusqlite::params![err_str, item.id],
                                );
                            }
                        }
                    }
                }

                // 动态调节并发
                adjust_concurrency(&max_conc, elapsed, false);

                let _ = progress_tx.send(ProcessingProgress {
                    total: total_c.load(Ordering::Relaxed),
                    processed: proc_count.load(Ordering::Relaxed),
                    failed: fail_count.load(Ordering::Relaxed),
                    active: act_count.load(Ordering::Relaxed),
                    current_item: None,
                    running: true,
                });
            });

            handles.push(handle);
        }

        // 后台等待完成
        let running = self.running.clone();
        let progress_tx = self.progress_tx.clone();
        let proc = self.processed_count.clone();
        let fail = self.failed_count.clone();
        let tot = self.total_count.clone();
        let act = self.active_count.clone();

        tokio::spawn(async move {
            for h in handles { let _ = h.await; }
            running.store(false, Ordering::Relaxed);
            let _ = progress_tx.send(ProcessingProgress {
                total: tot.load(Ordering::Relaxed),
                processed: proc.load(Ordering::Relaxed),
                failed: fail.load(Ordering::Relaxed),
                active: act.load(Ordering::Relaxed),
                current_item: None,
                running: false,
            });
        });

        Ok(total)
    }
}

/// 处理单个队列项
async fn process_queue_item(item: &QueueItem) -> anyhow::Result<()> {
    let content = item.content_text.as_deref()
        .ok_or_else(|| anyhow::anyhow!("内容为空"))?;

    let ai_config = crate::ai::load_ai_config();
    if ai_config.mode != "noop" {
        let result = crate::ai::process_inbox_with_ai(content).await;
        match result {
            Ok((ai_result, _)) => {
                let conn = db::open_db()?;
                let extracted = serde_json::to_string(&ai_result.extracted_info).ok();
                conn.execute(
                    "UPDATE inbox_items SET ai_category = ?1, ai_confidence = ?2, ai_extracted = ?3 WHERE id = ?4",
                    rusqlite::params![ai_result.category, ai_result.confidence, extracted, item.id],
                )?;
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("AI 处理失败: {}", e)),
        }
    } else {
        let parsed = parse::classify_document(content);
        let conn = db::open_db()?;
        let extracted = serde_json::to_string(&parsed).ok();
        conn.execute(
            "UPDATE inbox_items SET ai_category = ?1, ai_confidence = ?2, ai_extracted = ?3 WHERE id = ?4",
            rusqlite::params![parsed.doc_type, parsed.confidence, extracted, item.id],
        )?;
        Ok(())
    }
}

fn is_retryable(err: &str) -> bool {
    err.contains("timeout") || err.contains("429") || err.contains("rate")
        || err.contains("network") || err.contains("connection")
        || err.contains("500") || err.contains("502") || err.contains("503")
}

fn adjust_concurrency(max: &AtomicUsize, response_time_ms: u64, is_rate_limited: bool) {
    let current = max.load(Ordering::Relaxed);
    let new_val = if is_rate_limited {
        (current / 2).max(1)
    } else if response_time_ms > 5000 {
        current.saturating_sub(1).max(1)
    } else if response_time_ms < 1000 && current < 8 {
        (current + 1).min(8)
    } else {
        current
    };
    max.store(new_val, Ordering::Relaxed);
}

/// 启动批量处理
#[tauri::command]
pub async fn start_inbox_batch() -> Result<ProcessingProgress, String> {
    let processor = get_processor();
    processor.start_batch().await.map_err(|e| e.to_string())?;
    Ok(processor.get_progress())
}

/// 暂停批量处理
#[tauri::command]
pub async fn pause_inbox_batch() -> Result<(), String> {
    get_processor().paused.store(true, Ordering::Relaxed);
    Ok(())
}

/// 恢复批量处理
#[tauri::command]
pub async fn resume_inbox_batch() -> Result<(), String> {
    get_processor().paused.store(false, Ordering::Relaxed);
    Ok(())
}

/// 取消批量处理
#[tauri::command]
pub async fn cancel_inbox_batch() -> Result<(), String> {
    let p = get_processor();
    p.cancel.store(true, Ordering::Relaxed);
    p.paused.store(false, Ordering::Relaxed);
    Ok(())
}

/// 获取处理进度
#[tauri::command]
pub async fn get_inbox_progress() -> Result<ProcessingProgress, String> {
    Ok(get_processor().get_progress())
}

/// 重试单个失败项
#[tauri::command]
pub async fn retry_inbox_item(id: String) -> Result<(), String> {
    let conn = db::open_db().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE inbox_items SET status = 'pending', last_error = NULL WHERE id = ?1 AND status = 'failed'",
        rusqlite::params![id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// 重试某案件的所有失败项
#[tauri::command]
pub async fn retry_inbox_case(case_id: String) -> Result<usize, String> {
    let conn = db::open_db().map_err(|e| e.to_string())?;
    let count = conn.execute(
        "UPDATE inbox_items SET status = 'pending', last_error = NULL WHERE linked_case_id = ?1 AND status = 'failed'",
        rusqlite::params![case_id],
    ).map_err(|e| e.to_string())?;
    Ok(count)
}

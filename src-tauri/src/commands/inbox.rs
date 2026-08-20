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
    target_case_id: Option<String>,
    target_category: Option<String>,
    action: Option<String>,
    intent: Option<serde_json::Value>,
    _app: tauri::AppHandle,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let now = db::now_local();
        let action = action.unwrap_or_else(|| "file_to_case".to_string());
        let intent = intent.unwrap_or_else(|| serde_json::json!({}));

        // 收件项内容（不同动作读取不同字段）
        let (source_path, content_text): (Option<String>, String) = conn.query_row(
            "SELECT source_path, COALESCE(content_text, '') FROM inbox_items WHERE id = ?1",
            rusqlite::params![inbox_item_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|_| anyhow::anyhow!("收件项不存在"))?;

        let fallback_text = content_text;

        let result_id = match action.as_str() {
            // ── 文件归档（原有行为，向后兼容） ──
            "file_to_case" => {
                let target_case_id = target_case_id.ok_or_else(|| anyhow::anyhow!("归档动作需要目标案件"))?;
                let target_category = target_category.unwrap_or_else(|| "07_其他".to_string());
                let source = source_path.ok_or_else(|| anyhow::anyhow!("收件项无源文件路径"))?;

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

                std::fs::copy(source_path_obj, &target)?;
                let file_id = record_file(&conn, &target_case_id, &target, &target_category, source_path_obj)?;

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

                record_recommendation(&conn, &inbox_item_id, "file_to_case", Some(&target_case_id), Some(&target_category), "用户确认归档", 1.0)?;
                file_id
            }

            // ── 转为任务 ──
            "create_task" => {
                let task_id = insert_task_from_intent(&conn, &intent, &fallback_text, "action", "anytime")?;
                mark_inbox_processed(&conn, &inbox_item_id, "processed", now)?;
                record_recommendation(&conn, &inbox_item_id, "create_task", intent["caseId"].as_str(), None, "用户确认转为任务", 1.0)?;
                task_id
            }

            // ── 记为期限 ──
            "create_deadline" => {
                let task_id = insert_task_from_intent(&conn, &intent, &fallback_text, "action", "anytime")?;
                mark_inbox_processed(&conn, &inbox_item_id, "processed", now)?;
                record_recommendation(&conn, &inbox_item_id, "create_deadline", intent["caseId"].as_str(), None, "用户确认记为期限", 1.0)?;
                task_id
            }

            // ── 设置提醒（今天进入视线） ──
            "set_reminder" => {
                let task_id = insert_task_from_intent(&conn, &intent, &fallback_text, "action", "today")?;
                mark_inbox_processed(&conn, &inbox_item_id, "processed", now)?;
                record_recommendation(&conn, &inbox_item_id, "set_reminder", intent["caseId"].as_str(), None, "用户确认设置提醒", 1.0)?;
                task_id
            }

            // ── 存入知识库 ──
            "save_knowledge" => {
                let knowledge_id = insert_knowledge_from_intent(&conn, &intent, &fallback_text, now.clone())?;
                mark_inbox_processed(&conn, &inbox_item_id, "processed", now)?;
                record_recommendation(&conn, &inbox_item_id, "save_knowledge", None, None, "用户确认存入知识库", 1.0)?;
                knowledge_id
            }

            // ── 新建案件 ──
            "create_case" => {
                let case_id = insert_case_from_intent(&conn, &intent, &fallback_text, now.clone())?;
                mark_inbox_processed(&conn, &inbox_item_id, "processed", now)?;
                record_recommendation(&conn, &inbox_item_id, "create_case", Some(&case_id), None, "用户确认新建案件", 1.0)?;
                case_id
            }

            // ── 抓取送达文书（zxfw 短信链接 → 下载 PDF 到 Casy/inbox） ──
            "service_delivery" => {
                let url = intent["serviceUrl"].as_str().ok_or_else(|| anyhow::anyhow!("送达链接缺失"))?;
                let case_id = intent["caseId"].as_str().or_else(|| intent["matchedCaseId"].as_str()).map(|s| s.to_string());
                let rt = tokio::runtime::Runtime::new()?;
                let download = rt.block_on(download_service_delivery(url.to_string(), case_id.clone().unwrap_or_default()))
                    .map_err(|e| anyhow::anyhow!("下载送达文书失败: {}", e))?;
                // 关联案件（若已匹配）
                if let Some(cid) = &case_id {
                    conn.execute(
                        "UPDATE inbox_items SET linked_case_id = ?1 WHERE id = ?2",
                        rusqlite::params![cid, inbox_item_id],
                    )?;
                }
                mark_inbox_processed(&conn, &inbox_item_id, "processed", now)?;
                record_recommendation(&conn, &inbox_item_id, "service_delivery", case_id.as_deref(), None, "用户确认抓取送达文书", 1.0)?;
                serde_json::to_string(&download).unwrap_or_default()
            }

            _ => return Err(anyhow::anyhow!("未知动作: {}", action).into()),
        };

        Ok(result_id)
    })
    .await
}

/// 根据意图数据插入任务（create_task / create_deadline / set_reminder 共用）
fn insert_task_from_intent(
    conn: &rusqlite::Connection,
    intent: &serde_json::Value,
    fallback_text: &str,
    task_type: &str,
    start_bucket: &str,
) -> anyhow::Result<String> {
    let id = db::new_id();
    let now = db::now_local();
    let task_name = intent["taskName"].as_str()
        .or_else(|| intent["title"].as_str())
        .or_else(|| intent["name"].as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| truncate_text(fallback_text, 60));
    let due = intent["dueDate"].as_str().or_else(|| intent["remindAt"].as_str());
    let case_id = intent["caseId"].as_str();

    conn.execute(
        "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed, assignee, finish_note,
         task_type, start_date, due_date, waiting_for, follow_up_date, context, flagged, sequential, blocked, sequence_order,
         start_bucket, today_index, estimated_minutes, area_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
        rusqlite::params![
            id,
            case_id,
            task_name,
            fallback_text,
            now,
            due,
            "normal",
            "",
            "",
            task_type,
            None::<String>,
            due,
            None::<String>,
            None::<String>,
            None::<String>,
            0,
            0,
            0,
            0,
            start_bucket,
            0,
            None::<i64>,
            None::<String>,
            now,
        ],
    )?;

    conn.execute(
        "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor) VALUES (?1, ?2, 'created', ?3, 'inbox')",
        rusqlite::params![db::new_id(), id, now],
    )?;

    Ok(id)
}

/// 根据意图数据插入知识条目
fn insert_knowledge_from_intent(
    conn: &rusqlite::Connection,
    intent: &serde_json::Value,
    fallback_text: &str,
    now: String,
) -> anyhow::Result<String> {
    let id = db::new_id();
    let title = intent["title"].as_str().map(|s| s.to_string()).unwrap_or_else(|| truncate_text(fallback_text, 40));
    let content = intent["content"].as_str().unwrap_or(fallback_text);
    let category = intent["category"].as_str().unwrap_or("reference");
    let case_id = intent["caseId"].as_str();

    conn.execute(
        "INSERT INTO knowledge_items (id, title, category, content, tags, source_type, source_id,
         linked_case_id, law_name, article_no, effective_date, status, parent_id, block_type, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
        rusqlite::params![
            id, title, category, content, None::<String>, "inbox", None::<String>,
            case_id, None::<String>, None::<String>, None::<String>,
            "active", None::<String>, "page", now,
        ],
    )?;
    Ok(id)
}

/// 根据意图数据创建最小案件
fn insert_case_from_intent(
    conn: &rusqlite::Connection,
    intent: &serde_json::Value,
    fallback_text: &str,
    now: String,
) -> anyhow::Result<String> {
    let id = db::new_id();
    let case_name = intent["caseName"].as_str().map(|s| s.to_string()).unwrap_or_else(|| truncate_text(fallback_text, 40));
    let client_name = intent["clientName"].as_str().unwrap_or("待定");
    let track = intent["track"].as_str().unwrap_or("other");

    conn.execute(
        "INSERT INTO cases (id, track, case_name, client_name, opponent_name, case_status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, '', '进行中', ?5, ?5)",
        rusqlite::params![id, track, case_name, client_name, now],
    )?;
    Ok(id)
}

/// 更新收件项状态
fn mark_inbox_processed(
    conn: &rusqlite::Connection,
    inbox_item_id: &str,
    status: &str,
    now: String,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE inbox_items SET status = ?1, processed_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, inbox_item_id],
    )?;
    Ok(())
}

/// 写入推荐采纳记录
fn record_recommendation(
    conn: &rusqlite::Connection,
    inbox_item_id: &str,
    action: &str,
    case_id: Option<&str>,
    folder: Option<&str>,
    reason: &str,
    confidence: f64,
) -> anyhow::Result<()> {
    let rec_id = db::new_id();
    conn.execute(
        "INSERT INTO inbox_recommendations (id, inbox_item_id, action, target_case_id, target_folder, reason, confidence, accepted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        rusqlite::params![rec_id, inbox_item_id, action, case_id, folder, reason, confidence],
    )?;
    Ok(())
}

/// AI 分析（带缓存：ai_analyzed=1 直接返回缓存结果）
#[tauri::command]
pub async fn ai_analyze_inbox_item(id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 检查缓存
        let (ai_analyzed, ai_extracted, ai_category, content_text): (i32, Option<String>, Option<String>, String) = conn.query_row(
            "SELECT ai_analyzed, ai_extracted, ai_category, COALESCE(content_text, '') FROM inbox_items WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).map_err(|_| anyhow::anyhow!("收件项不存在"))?;

        // 如果已分析过，直接返回缓存（含 AI 意图）
        if ai_analyzed == 1 && ai_extracted.is_some() {
            let cached: serde_json::Value = serde_json::from_str(&ai_extracted.unwrap_or_default())
                .unwrap_or(serde_json::json!({}));
            return Ok(serde_json::json!({
                "cached": true,
                "result": cached,
                "intent": ai_category.as_deref().and_then(ai_category_to_intent),
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
            "intent": ai_category_to_intent(&category),
        }))
    })
    .await
}

/// AI 文档分类 → 推荐动作（本地规则兜底之外的 AI 兜底增强，§10）
/// 法律文书类（传票/通知/判决等）→ 归入案件；委托指示 → 转为任务；其余 None（留给本地规则）
fn ai_category_to_intent(category: &str) -> Option<serde_json::Value> {
    match category {
        "summons" | "hearing_notice" | "judgment" | "complaint" | "defense" | "correspondence" | "opposing_counsel" => {
            Some(serde_json::json!({
                "action": "file_to_case",
                "docType": category,
            }))
        }
        "client_instruction" => {
            Some(serde_json::json!({
                "action": "create_task",
            }))
        }
        _ => None,
    }
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

/// 通过 app handle 向前端 emit 批处理进度（architecture 目标 B）
///
/// 轮询命令 get_inbox_progress 保留不动，事件是增量增强；
/// 照 reminder.rs 的 reminder:triggered 模式，app 未就绪时静默跳过。
fn emit_batch_progress(progress: &ProcessingProgress) {
    if let Some(handle) = crate::get_app_handle() {
        let _ = tauri::Emitter::emit(handle, "inbox:batch-progress", progress);
    }
}

/// 批次结束事件（含 total/processed/failed 汇总）
fn emit_batch_finished(total: usize, processed: usize, failed: usize) {
    if let Some(handle) = crate::get_app_handle() {
        let _ = tauri::Emitter::emit(
            handle,
            "inbox:batch-finished",
            serde_json::json!({
                "total": total,
                "processed": processed,
                "failed": failed,
            }),
        );
    }
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
            emit_batch_progress(&self.get_progress());
            emit_batch_finished(0, 0, 0);
            return Ok(0);
        }

        // 批次开始：推送初始进度
        emit_batch_progress(&self.get_progress());

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

                // 条目完成/失败：广播 + 事件推送
                let progress = ProcessingProgress {
                    total: total_c.load(Ordering::Relaxed),
                    processed: proc_count.load(Ordering::Relaxed),
                    failed: fail_count.load(Ordering::Relaxed),
                    active: act_count.load(Ordering::Relaxed),
                    current_item: None,
                    running: true,
                };
                let _ = progress_tx.send(progress.clone());
                emit_batch_progress(&progress);
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
            // 批次结束：广播 + 事件推送（含 finished 汇总）
            let progress = ProcessingProgress {
                total: tot.load(Ordering::Relaxed),
                processed: proc.load(Ordering::Relaxed),
                failed: fail.load(Ordering::Relaxed),
                active: act.load(Ordering::Relaxed),
                current_item: None,
                running: false,
            };
            let _ = progress_tx.send(progress.clone());
            emit_batch_progress(&progress);
            emit_batch_finished(progress.total, progress.processed, progress.failed);
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
    let p = get_processor();
    p.paused.store(true, Ordering::Relaxed);
    emit_batch_progress(&p.get_progress());
    Ok(())
}

/// 恢复批量处理
#[tauri::command]
pub async fn resume_inbox_batch() -> Result<(), String> {
    let p = get_processor();
    p.paused.store(false, Ordering::Relaxed);
    emit_batch_progress(&p.get_progress());
    Ok(())
}

/// 取消批量处理
#[tauri::command]
pub async fn cancel_inbox_batch() -> Result<(), String> {
    let p = get_processor();
    p.cancel.store(true, Ordering::Relaxed);
    p.paused.store(false, Ordering::Relaxed);
    emit_batch_progress(&p.get_progress());
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

// ═══════════════════════════════════════════════════════════
// 多通道捕获（设计哲学 §10）
// ═══════════════════════════════════════════════════════════

/// 截屏捕获：截取当前屏幕并保存到收件箱
#[tauri::command]
pub async fn capture_screenshot() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let screen = screenshots::Screen::all()
            .map_err(|e| anyhow::anyhow!("获取屏幕失败: {}", e))?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("未找到屏幕"))?;

        let image = screen
            .capture()
            .map_err(|e| anyhow::anyhow!("截屏失败: {}", e))?;

        // 保存到临时文件
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("screenshot_{}.png", timestamp);
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(&filename);

        let png = image
            .to_png()
            .map_err(|e| anyhow::anyhow!("编码截图失败: {}", e))?;
        std::fs::write(&file_path, &png)
            .map_err(|e| anyhow::anyhow!("保存截屏失败: {}", e))?;

        // 添加到收件箱
        let conn = db::open_db()?;
        let id = db::new_id();
        conn.execute(
            "INSERT INTO inbox_items (id, source_type, title, source_path, status, created_at)
             VALUES (?1, 'screenshot', ?2, ?3, 'pending', ?4)",
            rusqlite::params![
                id,
                format!("截屏 {}", chrono::Local::now().format("%H:%M:%S")),
                file_path.to_string_lossy().to_string(),
                db::now_local(),
            ],
        )?;

        Ok(serde_json::json!({
            "id": id,
            "path": file_path.to_string_lossy().to_string(),
            "filename": filename,
        }))
    })
    .await
}

/// 读取剪贴板内容并添加到收件箱
#[tauri::command]
pub async fn capture_clipboard() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| anyhow::anyhow!("剪贴板访问失败: {}", e))?;

        // 尝试读取文本
        if let Ok(text) = clipboard.get_text() {
            if !text.trim().is_empty() {
                let conn = db::open_db()?;
                let id = db::new_id();
                let title = text.chars().take(50).collect::<String>();
                conn.execute(
                    "INSERT INTO inbox_items (id, source_type, title, content_text, status, created_at)
                     VALUES (?1, 'paste', ?2, ?3, 'pending', ?4)",
                    rusqlite::params![id, title, text, db::now_local()],
                )?;
                return Ok(serde_json::json!({
                    "id": id,
                    "type": "text",
                    "preview": title,
                }));
            }
        }

        // 尝试读取图片
        if let Ok(image) = clipboard.get_image() {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            let filename = format!("clipboard_{}.png", timestamp);
            let temp_dir = std::env::temp_dir();
            let file_path = temp_dir.join(&filename);

            // 保存图片数据：arboard 返回的是原始 RGBA 像素，不能直接当 PNG 写盘。
            // 复用 screenshots 0.6 自带的 Image::new + to_png()（png 编码器）编码为真 PNG。
            let png = screenshots::Image::new(
                image.width as u32,
                image.height as u32,
                image.bytes.into_owned(),
            )
            .to_png()
            .map_err(|e| anyhow::anyhow!("编码剪贴板图片失败: {}", e))?;
            std::fs::write(&file_path, &png)
                .map_err(|e| anyhow::anyhow!("保存剪贴板图片失败: {}", e))?;

            let conn = db::open_db()?;
            let id = db::new_id();
            conn.execute(
                "INSERT INTO inbox_items (id, source_type, title, source_path, status, created_at)
                 VALUES (?1, 'paste', ?2, ?3, 'pending', ?4)",
                rusqlite::params![
                    id,
                    format!("剪贴板图片 {}", chrono::Local::now().format("%H:%M:%S")),
                    file_path.to_string_lossy().to_string(),
                    db::now_local(),
                ],
            )?;
            return Ok(serde_json::json!({
                "id": id,
                "type": "image",
                "path": file_path.to_string_lossy().to_string(),
            }));
        }

        Err(anyhow::anyhow!("剪贴板为空"))
    })
    .await
}

/// 监听剪贴板变化（启动后台任务）
///
/// 现状说明：前端 useCapture.ts 会调用本命令启动监听，但当前仅检测文本变化并写日志，
/// **不自动入袋**——实际入袋由用户主动触发（capture_clipboard 命令 / 全局热键 quick capture）。
/// 用 SHA-256 hash 去重：内容不变时跳过，且不在内存中长期持有剪贴板原文。
#[tauri::command]
pub async fn start_clipboard_monitor() -> Result<String, String> {
    let _handle = tokio::spawn(async {
        use sha2::Digest;
        let mut last_hash: Option<Vec<u8>> = None;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(text) = clipboard.get_text() {
                    if !text.is_empty() {
                        let hash = sha2::Sha256::digest(text.as_bytes()).to_vec();
                        if last_hash.as_deref() != Some(hash.as_slice()) {
                            last_hash = Some(hash);
                            log::info!("剪贴板内容变化: {}字节", text.len());
                        }
                    }
                }
            }
        }
    });
    Ok("剪贴板监听已启动".to_string())
}

/// 保存语音速记文件
#[tauri::command]
pub async fn save_voice_note(audio_data: String, mime_type: String) -> Result<serde_json::Value, String> {
    use base64::Engine;

    run_blocking(move || {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&audio_data)
            .map_err(|e| anyhow::anyhow!("base64 解码失败: {}", e))?;

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let ext = if mime_type.contains("webm") { "webm" } else { "ogg" };
        let filename = format!("voice_{}.{}", timestamp, ext);
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(&filename);

        std::fs::write(&file_path, &bytes)
            .map_err(|e| anyhow::anyhow!("保存语音文件失败: {}", e))?;

        Ok(serde_json::json!({
            "path": file_path.to_string_lossy().to_string(),
            "filename": filename,
            "size": bytes.len(),
        }))
    })
    .await
}

/// 语音速记转写（设计哲学 §10，可选能力，降级优先）
///
/// 找到收件项关联的音频文件（save_voice_note 保存、前端经 source_path 关联），
/// 仅当 AI 配置为 OpenAI 兼容 API 时 POST {base_url}/audio/transcriptions；
/// 本地 Ollama / 未配置 API key → 友好错误，不 panic。
/// 成功把转写文本写回 inbox_items.content_text（保留 source_path 音频引用）并返回文本。
#[tauri::command]
pub async fn transcribe_voice_note(inbox_item_id: String) -> Result<String, String> {
    // 同步部分：读收件项音频路径 + AI/STT 配置（不满足条件直接友好报错）
    let id_for_read = inbox_item_id.clone();
    let (audio_path, stt_model, base_url, api_key) = run_blocking(move || {
        let conn = db::open_db()?;
        let source_path: Option<String> = conn
            .query_row(
                "SELECT source_path FROM inbox_items WHERE id = ?1",
                rusqlite::params![id_for_read],
                |r| r.get(0),
            )
            .map_err(|_| anyhow::anyhow!("收件项不存在"))?;
        let audio_path = source_path
            .filter(|p| !p.is_empty())
            .ok_or_else(|| anyhow::anyhow!("该收件项没有关联的音频文件"))?;

        let config = crate::ai::load_ai_config();
        let api_key = config.api_key.clone().unwrap_or_default();
        if config.mode != "openai" || api_key.is_empty() {
            anyhow::bail!("当前 AI 后端不支持语音转写，请配置 OpenAI 兼容 API 或手动填写");
        }

        let stt_model = db::get_setting(&conn, "stt_model")?
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "whisper-1".to_string());
        let base_url = config
            .api_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".into());

        Ok((audio_path, stt_model, base_url, api_key))
    })
    .await?;

    let bytes = std::fs::read(&audio_path)
        .map_err(|e| format!("读取音频文件失败: {}", e))?;

    use sha2::Digest;
    let input_hash = hex::encode(sha2::Sha256::digest(&bytes));

    // 手工构造 multipart/form-data（不引入新 crate）：model 字段 + file 字段
    let boundary = format!("casy-stt-{}", db::new_id());
    let file_name = std::path::Path::new(&audio_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "voice.ogg".to_string());
    let file_mime = if file_name.ends_with(".webm") {
        "audio/webm"
    } else {
        "audio/ogg"
    };

    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(
        format!(
            "--{}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{}\r\n",
            boundary, stt_model
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
            boundary, file_name, file_mime
        )
        .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP client 失败: {}", e))?;

    let url = format!("{}/audio/transcriptions", base_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            let _ = super::ai_routes::log_ai_run(
                "openai", &stt_model, "voice_transcription", Some("v1"),
                &input_hash, None, "failed", Some(&e.to_string()),
            );
            return Err(format!("转写请求失败: {}", e));
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        let _ = super::ai_routes::log_ai_run(
            "openai", &stt_model, "voice_transcription", Some("v1"),
            &input_hash, None, "failed",
            Some(&format!("HTTP {}: {}", status, err_body)),
        );
        return Err(format!("转写接口错误 {}: {}", status, err_body));
    }

    let result: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析转写响应失败: {}", e))?;
    let text = result["text"].as_str().unwrap_or("").trim().to_string();
    if text.is_empty() {
        let _ = super::ai_routes::log_ai_run(
            "openai", &stt_model, "voice_transcription", Some("v1"),
            &input_hash, None, "failed", Some("转写结果为空"),
        );
        return Err("转写结果为空，请手动填写".to_string());
    }

    // 审计（失败不阻塞主流程）
    let output_hash = hex::encode(sha2::Sha256::digest(text.as_bytes()));
    if let Err(e) = super::ai_routes::log_ai_run(
        "openai", &stt_model, "voice_transcription", Some("v1"),
        &input_hash, Some(&output_hash), "completed", None,
    ) {
        log::warn!("AI 审计日志写入失败: {}", e);
    }

    // 写回 content_text（保留 source_path 音频引用）
    let text_for_write = text.clone();
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "UPDATE inbox_items SET content_text = ?1 WHERE id = ?2",
            rusqlite::params![text_for_write, inbox_item_id],
        )?;
        Ok(())
    })
    .await?;

    Ok(text)
}

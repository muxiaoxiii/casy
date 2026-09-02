//! AI 路由与确认机制
//!
//! 实现双路径路由表和最小 Confirmer

use crate::db;
use rusqlite::params;
use rusqlite::OptionalExtension;

/// 命令路由信息
#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandRoute {
    pub command_name: String,
    pub route_type: String, // 'rule', 'ai', 'hybrid'
    pub description: String,
    pub requires_confirmation: bool,
    pub min_confirm_level: String, // 'L1', 'L2', 'L3'
}

/// 确认等级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmLevel {
    L1, // 可读确认
    L2, // 逐项确认
    L3, // 双人复核
}

impl std::str::FromStr for ConfirmLevel {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "L1" => ConfirmLevel::L1,
            "L2" => ConfirmLevel::L2,
            "L3" => ConfirmLevel::L3,
            _ => ConfirmLevel::L1,
        })
    }
}

impl ConfirmLevel {
    pub fn parse_level(s: &str) -> Self {
        s.parse().unwrap_or(ConfirmLevel::L1)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ConfirmLevel::L1 => "L1",
            ConfirmLevel::L2 => "L2",
            ConfirmLevel::L3 => "L3",
        }
    }

    /// 计算 effective_policy = max(system_minimum, scenario, model, user)
    pub fn max(self, other: Self) -> Self {
        match (self, other) {
            (ConfirmLevel::L3, _) | (_, ConfirmLevel::L3) => ConfirmLevel::L3,
            (ConfirmLevel::L2, _) | (_, ConfirmLevel::L2) => ConfirmLevel::L2,
            _ => ConfirmLevel::L1,
        }
    }
}

/// 获取命令路由信息
pub fn get_command_route(command_name: &str) -> Result<Option<CommandRoute>, anyhow::Error> {
    let conn = db::open_db()?;

    let mut stmt = conn.prepare(
        "SELECT command_name, route_type, description, requires_confirmation, min_confirm_level 
         FROM command_routes WHERE command_name = ?1",
    )?;

    let route = stmt
        .query_row(params![command_name], |row| {
            Ok(CommandRoute {
                command_name: row.get(0)?,
                route_type: row.get(1)?,
                description: row.get(2)?,
                requires_confirmation: row.get::<_, i32>(3)? != 0,
                min_confirm_level: row.get(4)?,
            })
        })
        .optional()?;

    Ok(route)
}

/// 检查命令是否需要确认
pub fn requires_confirmation(command_name: &str) -> Result<bool, anyhow::Error> {
    let route = get_command_route(command_name)?;
    Ok(route.map(|r| r.requires_confirmation).unwrap_or(false))
}

/// 获取命令的最小确认等级
pub fn get_min_confirm_level(command_name: &str) -> Result<ConfirmLevel, anyhow::Error> {
    let route = get_command_route(command_name)?;
    Ok(route
        .map(|r| ConfirmLevel::parse_level(&r.min_confirm_level))
        .unwrap_or(ConfirmLevel::L1))
}

/// 计算 effective_policy
///
/// effective_policy = max(
///   system_minimum_policy,  -- 系统安全下限（外部写 = L3），硬编码，不可降低
///   scenario_policy,        -- 场景风险（推荐 L1 / 提取 L2 / 外部写 L3）
///   model_policy,           -- 模型质量（本地小模型 +1 级）
///   user_policy             -- 用户设置（可提高，不能降低）
/// )
pub fn calculate_effective_policy(
    command_name: &str,
    is_external_write: bool,
    model_quality: Option<&str>,
    user_policy: Option<&str>,
) -> Result<ConfirmLevel, anyhow::Error> {
    // 1. 系统安全下限（硬编码）
    let system_minimum = if is_external_write {
        ConfirmLevel::L3
    } else {
        ConfirmLevel::L1
    };

    // 2. 场景风险
    let scenario_policy = get_min_confirm_level(command_name)?;

    // 3. 模型质量
    let model_policy = match model_quality {
        Some("local_small") => ConfirmLevel::L2, // 本地小模型 +1 级
        Some("local_large") => ConfirmLevel::L1,
        Some("cloud") => ConfirmLevel::L1,
        _ => ConfirmLevel::L1,
    };

    // 4. 用户设置
    let user_policy = user_policy
        .map(ConfirmLevel::parse_level)
        .unwrap_or(ConfirmLevel::L1);

    // 计算 effective_policy = max(所有策略)
    let effective = system_minimum
        .max(scenario_policy)
        .max(model_policy)
        .max(user_policy);

    Ok(effective)
}

/// 记录 AI 运行到 ai_runs 表
#[allow(clippy::too_many_arguments)]
pub fn log_ai_run(
    provider: &str,
    model: &str,
    purpose: &str,
    prompt_version: Option<&str>,
    input_hash: &str,
    output_hash: Option<&str>,
    status: &str,
    error_message: Option<&str>,
) -> Result<String, anyhow::Error> {
    let conn = db::open_db()?;
    let id = db::new_id();
    let now = db::now_local();

    conn.execute(
        "INSERT INTO ai_runs (id, provider, model, purpose, prompt_version, status, input_hash, output_hash, error_message, created_at, completed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            provider,
            model,
            purpose,
            prompt_version,
            status,
            input_hash,
            output_hash,
            error_message,
            now,
            if status == "completed" || status == "failed" { Some(&now) } else { None },
        ],
    )?;

    Ok(id)
}

/// 记录 AI 上下文项到 ai_context_items 表
// 预留：AI 上下文日志（B1 学习闭环接入时启用）
#[allow(dead_code)]
pub fn log_ai_context_item(
    run_id: &str,
    source_type: &str,
    source_id: &str,
    source_field: Option<&str>,
    content_hash: &str,
    snapshot_version: Option<&str>,
) -> Result<(), anyhow::Error> {
    let conn = db::open_db()?;
    let id = db::new_id();

    conn.execute(
        "INSERT INTO ai_context_items (id, run_id, source_type, source_id, source_field, content_hash, snapshot_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, run_id, source_type, source_id, source_field, content_hash, snapshot_version],
    )?;

    Ok(())
}

/// 获取 AI 运行历史
pub fn get_ai_runs(
    limit: Option<i64>,
    purpose: Option<&str>,
) -> Result<Vec<serde_json::Value>, anyhow::Error> {
    let conn = db::open_db()?;

    let mut sql = String::from(
        "SELECT id, provider, model, purpose, status, input_hash, output_hash, created_at, completed_at
         FROM ai_runs WHERE 1=1"
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let idx = 1;

    if let Some(p) = purpose {
        sql.push_str(&format!(" AND purpose = ?{}", idx));
        params.push(Box::new(p.to_string()));
    }

    sql.push_str(" ORDER BY created_at DESC");

    if let Some(l) = limit {
        sql.push_str(&format!(" LIMIT {}", l));
    }

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let runs: Vec<serde_json::Value> = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "provider": row.get::<_, String>(1)?,
                "model": row.get::<_, String>(2)?,
                "purpose": row.get::<_, String>(3)?,
                "status": row.get::<_, String>(4)?,
                "inputHash": row.get::<_, Option<String>>(5)?,
                "outputHash": row.get::<_, Option<String>>(6)?,
                "createdAt": row.get::<_, String>(7)?,
                "completedAt": row.get::<_, Option<String>>(8)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(runs)
}

/// Tauri 命令：获取命令路由信息
#[tauri::command]
pub async fn get_command_route_info(command_name: String) -> Result<Option<CommandRoute>, String> {
    get_command_route(&command_name).map_err(|e| e.to_string())
}

/// Tauri 命令：获取 AI 运行历史
#[tauri::command]
pub async fn get_ai_run_history(
    limit: Option<i64>,
    purpose: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    get_ai_runs(limit, purpose.as_deref()).map_err(|e| e.to_string())
}

/// Tauri 命令：检查命令是否需要确认
#[tauri::command]
pub async fn check_confirmation_required(command_name: String) -> Result<bool, String> {
    requires_confirmation(&command_name).map_err(|e| e.to_string())
}

/// Tauri 命令：计算有效确认策略
#[tauri::command]
pub async fn calculate_effective_policy_cmd(
    command_name: String,
    is_external_write: bool,
    model_quality: Option<String>,
    user_policy: Option<String>,
) -> Result<String, String> {
    let policy = calculate_effective_policy(
        &command_name,
        is_external_write,
        model_quality.as_deref(),
        user_policy.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    Ok(policy.as_str().to_string())
}

/// Tauri 命令：创建 AI 操作提案（P0-2 服务端授权网关）
#[tauri::command]
pub async fn create_ai_proposal(
    tool_name: String,
    target_entity_type: String,
    target_entity_id: Option<String>,
    _pre_state_hash: Option<String>,
    payload_json: String,
    ttl_seconds: Option<i64>,
) -> Result<crate::ai::gateway::AiProposal, String> {
    super::run_blocking(move || {
        let conn = db::open_db()?;
        let trusted_pre_state_hash = match target_entity_id.as_deref() {
            Some(entity_id) => Some(crate::ai::gateway::compute_current_entity_hash(
                &conn,
                &target_entity_type,
                entity_id,
            )?),
            None => None,
        };
        crate::ai::gateway::create_proposal(
            &conn,
            &tool_name,
            &target_entity_type,
            target_entity_id.as_deref(),
            trusted_pre_state_hash.as_deref(),
            &payload_json,
            ttl_seconds,
        )
    })
    .await
}

/// Tauri 命令：获取单个提案详情
#[tauri::command]
pub async fn get_ai_proposal(
    proposal_id: String,
) -> Result<Option<crate::ai::gateway::AiProposal>, String> {
    super::run_blocking(move || {
        let conn = db::open_db()?;
        crate::ai::gateway::get_proposal(&conn, &proposal_id)
    })
    .await
}

/// Tauri 命令：用户授权通过提案（获取一次性 auth_token）
#[tauri::command]
pub async fn approve_ai_proposal(proposal_id: String) -> Result<String, String> {
    super::run_blocking(move || {
        let conn = db::open_db()?;
        crate::ai::gateway::approve_proposal(&conn, &proposal_id)
    })
    .await
}

/// Tauri 命令：用户拒绝提案
#[tauri::command]
pub async fn reject_ai_proposal(proposal_id: String) -> Result<(), String> {
    super::run_blocking(move || {
        let conn = db::open_db()?;
        crate::ai::gateway::reject_proposal(&conn, &proposal_id)
    })
    .await
}

// ============================================================
// W1：Cursor 式 AI Diff 确认视图 —— 提案预览
// ============================================================

/// 字段级 Diff 行（before = 目标实体当前值，after = 提案将写入的值）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProposalFieldDiff {
    /// 字段名（payload_json 中的 camelCase key）
    pub field: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub changed: bool,
}

/// 提案预览：proposal 全字段 + 目标实体当前状态 + 字段级 Diff
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProposalPreviewDto {
    pub proposal: crate::ai::gateway::AiProposal,
    /// 目标实体显示名（任务名/案件名/知识标题/文件名）
    pub target_entity_name: Option<String>,
    /// 目标实体当前整行状态（snake_case 列名）；新建类提案为 None
    pub current_state: Option<serde_json::Value>,
    pub field_diffs: Vec<ProposalFieldDiff>,
}

/// 实体类型 → (主表名, 显示名列)。白名单制，未知类型不查询。
fn proposal_entity_table(entity_type: &str) -> Option<(&'static str, &'static str)> {
    match entity_type {
        "task" | "tasks" => Some(("tasks", "task_name")),
        "case" | "cases" => Some(("cases", "case_name")),
        "knowledge" | "knowledge_item" | "knowledge_items" => Some(("knowledge_items", "title")),
        "file" | "case_file" | "case_files" => Some(("case_files", "file_name")),
        _ => None,
    }
}

/// camelCase → snake_case（payload 字段名 → 数据库列名）
fn camel_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// 读取实体整行为 JSON（key 为 snake_case 列名）
fn load_entity_state(
    conn: &rusqlite::Connection,
    table: &str,
    id: &str,
) -> Result<Option<serde_json::Map<String, serde_json::Value>>, anyhow::Error> {
    // table 仅来自 proposal_entity_table 白名单常量，拼接安全
    let sql = format!("SELECT * FROM {} WHERE id = ?1", table);
    let mut stmt = conn.prepare(&sql)?;
    let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        let mut map = serde_json::Map::new();
        for (i, name) in col_names.iter().enumerate() {
            let v = row.get_ref(i)?;
            let jv = match v {
                rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                rusqlite::types::ValueRef::Integer(n) => serde_json::Value::from(n),
                rusqlite::types::ValueRef::Real(f) => serde_json::Value::from(f),
                rusqlite::types::ValueRef::Text(t) => {
                    serde_json::Value::from(String::from_utf8_lossy(t).to_string())
                }
                rusqlite::types::ValueRef::Blob(b) => {
                    serde_json::Value::from(format!("<blob {} bytes>", b.len()))
                }
            };
            map.insert(name.clone(), jv);
        }
        Ok(Some(map))
    } else {
        Ok(None)
    }
}

/// 从 payload_json 提取将写入的字段（兼容平铺与 { data: {...} } 嵌套两种形态）
fn collect_patch_fields(payload: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    fn is_meta_key(k: &str) -> bool {
        matches!(
            k,
            "id" | "origin" | "proposalToken" | "proposal_token" | "data"
        )
    }
    let mut out = serde_json::Map::new();
    if let Some(obj) = payload.as_object() {
        for (k, v) in obj {
            if !is_meta_key(k) {
                out.insert(k.clone(), v.clone());
            }
        }
        // data 嵌套形态（如 update_task 的 patch 体）：其字段优先
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            for (k, v) in data {
                if !is_meta_key(k) {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
    }
    out
}

/// Tauri 命令：获取提案预览（供前端渲染字段级 before→after diff）
#[tauri::command]
pub async fn get_proposal_preview(proposal_id: String) -> Result<ProposalPreviewDto, String> {
    super::run_blocking(move || {
        let conn = db::open_db()?;
        let mut proposal = crate::ai::gateway::get_proposal(&conn, &proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("提案不存在: {}", proposal_id))?;

        // 惰性过期：pending 且已过期的提案在读取时落终态
        if proposal.status == "pending" && proposal.expires_at < db::now_local() {
            conn.execute(
                "UPDATE ai_proposals SET status = 'expired' WHERE id = ?1 AND status = 'pending'",
                params![proposal_id],
            )?;
            proposal.status = "expired".to_string();
        }

        let payload: serde_json::Value =
            serde_json::from_str(&proposal.payload_json).unwrap_or(serde_json::Value::Null);
        let patch = collect_patch_fields(&payload);

        let mut entity_name: Option<String> = None;
        let mut current_state: Option<serde_json::Value> = None;
        let mut field_diffs: Vec<ProposalFieldDiff> = Vec::new();

        let resolved =
            proposal_entity_table(&proposal.target_entity_type).and_then(|(table, name_col)| {
                let eid = proposal.target_entity_id.as_deref()?;
                let row = load_entity_state(&conn, table, eid).ok().flatten()?;
                let name = row
                    .get(name_col)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Some((row, name))
            });

        match resolved {
            Some((row, name)) => {
                entity_name = name;
                for (k, v) in &patch {
                    let col = camel_to_snake(k);
                    let before = row.get(&col).cloned();
                    let changed = before.as_ref() != Some(v);
                    field_diffs.push(ProposalFieldDiff {
                        field: k.clone(),
                        before,
                        after: Some(v.clone()),
                        changed,
                    });
                }
                current_state = Some(serde_json::Value::Object(row));
            }
            None => {
                // 新建类提案 / 未知实体类型 / 实体已不存在：仅有 after
                for (k, v) in &patch {
                    field_diffs.push(ProposalFieldDiff {
                        field: k.clone(),
                        before: None,
                        after: Some(v.clone()),
                        changed: true,
                    });
                }
            }
        }

        Ok(ProposalPreviewDto {
            proposal,
            target_entity_name: entity_name,
            current_state,
            field_diffs,
        })
    })
    .await
}

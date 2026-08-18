//! AI 路由与确认机制
//!
//! 实现双路径路由表和最小 Confirmer

use crate::db;
use rusqlite::params;
use rusqlite::OptionalExtension;

/// 命令路由信息
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRoute {
    pub command_name: String,
    pub route_type: String,  // 'rule', 'ai', 'hybrid'
    pub description: String,
    pub requires_confirmation: bool,
    pub min_confirm_level: String,  // 'L1', 'L2', 'L3'
}

/// 确认等级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmLevel {
    L1,  // 可读确认
    L2,  // 逐项确认
    L3,  // 双人复核
}

impl ConfirmLevel {
    pub fn from_str(s: &str) -> Self {
        match s {
            "L1" => ConfirmLevel::L1,
            "L2" => ConfirmLevel::L2,
            "L3" => ConfirmLevel::L3,
            _ => ConfirmLevel::L1,
        }
    }

    pub fn to_str(&self) -> &'static str {
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
         FROM command_routes WHERE command_name = ?1"
    )?;
    
    let route = stmt.query_row(params![command_name], |row| {
        Ok(CommandRoute {
            command_name: row.get(0)?,
            route_type: row.get(1)?,
            description: row.get(2)?,
            requires_confirmation: row.get::<_, i32>(3)? != 0,
            min_confirm_level: row.get(4)?,
        })
    }).optional()?;
    
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
        .map(|r| ConfirmLevel::from_str(&r.min_confirm_level))
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
        Some("local_small") => ConfirmLevel::L2,  // 本地小模型 +1 级
        Some("local_large") => ConfirmLevel::L1,
        Some("cloud") => ConfirmLevel::L1,
        _ => ConfirmLevel::L1,
    };
    
    // 4. 用户设置
    let user_policy = user_policy
        .map(ConfirmLevel::from_str)
        .unwrap_or(ConfirmLevel::L1);
    
    // 计算 effective_policy = max(所有策略)
    let effective = system_minimum
        .max(scenario_policy)
        .max(model_policy)
        .max(user_policy);
    
    Ok(effective)
}

/// 记录 AI 运行到 ai_runs 表
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
    let mut idx = 1;
    
    if let Some(p) = purpose {
        sql.push_str(&format!(" AND purpose = ?{}", idx));
        params.push(Box::new(p.to_string()));
        idx += 1;
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
pub async fn get_ai_run_history(limit: Option<i64>, purpose: Option<String>) -> Result<Vec<serde_json::Value>, String> {
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
    ).map_err(|e| e.to_string())?;
    
    Ok(policy.to_str().to_string())
}

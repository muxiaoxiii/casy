use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// AI 操作提案数据结构（P0-2：服务端不可绕过授权网关）
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiProposal {
    pub id: String,
    pub tool_name: String,
    pub target_entity_type: String,
    pub target_entity_id: Option<String>,
    pub pre_state_hash: Option<String>,
    pub payload_json: String,
    pub auth_token: String,
    pub expires_at: String,
    pub status: String, // "pending", "approved", "executed", "rejected", "expired"
    pub created_at: String,
    pub executed_at: Option<String>,
}

/// 计算实体状态哈希（用于前置状态防并发漂移校验）
#[allow(dead_code)]
pub fn compute_entity_hash(data: &serde_json::Value) -> String {
    let canonical = match serde_json::to_string(data) {
        Ok(s) => s,
        Err(_) => data.to_string(),
    };
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// 创建 AI 变更提案（初始状态为 pending，生成高熵 auth_token）
pub fn create_proposal(
    conn: &Connection,
    tool_name: &str,
    target_entity_type: &str,
    target_entity_id: Option<&str>,
    pre_state_hash: Option<&str>,
    payload_json: &str,
    ttl_seconds: Option<i64>,
) -> Result<AiProposal> {
    let id = crate::db::new_id();
    let auth_token = format!("tok_{}", uuid::Uuid::new_v4().simple());
    let ttl = ttl_seconds.unwrap_or(300); // 默认 5 分钟有效期

    let now_dt = chrono::Local::now();
    let expires_dt = now_dt + chrono::Duration::seconds(ttl);
    let created_at = now_dt.naive_local().format("%Y-%m-%d %H:%M:%S").to_string();
    let expires_at = expires_dt
        .naive_local()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    conn.execute(
        "INSERT INTO ai_proposals (
            id, tool_name, target_entity_type, target_entity_id,
            pre_state_hash, payload_json, auth_token, expires_at,
            status, created_at, executed_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, NULL)",
        params![
            id,
            tool_name,
            target_entity_type,
            target_entity_id,
            pre_state_hash,
            payload_json,
            auth_token,
            expires_at,
            created_at,
        ],
    )?;

    Ok(AiProposal {
        id,
        tool_name: tool_name.to_string(),
        target_entity_type: target_entity_type.to_string(),
        target_entity_id: target_entity_id.map(|s| s.to_string()),
        pre_state_hash: pre_state_hash.map(|s| s.to_string()),
        payload_json: payload_json.to_string(),
        auth_token,
        expires_at,
        status: "pending".to_string(),
        created_at,
        executed_at: None,
    })
}

/// 根据 ID 获取提案
pub fn get_proposal(conn: &Connection, proposal_id: &str) -> Result<Option<AiProposal>> {
    let mut stmt = conn.prepare(
        "SELECT id, tool_name, target_entity_type, target_entity_id,
                pre_state_hash, payload_json, auth_token, expires_at,
                status, created_at, executed_at
         FROM ai_proposals WHERE id = ?1",
    )?;

    let mut rows = stmt.query(params![proposal_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(AiProposal {
            id: row.get(0)?,
            tool_name: row.get(1)?,
            target_entity_type: row.get(2)?,
            target_entity_id: row.get(3)?,
            pre_state_hash: row.get(4)?,
            payload_json: row.get(5)?,
            auth_token: row.get(6)?,
            expires_at: row.get(7)?,
            status: row.get(8)?,
            created_at: row.get(9)?,
            executed_at: row.get(10)?,
        }))
    } else {
        Ok(None)
    }
}

/// 根据 token 获取提案
pub fn get_proposal_by_token(conn: &Connection, auth_token: &str) -> Result<Option<AiProposal>> {
    let mut stmt = conn.prepare(
        "SELECT id, tool_name, target_entity_type, target_entity_id,
                pre_state_hash, payload_json, auth_token, expires_at,
                status, created_at, executed_at
         FROM ai_proposals WHERE auth_token = ?1",
    )?;

    let mut rows = stmt.query(params![auth_token])?;
    if let Some(row) = rows.next()? {
        Ok(Some(AiProposal {
            id: row.get(0)?,
            tool_name: row.get(1)?,
            target_entity_type: row.get(2)?,
            target_entity_id: row.get(3)?,
            pre_state_hash: row.get(4)?,
            payload_json: row.get(5)?,
            auth_token: row.get(6)?,
            expires_at: row.get(7)?,
            status: row.get(8)?,
            created_at: row.get(9)?,
            executed_at: row.get(10)?,
        }))
    } else {
        Ok(None)
    }
}

/// 用户在 UI 确认授权提案（状态由 pending 变为 approved）
pub fn approve_proposal(conn: &Connection, proposal_id: &str) -> Result<String> {
    let proposal = get_proposal(conn, proposal_id)?
        .ok_or_else(|| anyhow!("Proposal not found: {}", proposal_id))?;

    if proposal.status != "pending" {
        return Err(anyhow!(
            "Cannot approve proposal in '{}' status",
            proposal.status
        ));
    }

    let now = crate::db::now_local();
    if proposal.expires_at < now {
        conn.execute(
            "UPDATE ai_proposals SET status = 'expired' WHERE id = ?1",
            params![proposal_id],
        )?;
        return Err(anyhow!("Proposal has expired"));
    }

    conn.execute(
        "UPDATE ai_proposals SET status = 'approved' WHERE id = ?1",
        params![proposal_id],
    )?;

    Ok(proposal.auth_token)
}

/// 拒绝提案
pub fn reject_proposal(conn: &Connection, proposal_id: &str) -> Result<()> {
    let rows_affected = conn.execute(
        "UPDATE ai_proposals SET status = 'rejected' WHERE id = ?1 AND status = 'pending'",
        params![proposal_id],
    )?;

    if rows_affected == 0 {
        return Err(anyhow!("Proposal not found or not in pending state"));
    }

    Ok(())
}

/// 校验并一次性消费授权 Token（在执行真实写操作的同一事务中原子调用）
pub fn validate_and_consume_token(
    conn: &Connection,
    auth_token: &str,
    expected_tool: &str,
    expected_entity_type: &str,
    expected_entity_id: Option<&str>,
    current_state_hash: Option<&str>,
) -> Result<AiProposal> {
    let proposal = get_proposal_by_token(conn, auth_token)?
        .ok_or_else(|| anyhow!("Invalid authorization token"))?;

    let now = crate::db::now_local();

    // 1. 检查状态必须为 approved
    if proposal.status == "executed" {
        return Err(anyhow!(
            "Token has already been consumed (idempotent rejection)"
        ));
    }
    if proposal.status == "rejected" {
        return Err(anyhow!("Proposal was rejected by user"));
    }
    if proposal.status == "expired" || proposal.expires_at < now {
        let _ = conn.execute(
            "UPDATE ai_proposals SET status = 'expired' WHERE id = ?1",
            params![proposal.id],
        );
        return Err(anyhow!("Authorization token has expired"));
    }
    if proposal.status != "approved" {
        return Err(anyhow!(
            "Proposal is not approved by user (current status: '{}')",
            proposal.status
        ));
    }

    // 2. 校验工具名称与实体类型匹配
    if proposal.tool_name != expected_tool {
        return Err(anyhow!(
            "Tool mismatch: proposal is for '{}', requested '{}'",
            proposal.tool_name,
            expected_tool
        ));
    }
    if proposal.target_entity_type != expected_entity_type {
        return Err(anyhow!(
            "Entity type mismatch: proposal is for '{}', requested '{}'",
            proposal.target_entity_type,
            expected_entity_type
        ));
    }

    // 3. 校验实体 ID（如果有指定）
    if let (Some(prop_id), Some(exp_id)) =
        (proposal.target_entity_id.as_deref(), expected_entity_id)
    {
        if prop_id != exp_id {
            return Err(anyhow!(
                "Entity ID mismatch: proposal is for '{}', requested '{}'",
                prop_id,
                exp_id
            ));
        }
    }

    // 4. 校验前置状态 Hash 防止并发漂移
    if let (Some(prop_hash), Some(curr_hash)) =
        (proposal.pre_state_hash.as_deref(), current_state_hash)
    {
        if prop_hash != curr_hash {
            return Err(anyhow!(
                "State hash mismatch: target entity state was modified after proposal creation"
            ));
        }
    }

    // 5. 原子消费 Token，状态置为 executed
    let rows = conn.execute(
        "UPDATE ai_proposals SET status = 'executed', executed_at = ?1 WHERE id = ?2 AND status = 'approved'",
        params![now, proposal.id],
    )?;

    if rows == 0 {
        return Err(anyhow!("Concurrent token consumption conflict"));
    }

    Ok(proposal)
}

/// 服务端写命令通用 AI 授权守卫（P0-2：未授权默认拒绝）
pub fn verify_ai_mutation_authorized(
    conn: &Connection,
    origin: Option<&str>,
    proposal_token: Option<&str>,
    tool_name: &str,
    entity_type: &str,
    entity_id: Option<&str>,
    current_hash: Option<&str>,
) -> Result<()> {
    if origin == Some("ai") {
        let token = proposal_token.ok_or_else(|| {
            anyhow!("PermissionDenied: AI write operation requires an approved proposal token")
        })?;

        if token.trim().is_empty() {
            return Err(anyhow!(
                "PermissionDenied: AI write operation requires an approved proposal token"
            ));
        }

        validate_and_consume_token(conn, token, tool_name, entity_type, entity_id, current_hash)?;
    }

    Ok(())
}

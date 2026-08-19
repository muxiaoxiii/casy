//! Saved Filters 命令（设计哲学 §9：筛选/排序/分组规则可保存复用）

use super::run_blocking;
use crate::db;

/// 列出已保存筛选器（entity_type 与前端字段 module 等价，二者任一传参均可）
#[tauri::command]
pub async fn list_saved_filters(
    entity_type: Option<String>,
    module: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let entity = entity_type.or(module);

        let rows = match entity {
            Some(e) if !e.is_empty() => {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_type, name, filter_json, sort_order, created_at, updated_at
                     FROM saved_filters WHERE entity_type = ?1
                     ORDER BY sort_order, created_at",
                )?;
                query_filter_rows(&mut stmt, rusqlite::params![e])?
            }
            _ => {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_type, name, filter_json, sort_order, created_at, updated_at
                     FROM saved_filters
                     ORDER BY entity_type, sort_order, created_at",
                )?;
                query_filter_rows(&mut stmt, [])?
            }
        };

        Ok(rows)
    })
    .await
}

fn query_filter_rows(
    stmt: &mut rusqlite::Statement,
    params: impl rusqlite::Params,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let rows = stmt
        .query_map(params, |row| {
            let filter_json: String = row.get(3)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                filter_json,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(rows
        .into_iter()
        .map(|(id, entity_type, name, filter_json, sort_order, created_at, updated_at)| {
            filter_row_to_json(id, &entity_type, &name, &filter_json, sort_order, created_at, updated_at)
        })
        .collect())
}

/// 将 filter_json（{"filter":{...},"sortBy":...,"groupBy":...}）展开为前端 SavedFilter 形状
fn filter_row_to_json(
    id: i64,
    entity_type: &str,
    name: &str,
    filter_json: &str,
    sort_order: i64,
    created_at: Option<String>,
    updated_at: Option<String>,
) -> serde_json::Value {
    let parsed: serde_json::Value =
        serde_json::from_str(filter_json).unwrap_or_else(|_| serde_json::json!({}));

    // 兼容两种存储形态：包装形态 {filter, sortBy, groupBy} 或裸筛选对象
    let (filter, sort_by, group_by) = if parsed.get("filter").is_some() {
        (
            parsed["filter"].clone(),
            parsed["sortBy"].clone(),
            parsed["groupBy"].clone(),
        )
    } else {
        (parsed, serde_json::Value::Null, serde_json::Value::Null)
    };

    serde_json::json!({
        "id": id.to_string(),
        "name": name,
        "module": entity_type,
        "entityType": entity_type,
        "filter": filter,
        "sortBy": sort_by,
        "groupBy": group_by,
        "sortOrder": sort_order,
        "createdAt": created_at,
        "updatedAt": updated_at,
    })
}

/// 保存筛选器（同名同实体类型 → UPDATE，否则 INSERT）
/// 兼容两种调用形态：
///   1. save_filter({ filter: { name, module, filter, sortBy?, groupBy? } })  —— 前端 store
///   2. save_filter({ entityType, name, filterJson, sortOrder? })            —— 扁平参数
#[tauri::command]
pub async fn save_filter(
    filter: Option<serde_json::Value>,
    entity_type: Option<String>,
    name: Option<String>,
    filter_json: Option<String>,
    sort_order: Option<i64>,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 归一化参数
        let (entity, fname, fjson, forder) = if let Some(obj) = filter {
            let entity = obj["module"]
                .as_str()
                .or(obj["entityType"].as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 entity_type/module"))?
                .to_string();
            let name = obj["name"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("缺少 name"))?
                .to_string();
            let payload = serde_json::json!({
                "filter": obj["filter"],
                "sortBy": obj["sortBy"],
                "groupBy": obj["groupBy"],
            })
            .to_string();
            let order = obj["sortOrder"].as_i64().unwrap_or(0);
            (entity, name, payload, order)
        } else {
            let entity = entity_type
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("缺少 entity_type"))?;
            let name = name
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("缺少 name"))?;
            let payload = filter_json.ok_or_else(|| anyhow::anyhow!("缺少 filter_json"))?;
            // 校验是合法 JSON
            let _: serde_json::Value = serde_json::from_str(&payload)?;
            (entity, name, payload, sort_order.unwrap_or(0))
        };

        let now = db::now_local();

        // 同名同实体类型判重 → UPDATE
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM saved_filters WHERE entity_type = ?1 AND name = ?2",
                rusqlite::params![entity, fname],
                |r| r.get(0),
            )
            .ok();

        let id = if let Some(eid) = existing {
            conn.execute(
                "UPDATE saved_filters SET filter_json = ?1, sort_order = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![fjson, forder, now, eid],
            )?;
            eid
        } else {
            conn.execute(
                "INSERT INTO saved_filters (entity_type, name, filter_json, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![entity, fname, fjson, forder, now, now],
            )?;
            conn.last_insert_rowid()
        };

        Ok(serde_json::json!({ "id": id.to_string() }))
    })
    .await
}

/// 删除筛选器（id 兼容数字/字符串）
#[tauri::command]
pub async fn delete_filter(id: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id_num = parse_filter_id(&id)?;
        conn.execute(
            "DELETE FROM saved_filters WHERE id = ?1",
            rusqlite::params![id_num],
        )?;
        Ok(())
    })
    .await
}

fn parse_filter_id(id: &serde_json::Value) -> anyhow::Result<i64> {
    if let Some(n) = id.as_i64() {
        return Ok(n);
    }
    if let Some(s) = id.as_str() {
        return s
            .parse::<i64>()
            .map_err(|_| anyhow::anyhow!("无效的筛选器 id: {}", s));
    }
    Err(anyhow::anyhow!("无效的筛选器 id"))
}

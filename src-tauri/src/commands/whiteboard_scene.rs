//! Offline drawing scenes. Compare-and-swap saves prevent two windows overwriting each other.
use super::run_blocking;
use crate::db;
use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhiteboardSceneDto {
    pub scene_json: String,
    pub revision: i64,
    pub preview: Option<String>,
}

pub(crate) fn read_scene(
    conn: &Connection,
    case_id: &str,
    board_id: &str,
) -> Result<WhiteboardSceneDto> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM whiteboards WHERE id=?1 AND case_id=?2)",
        params![board_id, case_id],
        |r| r.get(0),
    )?;
    if !exists {
        bail!("白板不存在或不属于当前案件");
    }
    Ok(conn
        .query_row(
            "SELECT scene_json,revision,preview FROM whiteboard_scenes WHERE whiteboard_id=?1",
            [board_id],
            |r| {
                Ok(WhiteboardSceneDto {
                    scene_json: r.get(0)?,
                    revision: r.get(1)?,
                    preview: r.get(2)?,
                })
            },
        )
        .optional()?
        .unwrap_or(WhiteboardSceneDto {
            scene_json: "{\"elements\":[],\"appState\":{},\"files\":{}}".into(),
            revision: 0,
            preview: None,
        }))
}
fn save_scene(
    conn: &mut Connection,
    case_id: &str,
    board_id: &str,
    scene_json: &str,
    preview: Option<&str>,
    revision: i64,
) -> Result<i64> {
    let tx = conn.transaction()?;
    let revision = save_scene_in(&tx, case_id, board_id, scene_json, preview, revision)?;
    tx.commit()?;
    Ok(revision)
}
pub(crate) fn save_scene_in(
    conn: &Connection,
    case_id: &str,
    board_id: &str,
    scene_json: &str,
    preview: Option<&str>,
    revision: i64,
) -> Result<i64> {
    if scene_json.len() > 30 * 1024 * 1024 {
        bail!("画布超过 30 MiB，请压缩截图或分到其他白板");
    }
    let value: serde_json::Value = serde_json::from_str(scene_json)?;
    if !value["elements"].is_array()
        || !value["files"].is_object()
        || !value["appState"].is_object()
    {
        bail!("画布格式无效");
    }
    if preview
        .is_some_and(|v| !v.starts_with("data:image/png;base64,") || v.len() > 2 * 1024 * 1024)
    {
        bail!("预览必须为不超过 2 MiB 的 PNG");
    }

    let old = read_scene(conn, case_id, board_id)?;
    if old.revision != revision {
        bail!("白板已在其他窗口更新。请先导出当前画布，再重新打开以合并修改");
    }
    let old_value: serde_json::Value = serde_json::from_str(&old.scene_json)?;
    if old.revision > 0
        && (old_value["elements"] != value["elements"] || old_value["files"] != value["files"])
    {
        conn.execute("INSERT INTO whiteboard_scene_history(whiteboard_id,revision,scene_json,preview) VALUES(?1,?2,?3,?4)",params![board_id,old.revision,old.scene_json,old.preview])?;
        conn.execute("DELETE FROM whiteboard_scene_history WHERE whiteboard_id=?1 AND revision NOT IN (SELECT revision FROM whiteboard_scene_history WHERE whiteboard_id=?1 ORDER BY revision DESC LIMIT 5)",[board_id])?;
    }
    let next = revision + 1;
    conn.execute("INSERT INTO whiteboard_scenes(whiteboard_id,scene_json,revision,preview) VALUES(?1,?2,?3,?4) ON CONFLICT(whiteboard_id) DO UPDATE SET scene_json=excluded.scene_json,revision=excluded.revision,preview=COALESCE(excluded.preview,whiteboard_scenes.preview)",params![board_id,scene_json,next,preview])?;
    conn.execute(
        "UPDATE whiteboards SET updated_at=datetime('now','localtime') WHERE id=?1",
        [board_id],
    )?;
    Ok(next)
}
#[tauri::command]
pub async fn get_whiteboard_scene(
    case_id: String,
    whiteboard_id: String,
) -> Result<WhiteboardSceneDto, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        read_scene(&conn, &case_id, &whiteboard_id)
    })
    .await
}
#[tauri::command]
pub async fn save_whiteboard_scene(
    case_id: String,
    whiteboard_id: String,
    scene_json: String,
    preview: Option<String>,
    revision: i64,
) -> Result<i64, String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        save_scene(
            &mut conn,
            &case_id,
            &whiteboard_id,
            &scene_json,
            preview.as_deref(),
            revision,
        )
    })
    .await
}
#[tauri::command]
pub async fn list_whiteboard_scene_history(
    case_id: String,
    whiteboard_id: String,
) -> Result<Vec<WhiteboardSceneDto>, String> {
    run_blocking(move || {
        let conn=db::open_db()?;
        read_scene(&conn,&case_id,&whiteboard_id)?;
        let mut st=conn.prepare("SELECT scene_json,revision,preview FROM whiteboard_scene_history WHERE whiteboard_id=?1 ORDER BY revision DESC")?;
        let rows=st.query_map([whiteboard_id],|r|Ok(WhiteboardSceneDto{scene_json:r.get(0)?,revision:r.get(1)?,preview:r.get(2)?}))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scenes_preserve_images_reject_stale_and_cross_case_and_bound_history() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(db::schema::SCHEMA_SQL).unwrap();
        db::schema::run_migrations(&conn, 1).unwrap();
        conn.execute_batch("INSERT INTO cases(id,case_name,track,client_name) VALUES('a','A','other','test'),('b','B','other','test'); INSERT INTO whiteboards(id,case_id,name) VALUES('w','a','W');").unwrap();
        let scene = r#"{"elements":[{"type":"image","fileId":"pic"}],"appState":{},"files":{"pic":{"dataURL":"data:image/png;base64,eA=="}}}"#;
        assert!(save_scene(&mut conn, "b", "w", scene, None, 0).is_err());
        assert_eq!(save_scene(&mut conn, "a", "w", scene, None, 0).unwrap(), 1);
        assert!(save_scene(&mut conn, "a", "w", scene, None, 0).is_err());
        for revision in 1..8 {
            let mut value: serde_json::Value = serde_json::from_str(scene).unwrap();
            value["elements"][0]["x"] = revision.into();
            save_scene(&mut conn, "a", "w", &value.to_string(), None, revision).unwrap();
        }
        assert!(read_scene(&conn, "a", "w")
            .unwrap()
            .scene_json
            .contains("data:image/png;base64,eA=="));
        assert_eq!(
            conn.query_row("SELECT count(*) FROM whiteboard_scene_history", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            5
        );
        assert!(save_scene(&mut conn, "a", "w", "{}", None, 8).is_err());
    }
}

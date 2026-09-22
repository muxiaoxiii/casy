//! A whiteboard is one document: drawing, facts, citations and bound relationships commit together.
use super::{run_blocking, whiteboard_scene};
use crate::db;
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io::Write, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasFact {
    pub id: String,
    pub file_id: Option<String>,
    pub knowledge_id: Option<String>,
    pub source_case_id: Option<String>,
    pub source_title: String,
    pub page: Option<i64>,
    pub excerpt: String,
    pub note: Option<String>,
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
}
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhiteboardDocument {
    pub scene: whiteboard_scene::WhiteboardSceneDto,
    pub facts: Vec<CanvasFact>,
    pub edges: Vec<CanvasEdge>,
    pub facts_hash: String,
}
fn facts(conn: &Connection, board: &str) -> Result<(Vec<CanvasFact>, Vec<CanvasEdge>, String)> {
    let facts=conn.prepare("SELECT n.id,n.file_id,n.knowledge_id,f.case_id,COALESCE(f.file_name,k.title,n.source_title,'手动事实'),n.page,n.excerpt,n.note,n.x,n.y FROM fact_nodes n LEFT JOIN case_files f ON f.id=n.file_id LEFT JOIN knowledge_items k ON k.id=n.knowledge_id WHERE n.whiteboard_id=?1 ORDER BY n.id")?.query_map([board],|r|Ok(CanvasFact{id:r.get(0)?,file_id:r.get(1)?,knowledge_id:r.get(2)?,source_case_id:r.get(3)?,source_title:r.get(4)?,page:r.get(5)?,excerpt:r.get(6)?,note:r.get(7)?,x:r.get(8)?,y:r.get(9)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let edges=conn.prepare("SELECT id,source_node_id,target_node_id FROM whiteboard_edges WHERE whiteboard_id=?1 ORDER BY id")?.query_map([board],|r|Ok(CanvasEdge{id:r.get(0)?,source_node_id:r.get(1)?,target_node_id:r.get(2)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(&facts, &edges))?)
    );
    Ok((facts, edges, hash))
}
pub(crate) fn read(conn: &Connection, case: &str, board: &str) -> Result<WhiteboardDocument> {
    let scene = whiteboard_scene::read_scene(conn, case, board)?;
    let (facts, edges, facts_hash) = facts(conn, board)?;
    Ok(WhiteboardDocument {
        scene,
        facts,
        edges,
        facts_hash,
    })
}
#[tauri::command]
pub async fn get_whiteboard_document(
    case_id: String,
    whiteboard_id: String,
) -> Result<WhiteboardDocument, String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;
        let document = read(&tx, &case_id, &whiteboard_id)?;
        tx.commit()?;
        Ok(document)
    })
    .await
}
#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhiteboardDocumentInput {
    pub case_id: String,
    pub whiteboard_id: String,
    pub scene_json: String,
    pub preview: Option<String>,
    pub revision: i64,
    pub facts_hash: String,
    pub facts: Vec<CanvasFact>,
    pub edges: Vec<CanvasEdge>,
}
pub(crate) fn save(
    conn: &mut Connection,
    input: WhiteboardDocumentInput,
) -> Result<WhiteboardDocument> {
    ensure!(
        input.facts.len() <= 5000 && input.edges.len() <= 10000,
        "事实或关系数量超过单块白板上限"
    );
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let old = read(&tx, &input.case_id, &input.whiteboard_id)?;
    ensure!(
        old.scene.revision == input.revision && old.facts_hash == input.facts_hash,
        "白板或事实已在其他位置更新。请先导出当前内容，再重新打开核对"
    );
    let scene: serde_json::Value = serde_json::from_str(&input.scene_json)?;
    let elements = scene["elements"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("场景元素无效"))?;
    let managed: Vec<_> = elements.iter().filter(|e| e["isDeleted"] != true && e["type"] == "rectangle" && e["customData"]["casy"]["fact"].is_object()).collect();
    ensure!(managed.len() == input.facts.len(), "画布卡片与事实数量不一致");
    let mut element_ids = HashSet::new();
    for element in elements {
        ensure!(element["id"].as_str().is_some_and(|id| !id.is_empty() && element_ids.insert(id)), "画布元素标识重复或缺失");
    }
    let mut ids = HashSet::new();
    for fact in &input.facts {
        ensure!(
            !fact.id.is_empty() && ids.insert(fact.id.clone()),
            "重复或无效的事实标识"
        );
        ensure!(
            !fact.excerpt.trim().is_empty() && fact.excerpt.len() <= 80000,
            "事实内容为空或过长"
        );
        ensure!(
            fact.note.as_ref().is_none_or(|s| s.len() <= 80000),
            "批注过长"
        );
        ensure!(
            fact.page.is_none_or(|p| p > 0)
                && fact.x.is_finite()
                && fact.y.is_finite()
                && fact.x.abs() < 1e8
                && fact.y.abs() < 1e8,
            "页码或位置无效"
        );
        ensure!(
            fact.file_id.is_none() || fact.knowledge_id.is_none(),
            "一条事实请选择一个来源"
        );
        let owner: Option<String> = tx
            .query_row(
                "SELECT whiteboard_id FROM fact_nodes WHERE id=?1",
                [&fact.id],
                |r| r.get(0),
            )
            .optional()?;
        ensure!(
            owner.is_none_or(|id| id == input.whiteboard_id),
            "事实不属于当前白板"
        );
        let card = elements
            .iter()
            .find(|e| {
                e["isDeleted"] != true
                    && e["customData"]["casy"]["fact"]["id"] == fact.id
                    && e["type"] == "rectangle"
            })
            .ok_or_else(|| anyhow::anyhow!("事实缺少对应画布卡片"))?;
        ensure!(card["x"].as_f64()==Some(fact.x) && card["y"].as_f64()==Some(fact.y), "画布与事实位置不一致");
        let metadata = &card["customData"]["casy"]["fact"];
        let serialized = serde_json::to_value(fact)?;
        for key in ["fileId", "knowledgeId", "sourceCaseId", "sourceTitle", "page", "note"] {
            ensure!(metadata[key] == serialized[key], "画布与事实引用不一致");
        }
        let text_id = card["boundElements"]
            .as_array()
            .and_then(|a| a.iter().find(|b| b["type"] == "text"))
            .and_then(|v| v["id"].as_str())
            .ok_or_else(|| anyhow::anyhow!("事实卡片缺少文字"))?;
        let text = elements
            .iter()
            .find(|e| e["id"] == text_id && e["isDeleted"] != true)
            .ok_or_else(|| anyhow::anyhow!("事实文字不存在"))?;
        ensure!(
            text["originalText"]
                .as_str()
                .or_else(|| text["text"].as_str())
                == Some(fact.excerpt.as_str()),
            "画布文字与事实内容不一致"
        );
        ensure!(text["containerId"]==card["id"] && text["type"]=="text", "事实文字未绑定当前卡片");
        let unchanged_source = old.facts.iter().any(|f| f.id==fact.id && f.file_id==fact.file_id && f.knowledge_id==fact.knowledge_id && f.source_case_id==fact.source_case_id);
        let source_title = if let Some(file) = &fact.file_id {
            let source: Option<(String, String)> = tx
                .query_row(
                    "SELECT file_name,case_id FROM case_files WHERE id=?1 AND (deleted_at IS NULL OR ?2)",
                    params![file,unchanged_source],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (title, case) =
                source.ok_or_else(|| anyhow::anyhow!("来源文件已移除，请重新选择来源"))?;
            ensure!(
                fact.source_case_id.as_deref() == Some(case.as_str()),
                "文件来源案件不匹配"
            );
            title
        } else if let Some(knowledge) = &fact.knowledge_id {
            tx.query_row("SELECT title FROM knowledge_items WHERE id=?1 AND (COALESCE(status,'current')='current' OR ?2)",params![knowledge,unchanged_source],|r|r.get::<_,String>(0)).optional()?.ok_or_else(||anyhow::anyhow!("知识来源已移除或归档，请重新选择"))?
        } else {
            fact.source_title.chars().take(500).collect::<String>()
        };
        tx.execute("INSERT INTO fact_nodes(id,whiteboard_id,file_id,knowledge_id,source_title,page,excerpt,note,x,y) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
        ON CONFLICT(id) DO UPDATE SET file_id=excluded.file_id,knowledge_id=excluded.knowledge_id,source_title=excluded.source_title,page=excluded.page,excerpt=excluded.excerpt,note=excluded.note,x=excluded.x,y=excluded.y,updated_at=datetime('now','localtime')
        WHERE file_id IS NOT excluded.file_id OR knowledge_id IS NOT excluded.knowledge_id OR source_title IS NOT excluded.source_title OR page IS NOT excluded.page OR excerpt IS NOT excluded.excerpt OR note IS NOT excluded.note OR x IS NOT excluded.x OR y IS NOT excluded.y",
        params![fact.id,input.whiteboard_id,fact.file_id,fact.knowledge_id,source_title,fact.page,fact.excerpt,fact.note,fact.x,fact.y])?;
    }
    let bound_arrows: Vec<_> = elements.iter().filter(|e| e["isDeleted"]!=true && e["type"]=="arrow" && managed.iter().any(|c| c["id"]==e["startBinding"]["elementId"]) && managed.iter().any(|c| c["id"]==e["endBinding"]["elementId"])).collect();
    ensure!(bound_arrows.len()==input.edges.len(), "画布连线与事实关系数量不一致");
    let mut edge_ids = HashSet::new();
    for edge in &input.edges {
        ensure!(
            !edge.id.is_empty()
                && edge_ids.insert(edge.id.clone())
                && ids.contains(&edge.source_node_id)
                && ids.contains(&edge.target_node_id),
            "关系必须连接本白板中的两个事实"
        );
        ensure!(bound_arrows.iter().any(|arrow| {
            let id=arrow["customData"]["casy"]["edgeId"].as_str().or_else(|| arrow["id"].as_str());
            id==Some(edge.id.as_str()) && managed.iter().any(|c| c["id"]==arrow["startBinding"]["elementId"] && c["customData"]["casy"]["fact"]["id"]==edge.source_node_id) && managed.iter().any(|c| c["id"]==arrow["endBinding"]["elementId"] && c["customData"]["casy"]["fact"]["id"]==edge.target_node_id)
        }), "画布连线与事实关系不一致");
        let owner: Option<String> = tx
            .query_row(
                "SELECT whiteboard_id FROM whiteboard_edges WHERE id=?1",
                [&edge.id],
                |r| r.get(0),
            )
            .optional()?;
        ensure!(
            owner.is_none_or(|id| id == input.whiteboard_id),
            "关系不属于当前白板"
        );
        tx.execute("INSERT INTO whiteboard_edges(id,whiteboard_id,source_node_id,target_node_id) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET source_node_id=excluded.source_node_id,target_node_id=excluded.target_node_id",params![edge.id,input.whiteboard_id,edge.source_node_id,edge.target_node_id])?;
    }
    tx.execute("DELETE FROM whiteboard_edges WHERE whiteboard_id=?1 AND id NOT IN (SELECT value FROM json_each(?2))",params![input.whiteboard_id,serde_json::to_string(&edge_ids)?])?;
    tx.execute("DELETE FROM fact_nodes WHERE whiteboard_id=?1 AND id NOT IN (SELECT value FROM json_each(?2))",params![input.whiteboard_id,serde_json::to_string(&ids)?])?;
    whiteboard_scene::save_scene_in(
        &tx,
        &input.case_id,
        &input.whiteboard_id,
        &input.scene_json,
        input.preview.as_deref(),
        input.revision,
    )?;
    let document = read(&tx, &input.case_id, &input.whiteboard_id)?;
    tx.commit()?;
    Ok(document)
}
#[tauri::command]
pub async fn save_whiteboard_document(
    input: WhiteboardDocumentInput,
) -> Result<WhiteboardDocument, String> {
    run_blocking(move || save(&mut *db::open_db()?, input)).await
}
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhiteboardSource {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub case_id: Option<String>,
    pub case_name: Option<String>,
    pub preview: String,
    pub total_pages: Option<i64>,
}
#[derive(Debug, Serialize, specta::Type)]
pub struct WhiteboardSources {
    pub items: Vec<WhiteboardSource>,
    pub total: i64,
}
#[tauri::command]
pub async fn list_whiteboard_sources(
    case_id: String,
    scope: String,
    query: String,
    offset: i64,
) -> Result<WhiteboardSources, String> {
    run_blocking(move||{
        ensure!(offset>=0 && matches!(scope.as_str(),"case"|"other"|"knowledge"),"来源筛选无效");
        let mut conn=db::open_db()?;let tx=conn.transaction()?;
        let (sql,params)=if scope=="knowledge" {
            ("SELECT id,'knowledge',title,NULL,NULL,substr(COALESCE(content,''),1,1600),NULL FROM knowledge_items WHERE COALESCE(status,'current')='current' AND (instr(lower(title),lower(?1))>0 OR instr(lower(COALESCE(content,'')),lower(?1))>0)".to_owned(),vec![query])
        }else {
            (format!("SELECT f.id,'file',f.file_name,f.case_id,c.case_name,substr(COALESCE(f.ocr_text,''),1,1600),(SELECT total_pages FROM document_processing_jobs WHERE file_id=f.id AND status='completed' ORDER BY rowid DESC LIMIT 1) FROM case_files f JOIN cases c ON c.id=f.case_id WHERE f.deleted_at IS NULL AND f.case_id {} ?2 AND (instr(lower(f.file_name),lower(?1))>0 OR instr(lower(COALESCE(f.ocr_text,'')),lower(?1))>0 OR EXISTS(SELECT 1 FROM document_pages dp JOIN document_processing_jobs j ON j.id=dp.job_id WHERE dp.file_id=f.id AND j.status='completed' AND j.id=(SELECT j2.id FROM document_processing_jobs j2 WHERE j2.file_id=f.id AND j2.status='completed' ORDER BY j2.rowid DESC LIMIT 1) AND instr(lower(COALESCE(dp.plain_text,'') || COALESCE(dp.markdown,'')),lower(?1))>0))",if scope=="case"{"="}else{"!="}),vec![query,case_id])
        };
        let total=tx.query_row(&format!("SELECT count(*) FROM ({sql})"),rusqlite::params_from_iter(&params),|r|r.get(0))?;
        let items=tx.prepare(&format!("{sql} ORDER BY 3,1 LIMIT 30 OFFSET {offset}"))?.query_map(rusqlite::params_from_iter(&params),|r|Ok(WhiteboardSource{id:r.get(0)?,kind:r.get(1)?,title:r.get(2)?,case_id:r.get(3)?,case_name:r.get(4)?,preview:r.get(5)?,total_pages:r.get(6)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
        tx.commit()?;Ok(WhiteboardSources{items,total})
    }).await
}
pub(crate) fn write_export(path: &Path, format: &str, bytes: &[u8]) -> Result<()> {
    ensure!(
        path.is_absolute() && bytes.len() <= 60 * 1024 * 1024,
        "导出路径无效或文件超过 60 MiB"
    );
    ensure!(
        path.extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case(format)),
        "文件扩展名与导出格式不一致"
    );
    match format {
        "png" => ensure!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "PNG 数据无效"),
        "svg" => {
            let text = std::str::from_utf8(bytes)?;
            ensure!(
                text.contains("<svg") && text.contains("</svg>"),
                "SVG 数据无效"
            );
        }
        "excalidraw" => {
            let value: serde_json::Value = serde_json::from_slice(bytes)?;
            ensure!(
                value["type"] == "excalidraw"
                    && value["elements"].is_array()
                    && value["files"].is_object(),
                "可编辑画布数据无效"
            );
        }
        _ => anyhow::bail!("不支持此导出格式"),
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("请选择输出目录"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
#[tauri::command]
pub async fn write_whiteboard_export(
    case_id: String,
    whiteboard_id: String,
    output_path: String,
    format: String,
    data_base64: String,
) -> Result<String, String> {
    run_blocking(move || {
        use base64::Engine;
        let conn = db::open_db()?;
        whiteboard_scene::read_scene(&conn, &case_id, &whiteboard_id)?;
        drop(conn);
        ensure!(data_base64.len() <= 80 * 1024 * 1024, "导出内容过大");
        let bytes = base64::engine::general_purpose::STANDARD.decode(data_base64)?;
        write_export(Path::new(&output_path), &format, &bytes)?;
        Ok(output_path)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn database() -> Connection {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(db::schema::SCHEMA_SQL).unwrap();
        db::schema::run_migrations(&conn,1).unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; INSERT INTO cases(id,case_name,track,client_name) VALUES('a','A','other','test'),('b','B','other','test'); INSERT INTO whiteboards(id,case_id,name) VALUES('w','a','W'),('other','b','Other'); INSERT INTO knowledge_items(id,title,content,category,status) VALUES('k','知识来源','来源内容','reference','current');").unwrap();
        conn
    }
    fn fact(id:&str) -> CanvasFact {CanvasFact{id:id.into(),file_id:None,knowledge_id:None,source_case_id:None,source_title:"手动事实".into(),page:None,excerpt:"从转文日起算".into(),note:None,x:0.,y:0.}}
    fn input(conn:&Connection, facts:Vec<CanvasFact>, edges:Vec<CanvasEdge>) -> WhiteboardDocumentInput {
        let old=read(conn,"a","w").unwrap();
        let mut elements=Vec::new();
        for f in &facts {
            elements.push(json!({"id":f.id,"type":"rectangle","x":f.x,"y":f.y,"customData":{"casy":{"fact":f}},"boundElements":[{"id":format!("text-{}",f.id),"type":"text"}]}));
            elements.push(json!({"id":format!("text-{}",f.id),"type":"text","containerId":f.id,"originalText":f.excerpt}));
        }
        for e in &edges {elements.push(json!({"id":e.id,"type":"arrow","startBinding":{"elementId":e.source_node_id},"endBinding":{"elementId":e.target_node_id},"customData":{"casy":{"edgeId":e.id}}}));}
        WhiteboardDocumentInput{case_id:"a".into(),whiteboard_id:"w".into(),scene_json:json!({"elements":elements,"appState":{},"files":{}}).to_string(),preview:None,revision:old.scene.revision,facts_hash:old.facts_hash,facts,edges}
    }
    #[test]
    fn text_notes_sources_delete_undo_and_edges_commit_together() {
        let mut conn=database();let mut f=fact("one");f.knowledge_id=Some("k".into());f.source_title="知识来源".into();f.note=Some("待核实".into());f.page=Some(2);
        let edge=CanvasEdge{id:"edge".into(),source_node_id:"one".into(),target_node_id:"two".into()};
        let initial=input(&conn,vec![f.clone(),fact("two")],vec![edge.clone()]);save(&mut conn,initial).unwrap();
        f.excerpt="文字已修改".into();f.note=None;f.page=None;f.x=40.;
        let edit=input(&conn,vec![f.clone(),fact("two")],vec![edge.clone()]);let saved=save(&mut conn,edit).unwrap();assert_eq!(saved.facts[0].excerpt,"文字已修改");assert!(saved.facts[0].note.is_none());assert!(saved.facts[0].page.is_none());
        let delete=input(&conn,vec![],vec![]);assert!(save(&mut conn,delete).unwrap().facts.is_empty());
        let undo=input(&conn,vec![f,fact("two")],vec![edge]);let restored=save(&mut conn,undo).unwrap();assert_eq!(restored.edges.len(),1);assert_eq!(restored.facts.len(),2);
        assert!(conn.query_row("SELECT count(*) FROM audit_events WHERE event_type='fact_deleted'",[],|r|r.get::<_,i64>(0)).unwrap()>=2);
    }
    #[test]
    fn invalid_source_or_scene_rolls_back_all_facts_and_revision() {
        let mut conn=database();let mut bad=fact("bad");bad.knowledge_id=Some("missing".into());
        let request=input(&conn,vec![fact("valid"),bad],vec![]);assert!(save(&mut conn,request).is_err());assert!(read(&conn,"a","w").unwrap().facts.is_empty());
        let mut request=input(&conn,vec![fact("valid")],vec![]);request.preview=Some("bad-preview".into());assert!(save(&mut conn,request).is_err());assert_eq!(read(&conn,"a","w").unwrap().scene.revision,0);
        let mut request=input(&conn,vec![fact("valid")],vec![]);request.facts.clear();assert!(save(&mut conn,request).is_err());
    }
    #[test]
    fn stale_scene_external_fact_edits_and_cross_board_ids_are_rejected() {
        let mut conn=database();let stale=input(&conn,vec![],vec![]);let add=input(&conn,vec![fact("one")],vec![]);save(&mut conn,add).unwrap();assert!(save(&mut conn,stale).is_err());
        let stale=input(&conn,vec![fact("one")],vec![]);conn.execute("UPDATE fact_nodes SET excerpt='external' WHERE id='one'",[]).unwrap();assert!(save(&mut conn,stale).is_err());
        conn.execute("INSERT INTO fact_nodes(id,whiteboard_id,excerpt,x,y) VALUES('foreign','other','foreign',0,0)",[]).unwrap();let foreign=input(&conn,vec![fact("foreign")],vec![]);assert!(save(&mut conn,foreign).is_err());
        assert!(read(&conn,"b","w").is_err());
    }
    #[test]
    fn archived_existing_source_remains_saveable_but_new_reference_is_rejected() {
        let mut conn=database();let mut f=fact("one");f.knowledge_id=Some("k".into());f.source_title="知识来源".into();let add=input(&conn,vec![f.clone()],vec![]);save(&mut conn,add).unwrap();
        conn.execute("UPDATE knowledge_items SET status='archived' WHERE id='k'",[]).unwrap();f.note=Some("核查旧引用".into());let edit=input(&conn,vec![f.clone()],vec![]);save(&mut conn,edit).unwrap();
        f.id="new".into();let add=input(&conn,vec![f],vec![]);assert!(save(&mut conn,add).is_err());
    }
    #[test]
    fn export_writes_supported_formats_and_does_not_clobber_on_invalid_data() {
        let temp=tempfile::tempdir().unwrap();
        for (format,bytes) in [("png",b"\x89PNG\r\n\x1a\nrest".as_slice()),("svg",b"<svg></svg>"),("excalidraw",b"{\"type\":\"excalidraw\",\"elements\":[],\"files\":{}}")]{
            let path=temp.path().join(format!("board.{format}"));write_export(&path,format,bytes).unwrap();assert_eq!(std::fs::read(&path).unwrap(),bytes);assert!(write_export(&path,format,b"invalid").is_err());assert_eq!(std::fs::read(&path).unwrap(),bytes);
        }
        assert!(write_export(Path::new("relative.png"),"png",b"\x89PNG\r\n\x1a\n").is_err());
    }
}

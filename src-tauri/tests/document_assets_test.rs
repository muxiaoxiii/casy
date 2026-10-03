use base64::{engine::general_purpose::STANDARD, Engine};
use casy_lib::{
    commands::document_assets::{
        get_document_storage_state, optimize_document_storage, read_document_asset,
        rollback_document_storage,
    },
    db,
};
#[tokio::test]
async fn upgrade_preserves_old_version_and_serves_only_owned_verified_assets() {
    let temp = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", temp.path());
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    let root = temp.path().join("original-artifacts");
    std::fs::create_dir_all(&root).unwrap();
    let source = temp.path().join("source.pdf");
    std::fs::write(&source, b"original pdf bytes").unwrap();
    let hash = casy_lib::document_pipeline::sha256_file(&source).unwrap();
    let png=STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQKt8NAAITAVXMpdPnAAAAAElFTkSuQmCC").unwrap();
    let md = format!(
        "Text <figure><img src=\"data:image/png;base64,{}\"></figure>",
        STANDARD.encode(&png)
    );
    let page = serde_json::json!({"pageNumber":1,"width":100.0,"height":100.0,"plainText":"Text","markdown":md,"regions":[],"confidence":null,"layout":null,"timing":null,"nativeText":true,"extraMetadata":{"keep":"unchanged"}});
    let ir = root.join("source.document.json");
    let markdown = root.join("source.md");
    std::fs::write(&ir, serde_json::to_vec(&vec![page]).unwrap()).unwrap();
    std::fs::write(&markdown, format!("<!-- page 1 -->\n{md}")).unwrap();
    conn.execute_batch("INSERT INTO cases(id,case_name,client_name) VALUES('c','Case','Client');")
        .unwrap();
    conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f','c','source.pdf',?1,'evidence')",[source.to_string_lossy().as_ref()]).unwrap();
    conn.execute("INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,engine,total_pages,current_page,progress,page_ir_path,markdown_path) VALUES('old','f',?1,'completed','paddle-onnx-test',1,1,1,?2,?3)",rusqlite::params![hash,ir.to_string_lossy(),markdown.to_string_lossy()]).unwrap();
    conn.execute("INSERT INTO document_pages(job_id,file_id,page_number,width,height,plain_text,markdown) VALUES('old','f',1,100,100,'Text',?1)",[&md]).unwrap();
    let state =
        serde_json::to_value(get_document_storage_state("f".into()).await.unwrap()).unwrap();
    assert_eq!(state["canOptimize"], true);
    conn.execute("UPDATE document_pages SET width=200 WHERE job_id='old'", [])
        .unwrap();
    assert!(optimize_document_storage("f".into(), "old".into())
        .await
        .unwrap_err()
        .contains("数据库页面"));
    conn.execute("UPDATE document_pages SET width=100 WHERE job_id='old'", [])
        .unwrap();
    let upgraded = optimize_document_storage("f".into(), "old".into())
        .await
        .unwrap();
    let upgraded_ir: String = conn.query_row("SELECT page_ir_path FROM document_processing_jobs WHERE id=?1", [&upgraded], |row| row.get(0)).unwrap();
    let upgraded_records: serde_json::Value = serde_json::from_reader(std::fs::File::open(upgraded_ir).unwrap()).unwrap();
    assert_eq!(upgraded_records[0]["nativeText"], true);
    assert_eq!(upgraded_records[0]["extraMetadata"]["keep"], "unchanged");
    let new_md: String = conn
        .query_row(
            "SELECT markdown FROM document_pages WHERE job_id=?1",
            [&upgraded],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!new_md.contains("base64"));
    assert!(std::fs::read_to_string(&markdown)
        .unwrap()
        .contains("base64"));
    let id = new_md
        .split("assets/")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(
        STANDARD
            .decode(
                read_document_asset("f".into(), upgraded.clone(), id.into())
                    .await
                    .unwrap()
            )
            .unwrap(),
        png
    );
    assert!(
        read_document_asset("other".into(), upgraded.clone(), id.into())
            .await
            .is_err()
    );
    assert!(
        read_document_asset("f".into(), upgraded.clone(), "../source.pdf".into())
            .await
            .is_err()
    );
    assert!(optimize_document_storage("f".into(), "old".into())
        .await
        .is_err());
    assert_eq!(std::fs::read(&source).unwrap(), b"original pdf bytes");
    assert_eq!(
        optimize_document_storage("f".into(), upgraded.clone())
            .await
            .unwrap(),
        upgraded
    );
    let state =
        serde_json::to_value(get_document_storage_state("f".into()).await.unwrap()).unwrap();
    assert_eq!(state["canOptimize"], false);
    assert_eq!(state["canRollback"], true);
    std::fs::write(&markdown, "corrupt old version").unwrap();
    assert!(rollback_document_storage("f".into(), upgraded.clone())
        .await
        .is_err());
    std::fs::write(&markdown, format!("<!-- page 1 -->\n{md}")).unwrap();
    let mut connection = db::open_db().unwrap();
    let note = casy_lib::commands::knowledge::import_pageindex_inner(&mut connection, "f").unwrap();
    let snapshot: String = connection
        .query_row(
            "SELECT content FROM knowledge_items WHERE id=?1",
            [&note.knowledge_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(snapshot.contains(&STANDARD.encode(&png)));
    assert!(!snapshot.contains("src=\"assets/"));
    drop(connection);
    let exported = tempfile::tempdir().unwrap();
    let out = exported.path().join("snapshot.md");
    casy_lib::commands::knowledge::export_knowledge_markdown(
        note.knowledge_id.clone(), out.to_string_lossy().into_owned(),
    )
    .await
    .unwrap();
    let exported_md = std::fs::read_to_string(&out).unwrap();
    let relative = exported_md
        .split("src=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(std::fs::read(exported.path().join(relative)).unwrap(), png);
    drop(conn);
    db::reset_shared_conn();
    let reopened =
        serde_json::to_value(get_document_storage_state("f".into()).await.unwrap()).unwrap();
    assert_eq!(reopened["canRollback"], true);
    let archive = exported.path().join("assets.casy");
    casy_lib::commands::portable_backup::export_full_backup(
        archive.to_string_lossy().into_owned(),
        "asset-backup-password".into(),
    )
    .await
    .unwrap();
    casy_lib::commands::portable_backup::import_full_backup(
        archive.to_string_lossy().into_owned(),
        "asset-backup-password".into(),
    )
    .await
    .unwrap();
    let conn = db::open_db().unwrap();
    let restored_md: String = conn
        .query_row(
            "SELECT markdown_path FROM document_processing_jobs WHERE id=?1",
            [&upgraded],
            |r| r.get(0),
        )
        .unwrap();
    assert!(std::path::Path::new(&restored_md).starts_with(temp.path().join("restored")));
    assert_eq!(
        STANDARD
            .decode(
                read_document_asset("f".into(), upgraded.clone(), id.into())
                    .await
                    .unwrap()
            )
            .unwrap(),
        png
    );
    let restored_old: String = conn
        .query_row(
            "SELECT markdown_path FROM document_processing_jobs WHERE id='old'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(restored_old, markdown.to_string_lossy());
    assert_eq!(
        rollback_document_storage("f".into(), upgraded)
            .await
            .unwrap(),
        "old"
    );
    assert_eq!(
        conn.query_row(
            "SELECT ocr_markdown_path FROM case_files WHERE id='f'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        restored_old
    );
    drop(conn);
    db::reset_shared_conn();
}

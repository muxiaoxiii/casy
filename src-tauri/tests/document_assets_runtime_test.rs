use base64::{engine::general_purpose::STANDARD, Engine};
use casy_lib::{
    background_jobs,
    commands::{conversion, document_assets, document_intelligence, portable_backup},
    db, document_pipeline, workspace_sync,
};
use std::path::Path;

#[tokio::test]
#[ignore = "requires a freshly built CASY_DOC_ENGINE and verified local OCR models"]
async fn real_ocr_correction_export_and_restore_preserve_external_images() {
    let engine =
        std::env::var("CASY_DOC_ENGINE").expect("set CASY_DOC_ENGINE to the current build");
    assert!(Path::new(&engine).is_file());
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    let source = profile.path().join("chart.png");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/ocr-chart.png"),
        &source,
    )
    .unwrap();
    let original_hash = document_pipeline::sha256_file(&source).unwrap();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO cases(id,case_name,client_name) VALUES('c','Asset runtime','Client');",
    )
    .unwrap();
    conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f','c','chart.png',?1,'evidence')",
        [source.to_string_lossy().as_ref()]).unwrap();
    drop(conn);
    let job = document_intelligence::queue_document_processing("f".into())
        .await
        .unwrap();
    assert!(background_jobs::process_next_document_job().await.unwrap());
    let jobs = document_intelligence::list_document_jobs("f".into())
        .await
        .unwrap();
    let job = jobs.iter().find(|value| value.id == job.id).unwrap();
    assert_eq!(job.status, "completed", "{:?}", job.error_message);
    let root = Path::new(job.markdown_path.as_ref().unwrap())
        .parent()
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_reader(std::fs::File::open(root.join("source.assets.json")).unwrap()).unwrap();
    assert_eq!(manifest["version"], 2);
    let assets = manifest["assets"].as_object().unwrap();
    assert!(
        !assets.is_empty(),
        "chart fixture must yield at least one actual visual crop"
    );
    let markdown = workspace_sync::get_workspace_document("f".into())
        .await
        .unwrap();
    assert!(markdown["markdown"].as_str().unwrap().contains("assets/"));
    assert!(!markdown["markdown"].as_str().unwrap().contains("base64"));
    let pages: Vec<document_pipeline::DocumentPage> =
        serde_json::from_reader(std::fs::File::open(job.page_ir_path.as_ref().unwrap()).unwrap())
            .unwrap();
    let region = &pages[0].regions[0];
    let corrected_text = format!("{} corrected", region.text);
    let corrected = document_intelligence::correct_document_region(
        "f".into(),
        job.id.clone(),
        1,
        0,
        region.text.clone(),
        corrected_text.clone(),
    )
    .await
    .unwrap();
    let corrected_markdown = workspace_sync::get_workspace_document("f".into())
        .await
        .unwrap();
    assert!(corrected_markdown["markdown"]
        .as_str()
        .unwrap()
        .contains(&corrected_text));
    assert!(corrected_markdown["markdown"]
        .as_str()
        .unwrap()
        .contains("assets/"));
    assert!(!corrected_markdown["markdown"]
        .as_str()
        .unwrap()
        .contains("base64"));
    for id in assets.keys() {
        let original = std::fs::read(root.join("assets").join(id)).unwrap();
        assert_eq!(
            STANDARD
                .decode(
                    document_assets::read_document_asset("f".into(), corrected.clone(), id.clone())
                        .await
                        .unwrap()
                )
                .unwrap(),
            original
        );
    }
    let output = tempfile::tempdir().unwrap();
    let exported = conversion::convert_file_to_markdown(
        source.to_string_lossy().into_owned(),
        output.path().to_string_lossy().into_owned(),
        None,
        Some("both".into()),
    )
    .await
    .unwrap();
    let export_path = Path::new(exported["markdownPath"].as_str().unwrap());
    let export_markdown = std::fs::read_to_string(export_path).unwrap();
    assert!(!export_markdown.contains("base64"));
    let image = export_markdown
        .split("src=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let png = std::fs::read(export_path.parent().unwrap().join(image)).unwrap();
    assert!(image::load_from_memory(&png).is_ok());
    assert!(std::fs::read(exported["pdfPath"].as_str().unwrap())
        .unwrap()
        .starts_with(b"%PDF-"));
    let archive = output.path().join("runtime.casy");
    portable_backup::export_full_backup(
        archive.to_string_lossy().into_owned(),
        "runtime-test-password".into(),
    )
    .await
    .unwrap();
    portable_backup::import_full_backup(
        archive.to_string_lossy().into_owned(),
        "runtime-test-password".into(),
    )
    .await
    .unwrap();
    assert_eq!(
        workspace_sync::get_workspace_document("f".into())
            .await
            .unwrap()["markdown"],
        corrected_markdown["markdown"]
    );
    for id in assets.keys() {
        assert_eq!(
            STANDARD
                .decode(
                    document_assets::read_document_asset("f".into(), corrected.clone(), id.clone())
                        .await
                        .unwrap()
                )
                .unwrap(),
            std::fs::read(root.join("assets").join(id)).unwrap()
        );
    }
    assert_eq!(
        document_pipeline::sha256_file(&source).unwrap(),
        original_hash
    );
    db::reset_shared_conn();
}

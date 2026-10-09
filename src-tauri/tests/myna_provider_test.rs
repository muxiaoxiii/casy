//! Myna 远端 OCR provider 端到端验证（默认 ignore：需要本机运行 Myna）。
//!
//! 运行方式：
//!   curl -s http://127.0.0.1:18600/api/healthz   # 确认 Myna 在线
//!   CASY_OCR_PROVIDER=myna cargo test --test myna_provider_test -- --ignored
use casy_lib::{background_jobs, commands::document_intelligence, db, document_pipeline, ocr_provider};
use std::path::Path;

#[tokio::test]
#[ignore = "requires a running Myna OCR service on 127.0.0.1:18600"]
async fn myna_provider_produces_engine_compatible_artifacts() {
    std::env::set_var("CASY_OCR_PROVIDER", "myna");
    assert_eq!(ocr_provider::configured_provider(), ocr_provider::OcrProvider::Myna);

    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    // 带图 + 表格的合成页面（MinerU 应抽出图片并给出归一化几何）
    let source = profile.path().join("scan.png");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/ocr-chart.png"),
        &source,
    )
    .expect("测试 fixture 缺失");
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute_batch("INSERT INTO cases(id,case_name,client_name) VALUES('c','Myna 案件','委托人');")
        .unwrap();
    conn.execute(
        "INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f','c','scan.png',?1,'evidence')",
        [source.to_string_lossy().as_ref()],
    )
    .unwrap();
    drop(conn);

    let queued = document_intelligence::queue_document_processing("f".into()).await.unwrap();
    assert!(background_jobs::process_next_document_job().await.unwrap());

    let conn = db::open_db().unwrap();
    let (status, engine, page_count): (String, String, i64) = conn
        .query_row(
            "SELECT status,engine,total_pages FROM document_processing_jobs WHERE id=?1",
            [&queued.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(engine, "myna-mineru-4.0");
    assert!(page_count >= 1);

    // 页 IR / Markdown / 来源映射 / 可搜索 PDF 四件套齐备（jobs 表不存 map 路径，按布局推导）
    let (markdown_path, ir_path, pdf_path): (String, String, Option<String>) = conn
        .query_row(
            "SELECT markdown_path,page_ir_path,searchable_pdf_path FROM document_processing_jobs WHERE id=?1",
            [&queued.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let root = Path::new(&markdown_path).parent().unwrap();
    let map_path = root.join("source.map.json");
    assert!(Path::new(&ir_path).is_file(), "页 IR 缺失");
    assert!(map_path.is_file(), "来源映射缺失");
    let pdf = pdf_path.expect("可搜索 PDF 路径缺失");
    assert!(Path::new(&pdf).is_file(), "可搜索 PDF 缺失");
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));

    // Markdown 不得残留 base64 data URI（必须已外置）
    let markdown = std::fs::read_to_string(&markdown_path).unwrap();
    assert!(!markdown.contains("data:image/"), "Markdown 仍含 base64 图片");

    // 页面几何必须是像素坐标（不是 0–1 归一化）
    let pages: Vec<document_pipeline::DocumentPage> =
        serde_json::from_reader(std::fs::File::open(&ir_path).unwrap()).unwrap();
    let page = &pages[0];
    assert!(page.width.unwrap() > 1.0 && page.height.unwrap() > 1.0);
    for region in &page.regions {
        assert!(
            region.bbox[2] <= page.width.unwrap() + 1.0 && region.bbox[3] <= page.height.unwrap() + 1.0,
            "bbox 超出页面: {:?}",
            region.bbox
        );
    }

    // R-06 结果清单落库
    let results: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_job_results WHERE job_id=?1", [&queued.id], |r| r.get(0))
        .unwrap();
    assert_eq!(results, 1);

    // 图片资产落地（若该文档含图）
    let images = root.join("casy-images");
    if images.is_dir() {
        assert!(std::fs::read_dir(&images).unwrap().count() > 0, "资产目录为空");
    }

    // 探活失败时必须明确报错而非静默回退
    std::env::set_var("CASY_MYNA_URL", "http://127.0.0.1:1");
    let client = ocr_provider::MynaClient::new().unwrap();
    assert!(client.healthz().await.is_err(), "探活应失败");
}

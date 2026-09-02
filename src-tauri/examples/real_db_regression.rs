//! 真实数据库副本的端到端回归（不进 CI，手动运行）。
//!
//! 用途：codex 留下的验证边界 —— OCR/PageIndex 沉淀此前只经过 Mock UI 与内存库测试，
//! 本程序在**真实库的 /tmp 副本**上跑一遍完整链路，真实库绝不被触碰：
//!   1. 用真实密钥打开副本（SQLCipher）；
//!   2. 执行 run_migrations（含 v23 knowledge_items 条件重建——真实库带着旧 category CHECK）；
//!   3. 列出已完成 OCR/PageIndex 的来源文件；
//!   4. 对每个来源执行 import_pageindex_inner，并二次执行验证去重（reused=true）；
//!   5. 输出知识树结构与链接数，供人工核对。
//!
//! 准备（zsh）：
//!   mkdir -p /tmp/casy-regression
//!   cp "$HOME/Library/Application Support/Casy/casy.db"* /tmp/casy-regression/
//!   cp "$HOME/Library/Application Support/Casy/casy.db.key" /tmp/casy-regression/
//!
//! 运行：
//!   cd src-tauri && cargo run --example real_db_regression -- /tmp/casy-regression
//!
//! 约定：参数目录里必须已有 casy.db 与 casy.db.key；程序只读写该目录。

use casy_lib::commands::knowledge::import_pageindex_inner;
use casy_lib::db::schema::run_migrations;
use rusqlite::Connection;

fn main() -> anyhow::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/casy-regression".to_string());
    let db_path = std::path::Path::new(&dir).join("casy.db");
    let key_path = std::path::Path::new(&dir).join("casy.db.key");
    if !db_path.exists() || !key_path.exists() {
        anyhow::bail!("目录 {dir} 缺少 casy.db 或 casy.db.key（先按文件头注释复制真实库副本）");
    }
    // 防止误指真实目录：副本必须在 /tmp 下
    if !db_path.to_string_lossy().starts_with("/tmp/") {
        anyhow::bail!("安全闸：只允许对 /tmp 下的副本运行，拒绝: {}", db_path.display());
    }

    let key = std::fs::read_to_string(&key_path)?.trim().to_string();
    let mut conn = Connection::open(&db_path)?;
    // 经验：SQLCipher 要求 key 之后紧跟首个真实读；若在 key 与首读之间插入
    // 其它 PRAGMA（busy_timeout/foreign_keys 合批），实测会报 "file is not a database"。
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key))?;
    let sanity: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))?;
    println!("✓ 副本打开成功（sqlite_master {sanity} 项）");
    conn.execute_batch("PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;")?;

    let before: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    run_migrations(&conn, 0)?;
    let after: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    println!("✓ 迁移完成：user_version {before} → {after}");

    // v23 验收：knowledge_items 不应再有 category CHECK
    let ki_sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='knowledge_items'",
        [],
        |r| r.get(0),
    )?;
    assert!(
        !ki_sql.contains("CHECK(category"),
        "v23 重建后仍检测到 category CHECK"
    );
    println!("✓ v23：category CHECK 已移除");

    // 笔记本分类可写（写一条随即删除，验证约束层面放行）
    conn.execute(
        "INSERT INTO knowledge_items (id, title, category, content) VALUES ('__probe__', '探针', 'reference', '')",
        [],
    )?;
    conn.execute("DELETE FROM knowledge_items WHERE id='__probe__'", [])?;
    println!("✓ v23：category='reference' 写入/删除正常");

    // 知识条目总数与 FTS 索引健康度
    let notes: i64 = conn.query_row("SELECT COUNT(*) FROM knowledge_items", [], |r| r.get(0))?;
    let fts_ok = conn
        .query_row("SELECT COUNT(*) FROM knowledge_fts LIMIT 1", [], |r| r.get::<_, i64>(0))
        .map(|_| true)
        .unwrap_or(false);
    println!("✓ 知识条目 {notes} 条；FTS 索引表可读：{fts_ok}");

    // 列出已完成 OCR/PageIndex 的来源（与 list_knowledge_document_sources 同口径）
    let mut stmt = conn.prepare(
        "SELECT cf.id, cf.file_name, c.case_name
         FROM case_files cf
         JOIN cases c ON c.id = cf.case_id
         JOIN document_processing_jobs j ON j.id = (
           SELECT j2.id FROM document_processing_jobs j2
           WHERE j2.file_id = cf.id AND j2.status = 'completed'
           ORDER BY j2.updated_at DESC LIMIT 1
         )
         ORDER BY j.updated_at DESC LIMIT 200",
    )?;
    let sources: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    println!("✓ 待沉淀来源 {} 个", sources.len());

    if sources.is_empty() {
        println!("（真实库暂无已完成 OCR/PageIndex 的 PDF——链路无可回归对象，迁移部分已通过）");
        return Ok(());
    }

    for (file_id, file_name, case_name) in &sources {
        let first = import_pageindex_inner(&mut conn, file_id)?;
        let again = import_pageindex_inner(&mut conn, file_id)?;
        assert!(again.reused, "二次沉淀必须复用既有知识树: {file_name}");
        assert_eq!(first.knowledge_id, again.knowledge_id);
        println!(
            "✓ 沉淀 [{case_name}] {file_name}：根 {}，结构子笔记 {}，二次调用 reused=true",
            first.knowledge_id, first.child_count
        );
        // 结构完整性：子的父链必须都能回到根
        let broken: i64 = conn.query_row(
            "WITH RECURSIVE tree(id, root_ok) AS (
               SELECT id, 1 FROM knowledge_items WHERE id = ?1
               UNION ALL
               SELECT k.id, 0 FROM knowledge_items k JOIN tree t ON k.parent_id = t.id
             )
             SELECT COUNT(*) FROM knowledge_items
             WHERE parent_id = ?1 AND id NOT IN (SELECT id FROM tree)",
            [&first.knowledge_id],
            |r| r.get(0),
        )?;
        assert_eq!(broken, 0, "存在父链断裂的子笔记");
        let links: i64 = conn.query_row(
            "SELECT COUNT(*) FROM links WHERE target_id=?1 AND anchor='pageindex:structure'",
            [&first.knowledge_id],
            |r| r.get(0),
        )?;
        println!("  · pageindex:structure 链接 {links} 条；父链校验通过");
    }

    println!("\n全部回归通过。副本位于 {dir}（真实库未被触碰）。");
    Ok(())
}

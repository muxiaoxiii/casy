//! Background Jobs (Pure Rust OCR & PageIndex Scheduler)
use log::{info, error};
use std::time::Duration;
use tauri::AppHandle;
use tokio::time::sleep;

/// 启动闲时后台任务处理器
/// 负责扫描需要 OCR 或构建索引的文件，并在后台无感知地执行
pub fn start_background_worker(_app: AppHandle) {
    tokio::spawn(async move {
        info!("Rust Native Background OCR (OvisOCR2 GGUF) & PageIndex worker started.");
        
        // 并发锁：确保同一时间只有一个 OvisOCR2 推理实例在运行
        let ocr_semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        
        loop {
            // 每隔 60 秒检查一次是否有待处理任务
            // 真实场景下，可以增加“用户闲时检测（Idle Detection）”逻辑，
            // 这里为了演示简化处理
            sleep(Duration::from_secs(60)).await;

            // 1. 从数据库捞取 ocr_status = 'pending' 的 case_files
            let file_opt = {
                let conn_res = crate::db::open_db();
                if let Ok(conn) = conn_res {
                    let stmt = conn.prepare("SELECT id, file_path FROM case_files WHERE ocr_status = 'pending' LIMIT 1");
                    let opt = if let Ok(mut stmt) = stmt {
                        stmt.query_map([], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                        })
                        .and_then(|mut rows| rows.next().transpose())
                        .unwrap_or(None)
                    } else { None };
                    
                    if let Some((ref file_id, _)) = opt {
                        let _ = conn.execute("UPDATE case_files SET ocr_status = 'processing' WHERE id = ?1", rusqlite::params![file_id]);
                    }
                    opt
                } else {
                    None
                }
            }; // conn is dropped here

            if let Some((file_id, file_path)) = file_opt {
                // 获取并发锁，避免撑爆显存/内存
                let _permit = ocr_semaphore.acquire().await.ok();
                info!("Found pending OCR task for file_id: {}, acquired lock.", file_id);
                
                // 执行纯 Rust PDF 提取 (OvisOCR2)
                match crate::parse::pdf_extractor::extract_pdf_to_markdown(&file_path).await {
                    Ok(_markdown) => {
                        info!("OCR completed for file_id: {}", file_id);
                        if let Ok(conn) = crate::db::open_db() {
                            let _ = conn.execute("UPDATE case_files SET ocr_status = 'completed', index_status = 'pending' WHERE id = ?1", rusqlite::params![&file_id]);
                            
                            // 立即触发 PageIndex 构建 (传入 file_path 以支持懒加载建树)
                            if let Err(e) = crate::ai::page_index::build_page_index_tree(&file_id, &file_path).await {
                                error!("Failed to build PageIndex for {}: {}", file_id, e);
                                if let Ok(conn) = crate::db::open_db() {
                                    let _ = conn.execute("UPDATE case_files SET index_status = 'failed' WHERE id = ?1", rusqlite::params![&file_id]);
                                }
                            } else {
                                if let Ok(conn) = crate::db::open_db() {
                                    let _ = conn.execute("UPDATE case_files SET index_status = 'completed' WHERE id = ?1", rusqlite::params![&file_id]);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if e == "NeedsOCR" {
                            info!("File {} is a scanned document. Marking as requires_extension.", file_id);
                            if let Ok(conn) = crate::db::open_db() {
                                let _ = conn.execute("UPDATE case_files SET ocr_status = 'requires_extension' WHERE id = ?1", rusqlite::params![&file_id]);
                            }
                        } else {
                            error!("Fast extraction failed for {}: {}", file_id, e);
                            if let Ok(conn) = crate::db::open_db() {
                                let _ = conn.execute("UPDATE case_files SET ocr_status = 'failed' WHERE id = ?1", rusqlite::params![&file_id]);
                            }
                        }
                    }
                }
            }
        }
    });
}

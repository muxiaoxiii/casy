use super::run_blocking;
use crate::{
    db,
    deadline::procedure::{self, ProcedureAudit, ProcedureBoard, ProcedureEvent},
};
#[tauri::command]
pub async fn get_procedure_board(
    case_id: String,
    include_related: bool,
) -> Result<ProcedureBoard, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        procedure::board(&conn, &case_id, include_related)
    })
    .await
}
#[tauri::command]
pub async fn save_procedure_event(
    event: ProcedureEvent,
    reason: String,
) -> Result<ProcedureEvent, String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        procedure::save_event(&mut conn, event, &reason)
    })
    .await
}
#[tauri::command]
pub async fn set_procedure_item_state(
    case_id: String,
    item_id: String,
    fingerprint: String,
    status: String,
    note: String,
) -> Result<(), String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        procedure::set_item_state(&mut conn, &case_id, &item_id, &fingerprint, &status, &note)
    })
    .await
}
#[tauri::command]
pub async fn get_procedure_history(case_id: String) -> Result<Vec<ProcedureAudit>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        procedure::history(&conn, &case_id)
    })
    .await
}

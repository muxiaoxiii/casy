use casy_lib::db::{self, cases::{CaseFilter,list_cases}};
#[test]
fn advanced_filters_apply_to_rows_and_total_and_combined_parameters() {
    let conn=rusqlite::Connection::open_in_memory().unwrap();db::init_db(&conn).unwrap();
    conn.execute_batch("INSERT INTO cases(id,case_name,client_name,attorneys,trial_date,filing_date,relief_deadline,cause_action) VALUES
        ('a','A','Client','Alice','2026-10-12','2026-01-01','2026-10-14','行政'),
        ('b','B','Client','Bob','2026-11-12','2026-02-01','2026-11-14','行政');").unwrap();
    for value in [
        serde_json::json!({"deadlineFrom":"2026-10-01","deadlineTo":"2026-10-31"}),
        serde_json::json!({"hearingFrom":"2026-10-01","hearingTo":"2026-10-31"}),
        serde_json::json!({"operator":"Alice"}),
        serde_json::json!({"dateTo":"2026-12-31","deadlineTo":"2026-10-31","operator":"Alice","perPage":0}),
    ] {
        let filter:CaseFilter=serde_json::from_value(value).unwrap();
        let result=list_cases(&conn,&filter).unwrap();
        assert_eq!(result.total,1);assert_eq!(result.items.len(),1);assert_eq!(result.items[0].id,"a");
    }
}

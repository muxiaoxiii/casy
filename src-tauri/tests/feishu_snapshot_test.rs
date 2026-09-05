use casy_lib::{
    commands::feishu_snapshot::{field_display, import_snapshot},
    db::{self, schema},
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashMap;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    schema::run_migrations(&conn, 0).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    conn
}
fn fixture() -> Value {
    json!({"appToken":"test-base","tables":[
      {"table_id":"cases","name":"案件主表","fields":[
        {"field_name":"案件信息","type":1},{"field_name":"客户名称","type":1},{"field_name":"我方诉讼地位","type":1},{"field_name":"诉讼地位","type":3},
        {"field_name":"关联案件","type":18,"property":{"table_id":"cases"}}
      ],"records":[
        {"record_id":"rec-a","fields":{"案件信息":"案件甲","客户名称":"客户甲","我方诉讼地位":"原告","诉讼地位":"第三人","关联案件":[{"record_ids":["rec-b"],"table_id":"cases"}]}},
        {"record_id":"rec-b","fields":{"案件信息":"案件乙","客户名称":"客户乙"}},
        {"record_id":"rec-c","fields":{"案件信息":"案件丙","客户名称":"客户丙"}}
      ]},
      {"table_id":"logs","name":"办案日志","fields":[
        {"field_name":"事件概述","type":1},{"field_name":"操作内容","type":1},{"field_name":"发生时间","type":5,"property":{"date_formatter":"yyyy/MM/dd HH:mm"}},
        {"field_name":"案件名称","type":21,"property":{"table_id":"cases"}}
      ],"records":[{"record_id":"rec-log","fields":{"事件概述":"交文","操作内容":"提交证据","发生时间":1710432000000i64,"案件名称":[{"record_ids":["rec-a","rec-b"],"table_id":"cases"}]}}]}
    ]})
}

#[test]
fn field_types_control_dates_and_decode_options() {
    let options = HashMap::from([("opt1".into(), "进行中".into())]);
    assert_eq!(
        field_display(&json!(["opt1"]), &json!({"type":20}), &options),
        "进行中"
    );
    assert_eq!(
        field_display(
            &json!(1710432000000i64),
            &json!({"type":5,"property":{"date_formatter":"yyyy/MM/dd HH:mm"}}),
            &options
        ),
        "2024-03-15 00:00:00"
    );
    assert_eq!(
        field_display(&json!(1710432000000i64), &json!({"type":2}), &options),
        "1710432000000"
    );
    assert_eq!(
        field_display(
            &json!([{"text":"第一段"},{"text":"第二段"}]),
            &json!({"type":1}),
            &options
        ),
        "第一段第二段"
    );
}

#[test]
fn selected_case_import_follows_record_ids_and_is_repeatable() {
    let mut conn = database();
    let snapshot = fixture();
    let report = import_snapshot(&mut conn, &snapshot, "cases", &["rec-a".into()]).unwrap();
    assert_eq!(report.cases, 2);
    assert_eq!(report.logs, 1);
    let links: i64 = conn
        .query_row("SELECT count(*) FROM case_log_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(links, 2);
    assert_eq!(report.relations, 1);
    assert_eq!(report.source_records, 4);
    let a = db::cases::get_case(&conn, "feishu:test-base:cases:rec-a").unwrap();
    assert_eq!(a.our_role.as_deref(), Some("原告"));
    assert_eq!(a.opponent_role.as_deref(), Some("第三人"));
    db::cases::update_case(&conn, &a.id, &json!({"notes":"本地修改"})).unwrap();
    let again = import_snapshot(&mut conn, &snapshot, "cases", &[]).unwrap();
    assert_eq!(again.cases, 1);
    assert_eq!(again.logs, 0);
    assert_eq!(again.relations, 0);
    assert_eq!(
        db::cases::get_case(&conn, &a.id).unwrap().notes.as_deref(),
        Some("本地修改")
    );
    let violations: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
    db::cases::delete_case(&conn, &a.id).unwrap();
    let owner: String = conn
        .query_row("SELECT case_id FROM case_logs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(owner, "feishu:test-base:cases:rec-b");
    let count: i64 = conn
        .query_row("SELECT count(*) FROM case_logs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn failed_snapshot_rolls_back_source_and_business_rows() {
    let mut conn = database();
    let mut snapshot = fixture();
    snapshot["tables"][0]["records"][1]["fields"]["案件信息"] = json!("");
    assert!(import_snapshot(&mut conn, &snapshot, "cases", &[]).is_err());
    for table in ["cases", "imported_records", "imported_links", "case_logs"] {
        let count: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "{table} must roll back");
    }
}

#[test]
fn intake_roundtrip_and_child_failure_rollback() {
    let mut conn = database();
    let data = json!({"caseName":"第三人录入测试","track":"civil_tort","clientName":"客户",
      "ourRole":"原告","opponentName":"对方","opponentRole":"被告",
      "thirdParties":"[{\"name\":\"第三人甲\",\"role\":\"第三人\",\"agent\":\"代理人\"}]",
      "caseAmount":"10000.50","legalFees":"2000","claims":"停止侵权","completedText":"已交材料",
      "filingDate":"2026-09-01","trialDate":"2026-09-20 09:30:00",
      "hearings":[{"hearingName":"庭审","hearingDate":"2026-09-20 09:30:00","actualStatus":"未开"}],
      "logs":[{"eventSummary":"交文","eventType":"submitted","eventDate":"2026-09-01 10:00:00"}],
      "tasks":[{"taskName":"准备证据","deadline":"2026-09-19","completed":false}],
      "officials":[{"name":"法官甲","role":"法官","court":"法院甲","contactDetail":"12345"}]});
    db::intake::validate_case(&data).unwrap();
    let mut case: db::cases::Case = serde_json::from_value(data.clone()).unwrap();
    case.id = "new-case".into();
    let tx = conn.transaction().unwrap();
    db::cases::insert_case(&tx, &case).unwrap();
    db::intake::insert_children(&tx, &case.id, &data).unwrap();
    tx.commit().unwrap();
    let saved = db::cases::get_case(&conn, &case.id).unwrap();
    assert_eq!(saved.third_parties, case.third_parties);
    assert_eq!(saved.case_amount, case.case_amount);
    assert_eq!(saved.claims, case.claims);
    assert_eq!(saved.completed_text, case.completed_text);
    assert_eq!(saved.trial_date, case.trial_date);
    assert!(db::cases::update_case(&conn, &case.id, &json!({"caseAmount":"-1"})).is_err());
    assert!(db::cases::update_case(&conn, &case.id, &json!({"caseAmount":-1})).is_err());
    assert!(db::cases::update_case(
        &conn,
        &case.id,
        &json!({"thirdParties":{"name":"错误形状"}})
    )
    .is_err());
    assert!(
        db::cases::update_case(&conn, &case.id, &json!({"thirdParties":[{"name":" "}]})).is_err()
    );
    assert!(db::cases::update_case(&conn, &case.id, &json!({"caseName":"  "})).is_err());
    let mut bad = data.clone();
    bad["relatedCases"] = json!([{"caseId":"missing","relationType":"cross_reference"}]);
    case.id = "failed-case".into();
    {
        let tx = conn.transaction().unwrap();
        db::cases::insert_case(&tx, &case).unwrap();
        assert!(db::intake::insert_children(&tx, &case.id, &bad).is_err());
    }
    assert!(db::cases::get_case(&conn, "failed-case").is_err());
    for key in ["relatedCases", "hearings", "logs", "tasks", "officials"] {
        let mut malformed = data.clone();
        malformed[key] = json!({"name":"不能静默丢弃的节点"});
        {
            let tx = conn.transaction().unwrap();
            db::cases::insert_case(&tx, &case).unwrap();
            assert!(db::intake::insert_children(&tx, &case.id, &malformed).is_err());
        }
        assert!(db::cases::get_case(&conn, &case.id).is_err());
    }
}

#[test]
#[ignore = "Requires a private local snapshot; performs no network requests"]
fn real_snapshot_roundtrip() {
    let path = std::env::var("CASY_TEST_FEISHU_SNAPSHOT").expect("CASY_TEST_FEISHU_SNAPSHOT");
    let snapshot: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let output = std::env::var("CASY_TEST_FEISHU_DB").ok();
    let mut conn = if let Some(path) = output {
        Connection::open(path).unwrap()
    } else {
        Connection::open_in_memory().unwrap()
    };
    schema::run_migrations(&conn, 0).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    let primary = snapshot["tables"][0]["table_id"].as_str().unwrap();
    let result = import_snapshot(&mut conn, &snapshot, primary, &[]).unwrap();
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    assert_eq!(
        result.cases,
        snapshot["tables"][0]["records"].as_array().unwrap().len()
    );
    let source = snapshot["appToken"].as_str().unwrap();
    for table in snapshot["tables"].as_array().unwrap() {
        for record in table["records"].as_array().unwrap() {
            let raw:String=conn.query_row("SELECT fields_json FROM imported_records WHERE source=?1 AND table_id=?2 AND record_id=?3",
                params![source,table["table_id"].as_str().unwrap(),record["record_id"].as_str().unwrap()],|r|r.get(0)).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&raw).unwrap(),
                record["fields"]
            );
        }
    }
    use base64::Engine;
    for asset in snapshot["assets"].as_array().unwrap() {
        let stored: Vec<u8> = conn
            .query_row(
                "SELECT content FROM imported_assets WHERE source=?1 AND file_token=?2",
                params![source, asset["fileToken"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            stored,
            base64::engine::general_purpose::STANDARD
                .decode(asset["contentBase64"].as_str().unwrap())
                .unwrap()
        );
    }
    let mapping = [
        ("案件信息", "case_name"),
        ("案号", "case_no"),
        ("案由", "cause_action"),
        ("客户名称", "client_name"),
        ("我方诉讼地位", "our_role"),
        ("诉讼地位", "opponent_role"),
        ("对方名称", "opponent_name"),
        ("对方代理律所", "opponent_firm"),
        ("对方代理人", "opponent_agent"),
        ("案件进展", "case_progress"),
        ("案件结果", "case_result"),
        ("已完成", "completed_text"),
        ("备注", "notes"),
        ("专利名称", "patent_name"),
        ("专利申请号", "patent_app_no"),
        ("管辖异议", "jurisdiction_objection"),
    ];
    for record in snapshot["tables"][0]["records"].as_array().unwrap() {
        let id = format!(
            "feishu:{source}:{primary}:{}",
            record["record_id"].as_str().unwrap()
        );
        for (name, column) in mapping {
            if let Some(expected) = record["fields"][name]
                .as_str()
                .filter(|v| !v.trim().is_empty())
            {
                let actual: Option<String> = conn
                    .query_row(
                        &format!("SELECT {column} FROM cases WHERE id=?1"),
                        params![id],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(
                    actual.as_deref(),
                    Some(expected.trim()),
                    "field {name} was not preserved"
                );
            }
        }
    }
    let again = import_snapshot(&mut conn, &snapshot, primary, &[]).unwrap();
    assert_eq!(
        again.cases + again.logs + again.hearings + again.tasks + again.officials + again.relations,
        0
    );
    let fk: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(fk, 0);
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}

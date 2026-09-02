use std::collections::HashMap;
use tempfile::NamedTempFile;

use casy_lib::commands::import_excel::{
    clean_amount_str, clean_array_to_json, clean_date_str, excel_import_cases,
    excel_import_subtable, excel_inspect_sheet, infer_track_and_route, CaseImportConfig,
    SubtableImportConfig,
};
use rust_xlsxwriter::Workbook;

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_excel_serial_and_multi_format_date_cleaning() {
    let _guard = TEST_MUTEX.lock().await;
    // 1. Excel Serial Number
    // 45123 对应 2023-07-16
    let d1 = clean_date_str("45123");
    assert_eq!(d1, Some("2023-07-16".to_string()));

    // 2. 中文格式
    let d2 = clean_date_str("2024年05月01日");
    assert_eq!(d2, Some("2024-05-01".to_string()));

    let d3 = clean_date_str("2024年5月1日 14:30:00");
    assert_eq!(d3, Some("2024-05-01".to_string()));

    // 3. 点分 / 斜杠
    let d4 = clean_date_str("2024.5.1");
    assert_eq!(d4, Some("2024-05-01".to_string()));

    let d5 = clean_date_str("2024/05/01");
    assert_eq!(d5, Some("2024-05-01".to_string()));

    // 4. 2位年份
    let d6 = clean_date_str("24-05-01");
    assert_eq!(d6, Some("2024-05-01".to_string()));

    // 5. 8位紧凑
    let d7 = clean_date_str("20240501");
    assert_eq!(d7, Some("2024-05-01".to_string()));

    // 6. 空白或非法
    assert_eq!(clean_date_str(""), None);
    assert_eq!(clean_date_str("未知时间"), None);
}

#[test]
fn test_amount_and_array_cleaning() {
    // 金额清洗
    assert_eq!(
        clean_amount_str("￥1,200,000.00元"),
        Some("1200000.00".to_string())
    );
    assert_eq!(
        clean_amount_str("150.5万元"),
        Some("1505000.00".to_string())
    );
    assert_eq!(clean_amount_str("1.2 亿"), Some("120000000.00".to_string()));
    assert_eq!(clean_amount_str("50k"), Some("50000.00".to_string()));
    assert_eq!(clean_amount_str(""), None);

    // 人员多值拆分
    let attorneys_json = clean_array_to_json("张三、李四 / 王五, 赵六; 孙七\n周八").unwrap();
    let parsed: Vec<String> = serde_json::from_str(&attorneys_json).unwrap();
    assert_eq!(parsed, vec!["张三", "李四", "王五", "赵六", "孙七", "周八"]);
}

#[test]
fn test_track_and_route_inference() {
    let (t1, _raw1, r1) =
        infer_track_and_route(None, None, None, "某某发明专利权无效宣告", Some("专利无效"));
    assert_eq!(t1, "patent_invalidation");
    assert_eq!(r1, "专利无效");

    let (t2, _raw2, r2) =
        infer_track_and_route(None, None, None, "不服国知局行政决定起诉", Some("行政诉讼"));
    assert_eq!(t2, "admin_litigation");
    assert_eq!(r2, "行政诉讼");

    let (t3, _raw3, r3) = infer_track_and_route(
        None,
        None,
        None,
        "侵犯计算机软件著作权纠纷",
        Some("民事侵权"),
    );
    assert_eq!(t3, "civil_tort");
    assert_eq!(r3, "民事诉讼");
}

#[tokio::test]
async fn test_end_to_end_excel_import_with_header_offset_and_conflict() {
    let _guard = TEST_MUTEX.lock().await;
    casy_lib::db::enable_test_mode();
    casy_lib::db::init_db(&casy_lib::db::open_db().unwrap()).unwrap();
    // 1. 使用 rust_xlsxwriter 动态生成一个带表头偏移和不规则数据的真实 Excel
    let temp_dir = tempfile::tempdir().unwrap();
    let excel_path = temp_dir.path().join("test_cases_import.xlsx");

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // 第 0 行：大标题
    worksheet
        .write(0, 0, "XX律师事务所 2024 知识产权案件台账")
        .unwrap();
    // 第 1 行：副标题 / 制表人
    worksheet
        .write(1, 0, "制表人: 张三  日期: 2024-05-01")
        .unwrap();

    // 第 2 行：真实表头 (0-indexed 2)
    worksheet.write(2, 0, "法院案号").unwrap();
    worksheet.write(2, 1, "案件全称").unwrap();
    worksheet.write(2, 2, "委托人").unwrap();
    worksheet.write(2, 3, "对方当事人").unwrap();
    worksheet.write(2, 4, "管辖法院").unwrap();
    worksheet.write(2, 5, "主办律师").unwrap();
    worksheet.write(2, 6, "收案时间").unwrap();
    worksheet.write(2, 7, "备注说明").unwrap();

    let uid = &uuid::Uuid::new_v4().to_string()[..8];
    let case_no_1 = format!("(2024)测试知民终-{}", uid);
    let case_name_1 = format!("华为与中兴通信标准必要专利侵权案-{}", uid);
    let case_no_2 = format!("(2024)测试京民初-{}", uid);
    let case_name_2 = format!("华为与大唐移动芯片专利侵权案-{}", uid);

    // 第 3 行：数据行 1 (标准)
    worksheet.write(3, 0, &case_no_1).unwrap();
    worksheet.write(3, 1, &case_name_1).unwrap();
    worksheet.write(3, 2, "华为技术有限公司").unwrap();
    worksheet.write(3, 3, "中兴通讯股份有限公司").unwrap();
    worksheet.write(3, 4, "最高人民法院").unwrap();
    worksheet.write(3, 5, "李律师、王律师").unwrap();
    worksheet.write(3, 6, "2024.03.15").unwrap();
    worksheet.write(3, 7, "一审胜诉，二审跟进").unwrap();

    // 第 4 行：数据行 2 (委托人为空，模拟合并单元格需 Forward-Fill，日期为中文)
    worksheet.write(4, 0, &case_no_2).unwrap();
    worksheet.write(4, 1, &case_name_2).unwrap();
    worksheet.write(4, 2, "").unwrap(); // 空白，等待 forward-fill 继承上一行的华为
    worksheet.write(4, 3, "大唐移动通信").unwrap();
    worksheet.write(4, 4, "北京知识产权法院").unwrap();
    worksheet.write(4, 5, "赵律师 / 钱律师").unwrap();
    worksheet.write(4, 6, "2024年4月10日").unwrap();
    worksheet.write(4, 7, "待提交答辩状").unwrap();

    // 第 5 行：空行
    // 第 6 行：尾部汇总行
    worksheet.write(6, 0, "合计：共 2 件案件").unwrap();

    workbook.save(&excel_path).unwrap();

    let path_str = excel_path.to_string_lossy().to_string();

    // 2. 探测工作表结构与表头行
    let inspect_res = excel_inspect_sheet(path_str.clone(), "Sheet1".to_string(), None)
        .await
        .unwrap();

    // 验证智能表头探测器定位到第 2 行 (0-indexed)
    assert_eq!(inspect_res.detected_header_row, 2);
    assert_eq!(inspect_res.columns.len(), 8);

    // 3. 配置导入规则
    let mut mappings = HashMap::new();
    mappings.insert(0, "caseNo".to_string());
    mappings.insert(1, "caseName".to_string());
    mappings.insert(2, "clientName".to_string());
    mappings.insert(3, "opponentName".to_string());
    mappings.insert(4, "court".to_string());
    mappings.insert(5, "attorneys".to_string());
    mappings.insert(6, "filingDate".to_string());
    mappings.insert(7, "notes".to_string());

    let config = CaseImportConfig {
        header_row: 2,
        column_mappings: mappings,
        forward_fill_columns: vec![2], // 委托人列开启 Forward-Fill
        conflict_strategy: "skip".to_string(),
        default_track: Some("civil_tort".to_string()),
    };

    // 4. 执行导入
    let report = excel_import_cases(path_str.clone(), "Sheet1".to_string(), config.clone())
        .await
        .unwrap();

    println!("DEBUG report: {:?}", report);
    assert_eq!(report.errors, Vec::<String>::new());
    assert_eq!(report.created_count, 2);
    assert_eq!(report.failed_count, 0);

    // 5. 再次以 "skip" 策略导入相同文件，验证查重跳过（2件重复 + 2行空行/汇总 = 4）
    let report_dup = excel_import_cases(path_str.clone(), "Sheet1".to_string(), config)
        .await
        .unwrap();
    assert_eq!(report_dup.created_count, 0);
    assert_eq!(report_dup.skipped_count, 4);
}

#[tokio::test]
async fn test_real_user_excel_file_inspection_and_import() {
    let _guard = TEST_MUTEX.lock().await;
    casy_lib::db::enable_test_mode();
    casy_lib::db::init_db(&casy_lib::db::open_db().unwrap()).unwrap();
    let path = "/Users/only/Documents/Workspace/【案件进程表】2025.07.21更新.xlsx".to_string();
    // 1. 探测工作表结构与复合表头
    let inspect_res = excel_inspect_sheet(path.clone(), "Sheet1".to_string(), None)
        .await
        .unwrap();

    // 2. 将推荐映射转为导入配置
    let mut mappings = HashMap::new();
    for col in &inspect_res.columns {
        if let Some(f) = &col.suggested_field {
            mappings.insert(col.column_index, f.clone());
        }
    }

    let config = CaseImportConfig {
        header_row: inspect_res.detected_header_row,
        column_mappings: mappings,
        forward_fill_columns: vec![5, 6], // 我方名称、我方诉讼地位开启合并单元格向下填充
        conflict_strategy: "update".to_string(),
        default_track: Some("patent_invalidation".to_string()),
    };

    // 3. 执行真实导入
    let report = excel_import_cases(path.clone(), "Sheet1".to_string(), config)
        .await
        .unwrap();

    println!("Real Excel Import Report: {:?}", report);
    assert_eq!(
        report.failed_count, 0,
        "Errors occurred: {:?}",
        report.errors
    );
    assert!(report.total_rows_processed > 0, "No rows processed");
}

#[tokio::test]
async fn test_multi_row_case_hearings_and_notes_aggregation() {
    let _guard = TEST_MUTEX.lock().await;
    casy_lib::db::enable_test_mode();
    casy_lib::db::init_db(&casy_lib::db::open_db().unwrap()).unwrap();
    let test_case_no = format!(
        "(2024)京知民初{}号",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // 写入表头
    worksheet.write_string(0, 0, "案号").unwrap();
    worksheet.write_string(0, 1, "案件名称").unwrap();
    worksheet.write_string(0, 2, "委托方").unwrap();
    worksheet.write_string(0, 3, "开庭时间").unwrap();
    worksheet.write_string(0, 4, "办案日志").unwrap();

    // 第 1 行: 主案件与第 1 次开庭
    worksheet.write_string(1, 0, &test_case_no).unwrap();
    worksheet
        .write_string(1, 1, "某科技公司专利侵权案")
        .unwrap();
    worksheet.write_string(1, 2, "某科技有限公司").unwrap();
    worksheet.write_string(1, 3, "2025-04-10").unwrap();
    worksheet
        .write_string(1, 4, "第一次开庭完毕，法官要求补充证据")
        .unwrap();

    // 第 2 行: 相同案号，记录第 2 次开庭与差异日志
    worksheet.write_string(2, 0, &test_case_no).unwrap();
    worksheet
        .write_string(2, 1, "某科技公司专利侵权案")
        .unwrap();
    worksheet.write_string(2, 2, "某科技有限公司").unwrap();
    worksheet.write_string(2, 3, "2025-05-20").unwrap();
    worksheet
        .write_string(2, 4, "第二次开庭质证涉案专利权利要求")
        .unwrap();

    // 第 3 行: 相同案号，记录第 3 次开庭与差异日志
    worksheet.write_string(3, 0, &test_case_no).unwrap();
    worksheet
        .write_string(3, 1, "某科技公司专利侵权案")
        .unwrap();
    worksheet.write_string(3, 2, "某科技有限公司").unwrap();
    worksheet.write_string(3, 3, "2025-06-30").unwrap();
    worksheet.write_string(3, 4, "第三次口审听证总结").unwrap();

    // 第 4 行: 相同案号，记录第 4 次开庭
    worksheet.write_string(4, 0, &test_case_no).unwrap();
    worksheet
        .write_string(4, 1, "某科技公司专利侵权案")
        .unwrap();
    worksheet.write_string(4, 2, "某科技有限公司").unwrap();
    worksheet.write_string(4, 3, "2025-08-15").unwrap();
    worksheet.write_string(4, 4, "第四次开庭宣判").unwrap();

    let temp_file = NamedTempFile::new().unwrap();
    let path_buf = temp_file.into_temp_path();
    let path_str = path_buf.to_string_lossy().to_string();
    workbook.save(&path_str).unwrap();

    let mut mappings = HashMap::new();
    mappings.insert(0, "caseNo".to_string());
    mappings.insert(1, "caseName".to_string());
    mappings.insert(2, "clientName".to_string());
    mappings.insert(3, "trialDate".to_string());
    mappings.insert(4, "notes".to_string());

    let config = CaseImportConfig {
        header_row: 0,
        column_mappings: mappings,
        forward_fill_columns: vec![],
        conflict_strategy: "update".to_string(), // 差异行增量汇聚更新
        default_track: Some("patent_invalidation".to_string()),
    };

    let report = excel_import_cases(path_str, "Sheet1".to_string(), config)
        .await
        .unwrap();

    println!("Multi-row delta import report: {:?}", report);
    assert_eq!(report.failed_count, 0);
    // 4 行相同案件：第 1 行新建，第 2、3、4 行增量汇聚更新
    assert_eq!(report.created_count, 1);
    assert_eq!(report.updated_count, 3);

    // 验证数据库中 hearings 表已全量沉淀 4 次开庭记录
    let conn = casy_lib::db::open_db().unwrap();
    let hearing_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM hearings h JOIN cases c ON c.id = h.case_id WHERE c.case_no = ?1",
            rusqlite::params![&test_case_no],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        hearing_count, 4,
        "Expected 4 distinct hearings in sub-table, got {}",
        hearing_count
    );

    // 验证案件备忘录中的日志被无损合并追加
    let case_notes: String = conn
        .query_row(
            "SELECT notes FROM cases WHERE case_no = ?1",
            rusqlite::params![&test_case_no],
            |r| r.get(0),
        )
        .unwrap();
    assert!(case_notes.contains("第一次开庭"));
    assert!(case_notes.contains("第二次开庭"));
    assert!(case_notes.contains("第三次口审"));
    assert!(case_notes.contains("第四次开庭"));
}

#[tokio::test]
async fn test_subtable_tasks_and_hearings_relational_import() {
    let _guard = TEST_MUTEX.lock().await;
    casy_lib::db::enable_test_mode();
    casy_lib::db::init_db(&casy_lib::db::open_db().unwrap()).unwrap();
    let test_case_no = format!(
        "(2024)京知民分表{}号",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let conn = casy_lib::db::open_db().unwrap();
    let parent_case_id = casy_lib::db::new_id();

    // 1. 预先在案件主表中插入主案件
    conn.execute(
        "INSERT INTO cases (id, case_name, case_no, client_name, created_at, updated_at)
         VALUES (?1, '某芯片专利侵权案', ?2, '某微电子公司', datetime('now','localtime'), datetime('now','localtime'))",
        rusqlite::params![&parent_case_id, &test_case_no],
    ).unwrap();

    // 2. 构造任务分表 Excel
    let mut workbook = Workbook::new();
    let sheet_tasks = workbook.add_worksheet();
    sheet_tasks.set_name("任务清单").unwrap();

    sheet_tasks.write_string(0, 0, "任务名称").unwrap();
    sheet_tasks.write_string(0, 1, "关联案号").unwrap();
    sheet_tasks.write_string(0, 2, "截止日期").unwrap();
    sheet_tasks.write_string(0, 3, "优先级").unwrap();
    sheet_tasks.write_string(0, 4, "执行人").unwrap();
    sheet_tasks.write_string(0, 5, "完成状态").unwrap();

    sheet_tasks
        .write_string(1, 0, "撰写无效宣告请求书")
        .unwrap();
    sheet_tasks.write_string(1, 1, &test_case_no).unwrap();
    sheet_tasks.write_string(1, 2, "2025-05-01").unwrap();
    sheet_tasks.write_string(1, 3, "high").unwrap();
    sheet_tasks.write_string(1, 4, "张律师").unwrap();
    sheet_tasks.write_string(1, 5, "未完成").unwrap();

    sheet_tasks
        .write_string(2, 0, "检索对比文件与现有技术")
        .unwrap();
    sheet_tasks.write_string(2, 1, &test_case_no).unwrap();
    sheet_tasks.write_string(2, 2, "2025-04-15").unwrap();
    sheet_tasks.write_string(2, 3, "medium").unwrap();
    sheet_tasks.write_string(2, 4, "李助理").unwrap();
    sheet_tasks.write_string(2, 5, "已完成").unwrap();

    let temp_file = NamedTempFile::new().unwrap();
    let path_buf = temp_file.into_temp_path();
    let path_str = path_buf.to_string_lossy().to_string();
    workbook.save(&path_str).unwrap();

    let mut task_mappings = HashMap::new();
    task_mappings.insert(0, "taskName".to_string());
    task_mappings.insert(1, "caseNo".to_string());
    task_mappings.insert(2, "deadline".to_string());
    task_mappings.insert(3, "priority".to_string());
    task_mappings.insert(4, "assignee".to_string());
    task_mappings.insert(5, "completed".to_string());

    let task_config = SubtableImportConfig {
        target_entity: "tasks".to_string(),
        header_row: 0,
        column_mappings: task_mappings,
        forward_fill_columns: vec![],
    };

    // 3. 执行任务分表导入
    let task_report = excel_import_subtable(path_str.clone(), "任务清单".to_string(), task_config)
        .await
        .unwrap();

    println!("Task subtable import report: {:?}", task_report);
    assert_eq!(task_report.failed_count, 0);
    assert_eq!(task_report.created_count, 2);
    assert_eq!(task_report.linked_cases_count, 2);

    // 验证数据库中 tasks 表正确挂载了 parent_case_id
    let linked_task_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE case_id = ?1",
            rusqlite::params![&parent_case_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        linked_task_count, 2,
        "Tasks should be linked to parent case"
    );
}

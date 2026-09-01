use casy_lib::commands::import_feishu::{convert_feishu_val_to_string, extract_feishu_tokens};
use serde_json::json;

#[test]
fn test_extract_feishu_tokens_from_various_url_formats() {
    // 1. 标准带 table 参数的多维表格 URL
    let url1 =
        "https://myfirm.feishu.cn/base/bascny4kUu9B1234567890ABCDE?table=tblkL90XYZ123&view=vew098";
    let (app_token1, table_id1) = extract_feishu_tokens(url1);
    assert_eq!(app_token1, Some("bascny4kUu9B1234567890ABCDE".to_string()));
    assert_eq!(table_id1, Some("tblkL90XYZ123".to_string()));

    // 2. 仅有 base token 的 URL
    let url2 = "https://feishu.cn/base/bascn998877665544332211";
    let (app_token2, table_id2) = extract_feishu_tokens(url2);
    assert_eq!(app_token2, Some("bascn998877665544332211".to_string()));
    assert_eq!(table_id2, None);

    // 3. /apps/ 格式的飞书应用链接
    let url3 = "https://open.feishu.cn/apps/bascn11223344556677889900/tables?table=tbl1234567890";
    let (app_token3, table_id3) = extract_feishu_tokens(url3);
    assert_eq!(app_token3, Some("bascn11223344556677889900".to_string()));
    assert_eq!(table_id3, Some("tbl1234567890".to_string()));

    // 4. 纯 token 输入
    let (app_token4, _) = extract_feishu_tokens("bascnABCDEF1234567890");
    assert_eq!(app_token4, Some("bascnABCDEF1234567890".to_string()));

    // 5. 用户真实飞书多维表格链接 1 (带 from 参数)
    let url_user1 = "https://hs2wxdogy2.feishu.cn/base/TW48bdi2daCMCGsucGZcQoivnpV?from=from_copylink";
    let (app_token_u1, table_id_u1) = extract_feishu_tokens(url_user1);
    assert_eq!(app_token_u1, Some("TW48bdi2daCMCGsucGZcQoivnpV".to_string()));
    assert_eq!(table_id_u1, None);

    // 6. 用户真实飞书多维表格链接 2 (带 table 与 view 参数)
    let url_user2 = "https://my.feishu.cn/base/JYWEb4e0BayQrrsw5Tdct0c3n5g?table=tbl4fMNw2UJfXBgy&view=vew6lP2d80";
    let (app_token_u2, table_id_u2) = extract_feishu_tokens(url_user2);
    assert_eq!(app_token_u2, Some("JYWEb4e0BayQrrsw5Tdct0c3n5g".to_string()));
    assert_eq!(table_id_u2, Some("tbl4fMNw2UJfXBgy".to_string()));
}

#[test]
fn test_convert_feishu_field_values_to_normalized_string() {
    // 1. 纯字符串
    assert_eq!(
        convert_feishu_val_to_string(&json!("(2024)最高法知民终1001号")),
        "(2024)最高法知民终1001号"
    );

    // 2. 毫秒时间戳转换 (例如 2024-03-15 00:00:00 UTC 对应 1710460800000)
    let ts_val = json!(1710460800000i64);
    assert_eq!(convert_feishu_val_to_string(&ts_val), "2024-03-15");

    // 3. 普通数字
    let num_val = json!(1500000.5);
    assert_eq!(convert_feishu_val_to_string(&num_val), "1500000.50");

    // 4. 人员对象数组 (User Array)
    let users_val = json!([
        { "name": "李律师", "id": "ou_123" },
        { "name": "王律师", "id": "ou_456" }
    ]);
    assert_eq!(convert_feishu_val_to_string(&users_val), "李律师、王律师");

    // 5. 选项字符串数组 (MultiSelect)
    let tags_val = json!(["一审", "已立案"]);
    assert_eq!(convert_feishu_val_to_string(&tags_val), "一审、已立案");

    // 6. 链接/文本对象
    let link_val = json!({ "text": "北京知识产权法院", "link": "https://court.gov.cn" });
    assert_eq!(convert_feishu_val_to_string(&link_val), "北京知识产权法院");

    // 7. 空值
    assert_eq!(convert_feishu_val_to_string(&json!(null)), "");
}

#[test]
fn test_user_feishu_table_25_columns_matching() {
    use casy_lib::commands::import_excel::match_field_confidence;

    let columns = vec![
        ("案件信息", "caseName"),
        ("案由", "causeAction"),
        ("委托方", "clientName"),
        ("诉讼地位", "ourRole"),
        ("相对方", "opponentName"),
        ("案号", "caseNo"),
        ("进展", "caseProgress"),
        ("办案日志", "notes"),
        ("案件阶段", "caseLevel"),
        ("接案日期", "filingDate"),
        ("审理法院", "court"),
        ("法院电话", "clerk"),
        ("诉讼请求", "notes"),
        ("律师费用", "notes"),
        ("律师费到账情况", "notes"),
        ("负责律师", "attorneys"),
        ("一次开庭丨口审", "trialDate"),
        ("二次开庭丨口审", "trial2Date"),
        ("三次开庭丨口审", "trial3Date"),
        ("未来开庭", "trialDate"),
        ("最近已开庭", "trialDate"),
        ("承办法官", "judgePanel"),
        ("保全结束时间", "stayDate"),
        ("保全开始时间", "stayDate"),
        ("重要紧急程度", "priority"),
        ("提交续保书面申请", "notes"),
        ("案件编号", "internalNo"),
    ];

    for (header, expected_field) in columns {
        let (matched_field, conf) = match_field_confidence(header);
        assert_eq!(
            matched_field.as_deref(),
            Some(expected_field),
            "Column [{}] expected [{}] but got [{:?}]",
            header,
            expected_field,
            matched_field
        );
        assert!(conf >= 0.8, "Confidence for [{}] was too low: {}", header, conf);
    }
}

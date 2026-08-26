//! 演示数据种子器（Dogfooding / 可视化验证辅助）
//!
//! 防护三重（审计 P0#1 修复）：
//! 1. 仅当库内无任何案件时允许执行
//! 2. 整体事务包裹——任一步失败全部回滚，杜绝半套脏数据
//! 3. 外键合规：个人任务 case_id 走 NULL；hearing_record NOT NULL 已供值
//!
//! 空库集成测试见文件底部 tests 模块。

use super::run_blocking;
use crate::db;
use rusqlite::Connection;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedReport {
    pub cases: usize,
    pub projects_personal: usize,
    pub tasks: usize,
    pub areas: usize,
    pub knowledge: usize,
    pub hearings: usize,
}

fn plus_days(n: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(n))
        .format("%Y-%m-%d")
        .to_string()
}

/// 同步实现核心（命令层负责事务与异步壳；测试可直接调用）
/// 错误信息带 step_ 前缀用于定位失败语句。
fn seed_demo_data_impl(conn: &Connection) -> Result<SeedReport, String> {
    let existing: i64 = conn
        .query_row("SELECT COUNT(*) FROM cases", [], |r| r.get(0))
        .map_err(|e| format!("step_guard: {e}"))?;
    if existing > 0 {
        return Err("数据库中已有案件数据，拒绝注入演示数据（保护真实卷宗）".into());
    }

    let now = db::now_local();
    let mkid = || db::new_id();

    // ── 领域 ×3 ──
    let areas: Vec<(&str, &str)> = vec![
        ("area-demo-1", "执业发展"),
        ("area-demo-2", "客户与市场"),
        ("area-demo-3", "专业学习"),
    ];
    for (i, (id, name)) in areas.iter().enumerate() {
        conn.execute(
            "INSERT OR IGNORE INTO areas (id,name,sort_order,created_at,updated_at) VALUES (?1,?2,?3,?4,?4)",
            rusqlite::params![id, name, i as i32, now],
        )
        .map_err(|e| format!("step_areas: {e}"))?;
    }

    // ── 法律案件 ×2 ──
    let case_a = mkid();
    let case_b = mkid();
    for (id, no, name, client, court, track) in [
        (&case_a, "2026-0001", "张三诉某公司专利权无效宣告案", "张三", "国家知识产权局", "patent_invalidation"),
        (&case_b, "2026-0002", "李四发明专利行政纠纷案", "李四", "北京知识产权法院", "admin_litigation"),
    ] {
        conn.execute(
            "INSERT INTO cases (id, track, case_name, case_no, client_name, our_role,
                opponent_name, opponent_firm, court, filing_date, area_id, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,'代理人',?6,?7,?8,?9,'area-demo-1',?10,?10)",
            rusqlite::params![id, track, name, no, client,
                format!("对方当事人·{no}"), "某律所", court, plus_days(-20), now],
        )
        .map_err(|e| format!("step_cases: {e}"))?;
    }

    // 庭审锚点（hearing_record NOT NULL 已供值）
    for (cid, date, name, record) in [
        (&case_a, plus_days(3), "口头审理", "演示：口审记录占位"),
        (&case_b, plus_days(10), "一审开庭", "演示：开庭记录占位"),
    ] {
        conn.execute(
            "INSERT INTO hearings (id, case_id, hearing_date, hearing_name, hearing_record, created_at)
             VALUES (?1,?2,?3,?4,?5,?6)",
            rusqlite::params![mkid(), cid, date, name, record, now],
        )
        .map_err(|e| format!("step_hearings: {e}"))?;
    }
    let hearings_n = 2;

    // 分级期限 R1/R2/R3
    for (cid, dname, days, level) in [
        (&case_a, "专利权人陈述意见截止", 7_i64, 1),
        (&case_b, "答辩状提交截止", 2_i64, 2),
        (&case_b, "缴费回执上传", 0_i64, 3),
    ] {
        conn.execute(
            "INSERT INTO case_deadlines (id, case_id, deadline_name, due_date, completed,
                deadline_source, notes, created_at)
             VALUES (?1,?2,?3,?4,0,'manual',?5,?6)",
            rusqlite::params![mkid(), cid, dname, plus_days(days),
                format!("演示期限 R{level}"), now],
        )
        .map_err(|e| format!("step_deadlines: {e}"))?;
    }

    // ── 个人项目 ×1 ──
    conn.execute(
        "INSERT INTO projects (id,name,kind,description,status,created_at,updated_at)
         VALUES ('proj-demo-1','律所数字化改造','personal','把纸质流程迁到 Casy 的内部项目','active',?1,?1)",
        rusqlite::params![now],
    )
    .map_err(|e| format!("step_project: {e}"))?;

    // ── 任务群（bucket, case_id:Option, name, type, due_offset, due_time, est, focus, priority）──
    let tasks_spec: Vec<(&str, Option<&str>, &str, &str, Option<i64>, Option<&str>, Option<i32>, i32, &str)> = vec![
        ("today", Some(case_a.as_str()), "核对口审证据清单", "action", Some(0), Some("09:30"), Some(45), 1, "urgent_important"),
        ("today", Some(case_a.as_str()), "起草答辩状初稿", "action", Some(0), None, Some(120), 0, "important"),
        ("today", Some(case_b.as_str()), "回复客户进度询问", "waiting", Some(0), Some("16:00"), None, 0, "normal"),
        ("today", None, "整理会议行动项", "action", Some(0), None, Some(30), 0, "normal"),
        ("today", Some(case_b.as_str()), "复核缴费回执", "action", Some(0), Some("17:30"), Some(15), 1, "urgent_important"),
        ("today", None, "准备客户需求评审", "action", Some(1), Some("10:00"), Some(60), 0, "important"),
        ("today", None, "提交上季度报表", "action", Some(-2), Some("12:00"), Some(30), 0, "high"),
        ("anytime", None, "深度学习：无效程序新规", "action", None, None, Some(90), 0, "low"),
        ("anytime", None, "更新律师画像工作时段", "action", None, None, None, 0, "low"),
        ("someday", None, "搭建类案检索库", "someday", None, None, None, 0, "low"),
    ];

    let mut task_n = 0usize;
    let mut focus_parent: Option<String> = None;
    for (bucket, cid, name, ttype, due_off, due_time, est, focus, prio) in &tasks_spec {
        let id = mkid();
        let due_final = due_off.map(|d| plus_days(d));
        conn.execute(
            "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed,
                task_type, start_date, due_date, due_time, context, flagged, start_bucket, today_index,
                estimated_minutes, area_id, is_focus, recurrence_rule, created_at)
             VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, 0, ?7, ?8, ?9, ?10, '@办公室', 0, ?11, 0, ?12, 'area-demo-3', ?13, NULL, ?14)",
            rusqlite::params![
                id, cid, name, now, due_final, prio, ttype, due_final, due_final,
                due_time, bucket, est, focus, now,
            ],
        )
        .map_err(|e| format!("step_task_{name}: {e}"))?;
        if name.starts_with("核对") {
            focus_parent = Some(id);
        }
        task_n += 1;
    }

    // 子任务示例
    if let Some(parent) = focus_parent {
        for sub in ["打印证据清单终稿", "与客户确认出庭时间"] {
            conn.execute(
                "INSERT INTO tasks (id, case_id, task_name, created_date, priority, completed, task_type,
                    start_bucket, parent_task_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'normal', 0, 'action', 'today', ?5, ?6)",
                rusqlite::params![mkid(), &case_a, sub, now, parent, now],
            )
            .map_err(|e| format!("step_subtask: {e}"))?;
            task_n += 1;
        }
    }

    // ── 知识 ×3 ──（category 走 CHECK 白名单内的 'other'）
    for title in ["无效宣告程序时间轴速查", "口审应对清单", "客户沟通模板：进度同步"] {
        conn.execute(
            "INSERT INTO knowledge_items (id, title, category, content, created_at, updated_at)
             VALUES (?1,?2,'other',?3,?4,?4)",
            rusqlite::params![mkid(), title, format!("演示知识：{title}（Dogfooding 用）"), now],
        )
        .map_err(|e| format!("step_knowledge: {e}"))?;
    }
    let knowledge_n = 3;

    Ok(SeedReport {
        cases: 2,
        projects_personal: 1,
        tasks: task_n,
        areas: areas.len(),
        knowledge: knowledge_n,
        hearings: hearings_n,
    })
}

#[tauri::command]
pub async fn seed_demo_data() -> Result<SeedReport, String> {
    run_blocking(move || {
        db::with_conn(|conn| {
            // 审计 P0#1：整体事务——任一步失败回滚，杜绝半套脏数据
            let tx = conn.unchecked_transaction()?;
            let report = seed_demo_data_impl(&tx).map_err(anyhow::Error::msg)?;
            tx.commit()?;
            Ok(report)
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open memory db");
        crate::db::init_db(&conn).expect("init schema");
        conn
    }

    #[test]
    fn seed_on_empty_db_inserts_full_set() {
        let conn = fresh_db();
        let r = seed_demo_data_impl(&conn).expect("seed should succeed");
        assert_eq!(r.cases, 2);
        assert_eq!(r.projects_personal, 1);
        assert!(r.tasks >= 10);
        assert_eq!(r.hearings, 2);

        let orphans: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks t WHERE t.case_id IS NOT NULL
             AND NOT EXISTS (SELECT 1 FROM cases c WHERE c.id = t.case_id)",
            [], |r| r.get(0)).unwrap();
        assert_eq!(orphans, 0);

        let missing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM hearings WHERE hearing_record IS NULL OR hearing_record = ''",
            [], |r| r.get(0)).unwrap();
        assert_eq!(missing, 0);
    }

    #[test]
    fn seed_rejects_non_empty_db() {
        let conn = fresh_db();
        seed_demo_data_impl(&conn).expect("first seed ok");
        let err = seed_demo_data_impl(&conn).unwrap_err();
        assert!(err.contains("拒绝注入"), "应明确拒绝: {err}");
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM cases", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
    }
}

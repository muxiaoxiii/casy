//! 演示数据种子器（Dogfooding / 可视化验证辅助）
//!
//! 防护：仅当库内无任何案件时允许执行（首跑体验填充，不污染真实数据）。
//! 内容：2 个法律案件（含未来庭审与分级期限）+ 1 个个人项目 + 跨桶任务群
//! （含逾期/今日到期/重点标记）+ 领域 ×3 + 知识 ×3 —— 让看板四图与各透视有血肉。

use super::run_blocking;
use crate::db;

#[derive(serde::Serialize)]
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

#[tauri::command]
pub async fn seed_demo_data() -> Result<SeedReport, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // ── 防护：非空库拒绝 ──
        let existing: i64 =
            conn.query_row("SELECT COUNT(*) FROM cases", [], |r| r.get(0)).map_err(|e| anyhow::anyhow!("{e}"))?;
        if existing > 0 {
            return Err(anyhow::anyhow!("数据库中已有案件数据，拒绝注入演示数据（保护真实卷宗）"));
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
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        }

        // ── 法律案件 ×2（含庭审/期限锚点）──
        let case_a = mkid();
        let case_b = mkid();
        for (id, no, name, client, court, track, folder) in [
            (
                &case_a,
                "2026-0001",
                "张三诉某公司专利权无效宣告案",
                "张三",
                "国家知识产权局",
                "patent_invalidation",
                "cases/2026-0001",
            ),
            (
                &case_b,
                "2026-0002",
                "李四发明专利行政纠纷案",
                "李四",
                "北京知识产权法院",
                "admin_litigation",
                "cases/2026-0002",
            ),
        ] {
            conn.execute(
                "INSERT INTO cases (id, track, case_name, case_no, client_name, our_role,
                    opponent_name, opponent_firm, court, filing_date, area_id, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,'代理人',?6,?7,?8,?9,'area-demo-1',?10,?10)",
                rusqlite::params![id, track, name, no, client, format!("对方当事人·{no}"), "某律所", court, plus_days(-20), now],
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            void_folder(folder);
        }

        // 庭审锚点：A 案 3 天后口审、B 案 10 天后开庭
        for (cid, date, name) in [
            (&case_a, plus_days(3), "口头审理"),
            (&case_b, plus_days(10), "一审开庭"),
        ] {
            conn.execute(
                "INSERT INTO hearings (id, case_id, hearing_date, hearing_name, created_at)
                 VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![mkid(), cid, date, name, now],
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        }

        // 分级期限：R1(7d)/R2(2d)/R3(0d 今天)
        for (cid, dname, days, level) in [
            (&case_a, "专利权人陈述意见截止", 7_i64, 1),
            (&case_b, "答辩状提交截止", 2_i64, 2),
            (&case_b, "缴费回执上传", 0_i64, 3),
        ] {
            conn.execute(
                "INSERT INTO case_deadlines (id, case_id, deadline_name, due_date, completed, remark, created_at)
                 VALUES (?1,?2,?3,?4,0,?5,?6)",
                rusqlite::params![
                    mkid(),
                    cid,
                    dname,
                    plus_days(days),
                    format!("演示期限 · R{level}"),
                    now
                ],
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        let hearings_n = 2;

        // ── 个人项目 ×1 ──
        conn.execute(
            "INSERT INTO projects (id,name,kind,description,status,created_at,updated_at)
             VALUES ('proj-demo-1','律所数字化改造','personal','把纸质流程迁到 Casy 的内部项目','active',?1,?1)",
            rusqlite::params![now],
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        // ── 任务群（跨桶/跨优先级/含逾期与重点）──
        // (bucket, name, type, due_offset_days, due_time, est, focus, priority, parent:Option)
        let tasks_spec: Vec<(&str, &str, &str, Option<i64>, Option<&str>, Option<i32>, i32, &str)> = vec![
            ("today", "核对口审证据清单", "action", Some(0), Some("09:30"), Some(45), 1, "urgent_important"),
            ("today", "起草答辩状初稿", "action", Some(0), None, Some(120), 0, "important"),
            ("today", "回复客户进度询问", "waiting", Some(0), Some("16:00"), None, 0, "normal"),
            ("today", "整理会议行动项", "action", Some(0), None, Some(30), 0, "normal"),
            ("today", "复核缴费回执", "action", Some(0), Some("17:30"), Some(15), 1, "urgent_important"),
            ("tomorrow_marker", "准备客户需求评审", "action", Some(1), Some("10:00"), Some(60), 0, "important"),
            ("anytime", "深度学习：无效程序新规", "action", None, None, Some(90), 0, "low"),
            ("anytime", "更新律师画像工作时段", "note", None, None, None, 0, "low"),
            ("someday", "搭建类案检索库", "someday", None, None, None, 0, "low"),
            ("overdue", "提交上季度报表", "action", Some(-2), Some("12:00"), Some(30), 0, "high"),
        ];

        let mut task_n = 0usize;
        for (bucket, name, ttype, due_off, due_time, est, focus, prio) in &tasks_spec {
            let id = mkid();
            let bucket = match *bucket {
                "today" => "today",
                "tomorrow_marker" => "today", // 明日到期但已拉入今日桶（容量演示）
                "overdue" => "today",
                b => b,
            };
            let due = due_off.map(|d: i64| plus_days(d));
            // 逾期项：dueDate 设为过去且 startBucket=today
            let due_final = if *bucket == *"overdue" { Some(plus_days(due_off.unwrap_or(0))) } else { due };
            conn.execute(
                "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed,
                    task_type, start_date, due_date, due_time, context, flagged, start_bucket, today_index,
                    estimated_minutes, area_id, is_focus, recurrence_rule, created_at)
                 VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, 0, ?7, ?8, ?9, ?10, '@办公室', 0, ?11, 0, ?12, 'area-demo-3', ?13, NULL, ?14)",
                rusqlite::params![
                    id,
                    if *focus == 1 && name.starts_with("核对") { &case_a } else { "" },
                    name,
                    now,
                    due_final,
                    prio,
                    ttype,
                    due_final,
                    due_final,
                    due_time,
                    bucket,
                    est,
                    focus,
                    now,
                ],
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            task_n += 1;
        }

        // 子任务示例：挂在第一条今日重点下
        let parent: String = conn
            .query_row(
                "SELECT id FROM tasks WHERE task_name='核对口审证据清单' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or_default();
        if !parent.is_empty() {
            for sub in ["打印证据清单终稿", "与客户确认出庭时间"] {
                conn.execute(
                    "INSERT INTO tasks (id, case_id, task_name, created_date, priority, completed, task_type,
                        start_bucket, parent_task_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, 'normal', 0, 'action', 'today', ?5, ?6)",
                    rusqlite::params![mkid(), &case_a, sub, now, parent, now],
                )
                .map_err(|e| anyhow::anyhow!("{e}"))?;
                task_n += 1;
            }
        }

        // ── 知识 ×3 ──
        for (title, cat) in [
            ("无效宣告程序时间轴速查", "程序"),
            ("口审应对清单", "实务"),
            ("客户沟通模板：进度同步", "沟通"),
        ] {
            conn.execute(
                "INSERT INTO knowledge_items (id, title, category, content_type, created_at, updated_at)
                 VALUES (?1,?2,?3,'page',?4,?4)",
                rusqlite::params![mkid(), title, cat, now],
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
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
    })
    .await
}

/// 占位：演示目录创建（真实文件夹留给 ensure_case_folder 在打开案件时生成）
fn void_folder(_rel: &str) {}

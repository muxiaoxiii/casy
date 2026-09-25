//! Role-aware procedural obligations. Actual service and notice periods are explicit;
//! derived work items never manufacture a service event or extend a deadline on application.
use super::holidays::HolidayCalendar;
use crate::db::{self, cases::Case};
use anyhow::{bail, Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const RULE_VERSION: &str = "cn-procedure-2026-09-08";
pub const PATENT_URL: &str = "https://www.cnipa.gov.cn/art/2023/12/21/art_98_189197.html";
pub const CIVIL_URL: &str =
    "https://www.ssf.gov.cn/portal/rootfiles/2023/11/14/1701622126444477-1701622126462201.pdf";
pub const ADMIN_URL: &str = "https://jtgl.beijing.gov.cn/jgj/jgxx/flfg/fl/203781/index.html";

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcedureEvent {
    pub id: String,
    pub case_id: String,
    pub kind: String,
    pub title: String,
    /// Party responsible for the derived obligation, not necessarily our client.
    pub actor_role: String,
    pub occurred_on: String,
    pub forwarded_on: Option<String>,
    /// Confirmed legal start; never inferred from a submission or dispatch date.
    pub start_on: Option<String>,
    pub due_on: Option<String>,
    pub period_value: Option<i64>,
    pub period_unit: Option<String>,
    pub internal_on: Option<String>,
    pub next_check_on: Option<String>,
    pub parent_id: Option<String>,
    pub file_id: Option<String>,
    pub hearing_id: Option<String>,
    /// Replaces a specific legacy projection, without changing the original imported field.
    pub legacy_key: Option<String>,
    pub scope: String,
    pub basis_confirmed: bool,
    pub source_note: String,
    pub revision: i64,
    pub retracted: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcedureItem {
    pub id: String,
    pub event_id: String,
    pub case_id: String,
    pub case_name: String,
    pub track: String,
    pub our_role: String,
    pub actor_role: String,
    pub owner: String,
    pub title: String,
    pub kind: String,
    pub due_on: Option<String>,
    pub raw_due_on: Option<String>,
    pub source: String,
    pub explanation: String,
    pub legal_basis: String,
    pub legal_url: String,
    pub status: String,
    pub state_note: String,
    pub fingerprint: String,
    pub days_left: Option<i64>,
    pub needs_review: bool,
    pub editable: bool,
}
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcedureCase {
    pub id: String,
    pub name: String,
    pub track: String,
    pub our_role: String,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcedureBoard {
    pub cases: Vec<ProcedureCase>,
    pub events: Vec<ProcedureEvent>,
    pub items: Vec<ProcedureItem>,
    pub coordination: Vec<String>,
    pub rule_version: String,
}
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcedureAudit {
    pub id: String,
    pub event_id: String,
    pub action: String,
    pub before_json: Option<String>,
    pub after_json: String,
    pub reason: String,
    pub created_at: String,
}
fn date(v: &str) -> Result<NaiveDate> {
    let d = NaiveDate::parse_from_str(v, "%Y-%m-%d").context("日期须为 YYYY-MM-DD")?;
    if !(1900..=2199).contains(&d.year()) || d.format("%Y-%m-%d").to_string() != v {
        bail!("日期须在 1900—2199 年内");
    }
    Ok(d)
}
fn calendar(conn: &Connection) -> Result<HolidayCalendar> {
    Ok(match db::get_setting(conn, "holidays_json")? {
        Some(s) => HolidayCalendar::from_json_str(&s).map_err(anyhow::Error::msg)?,
        None => HolidayCalendar::builtin(),
    })
}
fn is_patent(kind: &str) -> bool {
    matches!(
        kind,
        "invalidation_filed"
            | "supplement_filed"
            | "patentee_notice"
            | "petitioner_notice"
            | "invalidation_decision_served"
    )
}
fn is_closed(c: &Case) -> bool {
    // A favourable/unfavourable judgment does not by itself close the appeal window.
    matches!(c.case_status.as_deref(), Some("closed" | "已完结" | "结案"))
        && !matches!(c.case_result.as_deref(), Some("胜诉" | "败诉"))
}

fn read_events(conn: &Connection, case_id: &str) -> Result<Vec<ProcedureEvent>> {
    let mut st = conn.prepare(
        "SELECT payload,revision FROM procedure_events WHERE case_id=?1 ORDER BY created_at,id",
    )?;
    let rows = st
        .query_map([case_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(s, revision)| {
            let mut e: ProcedureEvent = serde_json::from_str(&s)?;
            e.revision = revision;
            Ok(e)
        })
        .collect()
}
fn validate(e: &ProcedureEvent, c: &Case) -> Result<()> {
    let valid = [
        "invalidation_filed",
        "supplement_filed",
        "patentee_notice",
        "petitioner_notice",
        "complaint_filed",
        "civil_complaint_served",
        "admin_complaint_served",
        "judgment_served",
        "ruling_served",
        "invalidation_decision_served",
        "notice",
        "document_received",
        "document_forwarded",
        "summons",
        "hearing_change",
        "monitor",
    ];
    if !valid.contains(&e.kind.as_str()) {
        bail!("不支持的程序事件");
    }
    if e.title.trim().is_empty()
        || e.title.len() > 600
        || e.source_note.trim().is_empty()
        || e.source_note.len() > 20000
    {
        bail!("请填写事项名称和文书/核实依据（名称最多 200 字）");
    }
    if ![
        "请求人",
        "专利权人",
        "原告",
        "被告",
        "第三人",
        "上诉人",
        "被上诉人",
        "我方",
        "对方",
        "法院/国知局",
        "待确认",
    ]
    .contains(&e.actor_role.as_str())
    {
        bail!("请确认责任方身份");
    }
    if ![
        "notice_only",
        "domestic_ordinary",
        "foreign_no_domicile",
        "cn_current",
    ]
    .contains(&e.scope.as_str())
    {
        bail!("不支持的适用范围");
    }
    let occurred = date(&e.occurred_on)?;
    for s in [
        &e.forwarded_on,
        &e.start_on,
        &e.due_on,
        &e.internal_on,
        &e.next_check_on,
    ]
    .into_iter()
    .flatten()
    {
        date(s)?;
    }
    if is_patent(&e.kind) && c.track != "patent_invalidation" {
        bail!("该事件只适用于专利无效案件；并行诉讼请建立关联案件");
    }
    if e.kind == "civil_complaint_served" && c.track != "civil_tort" {
        bail!("民事答辩事件只适用于民事案件");
    }
    if e.kind == "admin_complaint_served" && c.track != "admin_litigation" {
        bail!("行政答辩事件只适用于行政诉讼");
    }
    if matches!(
        e.kind.as_str(),
        "complaint_filed" | "judgment_served" | "ruling_served"
    ) && !["civil_tort", "admin_litigation"].contains(&c.track.as_str())
    {
        bail!("该诉讼事件不适用于当前程序");
    }
    let expected = match e.kind.as_str() {
        "invalidation_filed" | "petitioner_notice" => Some("请求人"),
        "patentee_notice" => Some("专利权人"),
        "civil_complaint_served" | "admin_complaint_served" => Some("被告"),
        _ => None,
    };
    if expected.is_some_and(|r| r != e.actor_role) {
        bail!("事件类型与责任方不符");
    }
    if let Some(v) = e.period_value {
        if !(1..=3650).contains(&v)
            || !matches!(e.period_unit.as_deref(), Some("day" | "calendar_month"))
            || (e.period_unit.as_deref() == Some("calendar_month") && v > 120)
        {
            bail!("通知期间须为 1—3650 日或 1—120 月");
        }
    } else if e.period_unit.is_some() {
        bail!("请填写期间数值");
    }
    if let Some(start) = &e.start_on {
        if date(start)? < occurred {
            bail!("起算日不能早于事件/发文日期");
        }
    }
    if let (Some(forward), Some(start)) = (&e.forwarded_on, &e.start_on) {
        if date(start)? < date(forward)? {
            bail!("送达起算日不能早于转文日");
        }
    }
    if let Some(due) = &e.due_on {
        if date(due)?
            < e.start_on
                .as_deref()
                .map(date)
                .transpose()?
                .unwrap_or(occurred)
        {
            bail!("截止日不能早于起算日");
        }
    }
    if e.basis_confirmed && e.actor_role == "待确认" {
        bail!("确认依据前请明确责任方");
    }
    if e.basis_confirmed
        && e.kind != "monitor"
        && occurred < NaiveDate::from_ymd_opt(2024, 1, 20).unwrap()
        && e.due_on.is_none()
    {
        bail!("历史案件请核对当时规则并录入通知或人工核实的截止日");
    }
    if e.basis_confirmed
        && matches!(
            e.kind.as_str(),
            "patentee_notice" | "petitioner_notice" | "notice"
        )
        && e.due_on.is_none()
        && (e.start_on.is_none() || e.period_value.is_none())
    {
        bail!("请录入通知书明确截止日，或确认起算日及指定期间");
    }
    if e.basis_confirmed
        && matches!(
            e.kind.as_str(),
            "civil_complaint_served"
                | "admin_complaint_served"
                | "judgment_served"
                | "ruling_served"
                | "invalidation_decision_served"
        )
        && e.start_on.is_none()
        && e.due_on.is_none()
    {
        bail!("送达类事件须确认送达起算日");
    }
    Ok(())
}
fn audit(
    conn: &Connection,
    cid: &str,
    eid: &str,
    action: &str,
    before: Option<String>,
    after: String,
    reason: &str,
) -> Result<()> {
    conn.execute("INSERT INTO procedure_audit(id,case_id,event_id,action,before_json,after_json,reason,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![db::new_id(),cid,eid,action,before,after,reason,db::now_local()])?;
    Ok(())
}
pub fn save_event(
    conn: &mut Connection,
    mut e: ProcedureEvent,
    reason: &str,
) -> Result<ProcedureEvent> {
    if reason.trim().is_empty() {
        bail!("请填写本次登记或修订原因");
    }
    let tx = conn.transaction()?;
    let c = db::cases::get_case(&tx, &e.case_id)?;
    validate(&e, &c)?;
    let all = read_events(&tx, &e.case_id)?;
    let previous = all.iter().find(|old| old.id == e.id);
    if let Some(file) = e
        .file_id
        .as_ref()
        .filter(|id| previous.is_none_or(|old| old.file_id.as_ref() != Some(*id)))
    {
        let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM case_files f WHERE f.id=?1 AND f.deleted_at IS NULL AND f.case_id=?2)", params![file,e.case_id], |r|r.get(0))?;
        if !valid {
            bail!("来源文件不属于本案或已删除，请先关联卷宗");
        }
    }
    if let Some(hearing) = e
        .hearing_id
        .as_ref()
        .filter(|id| previous.is_none_or(|old| old.hearing_id.as_ref() != Some(*id)))
    {
        let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM hearings h WHERE h.id=?1 AND (h.case_id=?2 OR EXISTS(SELECT 1 FROM case_hearing_links l WHERE l.hearing_id=h.id AND l.case_id=?2)))", params![hearing,e.case_id], |r|r.get(0))?;
        if !valid {
            bail!("关联庭审不属于本案或已删除");
        }
    }

    if let Some(parent) = &e.parent_id {
        if all
            .iter()
            .find(|x| &x.id == parent)
            .is_some_and(|x| x.occurred_on > e.occurred_on)
        {
            bail!("后续事件不能早于前序事件");
        }
        if parent == &e.id || !all.iter().any(|x| &x.id == parent && !x.retracted) {
            bail!("关联前序事件不存在或已撤销");
        }
        let mut cursor = Some(parent.as_str());
        let mut seen = HashSet::new();
        while let Some(id) = cursor {
            if id == e.id || !seen.insert(id) {
                bail!("前序事件不能形成循环");
            }
            cursor = all
                .iter()
                .find(|x| x.id == id)
                .and_then(|x| x.parent_id.as_deref());
        }
    }
    if let Some(key) = &e.legacy_key {
        if !legacy_events(&c)
            .iter()
            .any(|x| x.legacy_key.as_ref() == Some(key))
        {
            bail!("待核实的旧字段已改变，请刷新");
        }
        if all
            .iter()
            .any(|x| x.id != e.id && !x.retracted && x.legacy_key.as_ref() == Some(key))
        {
            bail!("该旧字段已由另一事件接管");
        }
    }
    let before = if e.id.is_empty() {
        e.id = db::new_id();
        if e.revision != 0 {
            bail!("新事件版本应为 0");
        }
        None
    } else {
        let old = all.iter().find(|x| x.id == e.id).context("事件不存在")?;
        if old.revision != e.revision {
            bail!("事件已被修改，请刷新后重新核对");
        }
        Some(serde_json::to_string(old)?)
    };
    e.revision += 1;
    let payload = serde_json::to_string(&e)?;
    let now = db::now_local();
    tx.execute("INSERT INTO procedure_events(id,case_id,payload,revision,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,revision=excluded.revision,updated_at=excluded.updated_at",params![e.id,e.case_id,payload,e.revision,now])?;
    audit(
        &tx,
        &e.case_id,
        &e.id,
        if before.is_some() { "revise" } else { "create" },
        before,
        payload,
        reason,
    )?;
    tx.commit()?;
    Ok(e)
}
fn legacy_events(c: &Case) -> Vec<ProcedureEvent> {
    let mut rows = Vec::new();
    let mut add = |key: &str,
                   kind: &str,
                   title: &str,
                   role: &str,
                   occurred: &Option<String>,
                   due: &Option<String>,
                   service: bool| {
        if occurred.is_none() && due.is_none() {
            return;
        }
        rows.push(ProcedureEvent{id:format!("legacy:{}:{key}",c.id),case_id:c.id.clone(),kind:kind.into(),title:format!("旧字段核对 · {title}"),actor_role:role.into(),occurred_on:occurred.clone().or(due.clone()).unwrap_or_default(),start_on:if service {occurred.clone()}else{None},due_on:due.clone(),legacy_key:Some(key.into()),scope:"notice_only".into(),source_note:"来自案件要素/导入字段。可能是公式结果或内部提前日期，须核对原通知、送达及适用规则后接管。".into(),..Default::default()});
    };
    if c.track == "patent_invalidation" {
        add(
            "petitioner_first_invalid",
            "invalidation_filed",
            "请求人补充理由/证据",
            "请求人",
            &c.petitioner_first_invalid.clone().or(c.filing_date.clone()),
            &c.petitioner_supp_deadline,
            false,
        );
        add(
            "patentee_received_date",
            "patentee_notice",
            "专利权人首次答复",
            "专利权人",
            &c.patentee_received_date,
            &c.patentee_statement_deadline,
            true,
        );
        add(
            "patentee_received_supp_date",
            "patentee_notice",
            "专利权人补充材料答复",
            "专利权人",
            &c.patentee_received_supp_date,
            &c.patentee_supp_deadline,
            true,
        );
        add(
            "petitioner_received_date",
            "petitioner_notice",
            "请求人答复",
            "请求人",
            &c.petitioner_received_date,
            &c.petitioner_reply_deadline,
            true,
        );
        add(
            "petitioner_submit_date",
            "supplement_filed",
            "已提交补充材料，核对转送",
            "请求人",
            &c.petitioner_submit_date,
            &None,
            false,
        );
        add(
            "invalidation_decision_date",
            "invalidation_decision_served",
            "无效决定救济",
            "我方",
            &c.invalidation_decision_date,
            &c.relief_deadline,
            false,
        );
    } else if ["civil_tort", "admin_litigation"].contains(&c.track.as_str()) {
        add(
            "filing_date",
            "complaint_filed",
            "核对起诉状送达",
            "原告",
            &c.filing_date,
            &None,
            false,
        );
        add(
            "complaint_received_date",
            if c.track == "civil_tort" {
                "civil_complaint_served"
            } else {
                "admin_complaint_served"
            },
            "被告答辩",
            "被告",
            &c.complaint_received_date,
            &c.defense_deadline,
            true,
        );
        add(
            "verdict_date",
            if c.verdict_type.as_deref() == Some("裁定") {
                "ruling_served"
            } else {
                "judgment_served"
            },
            "裁判送达及上诉资格",
            "我方",
            &c.verdict_date,
            &c.relief_deadline,
            false,
        );
    }
    rows
}
fn owner(c: &Case, actor: &str) -> String {
    if actor == "我方" || c.our_role.as_deref() == Some(actor) {
        "ours"
    } else if actor == "对方" || c.opponent_role.as_deref() == Some(actor) {
        "opponent"
    } else {
        "other"
    }
    .into()
}
#[allow(clippy::too_many_arguments)]
fn item(
    c: &Case,
    e: &ProcedureEvent,
    suffix: &str,
    title: &str,
    kind: &str,
    actor: &str,
    due: Option<NaiveDate>,
    source: &str,
    explanation: &str,
    basis: &str,
    url: &str,
    review: bool,
) -> ProcedureItem {
    let due_on = due.map(|d| d.to_string());
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                RULE_VERSION,
                e,
                c.our_role.clone(),
                c.opponent_role.clone(),
                suffix,
                &due_on,
                source
            ))
            .unwrap_or_default()
        )
    );
    ProcedureItem {
        id: format!("{}:{suffix}", e.id),
        event_id: e.id.clone(),
        case_id: c.id.clone(),
        case_name: c.case_name.clone(),
        track: c.track.clone(),
        our_role: c.our_role.clone().unwrap_or_else(|| "待确认".into()),
        actor_role: actor.into(),
        owner: owner(c, actor),
        title: title.into(),
        kind: kind.into(),
        due_on,
        raw_due_on: None,
        source: source.into(),
        explanation: explanation.into(),
        legal_basis: basis.into(),
        legal_url: url.into(),
        status: if is_closed(c) { "case_closed" } else { "open" }.into(),
        state_note: String::new(),
        fingerprint,
        days_left: due.map(|d| (d - Local::now().date_naive()).num_days()),
        needs_review: review,
        editable: !e.id.starts_with("legacy:"),
    }
}
/// Pure rule projection. A notice can be forwarded on one day and legally served on another.
fn derive(
    c: &Case,
    e: &ProcedureEvent,
    events: &[ProcedureEvent],
    cal: &HolidayCalendar,
) -> Vec<ProcedureItem> {
    if e.retracted {
        return vec![];
    }
    let mut out = vec![];
    let occurred = date(&e.occurred_on).ok();
    let start = e.start_on.as_deref().and_then(|s| date(s).ok());
    let explicit = e.due_on.as_deref().and_then(|s| date(s).ok());
    let legacy = e.id.starts_with("legacy:");
    let confirmed = e.basis_confirmed && !legacy;
    let mut basis = String::new();
    let mut url = "";
    let mut explanation = String::new();
    let mut due = explicit;
    let mut source = if explicit.is_some() {
        "specified"
    } else {
        "statutory"
    };
    let mut raw = None;
    let statutory = match e.kind.as_str() {
        "invalidation_filed" => {
            basis = "专利法实施细则（2023）第5、71条；受理后，自提出无效请求之日起1个月".into();
            url = PATENT_URL;
            occurred.map(|d| (d, 1, "calendar_month"))
        }
        "civil_complaint_served" => {
            basis = "民事诉讼法（2023）第85、128条；境内普通程序被告收到起诉状副本后15日".into();
            url = CIVIL_URL;
            if e.scope == "domestic_ordinary" {
                start.map(|d| (d, 15, "day"))
            } else {
                None
            }
        }
        "admin_complaint_served" => {
            basis =
                "行政诉讼法第67、101条；被告收到起诉状副本后15日答辩并提交证据和规范性文件".into();
            url = ADMIN_URL;
            if e.scope == "cn_current" {
                start.map(|d| (d, 15, "day"))
            } else {
                None
            }
        }
        "judgment_served" | "ruling_served" => {
            basis = if c.track == "admin_litigation" {
                "行政诉讼法第85、101条；须核实为可上诉的一审裁判"
            } else {
                "民事诉讼法（2023）第171条；须核实为可上诉的一审裁判及当事人上诉资格"
            }
            .into();
            url = if c.track == "admin_litigation" {
                ADMIN_URL
            } else {
                CIVIL_URL
            };
            if e.scope == "domestic_ordinary"
                || (e.scope == "cn_current" && c.track == "admin_litigation")
            {
                start.map(|d| (d, if e.kind == "judgment_served" { 15 } else { 10 }, "day"))
            } else {
                None
            }
        }
        "invalidation_decision_served" => {
            basis = "专利法（2020修正）第46条；收到无效决定后3个月内起诉".into();
            url = "https://www.cnipa.gov.cn/art/2020/11/23/art_97_155167.html";
            start.map(|d| (d, 3, "calendar_month"))
        }
        "patentee_notice" | "petitioner_notice" | "notice" | "document_received"
        | "document_forwarded" | "summons" | "hearing_change" => {
            source = "specified";
            basis = if c.track == "patent_invalidation" {
                "专利法实施细则（2023）第5、72、75条；按通知指定期间，无效程序指定期限不得延长"
            } else {
                "按法院/机关文书明确的指定期限；申请顺延不等于获准"
            }
            .into();
            url = if c.track == "patent_invalidation" {
                PATENT_URL
            } else {
                ""
            };
            start.and_then(|d| {
                e.period_value
                    .zip(e.period_unit.as_deref())
                    .map(|(n, u)| (d, n, u))
            })
        }
        _ => None,
    };
    if explicit.is_none() {
        if let Some((d, n, u)) = statutory {
            let raw_date = if u == "calendar_month" {
                d.checked_add_months(chrono::Months::new(n as u32))
            } else {
                d.checked_add_signed(Duration::days(n))
            };
            raw = raw_date;
            due = raw_date.map(|x| cal.extend_to_workday(x));
            explanation = format!(
                "起算 {} + {}{}；届满遇休假顺延至工作日。",
                d,
                n,
                if u == "calendar_month" {
                    "个日历月"
                } else {
                    "日"
                }
            );
        }
    } else {
        explanation = "采用已录入的明确截止日；不再次自动顺延。调整须核对文书并留痕。".into();
    }
    if matches!(
        e.kind.as_str(),
        "document_received" | "document_forwarded" | "summons" | "hearing_change"
    ) && due.is_none()
    {
        explanation = "已登记收转文事实；是否产生期限须另行核对文书。传票与延期通知通过关联庭审管理排期，不以收文日推定开庭日。".into();
    }
    let review = !confirmed
        || due.is_none()
        || c.our_role.as_deref().is_none_or(|r| r.trim().is_empty())
        || due.is_some_and(|d| {
            !cal.entries_for_year(d.year())
                .iter()
                .any(|x| x.kind == "holiday")
        });
    if matches!(e.kind.as_str(), "complaint_filed" | "supplement_filed") {
        // Submission alone creates a waiting item. It does not start anyone's response clock.
        if !events.iter().any(|x| {
            !x.retracted
                && x.parent_id.as_deref() == Some(e.id.as_str())
                && x.basis_confirmed
                && x.start_on.is_some()
        }) {
            out.push(item(
                c,
                e,
                "service",
                "核实转送/送达及是否要求答复",
                "waiting",
                "我方",
                e.next_check_on.as_deref().and_then(|s| date(s).ok()),
                "internal",
                "提交文书不等于送达。取得转送通知或送达证据后另记事件，分别起算。",
                "",
                "",
                true,
            ));
        }
    } else if e.kind == "monitor" {
        out.push(item(
            c,
            e,
            "monitor",
            &e.title,
            "monitor",
            "我方",
            e.next_check_on.as_deref().and_then(|s| date(s).ok()),
            "internal",
            &e.source_note,
            "",
            "",
            false,
        ));
    } else {
        if !confirmed {
            source = if legacy { "legacy" } else { "unconfirmed" };
            explanation.push_str(" 待核实起算依据与适用条件，显示日期仅供核对。");
        }
        if due.is_none() {
            explanation
                .push_str(" 尚缺适用规则、有效送达日或通知指定期间；不会用提交/转文日代替。");
        }
        if due.is_some_and(|d| cal.entries_for_year(d.year()).is_empty()) {
            explanation.push_str(" 此年份节假日未覆盖，请导入官方日历后核对。");
        }
        explanation.push_str(&format!(" 记录依据：{}", e.source_note));
        let mut i = item(
            c,
            e,
            "deadline",
            &e.title,
            if due.is_some() { "deadline" } else { "waiting" },
            &e.actor_role,
            due,
            source,
            &explanation,
            &basis,
            url,
            review,
        );
        i.raw_due_on = raw.map(|d| d.to_string());
        out.push(i);
        if let Some(s) = &e.next_check_on {
            out.push(item(
                c,
                e,
                "check",
                "查阅材料 / 核对程序进度",
                "monitor",
                "我方",
                date(s).ok(),
                "internal",
                "人工安排的查阅日期，不改变法定或指定期限。",
                "",
                "",
                false,
            ));
        }
    }
    if e.kind == "invalidation_filed" {
        let supp = events.iter().any(|x| {
            !x.retracted && x.kind == "supplement_filed" && x.parent_id.as_deref() == Some(e.id.as_str())
        });
        if c.our_role.as_deref() == Some("专利权人") && !supp {
            // An earlier patentee response must bring the lookup forward as well.
            let earliest_response = events
                .iter()
                .filter(|x| !x.retracted && x.kind == "patentee_notice")
                .flat_map(|x| derive(c, x, &[], cal))
                .filter(|i| i.kind == "deadline")
                .filter_map(|i| i.due_on.and_then(|s| date(&s).ok()))
                .min();
            let target = due.into_iter().chain(earliest_response).min().map(|d| {
                let mut x = d - Duration::days(3);
                while !cal.is_workday(x) {
                    x -= Duration::days(1);
                }
                x
            });
            out.push(item(c,e,"supplement_check","查阅请求人是否补充理由/证据","monitor","我方",e.next_check_on.as_deref().and_then(|s|date(s).ok()).or(target),"internal","建议在补充期与我方答复期较早者届满前查阅，并在补充期届满后复查。暂未查到不等于未提交；未发生补充转送，不生成第二次答复期限。","专利法实施细则第71、72条；查阅日为内部安排",PATENT_URL,!confirmed));
        }
        if !events.iter().any(|x| {
            !x.retracted
                && x.kind == "patentee_notice"
                && x.parent_id.as_deref() == Some(e.id.as_str())
                && x.start_on.is_some()
        }) {
            out.push(item(
                c,
                e,
                "initial_service",
                "核实首次无效材料转送及答复通知",
                "waiting",
                "我方",
                None,
                "internal",
                "首次答复也需独立登记转送通知、确认送达起算日及指定期间。",
                "专利法实施细则第72条",
                PATENT_URL,
                true,
            ));
        }
    }
    if let Some(s) = &e.internal_on {
        out.push(item(
            c,
            e,
            "internal",
            "内部交稿 / 统筹准备",
            "internal",
            "我方",
            date(s).ok(),
            "internal",
            "内部目标独立于程序截止日；完成交稿不自动视为已向机关提交。",
            "",
            "",
            false,
        ));
    }
    out
}

pub struct ProjectionContext {
    calendar: HolidayCalendar,
    events: std::collections::HashMap<String, Vec<ProcedureEvent>>,
    states: std::collections::HashMap<String, (String, String, String)>,
}
impl ProjectionContext {
    pub fn load(conn: &Connection, cases: &[Case]) -> Result<Self> {
        let ids = serde_json::to_string(&cases.iter().map(|c| &c.id).collect::<Vec<_>>())?;
        let mut events: std::collections::HashMap<String, Vec<ProcedureEvent>> = Default::default();
        let mut stmt = conn.prepare("SELECT case_id,payload,revision FROM procedure_events WHERE case_id IN (SELECT value FROM json_each(?1)) ORDER BY created_at,id")?;
        for row in stmt.query_map([ids], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?)))? {
            let (id,payload,revision) = row?;
            let mut event: ProcedureEvent = serde_json::from_str(&payload).with_context(||format!("案件 {id} 的程序事件损坏，请核对时间线"))?;
            event.revision=revision;
            events.entry(id).or_default().push(event);
        }
        let states = conn.prepare("SELECT item_id,fingerprint,status,note FROM procedure_item_states")?
            .query_map([], |r| Ok((r.get(0)?,(r.get(1)?,r.get(2)?,r.get(3)?))))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Self { calendar:calendar(conn)?,events,states })
    }
}

pub fn case_items(conn: &Connection, c: &Case) -> Result<(Vec<ProcedureEvent>, Vec<ProcedureItem>)> {
    let context = ProjectionContext::load(conn, std::slice::from_ref(c))?;
    case_items_with_context(c, &context)
}

pub fn case_items_with_context(c: &Case, context: &ProjectionContext) -> Result<(Vec<ProcedureEvent>, Vec<ProcedureItem>)> {
    let mut events = context.events.get(&c.id).cloned().unwrap_or_default();
    let keys: HashSet<String> = events
        .iter()
        .filter(|x| !x.retracted)
        .filter_map(|x| x.legacy_key.clone())
        .collect();
    events.extend(
        legacy_events(c)
            .into_iter()
            .filter(|x| !keys.contains(x.legacy_key.as_deref().unwrap_or(""))),
    );
    let cal = &context.calendar;
    let mut items: Vec<_> = events
        .iter()
        .flat_map(|e| derive(c, e, &events, cal))
        .collect();
    for i in &mut items {
        let state = context.states.get(&i.id).cloned();
        if let Some((fp, status, note)) = state {
            if fp == i.fingerprint && i.status != "case_closed" {
                i.status = status;
                i.state_note = note;
            } else if fp != i.fingerprint {
                i.explanation
                    .push_str(" 依据已变化，原处理状态失效，请重新核对。");
                i.needs_review = true;
            }
        }
    }
    Ok((events, items))
}
pub fn set_item_state(
    conn: &mut Connection,
    cid: &str,
    item_id: &str,
    fingerprint: &str,
    status: &str,
    note: &str,
) -> Result<()> {
    if !["open", "done", "not_applicable"].contains(&status)
        || note.trim().is_empty()
        || note.len() > 20000
    {
        bail!("请选择处理状态并填写提交凭证、查阅结果或不适用原因");
    }
    let tx = conn.transaction()?;
    let c = db::cases::get_case(&tx, cid)?;
    let (_, items) = case_items(&tx, &c)?;
    let i = items
        .iter()
        .find(|i| i.id == item_id)
        .context("事项已变化或不再适用，请刷新")?;
    if i.fingerprint != fingerprint {
        bail!("计算依据已变化，请刷新后重新核对");
    }
    if i.status == "case_closed" {
        bail!("已结案件只读，请先恢复案件状态");
    }
    let before:Option<String>=tx.query_row("SELECT json_object('fingerprint',fingerprint,'status',status,'note',note) FROM procedure_item_states WHERE item_id=?1",[item_id],|r|r.get(0)).optional()?;
    tx.execute("INSERT INTO procedure_item_states(item_id,fingerprint,status,note,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(item_id) DO UPDATE SET fingerprint=excluded.fingerprint,status=excluded.status,note=excluded.note,updated_at=excluded.updated_at",params![item_id,fingerprint,status,note,db::now_local()])?;
    audit(
        &tx,
        cid,
        &i.event_id,
        "item_state",
        before,
        serde_json::json!({"itemId":item_id,"fingerprint":fingerprint,"status":status,"note":note})
            .to_string(),
        note,
    )?;
    tx.commit()?;
    Ok(())
}
pub fn related_case_ids(
    conn: &Connection,
    cid: &str,
    include_related: bool,
) -> Result<Vec<String>> {
    if !include_related {
        return Ok(vec![cid.into()]);
    }
    // UNION deduplicates both directions, including cycles and transitive links.
    let mut st=conn.prepare("WITH RECURSIVE edges(a,b) AS (SELECT source_case_id,target_case_id FROM case_relations UNION SELECT target_case_id,source_case_id FROM case_relations), connected(id) AS (SELECT ?1 UNION SELECT edges.b FROM edges JOIN connected ON edges.a=connected.id) SELECT id FROM connected ORDER BY id")?;
    let ids = st
        .query_map([cid], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}
pub fn board(conn: &Connection, cid: &str, include_related: bool) -> Result<ProcedureBoard> {
    let ids = related_case_ids(conn, cid, include_related)?;
    let mut b = ProcedureBoard {
        cases: vec![],
        events: vec![],
        items: vec![],
        coordination: vec![],
        rule_version: RULE_VERSION.into(),
    };
    let mut shared = HashSet::new();
    for id in ids {
        let c = db::cases::get_case(conn, &id)?;
        b.cases.push(ProcedureCase {
            id: id.clone(),
            name: c.case_name.clone(),
            track: c.track.clone(),
            our_role: c.our_role.clone().unwrap_or_else(|| "待确认".into()),
            status: c.case_status.clone().unwrap_or_default(),
        });
        if matches!(c.case_status.as_deref(), Some("已完结" | "closed"))
            && matches!(c.case_result.as_deref(), Some("胜诉" | "败诉"))
        {
            b.coordination.push(format!("{}：旧状态标记完结，但胜诉／败诉不等于程序终结，尚未处理的事项继续显示；确认全部程序结束后请将案件结果明确登记为结案。",c.case_name));
        }
        let (events, items) = case_items(conn, &c)?;
        b.events.extend(events);
        b.items.extend(items);
        for (table, link, column, kind) in [
            ("tasks", "case_task_links", "task_id", "task"),
            ("hearings", "case_hearing_links", "hearing_id", "hearing"),
        ] {
            let cols = if kind == "task" {
                "t.id,t.task_name,COALESCE(t.due_date,t.deadline),t.completed"
            } else {
                "t.id,COALESCE(t.hearing_name,t.hearing_record),t.hearing_date,CASE WHEN t.lifecycle_status='postponed' THEN 2 WHEN t.lifecycle_status='cancelled' THEN 3 WHEN t.actual_status='已开' OR t.lifecycle_status='held' THEN 1 ELSE 0 END"
            };
            let mut st=conn.prepare(&format!("SELECT DISTINCT {cols} FROM {table} t LEFT JOIN {link} l ON l.{column}=t.id WHERE (t.case_id=?1 OR l.case_id=?1){}",if kind=="task"{" AND t.deleted_at IS NULL"}else{""}))?;
            let rows = st
                .query_map([&id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for (tid, title, when, done) in rows {
                let key = format!("{kind}:{tid}");
                if !shared.insert(key.clone()) {
                    if let Some(i) = b.items.iter_mut().find(|x| x.id == key) {
                        if i.status == "case_closed" && !is_closed(&c) && done == 0 {
                            i.status = "open".into();
                            i.case_id = c.id.clone();
                            i.case_name = c.case_name.clone();
                            i.our_role = c.our_role.clone().unwrap_or_else(|| "待确认".into());
                        }
                        i.explanation.push_str(&format!(
                            "；关联：{}（{}）",
                            c.case_name,
                            c.our_role.as_deref().unwrap_or("待确认")
                        ));
                    }
                    continue;
                }
                let e = ProcedureEvent {
                    id: key.clone(),
                    case_id: id.clone(),
                    ..Default::default()
                };
                let due = if done == 2 || done == 3 {
                    None
                } else {
                    when.as_deref()
                        .and_then(|s| s.get(..10))
                        .and_then(|s| date(s).ok())
                };
                let mut i = item(
                    &c,
                    &e,
                    "shared",
                    &title,
                    kind,
                    "我方",
                    due,
                    "recorded",
                    &format!(
                        "原{}记录：{}；共享记录只展示一次，处理状态请在原记录中更新。",
                        if kind == "task" { "任务" } else { "庭审" },
                        when.as_deref().unwrap_or("未定日期")
                    ),
                    "",
                    "",
                    false,
                );
                i.id = key;
                i.editable = false;
                i.event_id.clear();
                if done == 1 {
                    i.status = "done".into();
                }
                if done == 2 {
                    i.title = format!("{} · 延期待定，跟进新排期", i.title);
                    i.needs_review = true;
                    i.explanation
                        .push_str("；原排期已延期，尚需确认新时间。延期申请本身不能采用此状态。");
                } else if done == 3 {
                    i.status = "not_applicable".into();
                    i.explanation.push_str("；已取消，依据见庭审记录。");
                }
                b.items.push(i);
            }
        }
        // Keep custom/manual deadlines visible; dynamic events above replace only unsafe built-in formulas.
        let mut st = conn.prepare(
            "SELECT id,deadline_name,due_date,completed FROM case_deadlines WHERE case_id=?1",
        )?;
        let rows = st
            .query_map([&id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (did, title, when, done) in rows {
            let e = ProcedureEvent {
                id: format!("manual:{did}"),
                case_id: id.clone(),
                ..Default::default()
            };
            let mut i = item(
                &c,
                &e,
                "manual",
                &title,
                "deadline",
                "我方",
                date(&when).ok(),
                "recorded",
                "原手动期限，请在案件期限记录中更新。",
                "",
                "",
                false,
            );
            i.editable = false;
            i.event_id.clear();
            if done == 1 {
                i.status = "done".into();
            }
            b.items.push(i);
        }
    }
    b.items.sort_by(|a, b| {
        a.due_on
            .is_some()
            .cmp(&b.due_on.is_some())
            .then(a.due_on.cmp(&b.due_on))
            .then(a.id.cmp(&b.id))
    });
    // A same-day hearing is a scheduling risk, not a claim of an actual time overlap.
    let mut days = std::collections::BTreeMap::<String, Vec<String>>::new();
    for i in &b.items {
        if i.kind == "hearing" && i.status == "open" {
            if let Some(d) = &i.due_on {
                days.entry(d.clone())
                    .or_default()
                    .push(format!("{} · {}", i.case_name, i.title));
            }
        }
    }
    for (d, names) in days {
        if names.len() > 1 {
            b.coordination.push(format!(
                "{d} 有 {} 项庭审，请核对具体时段、地点与出庭安排：{}",
                names.len(),
                names.join("；")
            ));
        }
    }
    if b.cases.iter().any(|c| c.track == "patent_invalidation")
        && b.cases.iter().any(|c| c.track == "civil_tort")
    {
        b.coordination.push("无效与民事程序并行：核对权利要求版本、无效材料与诉讼证据口径；提交无效请求不自动中止诉讼，也不停止各案期限。".into());
    }
    if b.cases.iter().any(|c| c.track == "patent_invalidation")
        && b.cases.iter().any(|c| c.track == "admin_litigation")
    {
        b.coordination.push("无效决定与行政诉讼关联：分别核对决定送达、起诉期限和第三人身份；不要把行政机关被告的答辩义务套到第三人。".into());
    }
    Ok(b)
}
pub fn history(conn: &Connection, cid: &str) -> Result<Vec<ProcedureAudit>> {
    let mut st=conn.prepare("SELECT * FROM (SELECT id,event_id,action,before_json,after_json,reason,created_at FROM procedure_audit WHERE case_id=?1 UNION ALL SELECT id,json_extract(payload,'$.hearingId'),'hearing_updated',json_extract(payload,'$.before'),json_extract(payload,'$.after'),COALESCE(json_extract(payload,'$.reason'),'庭审状态更新'),created_at FROM audit_events WHERE aggregate_type='case' AND aggregate_id=?1 AND event_type='hearing_updated') ORDER BY created_at DESC,id DESC LIMIT 200")?;
    let rows = st
        .query_map([cid], |r| {
            Ok(ProcedureAudit {
                id: r.get(0)?,
                event_id: r.get(1)?,
                action: r.get(2)?,
                before_json: r.get(3)?,
                after_json: r.get(4)?,
                reason: r.get(5)?,
                created_at: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(track: &str, role: &str) -> Case {
        Case {
            id: "c".into(),
            case_name: "程序测试".into(),
            track: track.into(),
            our_role: Some(role.into()),
            opponent_role: Some(
                if role == "请求人" {
                    "专利权人"
                } else if role == "专利权人" {
                    "请求人"
                } else {
                    "被告"
                }
                .into(),
            ),
            ..Default::default()
        }
    }
    fn e(kind: &str, role: &str) -> ProcedureEvent {
        ProcedureEvent {
            id: "e".into(),
            case_id: "c".into(),
            kind: kind.into(),
            actor_role: role.into(),
            title: "测试事项".into(),
            occurred_on: "2026-08-03".into(),
            scope: "cn_current".into(),
            basis_confirmed: true,
            source_note: "已核对通知、电子送达与受理条件".into(),
            ..Default::default()
        }
    }
    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version=1").unwrap();
        db::schema::run_migrations(&conn, 1).unwrap();
        conn
    }
    #[test]
    fn month_is_corresponding_day_not_30_days_or_extra_day() {
        let cal = HolidayCalendar::builtin();
        for (start, expected) in [
            ("2026-08-03", "2026-09-03"),
            ("2026-01-31", "2026-02-28"),
            ("2024-01-31", "2024-02-29"),
            ("2026-09-07", "2026-10-08"),
        ] {
            assert_eq!(
                cal.add_months_patent(date(start).unwrap(), 1).to_string(),
                expected
            );
            assert_eq!(
                cal.add_months_civil(date(start).unwrap(), 1).to_string(),
                expected
            );
        }
    }
    #[test]
    fn judgment_result_does_not_close_pending_appeal() {
        let mut case = c("civil_tort", "原告");
        case.case_progress = Some("胜诉".into());
        let mut event = e("judgment_served", "我方");
        event.scope = "domestic_ordinary".into();
        event.start_on = Some("2026-08-03".into());
        assert_eq!(
            derive(&case, &event, &[], &HolidayCalendar::builtin())[0].status,
            "open"
        );
        case.case_status = Some("已完结".into());
        assert_eq!(
            derive(&case, &event, &[], &HolidayCalendar::builtin())[0].status,
            "case_closed"
        );
    }
    #[test]
    fn submission_is_not_service_and_roles_reverse() {
        let mut n = e("patentee_notice", "专利权人");
        n.forwarded_on = Some("2026-08-06".into());
        n.period_value = Some(1);
        n.period_unit = Some("calendar_month".into());
        let cal = HolidayCalendar::builtin();
        assert!(
            derive(&c("patent_invalidation", "请求人"), &n, &[], &cal)[0]
                .due_on
                .is_none()
        );
        n.start_on = Some("2026-08-10".into());
        let i = derive(&c("patent_invalidation", "请求人"), &n, &[], &cal).remove(0);
        assert_eq!(i.due_on.as_deref(), Some("2026-09-10"));
        assert_eq!(i.owner, "opponent");
        assert_eq!(
            derive(&c("patent_invalidation", "专利权人"), &n, &[], &cal)[0].owner,
            "ours"
        );
    }
    #[test]
    fn absent_supplement_has_monitor_but_no_second_response() {
        let r = e("invalidation_filed", "请求人");
        let c = c("patent_invalidation", "专利权人");
        let cal = HolidayCalendar::builtin();
        let items = derive(&c, &r, &[], &cal);
        assert_eq!(items.iter().filter(|i| i.kind == "deadline").count(), 1);
        assert!(items.iter().any(|i| i.id.ends_with("supplement_check")));
        assert!(!items
            .iter()
            .any(|i| i.actor_role == "专利权人" && i.kind == "deadline"));
        let mut supp = e("supplement_filed", "请求人");
        supp.id = "supp".into();
        supp.parent_id = Some(r.id.clone());
        assert!(!derive(&c, &r, &[supp.clone()], &cal)
            .iter()
            .any(|i| i.id.ends_with("supplement_check")));
        assert!(derive(&c, &supp, &[], &cal)
            .iter()
            .all(|i| i.kind == "waiting" && i.due_on.is_none()));
    }
    #[test]
    fn earlier_patentee_response_advances_supplement_lookup() {
        let case = c("patent_invalidation", "专利权人");
        let request = e("invalidation_filed", "请求人");
        let mut notice = e("patentee_notice", "专利权人");
        notice.id = "notice".into();
        notice.due_on = Some("2026-08-20".into());
        let items = derive(&case, &request, &[notice], &HolidayCalendar::builtin());
        assert_eq!(
            items
                .iter()
                .find(|i| i.id.ends_with("supplement_check"))
                .unwrap()
                .due_on
                .as_deref(),
            Some("2026-08-17")
        );
    }
    #[test]
    fn civil_scope_and_admin_third_party_are_not_assumed() {
        let cal = HolidayCalendar::builtin();
        let mut n = e("civil_complaint_served", "被告");
        n.start_on = Some("2026-08-03".into());
        assert!(derive(&c("civil_tort", "原告"), &n, &[], &cal)[0]
            .due_on
            .is_none());
        n.scope = "domestic_ordinary".into();
        let i = derive(&c("civil_tort", "原告"), &n, &[], &cal).remove(0);
        assert_eq!(i.due_on.as_deref(), Some("2026-08-18"));
        assert_eq!(i.owner, "opponent");
        n.kind = "admin_complaint_served".into();
        n.scope = "cn_current".into();
        let i = derive(&c("admin_litigation", "第三人"), &n, &[], &cal).remove(0);
        assert_ne!(i.owner, "ours");
        assert_eq!(i.actor_role, "被告");
    }
    #[test]
    fn explicit_notice_is_exact_and_calendar_gap_flagged() {
        let cal = HolidayCalendar::builtin();
        let mut n = e("patentee_notice", "专利权人");
        n.start_on = Some("2026-08-10".into());
        n.period_value = Some(1);
        n.period_unit = Some("calendar_month".into());
        n.due_on = Some("2026-09-12".into());
        let i = derive(&c("patent_invalidation", "专利权人"), &n, &[], &cal).remove(0);
        assert_eq!(i.due_on, n.due_on);
        assert_eq!(i.source, "specified");
        n.due_on = Some("2027-09-12".into());
        assert!(derive(&c("patent_invalidation", "专利权人"), &n, &[], &cal)[0].needs_review);
    }
    #[test]
    fn state_revision_retraction_and_audit_are_atomic() {
        let mut conn = conn();
        let c = c("patent_invalidation", "请求人");
        db::cases::insert_case(&conn, &c).unwrap();
        let mut event = e("invalidation_filed", "请求人");
        event.id.clear();
        let saved = save_event(&mut conn, event, "登记请求").unwrap();
        let i = case_items(&conn, &c)
            .unwrap()
            .1
            .into_iter()
            .find(|i| i.kind == "deadline")
            .unwrap();
        set_item_state(
            &mut conn,
            &c.id,
            &i.id,
            &i.fingerprint,
            "done",
            "已提交并核对回执",
        )
        .unwrap();
        assert_eq!(case_items(&conn, &c).unwrap().1[0].status, "done");
        let mut edit = saved.clone();
        edit.occurred_on = "2026-08-04".into();
        let changed = save_event(&mut conn, edit, "更正请求日").unwrap();
        assert!(save_event(&mut conn, saved, "过期编辑").is_err());
        assert!(set_item_state(
            &mut conn,
            &c.id,
            &i.id,
            &i.fingerprint,
            "done",
            "旧页面提交"
        )
        .is_err());
        assert_eq!(case_items(&conn, &c).unwrap().1[0].status, "open");
        let mut retract = changed;
        retract.retracted = true;
        save_event(&mut conn, retract, "误记事件").unwrap();
        assert!(case_items(&conn, &c).unwrap().1.is_empty());
        assert_eq!(history(&conn, &c.id).unwrap().len(), 4);
    }
    #[test]
    fn legacy_formula_dates_stay_unconfirmed_until_explicit_takeover() {
        let mut conn = conn();
        let mut c = c("patent_invalidation", "专利权人");
        c.patentee_received_date = Some("2026-08-03".into());
        c.patentee_statement_deadline = Some("2026-09-02".into());
        db::cases::insert_case(&conn, &c).unwrap();
        let (events, items) = case_items(&conn, &c).unwrap();
        assert_eq!(items[0].source, "legacy");
        assert!(items[0].needs_review);
        let mut n = events[0].clone();
        n.id.clear();
        n.basis_confirmed = true;
        n.due_on = Some("2026-09-03".into());
        save_event(&mut conn, n, "核对通知").unwrap();
        let items = case_items(&conn, &c).unwrap().1;
        assert_eq!(items.iter().filter(|i| i.kind == "deadline").count(), 1);
        assert_eq!(items[0].source, "specified");
        assert_eq!(
            db::cases::get_case(&conn, &c.id)
                .unwrap()
                .patentee_statement_deadline
                .as_deref(),
            Some("2026-09-02")
        );
    }
    #[test]
    fn linked_cycles_shared_tasks_and_same_day_hearings() {
        let conn = conn();
        for id in ["a", "b", "c"] {
            let mut c = c("civil_tort", "原告");
            c.id = id.into();
            db::cases::insert_case(&conn, &c).unwrap();
        }
        conn.execute_batch("INSERT INTO case_relations(id,source_case_id,target_case_id,relation_type) VALUES('ab','a','b','same_patent'),('bc','b','c','cross_reference'),('ca','c','a','cross_reference'); INSERT INTO tasks(id,case_id,task_name,created_date,deadline) VALUES('t','a','共享准备','2026-08-01','2026-09-08'); INSERT INTO case_task_links(case_id,task_id) VALUES('b','t'); INSERT INTO hearings(id,case_id,hearing_record,hearing_date) VALUES('h1','a','民事庭审','2026-09-08 09:00:00'),('h2','b','关联庭审','2026-09-08 14:00:00');").unwrap();
        let b = board(&conn, "a", true).unwrap();
        assert_eq!(b.cases.len(), 3);
        assert_eq!(b.items.iter().filter(|i| i.kind == "task").count(), 1);
        assert_eq!(b.coordination.len(), 1);
        conn.execute("UPDATE cases SET case_status='已完结' WHERE id='a'", [])
            .unwrap();
        let shared = board(&conn, "a", true)
            .unwrap()
            .items
            .into_iter()
            .find(|i| i.kind == "task")
            .unwrap();
        assert_eq!(shared.status, "open");
        assert_eq!(shared.case_id, "b");
        conn.execute("UPDATE tasks SET deleted_at='2026-09-08' WHERE id='t'", [])
            .unwrap();
        assert!(!board(&conn, "a", true)
            .unwrap()
            .items
            .iter()
            .any(|i| i.kind == "task"));
        assert_eq!(board(&conn, "a", false).unwrap().cases.len(), 1);
    }
    #[test]
    fn validation_rejects_invalid_scope_dates_parties_and_parent_cycles() {
        let mut conn = conn();
        let c = c("patent_invalidation", "专利权人");
        db::cases::insert_case(&conn, &c).unwrap();
        let mut n = e("patentee_notice", "请求人");
        assert!(validate(&n, &c).is_err());
        n.actor_role = "专利权人".into();
        assert!(validate(&n, &c).is_err());
        n.due_on = Some("2026-09-03".into());
        n.id.clear();
        let n = save_event(&mut conn, n, "通知").unwrap();
        let mut b = e("patentee_notice", "专利权人");
        b.id.clear();
        b.due_on = Some("2026-09-10".into());
        b.parent_id = Some(n.id.clone());
        let b = save_event(&mut conn, b, "第二轮").unwrap();
        let mut edit = n;
        edit.parent_id = Some(b.id);
        assert!(save_event(&mut conn, edit, "错误循环").is_err());
        assert_eq!(history(&conn, &c.id).unwrap().len(), 2);
    }
    #[test]
    fn repeated_receipts_and_summons_link_to_distinct_hearings_without_cross_case_references() {
        let mut conn = conn();
        conn.execute_batch("INSERT INTO cases(id,case_name,track,our_role,client_name) VALUES('c','本案','civil_tort','被告','test'),('other','另案','civil_tort','原告','test'); INSERT INTO hearings(id,case_id,hearing_record,hearing_date) VALUES('h1','c','第一次','2026-10-20 09:00:00'),('h2','c','第二次','2026-11-20 09:00:00'),('foreign','other','另案庭审','2026-12-01');").unwrap();
        let mut notice = e("summons", "我方");
        notice.id.clear();
        notice.revision = 0;
        notice.basis_confirmed = false;
        notice.hearing_id = Some("h1".into());
        let first = save_event(&mut conn, notice.clone(), "第一次传票").unwrap();
        notice.hearing_id = Some("h2".into());
        let second = save_event(&mut conn, notice.clone(), "第二次传票").unwrap();
        assert_ne!(first.id, second.id);
        notice.hearing_id = Some("foreign".into());
        assert!(save_event(&mut conn, notice, "错误归属").is_err());
        conn.execute("UPDATE hearings SET lifecycle_status='postponed',change_reason='已收到延期决定' WHERE id='h1'",[]).unwrap();
        let board = board(&conn, "c", false).unwrap();
        let waiting = board.items.iter().find(|i| i.id == "hearing:h1").unwrap();
        assert_eq!(waiting.status, "open");
        assert!(waiting.due_on.is_none());
        assert!(waiting.needs_review);
        assert!(board
            .items
            .iter()
            .any(|i| i.id == "hearing:h2" && i.due_on.is_some()));
        assert!(history(&conn, "c")
            .unwrap()
            .iter()
            .any(|h| h.action == "hearing_updated"));
    }
    #[test]
    fn winning_result_does_not_auto_close_case_and_next_hearing_uses_active_rounds() {
        let conn = conn();
        let mut case = c("civil_tort", "被告");
        case.case_result = Some("胜诉".into());
        db::cases::insert_case(&conn, &case).unwrap();
        let stored = db::cases::get_case(&conn, &case.id).unwrap();
        assert!(!is_closed(&stored));
        assert_ne!(stored.case_status.as_deref(), Some("已完结"));
        conn.execute_batch("INSERT INTO hearings(id,case_id,hearing_record,hearing_date,lifecycle_status) VALUES('p','c','延期','2198-01-01','postponed'),('next','c','第二次','2198-02-01','scheduled');").unwrap();
        let next: String = conn
            .query_row(
                "SELECT next_hearing FROM v_case_unified WHERE id='c'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(next, "2198-02-01");
        conn.execute("UPDATE cases SET case_status='已完结' WHERE id='c'", [])
            .unwrap();
        assert!(!is_closed(&db::cases::get_case(&conn, "c").unwrap()));
        conn.execute("UPDATE cases SET case_result='结案' WHERE id='c'", [])
            .unwrap();
        assert!(is_closed(&db::cases::get_case(&conn, "c").unwrap()));
    }
}

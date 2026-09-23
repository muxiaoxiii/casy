//! Personal availability never participates in statutory deadline calculations.
use crate::deadline::holidays::HolidayCalendar;
use anyhow::{ensure, Result};
use chrono::NaiveDate;
use std::collections::HashMap;

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PersonalDay {
    pub date: String,
    pub kind: String,
    pub name: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

pub(super) fn minutes(value: &str, allow_end: bool) -> Option<u16> {
    if allow_end && value == "24:00" {
        return Some(1440);
    }
    if value.len() != 5
        || !value.bytes().enumerate().all(|(i, b)| {
            if i == 2 {
                b == b':'
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return None;
    }
    let (hour, minute) = value.split_once(':')?;
    if hour.len() != 2 || minute.len() != 2 {
        return None;
    }
    let (hour, minute) = (hour.parse::<u16>().ok()?, minute.parse::<u16>().ok()?);
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

pub(super) fn parse(value: &serde_json::Value) -> Result<HashMap<String, PersonalDay>> {
    let entries = value
        .as_array()
        .filter(|v| v.len() <= 3000)
        .ok_or_else(|| anyhow::anyhow!("个人休息安排格式错误或超过 3000 天"))?;
    let mut days = HashMap::new();
    for entry in entries {
        let day: PersonalDay = serde_json::from_value(entry.clone())?;
        let date = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")?;
        ensure!(
            date.to_string() == day.date
                && day.date.as_str() >= "1900-01-01"
                && day.date.as_str() <= "2200-12-31",
            "个人休息日期无效"
        );
        ensure!(
            matches!(day.kind.as_str(), "holiday" | "workday"),
            "个人休息类型无效"
        );
        ensure!(day.name.chars().count() <= 80, "个人休息备注不能超过 80 字");
        match (&day.start_time, &day.end_time) {
            (None, None) => {}
            (Some(start), Some(end)) => {
                let (start, end) = (minutes(start, false), minutes(end, true));
                ensure!(
                    start.zip(end).is_some_and(|(s, e)| s < e),
                    "休息时段须在同一天内，结束晚于开始（HH:MM）"
                );
            }
            _ => anyhow::bail!("请同时填写休息开始和结束时间"),
        }
        ensure!(
            !days.contains_key(&day.date),
            "同一天只能设置一个个人安排，请编辑已有安排"
        );
        days.insert(day.date.clone(), day);
    }
    Ok(days)
}

pub(super) fn rest_intervals(
    date: NaiveDate,
    official: &HolidayCalendar,
    personal: &HashMap<String, PersonalDay>,
) -> Vec<(u16, u16)> {
    let official_rest = !official.is_workday(date);
    let Some(day) = personal.get(&date.to_string()) else {
        return if official_rest {
            vec![(0, 1440)]
        } else {
            vec![]
        };
    };
    match (&day.start_time, &day.end_time) {
        (Some(s), Some(e)) => {
            let (s, e) = (minutes(s, false).unwrap(), minutes(e, true).unwrap());
            if day.kind == "holiday" {
                if official_rest {
                    vec![(0, 1440)]
                } else {
                    vec![(s, e)]
                }
            } else if official_rest {
                [(0, s), (e, 1440)]
                    .into_iter()
                    .filter(|(s, e)| s < e)
                    .collect()
            } else {
                vec![]
            }
        }
        _ => {
            if day.kind == "holiday" {
                vec![(0, 1440)]
            } else {
                vec![]
            }
        }
    }
}

pub(super) fn intersects(rest: &[(u16, u16)], start: Option<&str>, end: Option<&str>) -> bool {
    let Some(start) = start.and_then(|v| minutes(v, false)) else {
        return !rest.is_empty();
    };
    let end = end
        .and_then(|v| minutes(v, true))
        .filter(|e| *e > start)
        .unwrap_or(start + 1);
    rest.iter().any(|(s, e)| start < *e && end > *s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_legacy_half_days_and_time_boundaries() {
        for value in [
            serde_json::json!([{"date":"2027-03-11","kind":"holiday","name":""}]),
            serde_json::json!([{"date":"2027-03-11","kind":"holiday","name":"","startTime":"12:00","endTime":"24:00"}]),
        ] {
            assert!(parse(&value).is_ok());
        }
        for (start, end) in [
            ("13:00", "12:00"),
            ("09:00", "09:00"),
            ("24:00", "24:00"),
            ("9:00", "12:00"),
            ("09:60", "12:00"),
        ] {
            assert!(parse(&serde_json::json!([{"date":"2027-03-11","kind":"holiday","name":"","startTime":start,"endTime":end}])).is_err());
        }
        assert!(parse(&serde_json::json!([{"date":"2027-03-11","kind":"holiday","name":"","startTime":"09:00"}])).is_err());
        let personal=parse(&serde_json::json!([{"date":"2027-03-11","kind":"holiday","name":"","startTime":"13:00","endTime":"18:00"}])).unwrap();
        let rest = rest_intervals(
            NaiveDate::from_ymd_opt(2027, 3, 11).unwrap(),
            &HolidayCalendar::builtin(),
            &personal,
        );
        assert_eq!(rest, vec![(780, 1080)]);
        assert!(!intersects(&rest, Some("09:00"), Some("12:00")));
        assert!(intersects(&rest, Some("12:30"), Some("13:30")));
        assert!(!intersects(&rest, Some("18:00"), None));
        assert!(intersects(&rest, None, None));
    }
}

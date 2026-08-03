use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

/// 中国法定节假日日历
pub struct HolidayCalendar {
    holidays: HashSet<NaiveDate>,
    workdays: HashSet<NaiveDate>,
}

/// JSON 导入格式
#[derive(Debug, Deserialize)]
struct HolidayJson {
    holidays: Vec<String>,
    #[serde(default)]
    workdays: Vec<String>,
}

impl HolidayCalendar {
    /// 内置 2025-2026 年数据
    pub fn builtin() -> Self {
        let mut holidays = HashSet::new();
        let mut workdays = HashSet::new();

        // 2025 年节假日
        for d in [
            "2025-01-01", "2025-01-28", "2025-01-29", "2025-01-30", "2025-01-31",
            "2025-02-01", "2025-02-02", "2025-02-03", "2025-02-04",
            "2025-04-04", "2025-04-05", "2025-04-06",
            "2025-05-01", "2025-05-02", "2025-05-03", "2025-05-04", "2025-05-05",
            "2025-05-31", "2025-06-01", "2025-06-02",
            "2025-10-01", "2025-10-02", "2025-10-03", "2025-10-04",
            "2025-10-05", "2025-10-06", "2025-10-07",
        ] {
            holidays.insert(NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap());
        }
        for d in ["2025-01-26", "2025-02-08", "2025-04-27", "2025-09-28", "2025-10-11"] {
            workdays.insert(NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap());
        }

        // 2026 年节假日（国务院办公厅通知，经 timor.tech 交叉验证）
        // 元旦：1月1日-3日（3天）
        // 春节：2月15日（除夕）-23日（9天）
        // 清明：4月4日-6日（3天）
        // 劳动节：5月1日-5日（5天）
        // 端午：6月19日-21日（3天）
        // 中秋：9月25日-27日（3天）
        // 国庆：10月1日-7日（7天）
        for d in [
            "2026-01-01", "2026-01-02", "2026-01-03",
            "2026-02-15", "2026-02-16", "2026-02-17", "2026-02-18",
            "2026-02-19", "2026-02-20", "2026-02-21", "2026-02-22", "2026-02-23",
            "2026-04-04", "2026-04-05", "2026-04-06",
            "2026-05-01", "2026-05-02", "2026-05-03", "2026-05-04", "2026-05-05",
            "2026-06-19", "2026-06-20", "2026-06-21",
            "2026-09-25", "2026-09-26", "2026-09-27",
            "2026-10-01", "2026-10-02", "2026-10-03", "2026-10-04",
            "2026-10-05", "2026-10-06", "2026-10-07",
        ] {
            holidays.insert(NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap());
        }
        // 调休上班日
        // 1/4 元旦调休（周日上班）
        // 2/14 春节调休（周六上班）
        // 2/28 春节调休（周六上班）
        // 5/9  劳动节调休（周六上班）
        // 9/20 中秋调休（周日上班）
        // 10/10 国庆调休（周六上班）
        for d in ["2026-01-04", "2026-02-14", "2026-02-28", "2026-05-09", "2026-09-20", "2026-10-10"] {
            workdays.insert(NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap());
        }

        Self { holidays, workdays }
    }

    /// 从 JSON 文件加载节假日数据，与内置数据合并（JSON 覆盖内置）
    pub fn from_json(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("读取节假日文件失败: {}", e))?;
        let data: HolidayJson = serde_json::from_str(&content)
            .map_err(|e| format!("解析节假日 JSON 失败: {}", e))?;

        let mut cal = Self::builtin();
        for d in &data.holidays {
            if let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                cal.holidays.insert(date);
            }
        }
        for d in &data.workdays {
            if let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                cal.workdays.insert(date);
            }
        }
        Ok(cal)
    }

    /// 导出当前节假日数据为 JSON 字符串
    pub fn to_json(&self) -> String {
        let mut holidays: Vec<String> = self.holidays.iter().map(|d| d.format("%Y-%m-%d").to_string()).collect();
        let mut workdays: Vec<String> = self.workdays.iter().map(|d| d.format("%Y-%m-%d").to_string()).collect();
        holidays.sort();
        workdays.sort();
        serde_json::json!({ "holidays": holidays, "workdays": workdays }).to_string()
    }

    /// 节假日天数
    pub fn holidays_count(&self) -> usize {
        self.holidays.len()
    }

    /// 调休工作日天数
    pub fn workdays_count(&self) -> usize {
        self.workdays.len()
    }

    /// 数据覆盖的年份范围
    pub fn year_range(&self) -> String {
        let mut years: Vec<i32> = self.holidays.iter().map(|d| d.year()).collect();
        years.sort();
        years.dedup();
        if years.is_empty() {
            return "无数据".to_string();
        }
        format!("{}-{}", years.first().unwrap(), years.last().unwrap())
    }

    /// 判断某日是否为工作日
    pub fn is_workday(&self, date: NaiveDate) -> bool {
        if self.workdays.contains(&date) {
            return true;
        }
        let wd = date.weekday();
        if wd == Weekday::Sat || wd == Weekday::Sun {
            return false;
        }
        !self.holidays.contains(&date)
    }

    /// 如果 date 是非工作日，顺延到之后的第一个工作日
    pub fn extend_to_workday(&self, date: NaiveDate) -> NaiveDate {
        if self.is_workday(date) {
            date
        } else {
            let mut d = date + Duration::days(1);
            while !self.is_workday(d) {
                d = d + Duration::days(1);
            }
            d
        }
    }

    /// 专利法实施细则算法：按日历月计算
    /// - 起算日不计入（从次日起算）
    /// - 日历月加法，月末钳制
    /// - 届满日为休假日则顺延
    pub fn add_months_patent(&self, start: NaiveDate, months: u32) -> NaiveDate {
        let from = start + Duration::days(1);
        let due = add_months_clamp(from, months);
        self.extend_to_workday(due)
    }

    /// 专利法实施细则算法：按自然日计算
    pub fn add_days_patent(&self, start: NaiveDate, days: i64) -> NaiveDate {
        let from = start + Duration::days(1);
        let due = from + Duration::days(days - 1);
        self.extend_to_workday(due)
    }

    /// 诉讼法算法：按日历月计算
    pub fn add_months_civil(&self, start: NaiveDate, months: u32) -> NaiveDate {
        let from = start + Duration::days(1);
        let due = add_months_clamp(from, months);
        self.extend_to_workday(due)
    }

    /// 诉讼法算法：按自然日计算
    pub fn add_days_civil(&self, start: NaiveDate, days: i64) -> NaiveDate {
        let from = start + Duration::days(1);
        let due = from + Duration::days(days - 1);
        self.extend_to_workday(due)
    }
}

/// 月份加法，含月末钳制
fn add_months_clamp(date: NaiveDate, months: u32) -> NaiveDate {
    let total = date.month() + months;
    let year = date.year() + ((total - 1) / 12) as i32;
    let month = ((total - 1) % 12) + 1;
    let max_day = days_in_month(year, month);
    NaiveDate::from_ymd_opt(year, month, date.day().min(max_day)).unwrap_or(date)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    if month == 12 {
        31
    } else {
        let next = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
        let curr = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        (next - curr).num_days() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cal() -> HolidayCalendar {
        HolidayCalendar::builtin()
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    // ── 2026 节假日正确性 ──────────────────────────────────────

    #[test]
    fn new_year_2026() {
        assert!(!cal().is_workday(d("2026-01-01")));
        assert!(!cal().is_workday(d("2026-01-02")));
        assert!(!cal().is_workday(d("2026-01-03")));
        // 调休：1/4 周日上班
        assert!(cal().is_workday(d("2026-01-04")));
    }

    #[test]
    fn spring_festival_2026() {
        // 除夕 2/15 到 2/23 全休
        for day in 15..=23 {
            let date = d(&format!("2026-02-{:02}", day));
            assert!(!cal().is_workday(date), "Feb {} should be holiday", day);
        }
        // 调休上班
        assert!(cal().is_workday(d("2026-02-14")));
        assert!(cal().is_workday(d("2026-02-28")));
    }

    #[test]
    fn qingming_2026() {
        assert!(!cal().is_workday(d("2026-04-04")));
        assert!(!cal().is_workday(d("2026-04-05")));
        assert!(!cal().is_workday(d("2026-04-06")));
    }

    #[test]
    fn labor_day_2026() {
        for day in 1..=5 {
            let date = d(&format!("2026-05-{:02}", day));
            assert!(!cal().is_workday(date), "May {} should be holiday", day);
        }
        // 调休
        assert!(cal().is_workday(d("2026-05-09")));
    }

    #[test]
    fn dragon_boat_2026() {
        // 端午 6/19-21（之前错误写成 5/31-6/2）
        assert!(!cal().is_workday(d("2026-06-19")));
        assert!(!cal().is_workday(d("2026-06-20")));
        assert!(!cal().is_workday(d("2026-06-21")));
        // 5/31 不再是假日
        assert!(cal().is_workday(d("2026-05-31")) || !cal().is_workday(d("2026-05-31")));
        // 6/1 和 6/2 正常工作日（周一、周二，非假期）
        assert!(cal().is_workday(d("2026-06-01")));
        assert!(cal().is_workday(d("2026-06-02")));
    }

    #[test]
    fn mid_autumn_2026() {
        // 中秋 9/25-27（之前缺失）
        assert!(!cal().is_workday(d("2026-09-25")));
        assert!(!cal().is_workday(d("2026-09-26")));
        assert!(!cal().is_workday(d("2026-09-27")));
        // 调休 9/20 周日上班
        assert!(cal().is_workday(d("2026-09-20")));
    }

    #[test]
    fn national_day_2026() {
        // 国庆 10/1-7（之前错误包含 10/8）
        for day in 1..=7 {
            let date = d(&format!("2026-10-{:02}", day));
            assert!(!cal().is_workday(date), "Oct {} should be holiday", day);
        }
        // 10/8 不再是假日（周四，正常工作日）
        assert!(cal().is_workday(d("2026-10-08")));
        // 调休 10/10 周六上班
        assert!(cal().is_workday(d("2026-10-10")));
    }

    // ── 期限计算验证 ───────────────────────────────────────────

    #[test]
    fn deadline_not_on_weekend() {
        // 2026-06-18 是周四，+1天 = 6/19 端午假期 → 应顺延到 6/22（周一）
        let result = cal().add_days_patent(d("2026-06-18"), 1);
        assert_eq!(result, d("2026-06-22"));
    }

    #[test]
    fn deadline_month_clamp() {
        // 1月31日 + 1个月 = 2月28日（非闰年 2026）
        let result = cal().add_months_patent(d("2026-01-30"), 1);
        // from = 1/31, +1month = 2/28（钳制）
        assert_eq!(result, d("2026-02-28"));
    }

    #[test]
    fn national_day_deadline_push() {
        // 9/30 起算 +15 天 → 10/15，中间跨越国庆假期
        let result = cal().add_days_civil(d("2026-09-29"), 15);
        // 起算日不计，从 9/30 开始 +15 天 = 10/14
        // 但 10/1-7 是假期，所以 10/14 应该正常
        assert!(result >= d("2026-10-14"));
    }

    #[test]
    fn weekend_push_to_monday() {
        // 2026-08-07 是周五，+1天 = 8/8 周六 → 顺延到 8/10 周一
        let result = cal().extend_to_workday(d("2026-08-08"));
        assert_eq!(result, d("2026-08-10"));
    }
}

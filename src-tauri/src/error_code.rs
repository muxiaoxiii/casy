//! B1 统一错误码（已覆盖：tasks / calendar / inbox；cases 起步）
//!
//! 格式：`CAS-<域名缩写><序号>|<人话消息>`
//! - 前端 tauriBridge 原样透传 error 字符串，UI 展示 `|` 后的人话部分即可
//! - 支持侧按 `CAS-` 前缀聚合定位；升级为结构化错误对象后本模块成为映射表
//! - 序号分段规划：1xxx=tasks 2xxx=calendar 3xxx=inbox 4xxx=cases/projects …

pub mod codes {
    // 1xxx tasks
    pub const TASK_NOT_FOUND: &str = "CAS-1001";
    pub const TASK_INVALID_STATE: &str = "CAS-1002";
    // 2xxx calendar
    pub const CALENDAR_NOT_FOUND: &str = "CAS-2001";
    pub const CALENDAR_INVALID_RANGE: &str = "CAS-2002";
    // 3xxx inbox
    pub const INBOX_NOT_FOUND: &str = "CAS-3001";
    pub const INBOX_INVALID_STATE: &str = "CAS-3002";
    // 4xxx cases/projects
    pub const CASE_NOT_FOUND: &str = "CAS-4001";
}

/// 组装带码错误串（供既有 `Result<T, String>` 命令签名直接返回）
pub fn err(code: &str, message: impl Into<String>) -> String {
    format!("{code}|{}", message.into())
}

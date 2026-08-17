// 展示格式化（对齐原版 src/lib/utils/format.ts）

use chrono::{Local, TimeZone};

/// YYYY-MM-DD（本地时区，与原版 Date 方法一致）
pub fn format_date(ts_ms: i64) -> String {
    let d = Local.timestamp_millis_opt(ts_ms).single();
    match d {
        Some(d) => format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()),
        None => String::new(),
    }
}

pub fn time_ago(ts_ms: i64) -> String {
    let diff_ms = chrono::Utc::now().timestamp_millis() - ts_ms;
    let min = diff_ms / 60_000;
    if min < 1 {
        return "刚刚".to_string();
    }
    if min < 60 {
        return format!("{} 分钟前", min);
    }
    let hour = min / 60;
    if hour < 24 {
        return format!("{} 小时前", hour);
    }
    let day = hour / 24;
    if day < 30 {
        return format!("{} 天前", day);
    }
    let month = day / 30;
    if month < 12 {
        return format!("{} 个月前", month);
    }
    format!("{} 年前", month / 12)
}

pub fn format_count(n: i64) -> String {
    if n >= 10_000 {
        return format!("{:.1}w", n as f64 / 10_000.0);
    }
    if n >= 1000 {
        return format!("{:.1}k", n as f64 / 1000.0);
    }
    n.to_string()
}

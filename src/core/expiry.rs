// 工程有效期工具（对齐原版 src/lib/utils/expiry.ts）
// 非匿名工程过期后额外宽限一周（直接链接仍可访问）

pub const GRACE_MS: i64 = 7 * 24 * 60 * 60 * 1000;

pub fn is_anonymous_name(name: Option<&str>) -> bool {
    match name {
        None => true,
        Some(n) => n.trim() == "匿名",
    }
}

/// 解析 ISO 时间戳（毫秒）；失败返回 None
pub fn parse_time_ms(s: &str) -> Option<i64> {
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(d.timestamp_millis());
    }
    // 兼容 Postgres 默认样式 "YYYY-MM-DD HH:MM:SS[.ffffff]"
    if let Ok(d) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(d.and_utc().timestamp_millis());
    }
    if let Ok(d) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(d.and_utc().timestamp_millis());
    }
    None
}

/// 有效期限时间戳：无到期时间返回 None；非匿名工程在到期后追加一周宽限
pub fn effective_expires_at(expires_at: Option<&str>, author_name: Option<&str>) -> Option<i64> {
    let s = expires_at?;
    let t = parse_time_ms(s)?;
    if is_anonymous_name(author_name) {
        Some(t)
    } else {
        Some(t + GRACE_MS)
    }
}

/// 是否真正失效（超过有效期限含宽限）
pub fn is_expired_project(expires_at: Option<&str>, author_name: Option<&str>, now_ms: i64) -> bool {
    match effective_expires_at(expires_at, author_name) {
        Some(eff) => now_ms >= eff,
        None => false,
    }
}

/// 是否处于宽限期（已到到期日但还在宽限内，仅非匿名工程）
pub fn is_grace_period(expires_at: Option<&str>, author_name: Option<&str>, now_ms: i64) -> bool {
    let s = expires_at?;
    let due = parse_time_ms(s)?;
    if is_anonymous_name(author_name) {
        return false;
    }
    let eff = due + GRACE_MS;
    now_ms >= due && now_ms < eff
}

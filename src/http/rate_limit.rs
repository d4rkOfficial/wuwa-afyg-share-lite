// 简单内存限频（滑动窗口；对齐原版 src/lib/api/rate-limit.ts）

use std::collections::HashMap;

pub const MAX_BUCKETS: usize = 5000;

pub struct RateLimiter {
    buckets: HashMap<String, Vec<i64>>,
    limit: usize,
    window_ms: i64,
}

impl RateLimiter {
    pub fn new(limit: usize, window_ms: i64) -> Self {
        RateLimiter {
            buckets: HashMap::new(),
            limit,
            window_ms,
        }
    }

    /// 判断 key 是否放行；limit 次 / window_ms 毫秒 内超过则拒绝
    pub fn check(&mut self, key: &str, now_ms: i64) -> bool {
        let arr = self.buckets.entry(key.to_string()).or_default();
        arr.retain(|t| now_ms - t < self.window_ms);
        if arr.len() >= self.limit {
            return false;
        }
        arr.push(now_ms);
        if self.buckets.len() > MAX_BUCKETS {
            self.buckets.retain(|_, v| {
                !v.is_empty() && now_ms - v[v.len() - 1] <= self.window_ms
            });
        }
        true
    }
}

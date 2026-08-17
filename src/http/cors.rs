// 公共 API 的 CORS 支持（对齐原版 src/lib/api/cors.ts）

pub const CORS_HEADERS: [(&str, &str); 3] = [
    ("Access-Control-Allow-Origin", "*"),
    ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
    ("Access-Control-Allow-Headers", "Content-Type, Authorization"),
];

pub fn apply_cors(headers: &mut axum::http::HeaderMap) {
    for (k, v) in CORS_HEADERS {
        if let Ok(val) = axum::http::HeaderValue::from_str(v) {
            headers.insert(k, val);
        }
    }
}

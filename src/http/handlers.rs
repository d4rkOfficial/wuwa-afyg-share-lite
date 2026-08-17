// HTTP 路由处理器
// 公开接口（/api/public/projects、/api/buff-sets、/api/buff-sets/export、
// /share/{code}/download）与原版 wuwa-afyg-share 完全一致（含状态码/CORS/错误文案）；
// /api/auth/*、/api/me/*、/api/admin/* 为本地认证与管理的扩展接口。

use anyhow::bail;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use chrono::Datelike;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::core::compress::decompress_project;
use crate::core::extract::team_display_names;
use crate::db::{now_iso, now_ms};
use crate::http::cors::apply_cors;
use crate::http::server::AppState;
use crate::repo;
use crate::service::{announcements, auth, buff_sets, cleanup, demo, projects, snapshots};
use crate::types::{ProjectItemJson, PublicProjectItem, UserCtx, UserJson};

// ── 响应助手 ────────────────────────────────────────────

fn json_res(status: StatusCode, v: Value) -> Response {
    let mut r = Response::new(Body::from(v.to_string()));
    *r.status_mut() = status;
    r.headers_mut()
        .insert("Content-Type", "application/json; charset=utf-8".parse().unwrap());
    apply_cors(r.headers_mut());
    r
}

fn json_res_no_cors(status: StatusCode, v: Value) -> Response {
    let mut r = Response::new(Body::from(v.to_string()));
    *r.status_mut() = status;
    r.headers_mut()
        .insert("Content-Type", "application/json; charset=utf-8".parse().unwrap());
    r
}

fn err_json(status: StatusCode, msg: &str) -> Response {
    json_res(status, json!({ "error": msg }))
}

fn text_res(status: StatusCode, body: String, cors: bool, extra: Vec<(&'static str, String)>) -> Response {
    let mut r = Response::new(Body::from(body));
    *r.status_mut() = status;
    for (k, v) in extra {
        if let Ok(hv) = axum::http::HeaderValue::from_str(&v) {
            r.headers_mut().insert(k, hv);
        }
    }
    if cors {
        apply_cors(r.headers_mut());
    }
    r
}

/// 业务错误 → HTTP 状态码
fn classify(msg: &str) -> StatusCode {
    if msg == "请先登录" {
        StatusCode::UNAUTHORIZED
    } else if msg.starts_with("无权限") {
        StatusCode::FORBIDDEN
    } else if msg.contains("不存在") {
        StatusCode::NOT_FOUND
    } else if msg.contains("MB 限制") || msg.contains("压缩后仍过大") {
        StatusCode::PAYLOAD_TOO_LARGE
    } else if msg.chars().all(|c| c.is_ascii()) && !msg.is_empty() {
        // 数据库/IO/解压等系统错误（英文消息）
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::BAD_REQUEST
    }
}

/// 统一结果 → 响应
fn api_result<T: serde::Serialize>(r: Result<T, anyhow::Error>) -> Response {
    match r {
        Ok(v) => json_res(StatusCode::OK, serde_json::to_value(v).unwrap_or(Value::Null)),
        Err(e) => {
            let msg = e.to_string();
            err_json(classify(&msg), &msg)
        }
    }
}

fn parse_body(bytes: axum::body::Bytes) -> Result<Value, Response> {
    serde_json::from_slice(&bytes).map_err(|_| err_json(StatusCode::BAD_REQUEST, "请求格式错误"))
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// 是否为本机回环客户端（服务器中间件注入，非回环请求会被移除该标记）
fn is_local_root(headers: &HeaderMap) -> bool {
    headers
        .get(crate::http::server::LOCAL_ROOT_HEADER)
        .and_then(|v| v.to_str().ok())
        == Some("1")
}

fn auth_user(state: &AppState, headers: &HeaderMap) -> Result<UserCtx, Response> {
    let conn = state.db.lock().unwrap();
    if is_local_root(headers) {
        return auth::ensure_root_admin(&conn)
            .map_err(|e| err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()));
    }
    auth::require_user(&conn, bearer(headers).as_deref())
        .map_err(|e| err_json(StatusCode::UNAUTHORIZED, &e.to_string()))
}

fn auth_admin(state: &AppState, headers: &HeaderMap) -> Result<UserCtx, Response> {
    let conn = state.db.lock().unwrap();
    if is_local_root(headers) {
        return auth::ensure_root_admin(&conn)
            .map_err(|e| err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()));
    }
    auth::require_admin(&conn, bearer(headers).as_deref())
        .map_err(|e| err_json(classify(&e.to_string()), &e.to_string()))
}

pub async fn handle_options() -> Response {
    let mut r = Response::new(Body::empty());
    *r.status_mut() = StatusCode::NO_CONTENT;
    apply_cors(r.headers_mut());
    r
}

/// 内嵌 Web 界面首页（单文件原生 Web Components，零构建）
pub async fn index_page() -> Response {
    text_res(
        StatusCode::OK,
        crate::web::INDEX_HTML.to_string(),
        false,
        vec![("Content-Type", "text/html; charset=utf-8".to_string())],
    )
}

pub async fn not_found() -> Response {
    json_res_no_cors(StatusCode::NOT_FOUND, json!({ "error": "未找到该接口" }))
}

// ── 公开接口（与原版一致）────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub q: Option<String>,
    pub sort: Option<String>,
    pub exclude_anon: Option<String>,
}

pub async fn public_projects_get(
    State(state): State<AppState>,
    Query(q): Query<ProjectsQuery>,
) -> Response {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(12).clamp(1, 50);
    let filter = repo::PublicListFilter {
        q: q.q
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        sort: q.sort.unwrap_or_else(|| "newest".to_string()),
        exclude_anon: q.exclude_anon.as_deref() == Some("1"),
    };
    let now = now_iso();
    let conn = state.db.lock().unwrap();
    let result = (|| -> anyhow::Result<Value> {
        let total = repo::count_public_projects(&conn, &filter, &now)?;
        let rows = repo::list_public_projects(&conn, &filter, page, per_page, &now)?;
        let projects: Vec<PublicProjectItem> = rows
            .iter()
            .map(|p| PublicProjectItem {
                id: p.id.clone(),
                code: p.code.clone(),
                title: p.title.clone(),
                author_name: p.author_name.clone(),
                tags: p.tags.clone(),
                game_version: p.game_version.clone(),
                team_preview: p.team_preview.clone(),
                downloads: p.clone_count,
                created_at: p.created_at.clone(),
            })
            .collect();
        Ok(json!({ "projects": projects, "total": total, "page": page, "perPage": per_page }))
    })();
    api_result(result)
}

pub async fn public_projects_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let file_text = body.get("fileText").and_then(|v| v.as_str()).unwrap_or("");
    if file_text.trim().is_empty() {
        return err_json(StatusCode::BAD_REQUEST, "缺少工程文件内容");
    }

    // 匿名分享防滥用：按来源 IP 限频（对齐原版 cf-connecting-ip）
    let client_ip = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    {
        let mut limiter = state.limiter.lock().unwrap();
        if !limiter.check(&format!("post-ip:{}", client_ip), now_ms()) {
            return err_json(StatusCode::TOO_MANY_REQUESTS, "操作过于频繁，请稍后再试");
        }
    }

    let conn = state.db.lock().unwrap();
    match projects::anonymous_share(&conn, file_text) {
        Ok(code) => {
            let host = headers
                .get("host")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("localhost:3000");
            let url = format!(
                "{}/share/{}",
                state.site_url.as_deref().unwrap_or(&format!("http://{}", host)),
                code
            );
            json_res(StatusCode::CREATED, json!({ "code": code, "url": url }))
        }
        Err(e) => match e {
            projects::PrepError::Parse(m) => err_json(StatusCode::BAD_REQUEST, &m),
            projects::PrepError::TooLarge(m) => err_json(StatusCode::PAYLOAD_TOO_LARGE, &m),
            projects::PrepError::Other(m) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &m),
        },
    }
}

#[derive(Deserialize)]
pub struct BuffSetsQuery {
    pub entity_type: Option<String>,
    pub entity_name: Option<String>,
    pub q: Option<String>,
}

pub async fn buff_sets_get(State(state): State<AppState>, Query(q): Query<BuffSetsQuery>) -> Response {
    let conn = state.db.lock().unwrap();
    let result = buff_sets::list_buff_sets(
        &conn,
        q.entity_type.clone(),
        q.entity_name.clone(),
        q.q.clone(),
    );
    match result {
        Ok(rows) => {
            let total = rows.len();
            json_res(StatusCode::OK, json!({ "buffSets": rows, "total": total }))
        }
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// 公开 API：Buff 集全量 SQL 导出（对齐原版；无 CORS 头）
pub async fn buff_sets_export(State(state): State<AppState>) -> Response {
    let conn = state.db.lock().unwrap();
    let result = (|| -> anyhow::Result<(Vec<crate::types::BuffSetRow>, String)> {
        let chain = repo::list_snapshot_chain(&conn)?;
        let has_root = chain.iter().any(|s| s.is_root);
        if has_root {
            let items = repo::snapshot_chain_items(&chain);
            let rows = crate::core::buff_diff::rebuild_snapshot_state(&items, None)
                .ok_or_else(|| anyhow::anyhow!("快照状态缺失"))?;
            return Ok((rows, "最新快照".to_string()));
        }
        let rows = buff_sets::list_buff_sets(&conn, None, None, None)?;
        Ok((rows, "实时数据（暂无快照）".to_string()))
    })();
    match result {
        Ok((rows, source)) => {
            let meta = crate::core::export_sql::ExportMeta {
                source,
                exported_at: now_iso(),
            };
            let sql = crate::core::export_sql::buff_sets_to_sql(&rows, &meta);
            let now = chrono::Local::now();
            let date = format!("{:04}{:02}{:02}", now.year(), now.month(), now.day());
            text_res(
                StatusCode::OK,
                sql,
                false,
                vec![
                    ("Content-Type", "application/sql; charset=utf-8".to_string()),
                    (
                        "Content-Disposition",
                        format!("attachment; filename=\"buff-sets-{}.sql\"", date),
                    ),
                ],
            )
        }
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn share_download(State(state): State<AppState>, Path(code): Path<String>) -> Response {    let now = now_iso();
    let conn = state.db.lock().unwrap();
    let result = (|| -> anyhow::Result<Option<(String, Vec<u8>)>> {
        let Some(found) = repo::get_project_blob_by_code(&conn, &code, &now)? else {
            return Ok(None);
        };
        repo::bump_counter(&conn, &found.item.id, "clones")?;
        let text = decompress_project(&found.blob)?;
        Ok(Some((found.item.title, text.as_bytes().to_vec())))
    })();
    match result {
        Ok(Some((title, bytes))) => {
            let filename = format!(
                "{}%2Ejson",
                crate::core::percent::percent_encode_utf8(&title)
            );
            let body = String::from_utf8(bytes).unwrap_or_default();
            text_res(
                StatusCode::OK,
                body,
                true,
                vec![
                    ("Content-Type", "application/json; charset=utf-8".to_string()),
                    (
                        "Content-Disposition",
                        format!("attachment; filename*=UTF-8''{}", filename),
                    ),
                ],
            )
        }
        Ok(None) => text_res(StatusCode::NOT_FOUND, "分享已失效".to_string(), true, vec![]),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ── 上游名录（nanoka.cc；扩展公开接口）──────────────────

/// GET /api/catalog：全部名录（角色/武器/声骸/套装）
pub async fn catalog_get(State(state): State<AppState>) -> Response {
    let conn = state.db.lock().unwrap();
    match state.upstream.catalog(Some(&conn)) {
        Ok(data) => {
            let mut v = serde_json::to_value(&data).unwrap_or(Value::Null);
            if let Some(obj) = v.as_object_mut() {
                // 补充条目计数
                obj.insert(
                    "counts".to_string(),
                    json!({
                        "characters": data.characters.len(),
                        "weapons": data.weapons.len(),
                        "echoes": data.echoes.len(),
                        "sets": data.sets.len(),
                    }),
                );
            }
            json_res(StatusCode::OK, v)
        }
        Err(e) => err_json(StatusCode::BAD_GATEWAY, &e.to_string()),
    }
}

/// GET /api/catalog/{type}：单项名录（characters/weapons/echoes/sets）
pub async fn catalog_type_get(
    State(state): State<AppState>,
    Path(entity_type): Path<String>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = state.upstream.catalog(Some(&conn));
    match result {
        Ok(data) => {
            let key = match entity_type.as_str() {
                "characters" => "characters",
                "weapons" => "weapons",
                "echoes" => "echoes",
                "sets" => "sets",
                _ => return err_json(StatusCode::BAD_REQUEST, "无效的名录类型：characters/weapons/echoes/sets"),
            };
            let items = match key {
                "characters" => serde_json::to_value(&data.characters).unwrap_or(Value::Null),
                "weapons" => serde_json::to_value(&data.weapons).unwrap_or(Value::Null),
                "echoes" => serde_json::to_value(&data.echoes).unwrap_or(Value::Null),
                _ => serde_json::to_value(&data.sets).unwrap_or(Value::Null),
            };
            json_res(
                StatusCode::OK,
                json!({
                    "version": data.version,
                    "source": data.source,
                    "stale": data.stale,
                    key: items,
                }),
            )
        }
        Err(e) => err_json(StatusCode::BAD_GATEWAY, &e.to_string()),
    }
}

// ── 公告（公开读）────────────────────────────────────────

pub async fn announcements_get(State(state): State<AppState>) -> Response {
    let conn = state.db.lock().unwrap();
    match announcements::list_announcements(&conn) {
        Ok(rows) => json_res(StatusCode::OK, json!({ "announcements": rows })),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ── 本地认证扩展 ─────────────────────────────────────────

fn user_obj(u: &UserCtx) -> Value {
    serde_json::to_value(UserJson::from(u)).unwrap_or(Value::Null)
}

pub async fn auth_register(State(state): State<AppState>, bytes: axum::body::Bytes) -> Response {
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let username = body.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let password = body.get("password").and_then(|v| v.as_str()).unwrap_or("");
    let conn = state.db.lock().unwrap();
    match auth::register(&conn, username, password) {
        Ok((token, user)) => json_res(
            StatusCode::CREATED,
            json!({ "token": token, "user": user_obj(&user) }),
        ),
        Err(e) => err_json(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn auth_login(State(state): State<AppState>, bytes: axum::body::Bytes) -> Response {
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let username = body.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let password = body.get("password").and_then(|v| v.as_str()).unwrap_or("");
    let conn = state.db.lock().unwrap();
    match auth::login(&conn, username, password) {
        Ok((token, user)) => json_res(
            StatusCode::OK,
            json!({ "token": token, "user": user_obj(&user) }),
        ),
        Err(e) => err_json(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn auth_logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let conn = state.db.lock().unwrap();
    if let Some(token) = bearer(&headers) {
        let _ = auth::logout(&conn, &token);
    }
    json_res(StatusCode::OK, json!({}))
}

pub async fn auth_me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match auth_user(&state, &headers) {
        Ok(user) => json_res(StatusCode::OK, json!({ "user": user_obj(&user) })),
        Err(r) => r,
    }
}

// ── 我的工程（登录）─────────────────────────────────────

pub async fn me_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    let result = projects::my_projects(&conn, &user);
    api_result(result.map(|(items, stats)| {
        json!({
            "profile": user_obj(&user),
            "stats": {
                "projects": stats.projects,
                "expiredCount": stats.expired_count,
                "totalViews": stats.total_views,
                "totalClones": stats.total_clones,
            },
            "projects": items.iter().map(ProjectItemJson::from).collect::<Vec<_>>(),
        })
    }))
}

pub async fn me_update_username(State(state): State<AppState>, headers: HeaderMap, bytes: axum::body::Bytes) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let username = body.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let conn = state.db.lock().unwrap();
    api_result(projects::update_username(&conn, &user, username))
}

pub async fn me_projects_list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    let result = repo::list_my_projects(&conn, &user.id);
    match result {
        Ok(items) => json_res(
            StatusCode::OK,
            json!({ "projects": items.iter().map(ProjectItemJson::from).collect::<Vec<_>>() }),
        ),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn me_projects_publish(State(state): State<AppState>, headers: HeaderMap, bytes: axum::body::Bytes) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let file_text = body.get("fileText").and_then(|v| v.as_str()).unwrap_or("");
    if file_text.trim().is_empty() {
        return err_json(StatusCode::BAD_REQUEST, "缺少工程文件内容");
    }
    let input = projects::PublishInput {
        file_text: file_text.to_string(),
        description: body
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tags: body
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        expires_days: body.get("expiresDays").and_then(|v| v.as_i64()),
        expires_at: body.get("expiresAt").and_then(|v| v.as_str()).map(|s| s.to_string()),
    };
    let conn = state.db.lock().unwrap();
    match projects::publish_project(&conn, &user, &input) {
        Ok(code) => json_res(StatusCode::CREATED, json!({ "code": code })),
        Err(e) => match e.downcast_ref::<projects::PrepError>() {
            Some(prep) => match prep {
                projects::PrepError::Parse(m) => err_json(StatusCode::BAD_REQUEST, m),
                projects::PrepError::TooLarge(m) => err_json(StatusCode::PAYLOAD_TOO_LARGE, m),
                projects::PrepError::Other(m) => err_json(StatusCode::INTERNAL_SERVER_ERROR, m),
            },
            None => err_json(classify(&e.to_string()), &e.to_string()),
        },
    }
}

pub async fn me_project_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let input = projects::UpdateInput {
        title: body.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        description: body.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        tags: body
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    };
    let conn = state.db.lock().unwrap();
    api_result(projects::update_project(&conn, &user, &id, &input))
}

pub async fn me_project_set_expiry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let expires_at = body.get("expiresAt").and_then(|v| v.as_str()).map(|s| s.to_string());
    let conn = state.db.lock().unwrap();
    api_result(projects::set_expiry(&conn, &user, &id, expires_at.as_deref()))
}

pub async fn me_project_replace_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let file_text = body.get("fileText").and_then(|v| v.as_str()).unwrap_or("");
    if file_text.trim().is_empty() {
        return err_json(StatusCode::BAD_REQUEST, "缺少工程文件内容");
    }
    let conn = state.db.lock().unwrap();
    api_result(projects::replace_project_file(&conn, &user, &id, file_text))
}

pub async fn me_project_regenerate_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    match projects::regenerate_code(&conn, &user, &id) {
        Ok(code) => json_res(StatusCode::OK, json!({ "code": code })),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn me_project_toggle_publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    match projects::toggle_publish(&conn, &user, &id) {
        Ok(published) => json_res(StatusCode::OK, json!({ "published": published })),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn me_project_set_protected(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let is_protected = body.get("protected").and_then(|v| v.as_bool()).unwrap_or(false);
    let conn = state.db.lock().unwrap();
    api_result(projects::set_project_protected(&conn, &user, &id, is_protected))
}

pub async fn me_project_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    api_result(projects::delete_project(&conn, &user, &id))
}

pub async fn me_clear_content(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    match projects::delete_my_content(&conn, &user) {
        Ok(deleted_projects) => {
            json_res(StatusCode::OK, json!({ "deletedProjects": deleted_projects }))
        }
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ── 管理员扩展 ──────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminProjectsQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn admin_projects_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<AdminProjectsQuery>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    match projects::admin_list_projects(&conn, q.q.clone(), q.page.unwrap_or(1), q.page_size.unwrap_or(20)) {
        Ok((items, total)) => json_res(
            StatusCode::OK,
            json!({ "items": items.iter().map(ProjectItemJson::from).collect::<Vec<_>>(), "total": total }),
        ),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn admin_project_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let title = body.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
    let author_name = body.get("authorName").and_then(|v| v.as_str()).map(|s| s.to_string());
    let conn = state.db.lock().unwrap();
    api_result(projects::admin_update_project(&conn, &id, title, author_name))
}

pub async fn admin_project_set_expiry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let expires_at = body.get("expiresAt").and_then(|v| v.as_str()).map(|s| s.to_string());
    let conn = state.db.lock().unwrap();
    api_result(projects::admin_set_expiry(&conn, &id, expires_at.as_deref()))
}

pub async fn admin_project_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    api_result(projects::admin_delete_project(&conn, &id))
}

pub async fn admin_users_list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let me = match auth_admin(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    match projects::admin_list_admins(&conn, &me) {
        Ok((views, my_id)) => {
            let admins: Vec<Value> = views
                .iter()
                .map(|v| {
                    json!({
                        "id": v.id,
                        "username": v.username,
                        "grantedBy": v.granted_by,
                        "grantedByMe": v.granted_by_me,
                    })
                })
                .collect();
            json_res(StatusCode::OK, json!({ "admins": admins, "myId": my_id }))
        }
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn admin_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> Response {
    let me = match auth_admin(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    match projects::admin_grant(&conn, &me, &username) {
        Ok(message) => json_res(StatusCode::OK, json!({ "message": message })),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn admin_revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> Response {
    let me = match auth_admin(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    match projects::admin_revoke(&conn, &me, &username) {
        Ok(message) => json_res(StatusCode::OK, json!({ "message": message })),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn admin_user_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    match projects::admin_user_summary(&conn, &username) {
        Ok((name, count)) => {
            json_res(StatusCode::OK, json!({ "username": name, "projectCount": count }))
        }
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn admin_delete_user_content(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    match projects::admin_delete_user_content(&conn, &username) {
        Ok(deleted_projects) => {
            json_res(StatusCode::OK, json!({ "deletedProjects": deleted_projects }))
        }
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

// ── Buff 集管理（管理员）────────────────────────────────

pub async fn admin_buff_set_upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let input = match buff_sets::buff_set_input_from_json(&body) {
        Ok(i) => i,
        Err(e) => return err_json(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    let conn = state.db.lock().unwrap();
    api_result(buff_sets::upsert_buff_set(&conn, &input))
}

pub async fn admin_buff_set_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((entity_type, entity_name, buff_name)): Path<(String, String, String)>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    api_result(buff_sets::delete_buff_preset(&conn, &entity_type, &entity_name, &buff_name))
}

pub async fn admin_buff_entity_upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let input = match buff_sets::buff_entity_input_from_json(&body) {
        Ok(i) => i,
        Err(e) => return err_json(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    let conn = state.db.lock().unwrap();
    match buff_sets::upsert_buff_entity(&conn, &input) {
        Ok(saved) => json_res(StatusCode::OK, json!({ "saved": saved })),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn admin_buff_entity_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((entity_type, entity_name)): Path<(String, String)>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    api_result(buff_sets::delete_buff_entity(&conn, &entity_type, &entity_name))
}

// ── 快照管理（管理员）───────────────────────────────────

pub async fn admin_snapshots_list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    match snapshots::list_snapshots(&conn) {
        Ok(views) => {
            let list: Vec<Value> = views
                .iter()
                .map(|s| {
                    json!({
                        "id": s.id,
                        "isRoot": s.is_root,
                        "note": s.note,
                        "createdBy": s.created_by,
                        "createdAt": s.created_at,
                        "isLatest": s.is_latest,
                        "canDelete": s.can_delete,
                    })
                })
                .collect();
            json_res(StatusCode::OK, json!({ "snapshots": list }))
        }
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn admin_snapshot_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    let me = match auth_admin(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let note = body.get("note").and_then(|v| v.as_str()).unwrap_or("");
    let conn = state.db.lock().unwrap();
    match snapshots::save_snapshot(&conn, &me, note) {
        Ok((rows, mode)) => json_res(StatusCode::CREATED, json!({ "rows": rows, "mode": mode })),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn admin_snapshot_diff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    match snapshots::get_snapshot_diff(&conn, &id) {
        Ok(view) => json_res(
            StatusCode::OK,
            json!({
                "target": { "isRoot": view.target.is_root, "note": view.target.note, "createdAt": view.target.created_at },
                "diff": view.diff,
                "currentCount": view.current_count,
            }),
        ),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn admin_snapshot_restore(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let mut conn = state.db.lock().unwrap();
    match snapshots::restore_snapshot(&mut conn, &id) {
        Ok(restored) => json_res(StatusCode::OK, json!({ "restored": restored })),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

pub async fn admin_snapshot_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    match snapshots::delete_snapshot(&conn, &id) {
        Ok(message) => json_res(StatusCode::OK, json!({ "message": message })),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

// ── 公告管理（管理员）───────────────────────────────────

pub async fn admin_announcement_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let title = body.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let conn = state.db.lock().unwrap();
    api_result(announcements::create_announcement(&conn, title, content))
}

pub async fn admin_announcement_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let body = match parse_body(bytes) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let title = body.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let conn = state.db.lock().unwrap();
    api_result(announcements::update_announcement(&conn, &id, title, content))
}

pub async fn admin_announcement_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    api_result(announcements::delete_announcement(&conn, &id))
}

// ── 运维 ────────────────────────────────────────────────

pub async fn admin_cleanup(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    match cleanup::delete_expired_projects(&conn) {
        Ok(deleted) => json_res(StatusCode::OK, json!({ "deleted": deleted })),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn admin_demo(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(r) = auth_admin(&state, &headers) {
        return r;
    }
    let conn = state.db.lock().unwrap();
    match demo::seed_demo(&conn) {
        Ok(message) => json_res(StatusCode::OK, json!({ "message": message })),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// 公开详情（扩展接口：原版为 SSR 页面 /share/[code]，无 JSON API；
/// 本接口对齐页面语义：已发布未过期才可见，访问时浏览 +1）
pub async fn public_project_detail(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Response {
    let now = now_iso();
    let conn = state.db.lock().unwrap();
    let result = (|| -> anyhow::Result<Option<Value>> {
        let Some(p) = repo::get_project_item_by_code(&conn, &code)? else {
            return Ok(None);
        };
        if !p.published {
            return Ok(None);
        }
        if let Some(e) = &p.expires_at {
            if e.as_str() <= now.as_str() {
                return Ok(None);
            }
        }
        repo::bump_counter(&conn, &p.id, "views")?;
        let names = team_display_names(p.team_preview.as_ref());
        Ok(Some(json!({
            "id": p.id,
            "code": p.code,
            "title": p.title,
            "authorName": p.author_name,
            "description": p.description,
            "tags": p.tags,
            "gameVersion": p.game_version,
            "teamPreview": p.team_preview,
            "teamNames": names,
            "downloads": p.clone_count,
            "views": p.view_count + 1,
            "createdAt": p.created_at,
            "expiresAt": p.expires_at,
        })))
    })();
    match result {
        Ok(Some(v)) => json_res(StatusCode::OK, v),
        Ok(None) => err_json(StatusCode::NOT_FOUND, "工程不存在"),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// 我的工程详情（扩展接口：作者/管理员可看完整行，含未发布/已过期工程）
pub async fn me_project_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let user = match auth_user(&state, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let conn = state.db.lock().unwrap();
    let result = (|| -> anyhow::Result<Option<ProjectItemJson>> {
        let Some(p) = repo::get_project_item_by_id(&conn, &id)? else {
            return Ok(None);
        };
        if p.author_id.as_deref() != Some(user.id.as_str()) && !user.is_admin {
            bail!("无权限：仅作者或管理员可执行该操作");
        }
        Ok(Some(ProjectItemJson::from(&p)))
    })();
    match result {
        Ok(Some(v)) => json_res(StatusCode::OK, serde_json::to_value(v).unwrap_or(Value::Null)),
        Ok(None) => err_json(StatusCode::NOT_FOUND, "工程不存在"),
        Err(e) => err_json(classify(&e.to_string()), &e.to_string()),
    }
}

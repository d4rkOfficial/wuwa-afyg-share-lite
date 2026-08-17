// HTTP 服务：路由装配与启动（axum）

use anyhow::{Context, Result};
use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use crate::db::open_db;
use crate::http::handlers;
use crate::http::rate_limit::RateLimiter;
use crate::repo;
use crate::upstream::Upstream;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub limiter: Arc<Mutex<RateLimiter>>,
    /// 分享链接前缀（POST /api/public/projects 返回的 url 字段）
    pub site_url: Option<String>,
    /// 上游数据（nanoka.cc 角色/武器/声骸/套装名录）
    pub upstream: Arc<Upstream>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        // ── 公开接口（与原版一致）──
        .route(
            "/api/public/projects",
            get(handlers::public_projects_get)
                .post(handlers::public_projects_post)
                .options(handlers::handle_options),
        )
        .route(
            "/api/public/projects/{code}",
            get(handlers::public_project_detail),
        )
        .route(
            "/api/buff-sets",
            get(handlers::buff_sets_get).options(handlers::handle_options),
        )
        .route("/api/buff-sets/export", get(handlers::buff_sets_export))
        // ── 上游名录（nanoka.cc；扩展公开接口）──
        .route(
            "/api/catalog",
            get(handlers::catalog_get).options(handlers::handle_options),
        )
        .route(
            "/api/catalog/{entity_type}",
            get(handlers::catalog_type_get).options(handlers::handle_options),
        )
        .route(
            "/share/{code}/download",
            get(handlers::share_download).options(handlers::handle_options),
        )
        // ── 公开读：公告 ──
        .route("/api/announcements", get(handlers::announcements_get))
        // ── 本地认证扩展 ──
        .route("/api/auth/register", post(handlers::auth_register))
        .route("/api/auth/login", post(handlers::auth_login))
        .route("/api/auth/logout", post(handlers::auth_logout))
        .route("/api/auth/me", get(handlers::auth_me))
        // ── 我的工程 ──
        .route("/api/me", get(handlers::me_get))
        .route("/api/me/username", patch(handlers::me_update_username))
        .route(
            "/api/me/projects",
            get(handlers::me_projects_list).post(handlers::me_projects_publish),
        )
        .route(
            "/api/me/projects/{id}",
            get(handlers::me_project_get)
                .patch(handlers::me_project_update)
                .delete(handlers::me_project_delete),
        )
        .route("/api/me/projects/{id}/expiry", put(handlers::me_project_set_expiry))
        .route("/api/me/projects/{id}/file", put(handlers::me_project_replace_file))
        .route(
            "/api/me/projects/{id}/regenerate-code",
            post(handlers::me_project_regenerate_code),
        )
        .route(
            "/api/me/projects/{id}/toggle-publish",
            post(handlers::me_project_toggle_publish),
        )
        .route(
            "/api/me/projects/{id}/protected",
            put(handlers::me_project_set_protected),
        )
        .route("/api/me/clear-content", post(handlers::me_clear_content))
        // ── 管理员 ──
        .route("/api/admin/projects", get(handlers::admin_projects_list))
        .route(
            "/api/admin/projects/{id}",
            patch(handlers::admin_project_update).delete(handlers::admin_project_delete),
        )
        .route("/api/admin/projects/{id}/expiry", put(handlers::admin_project_set_expiry))
        .route("/api/admin/users", get(handlers::admin_users_list))
        .route("/api/admin/users/{username}/grant", post(handlers::admin_grant))
        .route("/api/admin/users/{username}/revoke", post(handlers::admin_revoke))
        .route(
            "/api/admin/users/{username}/summary",
            get(handlers::admin_user_summary),
        )
        .route(
            "/api/admin/users/{username}/content",
            delete(handlers::admin_delete_user_content),
        )
        .route("/api/admin/buff-sets", post(handlers::admin_buff_set_upsert))
        .route(
            "/api/admin/buff-sets/{entity_type}/{entity_name}/{buff_name}",
            delete(handlers::admin_buff_set_delete),
        )
        .route("/api/admin/buff-entities", post(handlers::admin_buff_entity_upsert))
        .route(
            "/api/admin/buff-entities/{entity_type}/{entity_name}",
            delete(handlers::admin_buff_entity_delete),
        )
        .route(
            "/api/admin/snapshots",
            get(handlers::admin_snapshots_list).post(handlers::admin_snapshot_create),
        )
        .route(
            "/api/admin/snapshots/{id}/diff",
            get(handlers::admin_snapshot_diff),
        )
        .route(
            "/api/admin/snapshots/{id}/restore",
            post(handlers::admin_snapshot_restore),
        )
        .route("/api/admin/snapshots/{id}", delete(handlers::admin_snapshot_delete))
        .route(
            "/api/admin/announcements",
            post(handlers::admin_announcement_create),
        )
        .route(
            "/api/admin/announcements/{id}",
            patch(handlers::admin_announcement_update).delete(handlers::admin_announcement_delete),
        )
        .route("/api/admin/cleanup", post(handlers::admin_cleanup))
        .route("/api/admin/demo", post(handlers::admin_demo))
        .fallback(handlers::not_found)
        // 本地免登录：回环地址客户端自动标记为 root_admin（非回环请求会移除该标记，防伪造）
        .layer(axum::middleware::from_fn(local_root_middleware))
        .with_state(state)
}

/// 回环地址（localhost）客户端 → 插入 x-lite-local-root 标记；
/// 非回环客户端 → 删除该标记（防止伪造）。
pub const LOCAL_ROOT_HEADER: &str = "x-lite-local-root";

async fn local_root_middleware(
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if addr.ip().is_loopback() {
        req.headers_mut().insert(
            axum::http::HeaderName::from_static(LOCAL_ROOT_HEADER),
            axum::http::HeaderValue::from_static("1"),
        );
    } else {
        req.headers_mut().remove(axum::http::HeaderName::from_static(LOCAL_ROOT_HEADER));
    }
    next.run(req).await
}

/// 启动 HTTP 服务；每 5 分钟自动清理过期工程（对齐原版 pg_cron）
pub async fn run_server(
    db_path: String,
    host: String,
    port: u16,
    site_url: Option<String>,
) -> Result<()> {
    let conn = open_db(&db_path)?;
    repo::purge_expired_sessions(&conn)?;
    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        limiter: Arc::new(Mutex::new(RateLimiter::new(20, 60_000))),
        site_url,
        upstream: Arc::new(Upstream::new(
            std::env::var("WUWA_AFYG_SHARE_WW_VERSION").ok(),
        )),
    };

    // 启动时清理一次 + 定时清理
    {
        let db = state.db.clone();
        let _ = cleanup_once(db.clone());
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            interval.tick().await; // 跳过立即触发
            loop {
                interval.tick().await;
                let _ = cleanup_once(db.clone());
            }
        });
    }

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("无法监听 {}", addr))?;
    println!("wuwa-afyg-share-lite 服务已启动");
    println!("  数据库：{}", db_path);
    println!("  监听：  http://{}", addr);
    if let Some(su) = &state.site_url {
        println!("  站点前缀：{}", su);
    }
    println!("  公开接口：GET/POST /api/public/projects、GET /api/buff-sets、GET /api/buff-sets/export、GET /share/<code>/download");
    println!("  上游名录：GET /api/catalog（nanoka.cc 角色/武器/声骸/套装）");
    println!("按 Ctrl+C 停止");

    axum::serve(
        listener,
        build_router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

fn cleanup_once(db: Arc<Mutex<Connection>>) -> rusqlite::Result<()> {
    if let Ok(conn) = db.lock() {
        let _ = crate::service::cleanup::delete_expired_projects(&conn);
    }
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

// 工程业务逻辑（对齐原版 src/lib/actions/projects.ts / admin-projects.ts /
// project-protection.ts / profile.ts 的行为，权限在服务层显式校验）

use anyhow::{bail, Result};
use rusqlite::Connection;

use crate::core::compress::{assert_raw_size, compress_project_text, MAX_RAW_BYTES};
use crate::core::extract::{extract_team_preview, TeamPreview};
use crate::core::parse::{parse_project_file, safe_json_parse, ProjectData, EXPORT_VERSION};
use crate::core::slug::generate_code;
use crate::db::{new_uuid, now_iso, now_ms, sanitize_tags, trunc_chars};
use crate::repo;
use crate::types::{ProjectListItem, UserCtx};

/// 匿名分享 10 分钟有效期（对齐原版）
pub const ANON_EXPIRES_MS: i64 = 10 * 60 * 1000;

#[derive(Debug)]
pub enum PrepError {
    Parse(String),
    TooLarge(String),
    Other(String),
}

impl PrepError {
    pub fn msg(&self) -> &str {
        match self {
            PrepError::Parse(m) | PrepError::TooLarge(m) | PrepError::Other(m) => m,
        }
    }
}

impl std::fmt::Display for PrepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg())
    }
}

impl std::error::Error for PrepError {}

pub struct Prepared {
    pub project: ProjectData,
    pub preview: TeamPreview,
    pub blob: Vec<u8>,
    pub file_size: i64,
}

/// 解析 + 提取预览 + 压缩工程文件（发布与换源共用；对齐 prepareProjectFile）
pub fn prepare_project_file(file_text: &str) -> Result<Prepared, PrepError> {
    let raw = safe_json_parse(file_text, MAX_RAW_BYTES).map_err(|e| {
        if e.to_string().contains("MB 限制") {
            PrepError::TooLarge(e.to_string())
        } else {
            PrepError::Parse(e.to_string())
        }
    })?;
    let project = parse_project_file(&raw).map_err(|e| PrepError::Parse(e.to_string()))?;
    let preview = extract_team_preview(&project);
    let file = crate::core::parse::ProjectFile {
        version: EXPORT_VERSION,
        exported_at: crate::core::parse::now_ms_f64(),
        project: project.clone(),
    };
    let text = serde_json::to_string(&file).map_err(|e| PrepError::Other(e.to_string()))?;
    let file_size = text.len() as i64;
    assert_raw_size(&text).map_err(|e| PrepError::TooLarge(e.to_string()))?;
    let blob = compress_project_text(&text).map_err(|e| PrepError::TooLarge(e.to_string()))?;
    Ok(Prepared { project, preview, blob, file_size })
}

pub struct PublishInput {
    pub file_text: String,
    pub description: String,
    pub tags: Vec<String>,
    pub expires_days: Option<i64>,
    pub expires_at: Option<String>,
}

/// 登录用户发布（对齐 publishProject）
pub fn publish_project(conn: &Connection, user: &UserCtx, input: &PublishInput) -> Result<String> {
    let prep = prepare_project_file(&input.file_text)?;
    let description = trunc_chars(input.description.trim(), 500);
    let tags = sanitize_tags(&input.tags);
    let expires_at = input.expires_at.clone().or_else(|| {
        input
            .expires_days
            .map(|d| now_iso_delta(d * 86_400_000))
    });
    let name = prep.project.name.trim().to_string();
    let author = trunc_chars(user.username.trim(), 20);

    for _ in 0..5 {
        let code = generate_code();
        let row = ProjectRowForInsert {
            code: &code,
            author_id: Some(&user.id),
            author_name: &author,
            title: &name,
            description: &description,
            tags: &tags,
            preview: &prep.preview,
            blob: &prep.blob,
            file_size: prep.file_size,
            expires_at: expires_at.as_deref(),
        };
        match repo::insert_project(conn, &row.to_row(), row.blob) {
            Ok(()) => return Ok(code),
            Err(e) if repo::unique_violation(&e) => continue,
            Err(e) => bail!(e.to_string()),
        }
    }
    bail!("分享码生成冲突，请重试")
}

/// 匿名分享（POST /api/public/projects 共用；对齐原版 POST 逻辑）
pub fn anonymous_share(conn: &Connection, file_text: &str) -> Result<String, PrepError> {
    let prep = prepare_project_file(file_text)?;
    let name = {
        let n = trunc_chars(prep.project.name.trim(), 60);
        if n.is_empty() {
            "未命名工程".to_string()
        } else {
            n
        }
    };
    let expires_at = now_iso_delta(ANON_EXPIRES_MS);

    for _ in 0..5 {
        let code = generate_code();
        let row = ProjectRowForInsert {
            code: &code,
            author_id: None,
            author_name: "匿名",
            title: &name,
            description: "",
            tags: &[],
            preview: &prep.preview,
            blob: &prep.blob,
            file_size: prep.file_size,
            expires_at: Some(&expires_at),
        };
        match repo::insert_project(conn, &row.to_row(), row.blob) {
            Ok(()) => return Ok(code),
            Err(e) if repo::unique_violation(&e) => continue,
            Err(e) => return Err(PrepError::Other(e.to_string())),
        }
    }
    Err(PrepError::Other("分享码生成冲突，请重试".to_string()))
}

struct ProjectRowForInsert<'a> {
    code: &'a str,
    author_id: Option<&'a str>,
    author_name: &'a str,
    title: &'a str,
    description: &'a str,
    tags: &'a [String],
    preview: &'a TeamPreview,
    blob: &'a [u8],
    file_size: i64,
    expires_at: Option<&'a str>,
}

impl ProjectRowForInsert<'_> {
    fn to_row(&self) -> crate::types::ProjectRow {
        crate::types::ProjectRow {
            id: new_uuid(),
            code: self.code.to_string(),
            author_id: self.author_id.map(|s| s.to_string()),
            author_name: self.author_name.to_string(),
            title: self.title.to_string(),
            description: self.description.to_string(),
            tags: self.tags.to_vec(),
            game_version: None,
            team_preview: Some(self.preview.clone()),
            file_size: self.file_size,
            published: true,
            expires_at: self.expires_at.map(|s| s.to_string()),
            view_count: 0,
            clone_count: 0,
            created_at: now_iso(),
            updated_at: now_iso(),
            protected: false,
        }
    }
}

fn now_iso_delta(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(now_ms() + ms)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(now_iso)
}

// ── 我的工程 ────────────────────────────────────────────

/// 校验工程归属（作者或管理员）；返回工程
fn require_owner_or_admin(conn: &Connection, user: &UserCtx, id: &str) -> Result<ProjectListItem> {
    let Some(project) = repo::get_project_item_by_id(conn, id)? else {
        bail!("工程不存在");
    };
    if project.author_id.as_deref() != Some(user.id.as_str()) && !user.is_admin {
        bail!("无权限：仅作者或管理员可执行该操作");
    }
    Ok(project)
}

pub struct UpdateInput {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
}

pub fn update_project(conn: &Connection, user: &UserCtx, id: &str, input: &UpdateInput) -> Result<()> {
    require_owner_or_admin(conn, user, id)?;
    let title = trunc_chars(input.title.trim(), 60);
    if title.is_empty() {
        bail!("标题不能为空");
    }
    repo::update_project_fields(
        conn,
        id,
        &title,
        &trunc_chars(input.description.trim(), 500),
        &sanitize_tags(&input.tags),
    )?;
    Ok(())
}

pub fn set_expiry(conn: &Connection, user: &UserCtx, id: &str, expires_at: Option<&str>) -> Result<()> {
    require_owner_or_admin(conn, user, id)?;
    repo::set_project_expiry(conn, id, expires_at)?;
    Ok(())
}

pub fn replace_project_file(conn: &Connection, user: &UserCtx, id: &str, file_text: &str) -> Result<()> {
    require_owner_or_admin(conn, user, id)?;
    let prep = prepare_project_file(file_text)?;
    repo::replace_project_file(
        conn,
        id,
        &trunc_chars(prep.project.name.trim(), 60),
        Some(&prep.preview),
        &prep.blob,
        prep.file_size,
    )?;
    Ok(())
}

pub fn regenerate_code(conn: &Connection, user: &UserCtx, id: &str) -> Result<String> {
    require_owner_or_admin(conn, user, id)?;
    for _ in 0..5 {
        let code = generate_code();
        match repo::update_project_code(conn, id, &code) {
            Ok(()) => return Ok(code),
            Err(e) if repo::unique_violation(&e) => continue,
            Err(e) => bail!(e.to_string()),
        }
    }
    bail!("分享码生成冲突，请重试")
}

pub fn toggle_publish(conn: &Connection, user: &UserCtx, id: &str) -> Result<bool> {
    let project = require_owner_or_admin(conn, user, id)?;
    let published = !project.published;
    repo::toggle_project_published(conn, id, published)?;
    Ok(published)
}

pub fn delete_project(conn: &Connection, user: &UserCtx, id: &str) -> Result<()> {
    let project = require_owner_or_admin(conn, user, id)?;
    if project.protected {
        bail!("该工程处于保护状态，请先解除保护后再删除");
    }
    repo::delete_project(conn, id)?;
    Ok(())
}

pub fn set_project_protected(conn: &Connection, user: &UserCtx, id: &str, is_protected: bool) -> Result<()> {
    let Some(project) = repo::get_project_item_by_id(conn, id)? else {
        bail!("工程不存在");
    };
    if project.author_id.as_deref() != Some(user.id.as_str()) && !user.is_admin {
        bail!("仅工程作者或管理员可设置保护状态");
    }
    repo::set_project_protected(conn, id, is_protected)?;
    Ok(())
}

pub fn delete_my_content(conn: &Connection, user: &UserCtx) -> Result<i64> {
    repo::delete_user_content(conn, &user.id)
}

pub fn update_username(conn: &Connection, user: &UserCtx, raw: &str) -> Result<()> {
    let username = raw.trim().to_string();
    if !crate::service::auth::valid_username(&username) {
        bail!("用户名需为 2-20 个字符，只能包含中文、字母、数字、下划线");
    }
    if let Some(other) = repo::get_profile_by_username(conn, &username)? {
        if other.id != user.id {
            bail!("用户名已被占用");
        }
    }
    repo::update_profile_username(conn, &user.id, &username)?;
    // 同步存量工程的作者名（author_name 为冗余列）
    repo::sync_author_name(conn, &user.id, &username)?;
    Ok(())
}

pub struct MyStats {
    pub projects: i64,
    pub expired_count: i64,
    pub total_views: i64,
    pub total_clones: i64,
}

/// 我的工程列表 + 统计（对齐 /me 页面）
pub fn my_projects(conn: &Connection, user: &UserCtx) -> Result<(Vec<ProjectListItem>, MyStats)> {
    let projects = repo::list_my_projects(conn, &user.id)?;
    let now = now_ms();
    let expired_count = projects
        .iter()
        .filter(|p| {
            crate::core::expiry::is_expired_project(
                p.expires_at.as_deref(),
                Some(p.author_name.as_str()),
                now,
            ) || crate::core::expiry::is_grace_period(
                p.expires_at.as_deref(),
                Some(p.author_name.as_str()),
                now,
            )
        })
        .count() as i64;
    let total_views = projects.iter().map(|p| p.view_count).sum();
    let total_clones = projects.iter().map(|p| p.clone_count).sum();
    let count = projects.len() as i64;
    Ok((
        projects,
        MyStats {
            projects: count,
            expired_count,
            total_views,
            total_clones,
        },
    ))
}

// ── 管理员 ──────────────────────────────────────────────

pub fn admin_list_projects(
    conn: &Connection,
    q: Option<String>,
    page: i64,
    page_size: i64,
) -> Result<(Vec<ProjectListItem>, i64)> {
    let page = page.max(1);
    let page_size = page_size.clamp(5, 50);
    repo::admin_list_projects(
        conn,
        &repo::AdminProjectQuery {
            q: q.map(|s| trunc_chars(s.trim(), 60)).filter(|s| !s.is_empty()),
            page,
            page_size,
        },
    )
}

pub fn admin_update_project(conn: &Connection, id: &str, title: Option<String>, author_name: Option<String>) -> Result<()> {
    let Some(project) = repo::get_project_item_by_id(conn, id)? else {
        bail!("工程不存在");
    };
    let _ = project;
    let mut fields: Vec<(String, String)> = Vec::new();
    if let Some(t) = title {
        let title = trunc_chars(t.trim(), 60);
        if title.is_empty() {
            bail!("标题不能为空");
        }
        fields.push(("title".to_string(), title));
    }
    if let Some(n) = author_name {
        let name = trunc_chars(n.trim(), 20);
        if name.is_empty() {
            bail!("作者名不能为空");
        }
        fields.push(("author_name".to_string(), name));
    }
    if fields.is_empty() {
        bail!("没有要更新的字段");
    }
    repo::admin_update_project_fields(conn, id, &fields)?;
    Ok(())
}

pub fn admin_set_expiry(conn: &Connection, id: &str, expires_at: Option<&str>) -> Result<()> {
    if repo::get_project_item_by_id(conn, id)?.is_none() {
        bail!("工程不存在");
    }
    repo::set_project_expiry(conn, id, expires_at)?;
    Ok(())
}

pub fn admin_delete_project(conn: &Connection, id: &str) -> Result<()> {
    let Some(project) = repo::get_project_item_by_id(conn, id)? else {
        bail!("工程不存在");
    };
    if project.protected {
        bail!("该工程处于保护状态，请先解除保护后再删除");
    }
    repo::delete_project(conn, id)?;
    Ok(())
}

/// 管理员按用户名删除任意用户全部工程（对齐 adminDeleteUserContent）
pub fn admin_delete_user_content(conn: &Connection, username_raw: &str) -> Result<i64> {
    let name = trunc_chars(username_raw.trim(), 20);
    if name.is_empty() {
        bail!("用户名不能为空");
    }
    let Some(profile) = repo::get_profile_by_username(conn, &name)? else {
        bail!("用户不存在或尚未设置用户名");
    };
    repo::delete_user_content(conn, &profile.id)
}

/// 管理员查询某用户工程统计（对齐 adminUserSummary）
pub fn admin_user_summary(conn: &Connection, username_raw: &str) -> Result<(String, i64)> {
    let name = trunc_chars(username_raw.trim(), 20);
    if name.is_empty() {
        bail!("用户名不能为空");
    }
    let Some(profile) = repo::get_profile_by_username(conn, &name)? else {
        bail!("用户不存在或尚未设置用户名");
    };
    let count = repo::count_user_projects(conn, &profile.id)?;
    Ok((profile.username, count))
}

// ── 管理员权限链（对齐 admin-users.ts + 0003 RPC）────────

pub struct AdminView {
    pub id: String,
    pub username: String,
    pub granted_by: Option<String>,
    pub granted_by_me: bool,
}

pub fn admin_list_admins(conn: &Connection, me: &UserCtx) -> Result<(Vec<AdminView>, String)> {
    let admins = repo::list_admin_profiles(conn)?;
    let grants = repo::list_grants(conn)?;
    let name_by_id: std::collections::HashMap<String, String> =
        admins.iter().cloned().collect();
    let views = admins
        .iter()
        .map(|(id, username)| {
            let edge = grants.iter().find(|g| g.grantee_id == *id);
            AdminView {
                id: id.clone(),
                username: username.clone(),
                granted_by: edge
                    .and_then(|g| g.granted_by.clone())
                    .and_then(|gb| name_by_id.get(&gb).cloned()),
                granted_by_me: edge.map(|g| g.granted_by.as_deref() == Some(me.id.as_str())).unwrap_or(false),
            }
        })
        .collect();
    Ok((views, me.id.clone()))
}

/// 授权（按用户名，大小写不敏感；对齐 grant_admin RPC）
pub fn admin_grant(conn: &Connection, me: &UserCtx, username_raw: &str) -> Result<String> {
    let username = trunc_chars(username_raw.trim(), 20);
    if username.is_empty() {
        bail!("用户名不能为空");
    }
    let Some(profile) = repo::get_profile_by_username(conn, &username)? else {
        bail!("用户不存在或尚未设置用户名");
    };
    if profile.id == me.id {
        bail!("不能授予自己管理员权限");
    }
    repo::insert_grant(conn, &profile.id, Some(&me.id))?;
    repo::set_profile_admin(conn, &profile.id, true)?;
    Ok(format!("已授予 {} 管理员权限", profile.username))
}

/// 撤销（仅授出者可收回自己的授权边；被撤者无其他入边时连坐收回其授出子树；
/// 对齐 revoke_admin RPC 的不动点级联）
pub fn admin_revoke(conn: &Connection, me: &UserCtx, username_raw: &str) -> Result<String> {
    let username = trunc_chars(username_raw.trim(), 20);
    if username.is_empty() {
        bail!("用户名不能为空");
    }
    let Some(profile) = repo::get_profile_by_username(conn, &username)? else {
        bail!("用户不存在或尚未设置用户名");
    };
    if profile.id == me.id {
        bail!("不能撤销自己的管理员权限");
    }
    let grants = repo::list_grants(conn)?;
    let edge_exists = grants
        .iter()
        .any(|g| g.granted_by.as_deref() == Some(me.id.as_str()) && g.grantee_id == profile.id);
    if !edge_exists {
        bail!("无权撤销该用户：仅其授权者（您）可撤销这份授权");
    }
    repo::delete_grant_edge(conn, &me.id, &profile.id)?;

    // 被撤者仍持有其他管理员授予的权限 → 身份保留
    if repo::has_grants_to(conn, &profile.id)? {
        return Ok(format!(
            "已收回您授予 {} 的管理员权限（其仍持有其他管理员授予的权限）",
            profile.username
        ));
    }

    // 无其他入边 → 不动点级联：收回其授出的整棵子树
    let mut doomed: Vec<String> = vec![profile.id.clone()];
    loop {
        let mut changed = false;
        for g in &grants {
            if let Some(gb) = &g.granted_by {
                if doomed.contains(gb) && !doomed.contains(&g.grantee_id) {
                    let has_other_edge = grants.iter().any(|o| {
                        o.grantee_id == g.grantee_id
                            && !(o.granted_by.as_ref().map(|x| doomed.contains(x)).unwrap_or(false))
                    });
                    if !has_other_edge {
                        doomed.push(g.grantee_id.clone());
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    repo::delete_grants_of(conn, &doomed)?;
    for id in &doomed {
        repo::set_profile_admin(conn, id, false)?;
    }
    let count = doomed.len() as i64;
    if count > 1 {
        return Ok(format!(
            "已收回 {} 及其授出的 {} 位管理员的管理员权限",
            profile.username,
            count - 1
        ));
    }
    Ok(format!("已收回 {} 的管理员权限", profile.username))
}

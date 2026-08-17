// 本地账号认证（原版用 Supabase Auth + 邮箱魔法链接；本版本地用户名+密码，
// 无邮件系统。首个注册账号自动成为根管理员。）

use anyhow::{bail, Result};
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordVerifier, SaltString};
use argon2::{Argon2, PasswordHasher};
use rand::RngCore;
use rusqlite::Connection;

use crate::db::{new_uuid, now_iso};
use crate::repo;
use crate::types::UserCtx;

pub const SESSION_DAYS: i64 = 30;

pub fn hash_password(pw: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("密码哈希失败：{}", e))
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(pw.as_bytes(), &parsed)
        .is_ok()
}

/// 用户名规则（对齐原版 updateUsername 的 USERNAME_RE：中文、字母、数字、下划线）
pub fn valid_username(username: &str) -> bool {
    let len = username.chars().count();
    if !(2..=20).contains(&len) {
        return false;
    }
    username.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn issue_session(conn: &Connection, user_id: &str) -> Result<String> {
    let token = new_token();
    let expires = now_iso();
    // 30 天后
    let expires_at = chrono::DateTime::parse_from_rfc3339(&expires)
        .map(|d| (d + chrono::Duration::days(SESSION_DAYS)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|_| expires);
    repo::create_session(conn, &token, user_id, &expires_at)?;
    Ok(token)
}

/// 注册（首个账号为根管理员）
pub fn register(conn: &Connection, username_raw: &str, password: &str) -> Result<(String, UserCtx)> {
    let username = username_raw.trim().to_string();
    if !valid_username(&username) {
        bail!("用户名需为 2-20 个字符，只能包含中文、字母、数字、下划线");
    }
    if password.chars().count() < 6 {
        bail!("密码至少需要 6 个字符");
    }
    if repo::get_profile_by_username(conn, &username)?.is_some() {
        bail!("用户名已被占用");
    }

    let is_first = repo::count_profiles(conn)? == 0;
    let id = new_uuid();
    let hash = hash_password(password)?;
    repo::insert_profile(conn, &id, &username, &hash, is_first)?;
    if is_first {
        // 根管理员：授权边 granted_by = NULL
        repo::insert_grant(conn, &id, None)?;
    }
    let token = issue_session(conn, &id)?;
    Ok((token, UserCtx { id, username, is_admin: is_first }))
}

pub fn login(conn: &Connection, username: &str, password: &str) -> Result<(String, UserCtx)> {
    let Some(profile) = repo::get_profile_by_username(conn, username)? else {
        bail!("用户名或密码错误");
    };
    let Some(hash) = repo::get_password_hash(conn, &profile.id)? else {
        bail!("用户名或密码错误");
    };
    if !verify_password(password, &hash) {
        bail!("用户名或密码错误");
    }
    let token = issue_session(conn, &profile.id)?;
    Ok((
        token,
        UserCtx {
            id: profile.id.clone(),
            username: profile.username.clone(),
            is_admin: profile.is_admin,
        },
    ))
}

pub fn logout(conn: &Connection, token: &str) -> Result<()> {
    repo::delete_session(conn, token)?;
    Ok(())
}

/// 从 Bearer token 解析当前用户；失败返回错误消息
pub fn require_user(conn: &Connection, token: Option<&str>) -> Result<UserCtx> {
    let Some(token) = token else {
        bail!("请先登录");
    };
    let token = token.strip_prefix("Bearer ").unwrap_or(token);
    let Some(user) = repo::get_user_by_token(conn, token)? else {
        bail!("请先登录");
    };
    Ok(user)
}

pub fn require_admin(conn: &Connection, token: Option<&str>) -> Result<UserCtx> {
    let user = require_user(conn, token)?;
    if !user.is_admin {
        bail!("无权限：仅管理员可执行该操作");
    }
    Ok(user)
}

/// 根管理员（localhost 免登录身份）：不存在则自动创建
/// （用户名 root_admin，随机密码不可登录；is_admin + 根授权边 granted_by = NULL）
pub fn ensure_root_admin(conn: &Connection) -> Result<UserCtx> {
    const ROOT_USERNAME: &str = "root_admin";
    if let Some(p) = repo::get_profile_by_username(conn, ROOT_USERNAME)? {
        if !p.is_admin {
            repo::set_profile_admin(conn, &p.id, true)?;
        }
        return Ok(UserCtx {
            id: p.id,
            username: p.username,
            is_admin: true,
        });
    }
    let id = new_uuid();
    let hash = hash_password(&new_token())?;
    repo::insert_profile(conn, &id, ROOT_USERNAME, &hash, true)?;
    repo::insert_grant(conn, &id, None)?;
    Ok(UserCtx {
        id,
        username: ROOT_USERNAME.to_string(),
        is_admin: true,
    })
}

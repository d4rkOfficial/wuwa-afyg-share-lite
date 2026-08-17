// SQLite 数据库层：打开连接、初始化 schema、时间/ID 工具

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

/// 全量 schema（对应原版 supabase/migrations/0001_init.sql 的实体结构，
/// 去掉 auth.users 依赖：profiles 自带本地账号（username + password_hash）。
/// sessions 为本地登录会话表（原版用 Supabase Auth 会话）。
pub const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 用户资料（原版 profiles 关联 auth.users；本版本地账号，首账号为根管理员）
CREATE TABLE IF NOT EXISTS profiles (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL,
    password_hash TEXT NOT NULL DEFAULT '',
    is_admin      INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS profiles_username_lower_idx ON profiles (lower(username));

-- 本地登录会话（原版由 Supabase Auth 提供）
CREATE TABLE IF NOT EXISTS sessions (
    token      TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_user_idx ON sessions (user_id);

-- 工程表（匿名分享：author_id 可空）
CREATE TABLE IF NOT EXISTS projects (
    id           TEXT PRIMARY KEY,
    code         TEXT NOT NULL UNIQUE,
    author_id    TEXT REFERENCES profiles(id) ON DELETE CASCADE,
    author_name  TEXT NOT NULL DEFAULT '匿名',
    title        TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    tags         TEXT NOT NULL DEFAULT '[]',
    game_version TEXT,
    team_preview TEXT,
    project_blob BLOB NOT NULL,
    file_size    INTEGER NOT NULL DEFAULT 0,
    published    INTEGER NOT NULL DEFAULT 1,
    expires_at   TEXT,
    view_count   INTEGER NOT NULL DEFAULT 0,
    clone_count  INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    protected    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS projects_created_idx ON projects (created_at DESC);
CREATE INDEX IF NOT EXISTS projects_clones_idx ON projects (clone_count DESC);
CREATE INDEX IF NOT EXISTS projects_expires_idx ON projects (expires_at);
CREATE INDEX IF NOT EXISTS projects_author_idx ON projects (author_id);

-- Buff 集表（游戏内实体固定 buff 数值库）
CREATE TABLE IF NOT EXISTS buff_sets (
    entity_type TEXT NOT NULL,
    entity_name TEXT NOT NULL,
    buff_name   TEXT NOT NULL,
    buff_set    TEXT NOT NULL DEFAULT '[]',
    scope       TEXT NOT NULL DEFAULT 'team',
    exclusive   INTEGER NOT NULL DEFAULT 0,
    condition   TEXT,
    PRIMARY KEY (entity_type, entity_name, buff_name),
    CHECK (entity_type IN ('character', 'weapon', 'echo', '1set', '2set', '3set', '4set', '5set')),
    CHECK (scope IN ('self', 'self_except', 'team', 'effect_only'))
);

-- 公告表（公开读，管理员增删改）
CREATE TABLE IF NOT EXISTS announcements (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL DEFAULT '',
    content    TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Buff 集快照（根 + 版本链）
CREATE TABLE IF NOT EXISTS buff_set_snapshot (
    id         TEXT PRIMARY KEY,
    created_by TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    note       TEXT NOT NULL DEFAULT '',
    state      TEXT,
    diff       TEXT,
    is_root    INTEGER NOT NULL DEFAULT 0,
    prev_id    TEXT REFERENCES buff_set_snapshot(id) ON DELETE CASCADE,
    CHECK (
        (is_root = 1 AND state IS NOT NULL AND diff IS NULL AND prev_id IS NULL)
        OR
        (is_root = 0 AND diff IS NOT NULL AND state IS NULL AND prev_id IS NOT NULL)
    )
);
-- 根至多一个
CREATE UNIQUE INDEX IF NOT EXISTS buff_set_snapshot_root_one ON buff_set_snapshot (is_root) WHERE is_root = 1;

-- 管理员权限链（granted_by NULL = 根管理员）
CREATE TABLE IF NOT EXISTS admin_grants (
    id         TEXT PRIMARY KEY,
    grantee_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    granted_by TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    granted_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS admin_grants_edge_unique ON admin_grants (granted_by, grantee_id) WHERE granted_by IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS admin_grants_root_unique ON admin_grants (grantee_id) WHERE granted_by IS NULL;
"#;

/// 打开（或创建）数据库并初始化 schema
pub fn open_db(path: &str) -> Result<Connection> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建数据库目录 {}", parent.display()))?;
        }
    }
    let conn = Connection::open(path).with_context(|| format!("无法打开数据库 {}", path))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(SCHEMA)
        .with_context(|| format!("初始化数据库 schema 失败：{}", path))?;
    Ok(conn)
}

/// ISO-8601 毫秒 UTC（对齐 JS toISOString）
pub fn now_iso() -> String {
    chrono::Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 按字符数截断（对齐 JS String.prototype.slice）
pub fn trunc_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 标签清洗（对齐原版 sanitizeTags：单个 ≤12 字符、去重、至多 8 个）
pub fn sanitize_tags(tags: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for t in tags {
        let tag = trunc_chars(t.trim(), 12);
        if !tag.is_empty() && seen.insert(tag.clone()) {
            out.push(tag);
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

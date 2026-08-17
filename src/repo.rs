// 数据访问层：所有 SQL 查询集中于此（JSON 文本列在此解析为强类型）

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::core::buff_diff::SnapshotChainItem;
use crate::db::{new_uuid, now_iso};
use crate::types::{
    AdminGrantRow, AnnouncementRow, BuffSetRow, ProfileRow, ProjectListItem, ProjectRow,
    SnapshotDiff, SnapshotRow, UserCtx,
};

// ── JSON 列解析 ─────────────────────────────────────────

pub fn parse_tags(s: Option<String>) -> Vec<String> {
    s.and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok()).unwrap_or_default()
}

pub fn parse_team_preview(s: Option<String>) -> Option<crate::core::extract::TeamPreview> {
    s.and_then(|v| serde_json::from_str(&v).ok())
}

pub fn parse_buff_set(s: String) -> Vec<crate::types::BuffZoneValue> {
    serde_json::from_str(&s).unwrap_or_default()
}

pub fn parse_condition(s: Option<String>) -> Option<crate::types::BuffCondition> {
    s.and_then(|v| serde_json::from_str(&v).ok())
}

pub fn parse_buff_rows(s: Option<String>) -> Option<Vec<BuffSetRow>> {
    s.and_then(|v| serde_json::from_str(&v).ok())
}

pub fn parse_snapshot_diff(s: Option<String>) -> Option<SnapshotDiff> {
    s.and_then(|v| serde_json::from_str(&v).ok())
}

// ── 工程行映射 ──────────────────────────────────────────

pub const PROJECT_ITEM_COLS: &str = "id, code, author_id, author_name, title, description, tags, game_version, team_preview, published, expires_at, view_count, clone_count, created_at, updated_at, protected";

fn row_to_project_item(r: &Row) -> rusqlite::Result<ProjectListItem> {
    Ok(ProjectListItem {
        id: r.get("id")?,
        code: r.get("code")?,
        author_id: r.get("author_id")?,
        author_name: r.get("author_name")?,
        title: r.get("title")?,
        description: r.get("description")?,
        tags: parse_tags(r.get("tags")?),
        game_version: r.get("game_version")?,
        team_preview: parse_team_preview(r.get("team_preview")?),
        published: r.get("published")?,
        expires_at: r.get("expires_at")?,
        view_count: r.get("view_count")?,
        clone_count: r.get("clone_count")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        protected: r.get("protected")?,
    })
}

// ── profiles ────────────────────────────────────────────

pub fn get_profile_by_id(conn: &Connection, id: &str) -> Result<Option<ProfileRow>> {
    let row = conn
        .query_row(
            "SELECT id, username, is_admin, created_at FROM profiles WHERE id = ?1",
            params![id],
            |r| {
                Ok(ProfileRow {
                    id: r.get(0)?,
                    username: r.get(1)?,
                    is_admin: r.get(2)?,
                    created_at: r.get(3)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

pub fn get_profile_by_username(conn: &Connection, username: &str) -> Result<Option<ProfileRow>> {
    let row = conn
        .query_row(
            "SELECT id, username, is_admin, created_at FROM profiles WHERE lower(username) = lower(?1) LIMIT 1",
            params![username],
            |r| {
                Ok(ProfileRow {
                    id: r.get(0)?,
                    username: r.get(1)?,
                    is_admin: r.get(2)?,
                    created_at: r.get(3)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

pub fn get_password_hash(conn: &Connection, id: &str) -> Result<Option<String>> {
    let v = conn
        .query_row(
            "SELECT password_hash FROM profiles WHERE id = ?1",
            params![id],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    Ok(v)
}

pub fn count_profiles(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))?)
}

pub fn insert_profile(conn: &Connection, id: &str, username: &str, password_hash: &str, is_admin: bool) -> Result<()> {
    conn.execute(
        "INSERT INTO profiles (id, username, password_hash, is_admin, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, username, password_hash, is_admin as i64, now_iso()],
    )?;
    Ok(())
}

pub fn update_profile_username(conn: &Connection, id: &str, username: &str) -> Result<()> {
    conn.execute(
        "UPDATE profiles SET username = ?1, updated_at = ?2 WHERE id = ?3",
        params![username, now_iso(), id],
    )?;
    Ok(())
}

pub fn set_profile_admin(conn: &Connection, id: &str, is_admin: bool) -> Result<()> {
    conn.execute(
        "UPDATE profiles SET is_admin = ?1, updated_at = ?2 WHERE id = ?3",
        params![is_admin as i64, now_iso(), id],
    )?;
    Ok(())
}

pub fn list_admin_profiles(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT id, username FROM profiles WHERE is_admin = 1 ORDER BY created_at ASC")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

// ── sessions ────────────────────────────────────────────

pub fn create_session(conn: &Connection, token: &str, user_id: &str, expires_at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
        params![token, user_id, now_iso(), expires_at],
    )?;
    Ok(())
}

pub fn get_user_by_token(conn: &Connection, token: &str) -> Result<Option<UserCtx>> {
    let now = now_iso();
    let row = conn
        .query_row(
            "SELECT p.id, p.username, p.is_admin FROM sessions s
             JOIN profiles p ON p.id = s.user_id
             WHERE s.token = ?1 AND s.expires_at > ?2",
            params![token, now],
            |r| {
                Ok(UserCtx {
                    id: r.get(0)?,
                    username: r.get(1)?,
                    is_admin: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

pub fn delete_session(conn: &Connection, token: &str) -> Result<()> {
    conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
    Ok(())
}

pub fn purge_expired_sessions(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", params![now_iso()])?;
    Ok(())
}

// ── projects ────────────────────────────────────────────

pub fn escape_like(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    for c in q.chars() {
        if c == '%' || c == '_' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub struct PublicListFilter {
    pub q: Option<String>,
    pub sort: String,
    pub exclude_anon: bool,
}

pub fn count_public_projects(conn: &Connection, f: &PublicListFilter, now: &str) -> Result<i64> {
    let (sql, params) = public_where_sql(f);
    let sql = format!("SELECT COUNT(*) FROM projects WHERE published = 1 AND (expires_at IS NULL OR expires_at > ?1) {sql}", );
    let mut all: Vec<rusqlite::types::Value> = vec![now.to_string().into()];
    all.extend(params);
    Ok(conn.query_row(&sql, rusqlite::params_from_iter(all), |r| r.get(0))?)
}

fn public_where_sql(f: &PublicListFilter) -> (String, Vec<rusqlite::types::Value>) {
    let mut clauses = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if f.exclude_anon {
        clauses.push("author_id IS NOT NULL".to_string());
    }
    if let Some(q) = &f.q {
        let esc = escape_like(q);
        clauses.push("(title LIKE ?2 ESCAPE '\\' OR author_name LIKE ?2 ESCAPE '\\')".to_string());
        params.push(format!("%{esc}%").into());
    }
    let sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" AND {}", clauses.join(" AND "))
    };
    (sql, params)
}

pub fn list_public_projects(
    conn: &Connection,
    f: &PublicListFilter,
    page: i64,
    per_page: i64,
    now: &str,
) -> Result<Vec<ProjectListItem>> {
    let (where_sql, mut params) = public_where_sql(f);
    let order = if f.sort == "heat" {
        "clone_count DESC, created_at DESC"
    } else {
        "created_at DESC"
    };
    let sql = format!(
        "SELECT {PROJECT_ITEM_COLS} FROM projects
         WHERE published = 1 AND (expires_at IS NULL OR expires_at > ?1) {where_sql}
         ORDER BY {order} LIMIT ?{n} OFFSET ?{n1}",
        n = 2 + params.len(),
        n1 = 3 + params.len(),
    );
    let mut all: Vec<rusqlite::types::Value> = vec![now.to_string().into()];
    all.append(&mut params);
    all.push((per_page).into());
    all.push(((page - 1) * per_page).into());
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(all), row_to_project_item)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_project_item_by_code(conn: &Connection, code: &str) -> Result<Option<ProjectListItem>> {
    let row = conn
        .query_row(
            &format!("SELECT {PROJECT_ITEM_COLS} FROM projects WHERE code = ?1"),
            params![code],
            row_to_project_item,
        )
        .optional()?;
    Ok(row)
}

pub fn get_project_item_by_id(conn: &Connection, id: &str) -> Result<Option<ProjectListItem>> {
    let row = conn
        .query_row(
            &format!("SELECT {PROJECT_ITEM_COLS} FROM projects WHERE id = ?1"),
            params![id],
            row_to_project_item,
        )
        .optional()?;
    Ok(row)
}

pub struct ProjectWithBlob {
    pub item: ProjectListItem,
    pub blob: Vec<u8>,
}

pub fn get_project_blob_by_code(conn: &Connection, code: &str, now: &str) -> Result<Option<ProjectWithBlob>> {
    let row = conn
        .query_row(
            &format!(
                "SELECT {PROJECT_ITEM_COLS}, file_size, project_blob FROM projects
                 WHERE code = ?1 AND published = 1 AND (expires_at IS NULL OR expires_at > ?2)"
            ),
            params![code, now],
            |r| {
                Ok(ProjectWithBlob {
                    item: row_to_project_item(r)?,
                    blob: r.get::<_, Vec<u8>>("project_blob")?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

pub fn insert_project(
    conn: &Connection,
    p: &ProjectRow,
    blob: &[u8],
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO projects (id, code, author_id, author_name, title, description, tags, game_version, team_preview, project_blob, file_size, published, expires_at, view_count, clone_count, created_at, updated_at, protected)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
        params![
            p.id,
            p.code,
            p.author_id,
            p.author_name,
            p.title,
            p.description,
            serde_json::to_string(&p.tags).unwrap_or_else(|_| "[]".into()),
            p.game_version,
            p.team_preview.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default()),
            blob,
            p.file_size,
            p.published as i64,
            p.expires_at,
            p.view_count,
            p.clone_count,
            p.created_at,
            p.updated_at,
            p.protected as i64,
        ],
    )?;
    Ok(())
}

pub fn update_project_fields(
    conn: &Connection,
    id: &str,
    title: &str,
    description: &str,
    tags: &[String],
) -> Result<()> {
    conn.execute(
        "UPDATE projects SET title = ?1, description = ?2, tags = ?3, updated_at = ?4 WHERE id = ?5",
        params![
            title,
            description,
            serde_json::to_string(tags).unwrap_or_else(|_| "[]".into()),
            now_iso(),
            id
        ],
    )?;
    Ok(())
}

pub fn set_project_expiry(conn: &Connection, id: &str, expires_at: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE projects SET expires_at = ?1, updated_at = ?2 WHERE id = ?3",
        params![expires_at, now_iso(), id],
    )?;
    Ok(())
}

pub fn replace_project_file(conn: &Connection, id: &str, title: &str, preview: Option<&crate::core::extract::TeamPreview>, blob: &[u8], file_size: i64) -> Result<()> {
    conn.execute(
        "UPDATE projects SET title = ?1, team_preview = ?2, project_blob = ?3, file_size = ?4, updated_at = ?5 WHERE id = ?6",
        params![
            title,
            preview.map(|t| serde_json::to_string(t).unwrap_or_default()),
            blob,
            file_size,
            now_iso(),
            id
        ],
    )?;
    Ok(())
}

pub fn update_project_code(conn: &Connection, id: &str, code: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE projects SET code = ?1, updated_at = ?2 WHERE id = ?3",
        params![code, now_iso(), id],
    )?;
    Ok(())
}

/// 管理员按字段更新工程（title / author_name）
pub fn admin_update_project_fields(conn: &Connection, id: &str, fields: &[(String, String)]) -> Result<()> {
    if fields.is_empty() {
        return Ok(());
    }
    let sets = fields
        .iter()
        .map(|(k, _)| format!("{} = ?", k))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("UPDATE projects SET {sets}, updated_at = ?{} WHERE id = ?{}", fields.len() + 1, fields.len() + 2);
    let mut all: Vec<rusqlite::types::Value> = fields.iter().map(|(_, v)| v.clone().into()).collect();
    all.push(now_iso().into());
    all.push(id.to_string().into());
    conn.execute(&sql, rusqlite::params_from_iter(all))?;
    Ok(())
}

/// 用户名变更后同步存量工程的作者名（author_name 为冗余列）
pub fn sync_author_name(conn: &Connection, user_id: &str, username: &str) -> Result<()> {
    conn.execute(
        "UPDATE projects SET author_name = ?1, updated_at = ?2 WHERE author_id = ?3",
        params![username, now_iso(), user_id],
    )?;
    Ok(())
}

pub fn toggle_project_published(conn: &Connection, id: &str, published: bool) -> Result<()> {
    conn.execute(
        "UPDATE projects SET published = ?1, updated_at = ?2 WHERE id = ?3",
        params![published as i64, now_iso(), id],
    )?;
    Ok(())
}

pub fn set_project_protected(conn: &Connection, id: &str, protected: bool) -> Result<()> {
    conn.execute(
        "UPDATE projects SET protected = ?1, updated_at = ?2 WHERE id = ?3",
        params![protected as i64, now_iso(), id],
    )?;
    Ok(())
}

pub fn delete_project(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn bump_counter(conn: &Connection, id: &str, col: &str) -> Result<()> {
    match col {
        "views" => {
            conn.execute("UPDATE projects SET view_count = view_count + 1 WHERE id = ?1", params![id])?;
        }
        "clones" => {
            conn.execute("UPDATE projects SET clone_count = clone_count + 1 WHERE id = ?1", params![id])?;
        }
        _ => {}
    }
    Ok(())
}

pub fn list_my_projects(conn: &Connection, user_id: &str) -> Result<Vec<ProjectListItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PROJECT_ITEM_COLS} FROM projects WHERE author_id = ?1 ORDER BY created_at DESC"
    ))?;
    let rows = stmt
        .query_map(params![user_id], row_to_project_item)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub struct AdminProjectQuery {
    pub q: Option<String>,
    pub page: i64,
    pub page_size: i64,
}

pub fn admin_list_projects(conn: &Connection, q: &AdminProjectQuery) -> Result<(Vec<ProjectListItem>, i64)> {
    let qq = q.q.clone().unwrap_or_default();
    let esc = escape_like(&qq);
    let (where_sql, params): (String, Vec<rusqlite::types::Value>) = if qq.is_empty() {
        (String::new(), Vec::new())
    } else {
        (
            "WHERE (title LIKE ?1 ESCAPE '\\' OR code LIKE ?1 ESCAPE '\\' OR author_name LIKE ?1 ESCAPE '\\')".to_string(),
            vec![format!("%{esc}%").into()],
        )
    };
    let count_sql = format!("SELECT COUNT(*) FROM projects {where_sql}");
    let total: i64 = conn.query_row(&count_sql, rusqlite::params_from_iter(params.clone()), |r| r.get(0))?;
    let sql = format!(
        "SELECT {PROJECT_ITEM_COLS} FROM projects {where_sql} ORDER BY created_at DESC LIMIT ?{} OFFSET ?{}",
        1 + params.len(),
        2 + params.len()
    );
    let mut all = params;
    all.push(q.page_size.into());
    all.push(((q.page - 1) * q.page_size).into());
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(all), row_to_project_item)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((rows, total))
}

pub fn delete_user_content(conn: &Connection, user_id: &str) -> Result<i64> {
    let n = conn.execute(
        "DELETE FROM projects WHERE author_id = ?1 AND protected = 0",
        params![user_id],
    )?;
    Ok(n as i64)
}

pub fn count_user_projects(conn: &Connection, user_id: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE author_id = ?1",
        params![user_id],
        |r| r.get(0),
    )?)
}

pub fn delete_expired_projects(conn: &Connection, now: &str) -> Result<i64> {
    let n = conn.execute(
        "DELETE FROM projects WHERE expires_at IS NOT NULL AND expires_at < ?1 AND protected = 0",
        params![now],
    )?;
    Ok(n as i64)
}

// ── buff_sets ───────────────────────────────────────────

pub struct BuffSetFilter {
    pub entity_type: Option<String>,
    pub entity_name: Option<String>,
    pub q: Option<String>,
}

fn row_to_buff_set(r: &Row) -> rusqlite::Result<BuffSetRow> {
    Ok(BuffSetRow {
        entity_type: r.get("entity_type")?,
        entity_name: r.get("entity_name")?,
        buff_name: r.get("buff_name")?,
        scope: r.get("scope")?,
        exclusive: r.get("exclusive")?,
        condition: parse_condition(r.get("condition")?),
        buff_set: parse_buff_set(r.get("buff_set")?),
    })
}

pub fn list_buff_sets(conn: &Connection, f: &BuffSetFilter) -> Result<Vec<BuffSetRow>> {
    let mut sql = String::from(
        "SELECT entity_type, entity_name, buff_name, scope, exclusive, condition, buff_set FROM buff_sets",
    );
    let mut clauses = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(t) = &f.entity_type {
        clauses.push("entity_type = ?1".to_string());
        params.push(t.clone().into());
    }
    if let Some(n) = &f.entity_name {
        clauses.push(format!("entity_name = ?{}", 1 + params.len()));
        params.push(n.clone().into());
    }
    if let Some(q) = &f.q {
        let esc = escape_like(q);
        clauses.push(format!(
            "(entity_name LIKE ?{} ESCAPE '\\' OR buff_name LIKE ?{} ESCAPE '\\')",
            1 + params.len(),
            2 + params.len()
        ));
        let pat = format!("%{esc}%");
        params.push(pat.clone().into());
        params.push(pat.into());
    }
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY entity_type ASC, entity_name ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), row_to_buff_set)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn upsert_buff_set(conn: &Connection, r: &BuffSetRow) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO buff_sets (entity_type, entity_name, buff_name, scope, exclusive, condition, buff_set)
         VALUES (?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT (entity_type, entity_name, buff_name) DO UPDATE SET
           scope = excluded.scope, exclusive = excluded.exclusive,
           condition = excluded.condition, buff_set = excluded.buff_set",
        params![
            r.entity_type,
            r.entity_name,
            r.buff_name,
            r.scope,
            r.exclusive as i64,
            r.condition.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default()),
            serde_json::to_string(&r.buff_set).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    Ok(())
}

pub fn delete_buff_set(conn: &Connection, entity_type: &str, entity_name: &str, buff_name: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM buff_sets WHERE entity_type = ?1 AND entity_name = ?2 AND buff_name = ?3",
        params![entity_type, entity_name, buff_name],
    )?;
    Ok(())
}

pub fn delete_buff_entity(conn: &Connection, entity_type: &str, entity_name: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM buff_sets WHERE entity_type = ?1 AND entity_name = ?2",
        params![entity_type, entity_name],
    )?;
    Ok(())
}

pub fn insert_buff_sets(conn: &Connection, rows: &[BuffSetRow]) -> rusqlite::Result<()> {
    for r in rows {
        upsert_buff_set(conn, r)?;
    }
    Ok(())
}

pub fn delete_all_buff_sets(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM buff_sets WHERE 1", [])?;
    Ok(())
}

// ── buff_set_snapshot ───────────────────────────────────

fn row_to_snapshot(r: &Row) -> rusqlite::Result<SnapshotRow> {
    Ok(SnapshotRow {
        id: r.get("id")?,
        created_by: r.get("created_by")?,
        created_at: r.get("created_at")?,
        note: r.get("note")?,
        is_root: r.get("is_root")?,
        state: parse_buff_rows(r.get("state")?),
        diff: parse_snapshot_diff(r.get("diff")?),
        prev_id: r.get("prev_id")?,
    })
}

/// 读取快照链（按创建时间升序：根在前、版本依次在后）
pub fn list_snapshot_chain(conn: &Connection) -> Result<Vec<SnapshotRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_by, created_at, note, is_root, state, diff, prev_id FROM buff_set_snapshot ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([], row_to_snapshot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_snapshot(conn: &Connection, id: &str) -> Result<Option<SnapshotRow>> {
    let row = conn
        .query_row(
            "SELECT id, created_by, created_at, note, is_root, state, diff, prev_id FROM buff_set_snapshot WHERE id = ?1",
            params![id],
            row_to_snapshot,
        )
        .optional()?;
    Ok(row)
}

pub fn insert_snapshot_root(conn: &Connection, created_by: &str, note: &str, state: &Value) -> Result<()> {
    conn.execute(
        "INSERT INTO buff_set_snapshot (id, created_by, created_at, note, state, is_root) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        params![new_uuid(), created_by, now_iso(), note, serde_json::to_string(state)?],
    )?;
    Ok(())
}

pub fn insert_snapshot_version(conn: &Connection, created_by: &str, note: &str, diff: &SnapshotDiff, prev_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO buff_set_snapshot (id, created_by, created_at, note, diff, prev_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![new_uuid(), created_by, now_iso(), note, serde_json::to_string(diff)?, prev_id],
    )?;
    Ok(())
}

pub fn delete_snapshot(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM buff_set_snapshot WHERE id = ?1", params![id])?;
    Ok(())
}

/// 级联删除比目标新的版本（从链尾沿 prev_id 收集，排除目标与根；git reset 语义）
pub fn delete_snapshots_newer_than(conn: &mut Connection, target_id: &str) -> Result<Vec<String>> {
    let chain = list_snapshot_chain(conn)?;
    let by_id: std::collections::HashMap<String, &SnapshotRow> =
        chain.iter().map(|s| (s.id.clone(), s)).collect();
    // 找链尾（无后继）
    let has_successor: std::collections::HashSet<String> =
        chain.iter().filter_map(|s| s.prev_id.clone()).collect();
    let Some(tail) = chain.iter().find(|s| !has_successor.contains(&s.id)) else {
        return Ok(Vec::new());
    };
    // 从链尾沿 prev_id 回溯到目标
    let mut doomed: Vec<String> = Vec::new();
    let mut cur: Option<&SnapshotRow> = Some(tail);
    while let Some(s) = cur {
        if s.id == target_id {
            break;
        }
        if !s.is_root {
            doomed.push(s.id.clone());
        }
        cur = s.prev_id.as_deref().and_then(|pid| by_id.get(pid).copied());
    }
    let tx = conn.transaction()?;
    for id in &doomed {
        tx.execute("DELETE FROM buff_set_snapshot WHERE id = ?1", params![id])?;
    }
    tx.commit()?;
    Ok(doomed)
}

// ── announcements ───────────────────────────────────────

fn row_to_announcement(r: &Row) -> rusqlite::Result<AnnouncementRow> {
    Ok(AnnouncementRow {
        id: r.get(0)?,
        title: r.get(1)?,
        content: r.get(2)?,
        created_at: r.get(3)?,
    })
}

pub fn list_announcements(conn: &Connection) -> Result<Vec<AnnouncementRow>> {
    let mut stmt = conn.prepare("SELECT id, title, content, created_at FROM announcements ORDER BY created_at DESC")?;
    let rows = stmt
        .query_map([], row_to_announcement)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_announcement(conn: &Connection, id: &str) -> Result<Option<AnnouncementRow>> {
    let row = conn
        .query_row(
            "SELECT id, title, content, created_at FROM announcements WHERE id = ?1",
            params![id],
            row_to_announcement,
        )
        .optional()?;
    Ok(row)
}

pub fn insert_announcement(conn: &Connection, title: &str, content: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO announcements (id, title, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![new_uuid(), title, content, now_iso()],
    )?;
    Ok(())
}

pub fn update_announcement(conn: &Connection, id: &str, title: &str, content: &str) -> Result<()> {
    conn.execute(
        "UPDATE announcements SET title = ?1, content = ?2 WHERE id = ?3",
        params![title, content, id],
    )?;
    Ok(())
}

pub fn delete_announcement(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM announcements WHERE id = ?1", params![id])?;
    Ok(())
}

// ── admin_grants ────────────────────────────────────────

pub fn list_grants(conn: &Connection) -> Result<Vec<AdminGrantRow>> {
    let mut stmt = conn.prepare("SELECT id, grantee_id, granted_by, granted_at FROM admin_grants")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(AdminGrantRow {
                id: r.get(0)?,
                grantee_id: r.get(1)?,
                granted_by: r.get(2)?,
                granted_at: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn insert_grant(conn: &Connection, grantee_id: &str, granted_by: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO admin_grants (id, grantee_id, granted_by, granted_at) VALUES (?1, ?2, ?3, ?4)",
        params![new_uuid(), grantee_id, granted_by, now_iso()],
    )?;
    Ok(())
}

pub fn delete_grant_edge(conn: &Connection, granted_by: &str, grantee_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM admin_grants WHERE granted_by = ?1 AND grantee_id = ?2",
        params![granted_by, grantee_id],
    )?;
    Ok(())
}

pub fn delete_grants_of(conn: &Connection, user_ids: &[String]) -> Result<()> {
    if user_ids.is_empty() {
        return Ok(());
    }
    let placeholders = user_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("DELETE FROM admin_grants WHERE grantee_id IN ({placeholders}) OR granted_by IN ({placeholders})");
    let mut all: Vec<rusqlite::types::Value> = Vec::new();
    for id in user_ids {
        all.push(id.clone().into());
    }
    for id in user_ids {
        all.push(id.clone().into());
    }
    conn.execute(&sql, rusqlite::params_from_iter(all))?;
    Ok(())
}

pub fn has_grants_to(conn: &Connection, grantee_id: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM admin_grants WHERE grantee_id = ?1",
        params![grantee_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn snapshot_chain_items(chain: &[SnapshotRow]) -> Vec<SnapshotChainItem> {
    chain
        .iter()
        .map(|s| SnapshotChainItem {
            id: s.id.clone(),
            is_root: s.is_root,
            state: s.state.clone(),
            diff: s.diff.clone(),
        })
        .collect()
}

pub fn unique_violation(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(ie, _) if ie.code == rusqlite::ErrorCode::ConstraintViolation)
}

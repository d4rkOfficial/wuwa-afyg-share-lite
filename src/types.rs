// 共享类型（对齐原版 src/lib/types/db.ts 与 project.ts 的 API 形态）

use serde::{Deserialize, Serialize};

use crate::core::extract::TeamPreview;

// ── Buff 集 ──────────────────────────────────────────────

#[allow(dead_code)]
pub type BuffEntityType = String;
#[allow(dead_code)]
pub type BuffScope = String;

/// 生效条件（多字段可并存，全部满足才生效）
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BuffCondition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refinement: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<String>>,
    #[serde(rename = "damageTypes", skip_serializing_if = "Option::is_none")]
    pub damage_types: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BuffZoneRef {
    #[serde(rename = "targetZoneId")]
    pub target_zone_id: String,
    pub pct: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lower: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upper: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discrete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divisor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f64>,
    #[serde(rename = "refOwner", skip_serializing_if = "Option::is_none")]
    pub ref_owner: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BuffZoneValue {
    #[serde(rename = "zoneId")]
    pub zone_id: String,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<BuffZoneRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#override: Option<bool>,
}

/// Buff 集数据行（API 字段名与原版 PostgREST 输出一致：snake_case）
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BuffSetRow {
    #[serde(rename = "entity_type")]
    pub entity_type: String,
    #[serde(rename = "entity_name")]
    pub entity_name: String,
    #[serde(rename = "buff_name")]
    pub buff_name: String,
    pub scope: String,
    pub exclusive: bool,
    pub condition: Option<BuffCondition>,
    #[serde(rename = "buff_set")]
    pub buff_set: Vec<BuffZoneValue>,
}

// ── 工程 ────────────────────────────────────────────────

/// 工程行（SQLite 投影；tags/team_preview 为 JSON 文本，读取时解析）
#[derive(Clone, Debug)]
pub struct ProjectRow {
    pub id: String,
    pub code: String,
    pub author_id: Option<String>,
    pub author_name: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub game_version: Option<String>,
    pub team_preview: Option<TeamPreview>,
    pub file_size: i64,
    pub published: bool,
    pub expires_at: Option<String>,
    pub view_count: i64,
    pub clone_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub protected: bool,
}

/// 列表页用投影（不含大字段 project_blob）
#[derive(Clone, Debug)]
pub struct ProjectListItem {
    pub id: String,
    pub code: String,
    pub author_id: Option<String>,
    pub author_name: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub game_version: Option<String>,
    pub team_preview: Option<TeamPreview>,
    pub published: bool,
    pub expires_at: Option<String>,
    pub view_count: i64,
    pub clone_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub protected: bool,
}

/// GET /api/public/projects 列表项（字段名与原版一致）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicProjectItem {
    pub id: String,
    pub code: String,
    pub title: String,
    #[serde(rename = "authorName")]
    pub author_name: String,
    pub tags: Vec<String>,
    #[serde(rename = "gameVersion")]
    pub game_version: Option<String>,
    #[serde(rename = "teamPreview")]
    pub team_preview: Option<TeamPreview>,
    pub downloads: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectListResponse {
    pub projects: Vec<PublicProjectItem>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// 工程完整行 JSON（字段名与原版 PostgREST 输出一致：snake_case）
#[derive(Clone, Debug, Serialize)]
pub struct ProjectItemJson {
    pub id: String,
    pub code: String,
    #[serde(rename = "author_id")]
    pub author_id: Option<String>,
    #[serde(rename = "author_name")]
    pub author_name: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    #[serde(rename = "game_version")]
    pub game_version: Option<String>,
    #[serde(rename = "team_preview")]
    pub team_preview: Option<TeamPreview>,
    pub published: bool,
    #[serde(rename = "expires_at")]
    pub expires_at: Option<String>,
    #[serde(rename = "view_count")]
    pub view_count: i64,
    #[serde(rename = "clone_count")]
    pub clone_count: i64,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
    pub protected: bool,
}

impl From<&ProjectListItem> for ProjectItemJson {
    fn from(p: &ProjectListItem) -> Self {
        ProjectItemJson {
            id: p.id.clone(),
            code: p.code.clone(),
            author_id: p.author_id.clone(),
            author_name: p.author_name.clone(),
            title: p.title.clone(),
            description: p.description.clone(),
            tags: p.tags.clone(),
            game_version: p.game_version.clone(),
            team_preview: p.team_preview.clone(),
            published: p.published,
            expires_at: p.expires_at.clone(),
            view_count: p.view_count,
            clone_count: p.clone_count,
            created_at: p.created_at.clone(),
            updated_at: p.updated_at.clone(),
            protected: p.protected,
        }
    }
}

/// 用户 JSON（本地认证扩展接口用）
#[derive(Clone, Debug, Serialize)]
pub struct UserJson {
    pub id: String,
    pub username: String,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
}

impl From<&UserCtx> for UserJson {
    fn from(u: &UserCtx) -> Self {
        UserJson {
            id: u.id.clone(),
            username: u.username.clone(),
            is_admin: u.is_admin,
        }
    }
}

// ── 用户 / 会话 ──────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct UserCtx {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileRow {
    pub id: String,
    pub username: String,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

// ── 公告 ────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnnouncementRow {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

// ── 快照 ────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SnapshotRow {
    pub id: String,
    pub created_by: Option<String>,
    pub created_at: String,
    pub note: String,
    pub is_root: bool,
    pub state: Option<Vec<BuffSetRow>>,
    pub diff: Option<SnapshotDiff>,
    pub prev_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SnapshotDiffModified {
    pub key: String,
    pub old: BuffSetRow,
    pub new: BuffSetRow,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SnapshotDiffRemoved {
    pub key: String,
    pub old: BuffSetRow,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SnapshotDiff {
    pub added: Vec<BuffSetRow>,
    pub modified: Vec<SnapshotDiffModified>,
    pub removed: Vec<SnapshotDiffRemoved>,
}

// ── 管理员授权边 ─────────────────────────────────────────

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct AdminGrantRow {
    pub id: String,
    pub grantee_id: String,
    pub granted_by: Option<String>,
    pub granted_at: String,
}

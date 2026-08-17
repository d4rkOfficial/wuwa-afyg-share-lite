// 共享类型（对齐原版 src/lib/types/db.ts 与 project.ts 的 API 形态）

use serde::{Deserialize, Serialize};

use crate::core::extract::TeamPreview;

// ── Buff 集 ──────────────────────────────────────────────

pub type BuffEntityType = String;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectListResponse {
    pub projects: Vec<PublicProjectItem>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
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

#[derive(Clone, Debug)]
pub struct AdminGrantRow {
    pub id: String,
    pub grantee_id: String,
    pub granted_by: Option<String>,
    pub granted_at: String,
}

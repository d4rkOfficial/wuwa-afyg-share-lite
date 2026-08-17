// Buff 集快照业务（对齐原版 src/lib/actions/buff-snapshots.ts：根 + 版本链）

use anyhow::{bail, Result};
use rusqlite::Connection;

use crate::core::buff_diff::{self, serialize_snapshot_state, SnapshotChainItem};
use crate::repo;
use crate::types::{BuffSetRow, SnapshotDiff, SnapshotRow, UserCtx};

fn fetch_all_buff_sets(conn: &Connection) -> Result<Vec<BuffSetRow>> {
    repo::list_buff_sets(
        conn,
        &repo::BuffSetFilter {
            entity_type: None,
            entity_name: None,
            q: None,
        },
    )
}

fn load_chain(conn: &Connection) -> Result<Vec<SnapshotRow>> {
    repo::list_snapshot_chain(conn)
}

pub struct BuffSnapshotView {
    pub id: String,
    pub is_root: bool,
    pub note: String,
    pub created_by: Option<String>,
    pub created_at: String,
    pub is_latest: bool,
    pub can_delete: bool,
}

pub fn list_snapshots(conn: &Connection) -> Result<Vec<BuffSnapshotView>> {
    let chain = load_chain(conn)?;
    let creator_ids: std::collections::HashSet<String> = chain
        .iter()
        .filter_map(|s| s.created_by.clone())
        .collect();
    let mut name_by_id = std::collections::HashMap::new();
    for id in &creator_ids {
        if let Some(p) = repo::get_profile_by_id(conn, id)? {
            name_by_id.insert(p.id.clone(), p.username);
        }
    }
    let latest_id = chain.last().map(|s| s.id.clone());
    Ok(chain
        .iter()
        .map(|s| BuffSnapshotView {
            id: s.id.clone(),
            is_root: s.is_root,
            note: s.note.clone(),
            created_by: s
                .created_by
                .as_ref()
                .and_then(|id| name_by_id.get(id).cloned()),
            created_at: s.created_at.clone(),
            is_latest: latest_id.as_deref() == Some(s.id.as_str()),
            can_delete: !s.is_root && latest_id.as_deref() == Some(s.id.as_str()),
        })
        .collect())
}

/// 创建 / 更新快照：无根 → 创建根（全量复制）；有根 → 追加版本（diff = 相对最新状态）
pub fn save_snapshot(conn: &Connection, user: &UserCtx, note_raw: &str) -> Result<(i64, &'static str)> {
    let chain = load_chain(conn)?;
    let note = crate::db::trunc_chars(note_raw.trim(), 100);
    let root = chain.iter().find(|s| s.is_root);

    if root.is_none() {
        let rows = fetch_all_buff_sets(conn)?;
        repo::insert_snapshot_root(conn, &user.id, &note, &serialize_snapshot_state(&rows))?;
        return Ok((rows.len() as i64, "root"));
    }

    // 有根：重建最新版本状态 → 计算差异 → 追加版本
    let latest_id = chain.last().map(|s| s.id.clone()).unwrap_or_default();
    let prev_state = buff_diff::rebuild_snapshot_state(&to_chain_items(&chain), Some(&latest_id))
        .unwrap_or_default();
    let current = fetch_all_buff_sets(conn)?;
    let diff = buff_diff::diff_buff_sets(&prev_state, &current);
    let diff_count = diff.added.len() + diff.modified.len() + diff.removed.len();
    if diff_count == 0 {
        bail!("当前 Buff 集与最新快照无差异，无需创建新版本");
    }
    repo::insert_snapshot_version(conn, &user.id, &note, &diff, &latest_id)?;
    Ok((diff_count as i64, "version"))
}

fn to_chain_items(chain: &[SnapshotRow]) -> Vec<SnapshotChainItem> {
    repo::snapshot_chain_items(chain)
}

pub struct SnapshotDiffView {
    pub target: TargetView,
    pub diff: SnapshotDiff,
    pub current_count: i64,
}

pub struct TargetView {
    pub is_root: bool,
    pub note: String,
    pub created_at: String,
}

/// 对比当前 Buff 集与指定快照（根或版本）的差异（现算）
pub fn get_snapshot_diff(conn: &Connection, target_id: &str) -> Result<SnapshotDiffView> {
    let chain = load_chain(conn)?;
    let Some(target) = chain.iter().find(|s| s.id == target_id) else {
        bail!("快照不存在");
    };
    let target_state =
        buff_diff::rebuild_snapshot_state(&to_chain_items(&chain), Some(target_id))
            .ok_or_else(|| anyhow::anyhow!("暂无根快照"))?;
    let current = fetch_all_buff_sets(conn)?;
    Ok(SnapshotDiffView {
        target: TargetView {
            is_root: target.is_root,
            note: target.note.clone(),
            created_at: target.created_at.clone(),
        },
        diff: buff_diff::diff_buff_sets(&target_state, &current),
        current_count: current.len() as i64,
    })
}

/// 恢复到指定快照（目标状态由服务端重建；级联删除比目标新的版本）
pub fn restore_snapshot(conn: &mut Connection, target_id: &str) -> Result<i64> {
    let chain = load_chain(conn)?;
    let target_state =
        buff_diff::rebuild_snapshot_state(&to_chain_items(&chain), Some(target_id))
            .ok_or_else(|| anyhow::anyhow!("快照不存在或缺少根快照"))?;
    repo::delete_snapshots_newer_than(conn, target_id)?;
    repo::delete_all_buff_sets(conn)?;
    let n = target_state.len() as i64;
    repo::insert_buff_sets(conn, &target_state)?;
    Ok(n)
}

/// 删除版本快照：仅最新版本可删（根与中间版本拒绝）
pub fn delete_snapshot(conn: &Connection, id: &str) -> Result<String> {
    let Some(snap) = repo::get_snapshot(conn, id)? else {
        bail!("快照不存在");
    };
    if snap.is_root {
        bail!("根快照不可删除");
    }
    let chain = load_chain(conn)?;
    let has_successor = chain.iter().any(|s| s.prev_id.as_deref() == Some(id));
    if has_successor {
        bail!("仅可删除最新版本快照");
    }
    repo::delete_snapshot(conn, id)?;
    Ok("已删除版本快照".to_string())
}

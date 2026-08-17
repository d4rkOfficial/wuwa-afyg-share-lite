// Buff 集快照 diff 引擎（纯函数，无 IO；对齐原版 src/lib/buff-snapshots/diff.ts）
// 根快照存全量基准；版本快照存相对前一状态的差异（{added, modified, removed}）。
// 任意版本状态 = 根 state + 按链顺序依次 apply_diff；差异只在对比/创建时现算，不落库。

use std::collections::HashMap;

use serde_json::Value;

use crate::types::{BuffSetRow, SnapshotDiff, SnapshotDiffModified, SnapshotDiffRemoved};

/// 行键：entity_type 为固定枚举不含分隔符；\u0000 不可能出现在合法输入中
pub fn buff_row_key(row: &BuffSetRow) -> String {
    format!("{}\u{0}{}\u{0}{}", row.entity_type, row.entity_name, row.buff_name)
}

/// 规范化行：buff_set 按 zoneId 排序（编辑/生成顺序可能不同，比较前归一化）
pub fn canonical_row(row: &BuffSetRow) -> Value {
    let mut buff_set: Vec<&crate::types::BuffZoneValue> = row.buff_set.iter().collect();
    buff_set.sort_by(|a, b| a.zone_id.cmp(&b.zone_id));
    serde_json::json!({
        "entity_type": row.entity_type,
        "entity_name": row.entity_name,
        "buff_name": row.buff_name,
        "scope": row.scope,
        "exclusive": row.exclusive,
        "condition": row.condition,
        "buff_set": buff_set,
    })
}

pub fn buff_rows_equal(a: &BuffSetRow, b: &BuffSetRow) -> bool {
    canonical_row(a) == canonical_row(b)
}

/// base = 基准状态（快照），current = 当前实时状态
pub fn diff_buff_sets(base: &[BuffSetRow], current: &[BuffSetRow]) -> SnapshotDiff {
    let base_map: HashMap<String, &BuffSetRow> =
        base.iter().map(|r| (buff_row_key(r), r)).collect();
    let current_map: HashMap<String, &BuffSetRow> =
        current.iter().map(|r| (buff_row_key(r), r)).collect();

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();

    for (key, row) in &current_map {
        match base_map.get(key) {
            None => added.push((*row).clone()),
            Some(base_row) => {
                if !buff_rows_equal(base_row, row) {
                    modified.push(SnapshotDiffModified {
                        key: key.clone(),
                        old: (*base_row).clone(),
                        new: (*row).clone(),
                    });
                }
            }
        }
    }
    for (key, row) in &base_map {
        if !current_map.contains_key(key) {
            removed.push(SnapshotDiffRemoved {
                key: key.clone(),
                old: (*row).clone(),
            });
        }
    }

    SnapshotDiff { added, modified, removed }
}

/// 快照 state 序列化：行数组（JSON 可序列化，直接存入 SQLite）
pub fn serialize_snapshot_state(rows: &[BuffSetRow]) -> Value {
    Value::Array(
        rows.iter()
            .map(|r| {
                serde_json::json!({
                    "entity_type": r.entity_type,
                    "entity_name": r.entity_name,
                    "buff_name": r.buff_name,
                    "scope": r.scope,
                    "exclusive": r.exclusive,
                    "condition": r.condition,
                    "buff_set": r.buff_set,
                })
            })
            .collect(),
    )
}

/// 应用 diff 到基准状态（链式重建用）：added 加入 / modified 替换 / removed 删除
pub fn apply_diff(base: &[BuffSetRow], diff: &SnapshotDiff) -> Vec<BuffSetRow> {
    let mut map: HashMap<String, BuffSetRow> =
        base.iter().map(|r| (buff_row_key(r), r.clone())).collect();
    for row in &diff.added {
        map.insert(buff_row_key(row), row.clone());
    }
    for m in &diff.modified {
        map.insert(buff_row_key(&m.new), m.new.clone());
    }
    for r in &diff.removed {
        map.remove(&buff_row_key(&r.old));
    }
    map.into_values().collect()
}

pub struct SnapshotChainItem {
    pub id: String,
    pub is_root: bool,
    pub state: Option<Vec<BuffSetRow>>,
    pub diff: Option<SnapshotDiff>,
}

/// 由根 state 与版本链重建任意快照的完整状态
/// chain：按创建时间升序（根在前、版本依次在后）；target_id 为根或 None 时返回根状态
pub fn rebuild_snapshot_state(chain: &[SnapshotChainItem], target_id: Option<&str>) -> Option<Vec<BuffSetRow>> {
    let root = chain.iter().find(|s| s.is_root)?;
    let mut state = root.state.clone()?;
    if target_id.is_none() || target_id == Some(root.id.as_str()) {
        return Some(state);
    }
    for snap in chain {
        if snap.is_root {
            continue;
        }
        if let Some(diff) = &snap.diff {
            state = apply_diff(&state, diff);
        }
        if snap.id.as_str() == target_id.unwrap_or("") {
            break;
        }
    }
    Some(state)
}

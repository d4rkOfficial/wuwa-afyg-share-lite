// 团队预览提取（对齐原版 src/lib/project/extract.ts）

use serde::{Deserialize, Serialize};

use crate::core::parse::{CharSlot, LockedMap, ProjectData};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeamPreview {
    pub slots: Vec<CharSlot>,
    pub names: Vec<String>,
    pub locked: LockedMap,
    pub version: Option<serde_json::Value>,
}

pub fn extract_team_preview(project: &ProjectData) -> TeamPreview {
    let names = project
        .locked_team_names
        .clone()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            project
                .team
                .iter()
                .filter_map(|s| s.character.clone())
                .collect()
        });

    TeamPreview {
        slots: project.team.clone(),
        names,
        locked: crate::core::parse::phases_locked(project),
        version: None,
    }
}

/// 列表/详情页展示用的角色名（锁定名优先，缺失则用 team 推导）
pub fn team_display_names(preview: Option<&TeamPreview>) -> Vec<String> {
    let Some(p) = preview else { return Vec::new() };
    if !p.names.is_empty() {
        return p.names.clone();
    }
    p.slots
        .iter()
        .filter_map(|s| s.character.clone())
        .collect()
}

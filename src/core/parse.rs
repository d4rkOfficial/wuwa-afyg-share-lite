// 工程文件解析（对齐原版 src/lib/project/parse.ts）
// 兼容三种形态：{version, exportedAt, project} / [project, ...] / 裸工程对象

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::compress::MAX_RAW_BYTES;

pub const EXPORT_VERSION: i64 = 1;

pub const PHASE_KEYS: [&str; 4] = ["team", "timeline", "calculation", "config"];

pub const PHASE_LABELS: [(&str, &str); 4] = [
    ("team", "队伍配置"),
    ("timeline", "排轴"),
    ("calculation", "拉表"),
    ("config", "词条/环境配置"),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectedSet {
    pub name: String,
    pub pieces: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EchoSlot {
    pub name: Option<String>,
    pub cost: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharSlot {
    pub character: Option<String>,
    pub weapon: Option<String>,
    #[serde(rename = "triggerSets")]
    pub trigger_sets: Vec<SelectedSet>,
    pub echoes: Vec<EchoSlot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhaseState {
    pub locked: bool,
    pub data: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectPhases {
    pub team: PhaseState,
    pub timeline: PhaseState,
    pub calculation: PhaseState,
    pub config: PhaseState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectData {
    pub id: String,
    pub name: String,
    #[serde(rename = "createdAt")]
    pub created_at: f64,
    pub team: Vec<CharSlot>,
    #[serde(rename = "customSkillHits")]
    pub custom_skill_hits: Value,
    #[serde(rename = "resultAnalysis", skip_serializing_if = "Option::is_none")]
    pub result_analysis: Option<Value>,
    #[serde(rename = "lockedTeamKey", skip_serializing_if = "Option::is_none")]
    pub locked_team_key: Option<String>,
    #[serde(rename = "lockedTeamNames", skip_serializing_if = "Option::is_none")]
    pub locked_team_names: Option<Vec<String>>,
    pub phases: ProjectPhases,
}

/// 主工具导出的文件：{ version, exportedAt, project }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: i64,
    #[serde(rename = "exportedAt")]
    pub exported_at: f64,
    pub project: ProjectData,
}

fn is_record(v: &Value) -> bool {
    v.is_object()
}

fn empty_char_slot() -> CharSlot {
    CharSlot {
        character: None,
        weapon: None,
        trigger_sets: Vec::new(),
        echoes: vec![
            EchoSlot { name: None, cost: 0.0 },
            EchoSlot { name: None, cost: 0.0 },
            EchoSlot { name: None, cost: 0.0 },
            EchoSlot { name: None, cost: 0.0 },
            EchoSlot { name: None, cost: 0.0 },
        ],
    }
}

fn sanitize_team(raw: &Value) -> Vec<CharSlot> {
    let slots: Vec<&Value> = if raw.is_array() {
        raw.as_array().unwrap().iter().collect()
    } else {
        Vec::new()
    };
    let mut team = vec![empty_char_slot(), empty_char_slot(), empty_char_slot()];
    for i in 0..3 {
        let Some(s) = slots.get(i) else { continue };
        if !is_record(s) {
            continue;
        }
        let obj = s.as_object().unwrap();
        let character = obj
            .get("character")
            .and_then(|v| v.as_str())
            .map(|x| x.to_string());
        let weapon = obj
            .get("weapon")
            .and_then(|v| v.as_str())
            .map(|x| x.to_string());
        let trigger_sets = obj
            .get("triggerSets")
            .filter(|v| v.is_array())
            .map(|v| v.as_array().unwrap())
            .unwrap_or(&Vec::new())
            .iter()
            .filter(|t| is_record(t))
            .map(|t| {
                let o = t.as_object().unwrap();
                SelectedSet {
                    name: o
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    pieces: o.get("pieces").and_then(|v| v.as_f64()).unwrap_or(0.0),
                }
            })
            .filter(|t| !t.name.is_empty())
            .collect::<Vec<_>>();
        let echoes_raw = obj.get("echoes").filter(|v| v.is_array());
        let to_echo = |raw: Option<&Value>| -> EchoSlot {
            match raw {
                Some(v) if is_record(v) => {
                    let o = v.as_object().unwrap();
                    EchoSlot {
                        name: o.get("name").and_then(|x| x.as_str()).map(|x| x.to_string()),
                        cost: o.get("cost").and_then(|x| x.as_f64()).unwrap_or(0.0),
                    }
                }
                _ => EchoSlot { name: None, cost: 0.0 },
            }
        };
        let echoes_arr = echoes_raw.map(|v| v.as_array().unwrap()).unwrap_or(&Vec::new());
        let echoes = vec![
            to_echo(echoes_arr.get(0).copied()),
            to_echo(echoes_arr.get(1).copied()),
            to_echo(echoes_arr.get(2).copied()),
            to_echo(echoes_arr.get(3).copied()),
            to_echo(echoes_arr.get(4).copied()),
        ];
        team[i] = CharSlot {
            character,
            weapon,
            trigger_sets,
            echoes,
        };
    }
    team
}

fn sanitize_phase_state(raw: &Value) -> PhaseState {
    if !is_record(raw) {
        return PhaseState { locked: false, data: None };
    }
    let obj = raw.as_object().unwrap();
    PhaseState {
        locked: obj.get("locked").and_then(|v| v.as_bool()).unwrap_or(false),
        data: if obj.contains_key("data") {
            obj.get("data").cloned()
        } else {
            None
        },
    }
}

/// 解析主工具导出的工程 JSON（对齐原版 parseProjectFile）
pub fn parse_project_file(raw: &Value) -> Result<ProjectData> {
    let project: Option<&Value> = if is_record(raw) {
        let obj = raw.as_object().unwrap();
        if let Some(inner) = obj.get("project") {
            if is_record(inner) {
                Some(inner)
            } else {
                None
            }
        } else if obj.contains_key("team") || obj.contains_key("name") {
            Some(raw)
        } else {
            None
        }
    } else if raw.is_array() {
        let arr = raw.as_array().unwrap();
        if !arr.is_empty() && is_record(&arr[0]) {
            Some(&arr[0])
        } else {
            None
        }
    } else {
        None
    };

    let Some(project) = project else {
        bail!("无法识别的工程文件结构");
    };
    let obj = project.as_object().unwrap();

    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "未命名项目".to_string());
    let created_at = obj
        .get("createdAt")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(now_ms_f64);

    let mut phases = ProjectPhases {
        team: PhaseState { locked: false, data: None },
        timeline: PhaseState { locked: false, data: None },
        calculation: PhaseState { locked: false, data: None },
        config: PhaseState { locked: false, data: None },
    };
    if let Some(p) = obj.get("phases") {
        if is_record(p) {
            let po = p.as_object().unwrap();
            phases.team = sanitize_phase_state(po.get("team").unwrap_or(&Value::Null));
            phases.timeline = sanitize_phase_state(po.get("timeline").unwrap_or(&Value::Null));
            phases.calculation = sanitize_phase_state(po.get("calculation").unwrap_or(&Value::Null));
            phases.config = sanitize_phase_state(po.get("config").unwrap_or(&Value::Null));
        }
    }

    let locked_team_names = obj
        .get("lockedTeamNames")
        .filter(|v| v.is_array())
        .map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        });

    Ok(ProjectData {
        id: obj
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        name,
        created_at,
        team: sanitize_team(obj.get("team").unwrap_or(&Value::Null)),
        custom_skill_hits: obj
            .get("customSkillHits")
            .filter(|v| is_record(v))
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default())),
        result_analysis: obj
            .get("resultAnalysis")
            .filter(|v| !v.is_null())
            .cloned(),
        locked_team_key: obj
            .get("lockedTeamKey")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        locked_team_names,
        phases,
    })
}

/// 解析前先校验为 JSON 且不超过尺寸上限（对齐 safeJsonParse）
pub fn safe_json_parse(text: &str, max_bytes: usize) -> Result<Value> {
    if text.len() > max_bytes {
        bail!("文件超过 {}MB 限制", max_bytes / 1024 / 1024);
    }
    serde_json::from_str(text).map_err(|_| anyhow::anyhow!("不是合法的 JSON 文件"))
}

pub fn phases_locked(project: &ProjectData) -> LockedMap {
    LockedMap {
        team: project.phases.team.locked,
        timeline: project.phases.timeline.locked,
        calculation: project.phases.calculation.locked,
        config: project.phases.config.locked,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockedMap {
    pub team: bool,
    pub timeline: bool,
    pub calculation: bool,
    pub config: bool,
}

pub fn now_ms_f64() -> f64 {
    chrono::Utc::now().timestamp_millis() as f64
}

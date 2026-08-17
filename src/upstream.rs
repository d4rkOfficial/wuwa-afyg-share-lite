// 上游数据适配器（对齐 wuwa-afyg-tool 的 nanoka 适配器：src/lib/api/provider/nanoka/）
// 从 nanoka.cc 直接获取《鸣潮》角色/武器/声骸/套装名录：
//   - manifest.json            → ww.latest 版本号（默认兜底 3.5）
//   - ww/{version}/character.json / weapon.json / echo.json / sonata.json
// 内存缓存 30 分钟；同时持久化到 SQLite meta 表，上游不可达时回退为缓存数据（stale）。

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use crate::repo;

pub const NANOKA_BASE: &str = "https://static.nanoka.cc";
pub const WW_BASE: &str = "https://static.nanoka.cc/ww";
pub const MANIFEST_URL: &str = "https://static.nanoka.cc/manifest.json";
pub const FALLBACK_VERSION: &str = "3.5";
const CACHE_TTL: Duration = Duration::from_secs(30 * 60);

// meta 持久化键
pub const META_VERSION: &str = "upstream.version";
pub const META_CHARACTER: &str = "upstream.character";
pub const META_WEAPON: &str = "upstream.weapon";
pub const META_ECHO: &str = "upstream.echo";
pub const META_SONATA: &str = "upstream.sonata";

// ── 游戏术语映射（与 wuwa-afyg-tool src/lib/consts/game-terms.ts 一致）──

const ELEMENT_MAP: [(i64, &str); 6] = [
    (1, "冷凝"),
    (2, "热熔"),
    (3, "导电"),
    (4, "气动"),
    (5, "衍射"),
    (6, "湮灭"),
];

const WEAPON_TYPE_MAP: [(i64, &str); 5] = [
    (1, "长刃"),
    (2, "迅刀"),
    (3, "佩枪"),
    (4, "臂铠"),
    (5, "音感仪"),
];

const COST_MAP: [(i64, i64); 4] = [(0, 1), (1, 3), (2, 4), (3, 4)];

fn element_name(id: &Value) -> String {
    let n = id.as_i64().unwrap_or(-1);
    ELEMENT_MAP
        .iter()
        .find(|(k, _)| *k == n)
        .map(|(_, v)| v.to_string())
        .unwrap_or_default()
}

fn weapon_type_name(id: &Value) -> String {
    let n = id.as_i64().unwrap_or(-1);
    WEAPON_TYPE_MAP
        .iter()
        .find(|(k, _)| *k == n)
        .map(|(_, v)| v.to_string())
        .unwrap_or_default()
}

fn echo_cost(intensity: &Value) -> i64 {
    let n = intensity.as_i64().unwrap_or(-1);
    COST_MAP
        .iter()
        .find(|(k, _)| *k == n)
        .map(|(_, v)| *v)
        .unwrap_or(1)
}

// ── 名录条目（API 输出结构）──

#[derive(Clone, Debug, Serialize)]
pub struct CharacterItem {
    pub name: String,
    pub star: i64,
    pub element: String,
    #[serde(rename = "weaponType")]
    pub weapon_type: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct WeaponItem {
    pub name: String,
    pub star: i64,
    #[serde(rename = "weaponType")]
    pub weapon_type: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EchoItem {
    pub name: String,
    pub sets: Vec<String>,
    pub cost: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SetItem {
    pub name: String,
    pub pieces: Vec<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CatalogData {
    pub version: String,
    pub source: String,
    /// 是否来自本地缓存（上游不可达时回退）
    pub stale: bool,
    pub characters: Vec<CharacterItem>,
    pub weapons: Vec<WeaponItem>,
    pub echoes: Vec<EchoItem>,
    pub sets: Vec<SetItem>,
}

// ── 转换（对齐工具 transformCharacterList 等）──

fn transform_characters(data: &Value) -> Vec<CharacterItem> {
    let mut seen = HashSet::new();
    let mut out: Vec<CharacterItem> = data
        .as_object()
        .map(|o| o.values().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|c| {
            let zh = c.get("zh").and_then(|v| v.as_str()).unwrap_or("");
            !zh.is_empty() && seen.insert(zh.to_string())
        })
        .map(|c| CharacterItem {
            name: c.get("zh").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            star: c.get("rank").and_then(|v| v.as_i64()).unwrap_or(0),
            element: element_name(c.get("element").unwrap_or(&Value::Null)),
            weapon_type: weapon_type_name(c.get("weapon").unwrap_or(&Value::Null)),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn transform_weapons(data: &Value) -> Vec<WeaponItem> {
    let mut out: Vec<WeaponItem> = data
        .as_object()
        .map(|o| o.values().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|w| {
            w.get("zh")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        })
        .map(|w| WeaponItem {
            name: w.get("zh").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            star: w.get("rank").and_then(|v| v.as_i64()).unwrap_or(0),
            weapon_type: weapon_type_name(w.get("type").unwrap_or(&Value::Null)),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn transform_echoes(data: &Value, sonata: &Value) -> Vec<EchoItem> {
    let sonata_obj = sonata.as_object().cloned().unwrap_or_default();
    let set_name = |gid: &Value| -> String {
        sonata_obj
            .get(&gid.to_string())
            .and_then(|s| s.get("name"))
            .and_then(|n| n.get("zh"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let mut seen = HashSet::new();
    let mut out: Vec<EchoItem> = data
        .as_object()
        .map(|o| o.values().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| {
            let zh = e.get("zh").and_then(|v| v.as_str()).unwrap_or("");
            !zh.is_empty() && seen.insert(zh.to_string())
        })
        .map(|e| EchoItem {
            name: e.get("zh").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            sets: e
                .get("group")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(set_name)
                .filter(|s| !s.is_empty())
                .collect(),
            cost: echo_cost(e.get("intensity").unwrap_or(&Value::Null)),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn transform_sets(data: &Value) -> Vec<SetItem> {
    let mut out: Vec<SetItem> = data
        .as_object()
        .map(|o| o.values().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|s| {
            s.get("name")
                .and_then(|n| n.get("zh"))
                .and_then(|v| v.as_str())
                .map(|x| !x.is_empty())
                .unwrap_or(false)
        })
        .map(|s| {
            let mut pieces: Vec<i64> = s
                .get("set")
                .and_then(|v| v.as_object())
                .map(|m| {
                    m.keys()
                        .filter_map(|k| k.parse::<i64>().ok())
                        .collect::<Vec<i64>>()
                })
                .unwrap_or_default();
            pieces.sort_unstable();
            SetItem {
                name: s
                    .get("name")
                    .and_then(|n| n.get("zh"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                pieces,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ── 上游客户端 ──────────────────────────────────────────

struct UpstreamState {
    version: Option<String>,
    version_fetched_at: Option<Instant>,
    lists: std::collections::HashMap<&'static str, (Instant, Value)>,
}

pub struct Upstream {
    agent: ureq::Agent,
    /// 版本号强制覆盖（环境变量 WUWA_AFYG_SHARE_WW_VERSION，调试用）
    version_override: Option<String>,
    state: Mutex<UpstreamState>,
}

impl Upstream {
    pub fn new(version_override: Option<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(20))
            .build();
        Upstream {
            agent,
            version_override,
            state: Mutex::new(UpstreamState {
                version: None,
                version_fetched_at: None,
                lists: std::collections::HashMap::new(),
            }),
        }
    }

    fn fetch_json(&self, url: &str) -> Result<Value> {
        let resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| anyhow::anyhow!("HTTP 请求失败 {}：{}", url, e))?;
        let text = resp
            .into_string()
            .map_err(|e| anyhow::anyhow!("读取响应失败：{}", e))?;
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("响应不是合法 JSON：{}", e))
    }

    /// 解析最新版本号（manifest；失败依次回退：override → 内存缓存 → SQLite → 默认 3.5）
    fn version(&self, conn: Option<&Connection>) -> String {
        if let Some(v) = &self.version_override {
            return v.clone();
        }
        let mut st = self.state.lock().unwrap();
        if let (Some(v), Some(t)) = (&st.version, st.version_fetched_at) {
            if t.elapsed() < CACHE_TTL {
                return v.clone();
            }
        }
        // 尝试拉取 manifest
        match self.fetch_json(MANIFEST_URL) {
            Ok(manifest) => {
                let v = manifest
                    .get("ww")
                    .and_then(|w| w.get("latest"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| FALLBACK_VERSION.to_string());
                st.version = Some(v.clone());
                st.version_fetched_at = Some(Instant::now());
                if let Some(c) = conn {
                    let _ = repo::set_meta(c, META_VERSION, &v);
                }
                v
            }
            Err(_) => {
                // 回退：SQLite → 内存 → 默认
                if let Some(c) = conn {
                    if let Ok(Some(v)) = repo::get_meta(c, META_VERSION) {
                        st.version = Some(v.clone());
                        return v;
                    }
                }
                st.version
                    .clone()
                    .unwrap_or_else(|| FALLBACK_VERSION.to_string())
            }
        }
    }

    /// 获取版本化列表文件（内存缓存 30 分钟）；
    /// 返回 (数据, 是否来自回退缓存)：上游可达且非内存缓存时为 (v, false)
    fn raw_list(&self, name: &'static str, conn: Option<&Connection>) -> Result<(Value, bool)> {
        {
            let st = self.state.lock().unwrap();
            if let Some((t, v)) = st.lists.get(name) {
                if t.elapsed() < CACHE_TTL {
                    return Ok((v.clone(), false));
                }
            }
        }
        let version = self.version(conn);
        let url = format!("{}/{}/{}.json", WW_BASE, version, name);
        match self.fetch_json(&url) {
            Ok(v) => {
                let mut st = self.state.lock().unwrap();
                st.lists.insert(name, (Instant::now(), v.clone()));
                if let Some(c) = conn {
                    let _ = repo::set_meta(
                        c,
                        match name {
                            "character" => META_CHARACTER,
                            "weapon" => META_WEAPON,
                            "echo" => META_ECHO,
                            _ => META_SONATA,
                        },
                        &serde_json::to_string(&v).unwrap_or_default(),
                    );
                }
                Ok((v, false))
            }
            Err(e) => {
                // 回退：SQLite 缓存的原始列表
                if let Some(c) = conn {
                    let key = match name {
                        "character" => META_CHARACTER,
                        "weapon" => META_WEAPON,
                        "echo" => META_ECHO,
                        _ => META_SONATA,
                    };
                    if let Ok(Some(raw)) = repo::get_meta(c, key) {
                        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                            return Ok((v, true));
                        }
                    }
                }
                bail!("{}（{}）", e, name)
            }
        }
    }

    /// 获取完整名录（角色/武器/声骸/套装）；上游不可达时用 SQLite 缓存（stale=true）
    pub fn catalog(&self, conn: Option<&Connection>) -> Result<CatalogData> {
        let version = self.version(conn);
        let c = self.raw_list("character", conn);
        let w = self.raw_list("weapon", conn);
        let e = self.raw_list("echo", conn);
        let s = self.raw_list("sonata", conn);

        match (c, w, e, s) {
            (Ok((character, cs)), Ok((weapon, ws)), Ok((echo, es)), Ok((sonata, ss))) => {
                let stale = cs || ws || es || ss;
                Ok(CatalogData {
                    version: version.clone(),
                    source: if stale {
                        format!("本地缓存（ww {}）", version)
                    } else {
                        format!("{}（ww {}）", NANOKA_BASE, version)
                    },
                    stale,
                    characters: transform_characters(&character),
                    weapons: transform_weapons(&weapon),
                    echoes: transform_echoes(&echo, &sonata),
                    sets: transform_sets(&sonata),
                })
            }
            _ => bail!("上游数据获取失败：{} 不可达", NANOKA_BASE),
        }
    }
}

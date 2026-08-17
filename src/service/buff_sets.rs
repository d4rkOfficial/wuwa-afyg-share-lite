// Buff 集业务逻辑（对齐原版 src/lib/actions/buff-sets.ts：白名单清洗 + 仅管理员写）

use anyhow::{bail, Result};
use rusqlite::Connection;
use serde_json::Value;

use crate::core::buff_zones::{
    is_buff_ref_zone, is_buff_scope, is_buff_zone, sanitize_condition, BUFF_ENTITY_TYPES,
    BUFF_SCOPES,
};
use crate::db::trunc_chars;
use crate::repo;
use crate::types::{BuffCondition, BuffSetRow, BuffZoneRef, BuffZoneValue};

pub fn list_buff_sets(
    conn: &Connection,
    entity_type: Option<String>,
    entity_name: Option<String>,
    q: Option<String>,
) -> Result<Vec<BuffSetRow>> {
    let rows = repo::list_buff_sets(
        conn,
        &repo::BuffSetFilter {
            entity_type: entity_type
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            entity_name: entity_name
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            q: q.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        },
    )?;
    // 与原版一致：仅返回白名单实体类型
    Ok(rows
        .into_iter()
        .filter(|r| BUFF_ENTITY_TYPES.contains(&r.entity_type.as_str()))
        .collect())
}

// ── 输入清洗（对齐原版 sanitizeZones / sanitizeRef / normalizeScope）──

fn sanitize_ref(raw: &Value) -> Option<BuffZoneRef> {
    if !raw.is_object() {
        return None;
    }
    let r = raw.as_object().unwrap();
    let target_zone_id = r
        .get("targetZoneId")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if !is_buff_ref_zone(&target_zone_id) {
        return None;
    }
    let pct = r.get("pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if !pct.is_finite() {
        return None;
    }
    let mut out = BuffZoneRef {
        target_zone_id,
        pct,
        threshold: None,
        lower: None,
        upper: None,
        discrete: None,
        divisor: None,
        multiplier: None,
        ref_owner: None,
    };
    let num = |key: &str, slot: &mut Option<f64>| {
        if let Some(v) = r.get(key).and_then(|v| v.as_f64()) {
            if v.is_finite() {
                *slot = Some(v);
            }
        }
    };
    num("threshold", &mut out.threshold);
    num("lower", &mut out.lower);
    num("upper", &mut out.upper);
    num("divisor", &mut out.divisor);
    num("multiplier", &mut out.multiplier);
    if r.get("discrete").and_then(|v| v.as_bool()).unwrap_or(false) {
        out.discrete = Some(true);
    }
    if let Some(ro) = r.get("refOwner").and_then(|v| v.as_str()) {
        if ro == "self" || ro == "owner" {
            out.ref_owner = Some(ro.to_string());
        }
    }
    Some(out)
}

pub fn sanitize_zones(zones: &Value) -> Vec<BuffZoneValue> {
    if !zones.is_array() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for z in zones.as_array().unwrap() {
        if !z.is_object() {
            continue;
        }
        let zo = z.as_object().unwrap();
        let zone_id = zo
            .get("zoneId")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if !is_buff_zone(&zone_id) || seen.contains(&zone_id) {
            continue;
        }
        seen.insert(zone_id.clone());
        let value = zo.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let value = if value.is_finite() { value } else { 0.0 };
        let r#ref = zo.get("ref").and_then(sanitize_ref);
        out.push(BuffZoneValue {
            zone_id,
            value,
            r#ref,
            r#override: zo
                .get("override")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                .then_some(true),
        });
    }
    out
}

fn normalize_scope(scope: &Value) -> String {
    let s = scope.as_str().unwrap_or("");
    if is_buff_scope(s) {
        s.to_string()
    } else {
        "team".to_string()
    }
}

// ── 写操作（仅管理员）───────────────────────────────────

pub struct BuffSetInput {
    pub entity_type: String,
    pub entity_name: String,
    pub buff_name: String,
    pub scope: String,
    pub exclusive: bool,
    pub condition: Option<BuffCondition>,
    pub zones: Vec<BuffZoneValue>,
}

/// 校验并构造 BuffSetRow；返回错误消息时直接 bail
pub fn validate_buff_set_input(input: &BuffSetInput) -> Result<BuffSetRow> {
    if !BUFF_ENTITY_TYPES.contains(&input.entity_type.as_str()) {
        bail!("无效的实体类型");
    }
    let entity_name = trunc_chars(input.entity_name.trim(), 60);
    let buff_name = trunc_chars(input.buff_name.trim(), 80);
    if entity_name.is_empty() || buff_name.is_empty() {
        bail!("实体名与增益名不能为空");
    }
    Ok(BuffSetRow {
        entity_type: input.entity_type.clone(),
        entity_name,
        buff_name,
        scope: input.scope.clone(),
        exclusive: input.exclusive,
        condition: input.condition.clone(),
        buff_set: input.zones.clone(),
    })
}

pub fn upsert_buff_set(conn: &Connection, input: &BuffSetInput) -> Result<()> {
    let row = validate_buff_set_input(input)?;
    repo::upsert_buff_set(conn, &row)?;
    Ok(())
}

pub fn delete_buff_preset(conn: &Connection, entity_type: &str, entity_name: &str, buff_name: &str) -> Result<()> {
    repo::delete_buff_set(conn, entity_type, entity_name, buff_name)?;
    Ok(())
}

pub struct BuffInput {
    pub buff_name: String,
    pub scope: String,
    pub exclusive: bool,
    pub condition: Option<BuffCondition>,
    pub zones: Vec<BuffZoneValue>,
}

pub struct BuffEntityInput {
    pub entity_type: String,
    pub entity_name: String,
    pub buffs: Vec<BuffInput>,
}

/// 整体替换实体：先删除该实体全部行，再写回（对齐 upsertBuffEntity）
pub fn upsert_buff_entity(conn: &Connection, input: &BuffEntityInput) -> Result<i64> {
    if !BUFF_ENTITY_TYPES.contains(&input.entity_type.as_str()) {
        bail!("无效的实体类型");
    }
    let entity_name = trunc_chars(input.entity_name.trim(), 60);
    if entity_name.is_empty() {
        bail!("实体名不能为空");
    }
    repo::delete_buff_entity(conn, &input.entity_type, &entity_name)?;

    let buffs: Vec<BuffSetRow> = input
        .buffs
        .iter()
        .map(|b| BuffSetRow {
            entity_type: input.entity_type.clone(),
            entity_name: entity_name.clone(),
            buff_name: trunc_chars(b.buff_name.trim(), 80),
            scope: b.scope.clone(),
            exclusive: b.exclusive,
            condition: b.condition.clone(),
            buff_set: b.zones.clone(),
        })
        .filter(|r| !r.buff_name.is_empty() && !r.buff_set.is_empty())
        .collect();

    if !buffs.is_empty() {
        repo::insert_buff_sets(conn, &buffs)?;
    }
    Ok(buffs.len() as i64)
}

pub fn delete_buff_entity(conn: &Connection, entity_type: &str, entity_name: &str) -> Result<()> {
    repo::delete_buff_entity(conn, entity_type, entity_name)?;
    Ok(())
}

/// 从 JSON 清洗构造 BuffSetInput（HTTP 入口用）
pub fn buff_set_input_from_json(v: &Value) -> Result<BuffSetInput> {
    let obj = v.as_object().ok_or_else(|| anyhow::anyhow!("请求格式错误"))?;
    let entity_type = obj
        .get("entityType")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let entity_name = obj
        .get("entityName")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let buff_name = obj
        .get("buffName")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let scope = normalize_scope(obj.get("scope").unwrap_or(&Value::Null));
    let exclusive = obj.get("exclusive").and_then(|x| x.as_bool()).unwrap_or(false);
    let condition = obj
        .get("condition")
        .map(sanitize_condition)
        .unwrap_or(None);
    let zones = sanitize_zones(obj.get("zones").unwrap_or(&Value::Null));
    Ok(BuffSetInput {
        entity_type,
        entity_name,
        buff_name,
        scope,
        exclusive,
        condition,
        zones,
    })
}

pub fn buff_entity_input_from_json(v: &Value) -> Result<BuffEntityInput> {
    let obj = v.as_object().ok_or_else(|| anyhow::anyhow!("请求格式错误"))?;
    let entity_type = obj
        .get("entityType")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let entity_name = obj
        .get("entityName")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let mut buffs = Vec::new();
    if let Some(arr) = obj.get("buffs").and_then(|x| x.as_array()) {
        for b in arr {
            let Some(bo) = b.as_object() else { continue };
            buffs.push(BuffInput {
                buff_name: bo
                    .get("buffName")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                scope: normalize_scope(bo.get("scope").unwrap_or(&Value::Null)),
                exclusive: bo.get("exclusive").and_then(|x| x.as_bool()).unwrap_or(false),
                condition: bo
                    .get("condition")
                    .map(sanitize_condition)
                    .unwrap_or(None),
                zones: sanitize_zones(bo.get("zones").unwrap_or(&Value::Null)),
            });
        }
    }
    Ok(BuffEntityInput {
        entity_type,
        entity_name,
        buffs,
    })
}

/// 兼容旧版 scope 常量（保留引用，避免未使用警告）
#[allow(dead_code)]
pub const SCOPES: &[&str] = BUFF_SCOPES;

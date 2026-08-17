// Buff 集常量与生效条件清洗（对齐原版 src/lib/consts/buff-zones.ts）

use serde_json::Value;

use crate::types::BuffCondition;

#[allow(dead_code)]
pub struct BuffZoneDef {
    pub id: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
}

/// 与 wuwa-afyg-tool 的 ZONE_DEFS 保持一致，作为 Buff 集 zoneId 白名单
pub const BUFF_ZONES: &[BuffZoneDef] = &[
    BuffZoneDef { id: "atkFlat", label: "攻击固定值", unit: "flat" },
    BuffZoneDef { id: "atkPct", label: "攻击百分比", unit: "%" },
    BuffZoneDef { id: "hpFlat", label: "生命固定值", unit: "flat" },
    BuffZoneDef { id: "hpPct", label: "生命百分比", unit: "%" },
    BuffZoneDef { id: "defFlat", label: "防御固定值", unit: "flat" },
    BuffZoneDef { id: "defPct", label: "防御百分比", unit: "%" },
    BuffZoneDef { id: "critRate", label: "暴击率", unit: "%" },
    BuffZoneDef { id: "critDmg", label: "暴击伤害", unit: "%" },
    BuffZoneDef { id: "recharge", label: "共鸣效率", unit: "%" },
    BuffZoneDef { id: "tuneBreakBoost", label: "谐度破坏增幅", unit: "flat" },
    BuffZoneDef { id: "offTuneBuildupRate", label: "偏谐值累积效率", unit: "%" },
    BuffZoneDef { id: "bonusDmg", label: "加成(增伤区)", unit: "%" },
    BuffZoneDef { id: "deepenDmg", label: "加深(加深区)", unit: "%" },
    BuffZoneDef { id: "resPen", label: "对目标属性抗性无视(穿抗)", unit: "%" },
    BuffZoneDef { id: "defPen", label: "对目标防御无视(穿防)", unit: "%" },
    BuffZoneDef { id: "defDown", label: "目标防御降低(减防)", unit: "%" },
    BuffZoneDef { id: "dmgRedPen", label: "对目标免伤无视(穿免)", unit: "%" },
    BuffZoneDef { id: "resDown", label: "目标抗性降低(减抗)", unit: "%" },
    BuffZoneDef { id: "tuneStrainLayer", label: "集谐干涉层数", unit: "flat" },
    BuffZoneDef { id: "finalDmg", label: "最终伤害(终伤区)", unit: "%" },
    BuffZoneDef { id: "dmgTakenInc", label: "伤害提升(易伤区)", unit: "%" },
    BuffZoneDef { id: "customFinalDmg", label: "倍率/其它(特殊终伤)", unit: "%" },
    BuffZoneDef { id: "customFinalDmgMul", label: "倍率/其它(特殊终伤·乘算)", unit: "%" },
    BuffZoneDef { id: "extraRatio", label: "额外倍率", unit: "%" },
];

pub const BUFF_ENTITY_TYPES: &[&str] = &["character", "weapon", "echo", "1set", "2set", "3set", "4set", "5set"];

#[allow(dead_code)]
pub const BUFF_ENTITY_LABELS: &[(&str, &str)] = &[
    ("character", "角色"),
    ("weapon", "武器"),
    ("echo", "首位声骸"),
    ("1set", "套装 1件"),
    ("2set", "套装 2件"),
    ("3set", "套装 3件"),
    ("4set", "套装 4件"),
    ("5set", "套装 5件"),
];

/// 引用乘区白名单（对齐 wuwa-afyg-tool 的 ZONE_REF_DEFS）
pub const BUFF_REF_ZONES: &[(&str, &str, &str)] = &[
    ("baseAtk", "攻击白值", "flat"),
    ("totalAtk", "当前攻击", "flat"),
    ("baseHp", "生命白值", "flat"),
    ("totalHp", "生命上限", "flat"),
    ("baseDef", "防御白值", "flat"),
    ("totalDef", "当前防御", "flat"),
    ("recharge", "共鸣效率", "%"),
    ("tuneBreakBoost", "谐度破坏增幅", "flat"),
    ("offTuneBuildupRate", "偏谐值累积效率", "%"),
    ("critRate", "暴击率", "%"),
    ("critDmg", "暴击伤害", "%"),
];

pub const BUFF_SCOPES: &[&str] = &["self", "self_except", "team", "effect_only"];

#[allow(dead_code)]
pub const BUFF_SCOPE_LABELS: &[(&str, &str)] = &[
    ("self", "对自己"),
    ("self_except", "自己除外"),
    ("team", "对全队"),
    ("effect_only", "效应专属"),
];

pub const CHAIN_MAX: i64 = 6;
pub const REFINE_MAX: i64 = 5;

pub const BUFF_ELEMENTS: &[&str] = &["物理", "冷凝", "热熔", "导电", "气动", "衍射", "湮灭"];

pub const BUFF_DAMAGE_TYPES: &[&str] = &[
    "普攻伤害",
    "重击伤害",
    "共鸣技能伤害",
    "共鸣解放伤害",
    "声骸技能伤害",
    "变奏技能伤害",
    "延奏技能伤害",
    "协同攻击伤害",
    "效应伤害",
    "其它类型伤害",
];

#[allow(dead_code)]
pub const BUFF_DAMAGE_TYPE_SHORT: &[(&str, &str)] = &[
    ("普攻伤害", "普攻"),
    ("重击伤害", "重击"),
    ("共鸣技能伤害", "共技"),
    ("共鸣解放伤害", "共解"),
    ("声骸技能伤害", "声骸"),
    ("变奏技能伤害", "变奏"),
    ("延奏技能伤害", "延奏"),
    ("协同攻击伤害", "协同"),
    ("效应伤害", "效应"),
    ("其它类型伤害", "其它"),
];

pub fn is_buff_zone(id: &str) -> bool {
    BUFF_ZONES.iter().any(|z| z.id == id)
}

pub fn is_buff_ref_zone(id: &str) -> bool {
    BUFF_REF_ZONES.iter().any(|z| z.0 == id)
}

pub fn is_buff_scope(s: &str) -> bool {
    BUFF_SCOPES.contains(&s)
}

/// 清洗生效条件：白名单校验 + 数值/数组归一化；兼容旧格式 {type:"chain"|"refinement",min}；全空返回 None
pub fn sanitize_condition(cond: &Value) -> Option<BuffCondition> {
    if !cond.is_object() {
        return None;
    }
    let c = cond.as_object().unwrap();

    // 旧格式兼容：{ type: 'chain'|'refinement', min } → 多字段
    if let Some(t) = c.get("type").and_then(|v| v.as_str()) {
        if t == "chain" || t == "refinement" {
            let min = c
                .get("min")
                .and_then(|v| v.as_f64())
                .map(|f| f.floor() as i64)
                .unwrap_or(0);
            let max = if t == "chain" { CHAIN_MAX } else { REFINE_MAX };
            let min_ok = if t == "chain" { min >= 0 } else { min >= 1 };
            if min_ok && min <= max {
                let mut out = BuffCondition {
                    chain: None,
                    refinement: None,
                    elements: None,
                    damage_types: None,
                };
                if t == "chain" {
                    out.chain = Some(min);
                } else {
                    out.refinement = Some(min);
                }
                return Some(out);
            }
            return None;
        }
    }

    let mut out = BuffCondition {
        chain: None,
        refinement: None,
        elements: None,
        damage_types: None,
    };
    if let Some(v) = c.get("chain").and_then(|v| v.as_f64()) {
        let min = v.floor() as i64;
        if (0..=CHAIN_MAX).contains(&min) {
            out.chain = Some(min);
        }
    }
    if let Some(v) = c.get("refinement").and_then(|v| v.as_f64()) {
        let min = v.floor() as i64;
        if (1..=REFINE_MAX).contains(&min) {
            out.refinement = Some(min);
        }
    }
    if let Some(arr) = c.get("elements").and_then(|v| v.as_array()) {
        let mut elems: Vec<String> = arr
            .iter()
            .filter_map(|e| e.as_str())
            .filter(|e| BUFF_ELEMENTS.contains(e))
            .map(|s| s.to_string())
            .collect();
        elems.dedup();
        if !elems.is_empty() {
            out.elements = Some(elems);
        }
    }
    if let Some(arr) = c.get("damageTypes").and_then(|v| v.as_array()) {
        let mut types: Vec<String> = arr
            .iter()
            .filter_map(|d| d.as_str())
            .filter(|d| BUFF_DAMAGE_TYPES.contains(d))
            .map(|s| s.to_string())
            .collect();
        types.dedup();
        if !types.is_empty() {
            out.damage_types = Some(types);
        }
    }
    if out.chain.is_none() && out.refinement.is_none() && out.elements.is_none() && out.damage_types.is_none() {
        None
    } else {
        Some(out)
    }
}

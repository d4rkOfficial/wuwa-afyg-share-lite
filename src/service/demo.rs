// 示例数据（管理员命令 `demo` 写入，便于本地体验/验证）

use anyhow::Result;
use rusqlite::Connection;
use serde_json::json;

use crate::repo;
use crate::types::BuffSetRow;

fn row(entity_type: &str, entity_name: &str, buff_name: &str, scope: &str, zones: serde_json::Value) -> BuffSetRow {
    BuffSetRow {
        entity_type: entity_type.to_string(),
        entity_name: entity_name.to_string(),
        buff_name: buff_name.to_string(),
        scope: scope.to_string(),
        exclusive: false,
        condition: None,
        buff_set: serde_json::from_value(zones).unwrap_or_default(),
    }
}

pub fn seed_demo(conn: &Connection) -> Result<String> {
    let rows = vec![
        row(
            "character",
            "维里奈",
            "共济",
            "team",
            json!([
                {"zoneId": "atkPct", "value": 12.0},
                {"zoneId": "recharge", "value": 10.0}
            ]),
        ),
        row(
            "character",
            "今汐",
            "渊鳞",
            "self",
            json!([{"zoneId": "bonusDmg", "value": 10.0}]),
        ),
        row(
            "weapon",
            "千古洑流",
            "剑心",
            "self",
            json!([
                {"zoneId": "atkPct", "value": 12.0},
                {"zoneId": "bonusDmg", "value": 12.0}
            ]),
        ),
        row(
            "weapon",
            "苍鳞千嶂",
            "龙威",
            "self",
            json!([
                {"zoneId": "critRate", "value": 12.0},
                {"zoneId": "atkPct", "value": 12.0}
            ]),
        ),
        row(
            "5set",
            "轻云出月",
            "五件套",
            "team",
            json!([{"zoneId": "recharge", "value": 10.0}]),
        ),
        row(
            "echo",
            "无常凶鹭",
            "主位",
            "self",
            json!([{"zoneId": "bonusDmg", "value": 12.0}]),
        ),
        row(
            "character",
            "长离",
            "离火（6链）",
            "self",
            json!([{"zoneId": "critDmg", "value": 15.0}]),
        ),
    ];
    for r in &rows {
        repo::upsert_buff_set(conn, r)?;
    }
    // 示例公告
    repo::insert_announcement(
        conn,
        "欢迎使用 wuwa-afyg-share-lite",
        "这是示例公告：本实例使用 SQLite 存储，仅提供 TUI/CLI 界面，HTTP API 与椰果工坊一致。",
    )?;
    Ok(format!("已写入 {} 条 Buff 集示例数据与 1 条示例公告", rows.len()))
}

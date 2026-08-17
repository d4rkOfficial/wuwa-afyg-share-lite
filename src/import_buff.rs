// Buff 集「导出格式 SQL」导入器
//
// 读取 `buff-sets` 导出（/api/buff-sets/export，即 export_sql::buff_sets_to_sql 生成的
// Postgres 风格 INSERT）并批量导入到本库 buff_sets 表。
//
// 支持两种输入来源（内容相同处理）：
//   - 从一个 .sql 文件路径 read
//   - 从交互输入（TUI 演示用）
//
// 解析规则：逐行找形如 `('A','B','C','D',bool, NULL|'...'::jsonb, '...'::jsonb)` 的值元组，
// 剥掉 `::jsonb` 后缀与字符串引号，把 condition / buff_set JSON 字符串解析为 serde_json::Value，
// 再写入 SQLite buff_sets 表（UPSERT）。
//
// 导出的布尔是裸 `true` / `false`（无引号），scope / exclusivity 对应第 4、5 列。

use anyhow::{bail, Result};
use rusqlite::Connection;

use crate::repo;
use crate::types::BuffSetRow;

/// 从导出 SQL 文本解析出 BuffSetRow 列表。
fn parse_export_sql(sql: &str) -> Result<Vec<BuffSetRow>> {
    let mut rows: Vec<BuffSetRow> = Vec::new();
    for line in sql.lines() {
        let line = line.trim();
        // 值元组以 '(' 开头且包含 7 个逗号分隔字段的整行（导出的 VALUES 行的行内括号）
        if !line.starts_with('(') {
            continue;
        }
        // 去掉行尾 ";" 或 ","（批量 VALUES 用逗号续行）
        let body = line.trim_end_matches([';', ',']);
        // 去掉首尾括号
        if !(body.starts_with('(') && body.ends_with(')')) {
            continue;
        }
        let inner = &body[1..body.len() - 1];
        // 拆分 7 个字段（注意可能含逗号的 JSON，但这里我们按逗号拆，因为每一字段整体是单引号串/布尔/NULL，内部 JSON 是单引号内的 JSON 字符串没有裸逗号问题——其实 JSON 字符串里逗号在引号内，所以整体仍以逗号作为字段分隔）
        let fields = split_fields(inner);
        if fields.len() != 7 {
            continue;
        }
        let entity_type = unquote(&fields[0]);
        let entity_name = unquote(&fields[1]);
        let buff_name = unquote(&fields[2]);
        let scope = unquote(&fields[3]);
        let exclusive = fields[4].trim().eq_ignore_ascii_case("true");
        let condition = parse_json_field(&fields[5])?;
        let buff_set = parse_json_array(&fields[6])?;
        if entity_type.is_empty() || entity_name.is_empty() || buff_name.is_empty() {
            continue;
        }
        rows.push(BuffSetRow {
            entity_type,
            entity_name,
            buff_name,
            scope,
            exclusive,
            condition,
            buff_set,
        });
    }
    Ok(rows)
}

/// 拆分字段：按逗号切，但忽略单引号内的逗号（JSON 字符串内可能含逗号）。
fn split_fields(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_single => {
                in_single = true;
                cur.push(c);
            }
            '\'' if in_single => {
                if chars.peek() == Some(&'\'') {
                    cur.push(c);
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                } else {
                    in_single = false;
                    cur.push(c);
                }
            }
            ',' if !in_single => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// 去掉 SQL 字符串外层单引号，并处理 '' → ' 转义。
fn unquote(field: &str) -> String {
    let f = field.trim();
    if f.len() >= 2 && f.starts_with('\'') && f.ends_with('\'') {
        let inner = &f[1..f.len() - 1];
        inner.replace("''", "'")
    } else {
        f.to_string()
    }
}

/// 解析 `NULL` 或 `'...'::jsonb` 字段为 BuffCondition。
fn parse_json_field(field: &str) -> Result<Option<crate::types::BuffCondition>> {
    let f = field.trim();
    if f.eq_ignore_ascii_case("null") || f == "NULL" {
        return Ok(None);
    }
    // 去掉 ::jsonb 后缀（可能在引号后）
    let no_cast = f
        .strip_suffix("::jsonb")
        .map(|s| s.trim())
        .unwrap_or(f);
    let inner = unquote(no_cast);
    if inner.is_empty() || inner.eq_ignore_ascii_case("null") {
        return Ok(None);
    }
    serde_json::from_str(&inner)
        .map(Some)
        .map_err(|e| anyhow::anyhow!("condition JSON 解析失败：{}（原文：{}）", e, inner))
}

/// 解析 `'...'::jsonb` 为 BuffZoneValue 数组。
fn parse_json_array(field: &str) -> Result<Vec<crate::types::BuffZoneValue>> {
    let f = field.trim();
    let no_cast = f
        .strip_suffix("::jsonb")
        .map(|s| s.trim())
        .unwrap_or(f);
    let inner = unquote(no_cast);
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&inner)
        .map_err(|e| anyhow::anyhow!("buff_set JSON 解析失败：{}（原文：{}）", e, inner))
}

/// 从 SQL 文件读入并批量导入（UPSERT），返回导入条数。
pub fn import_from_sql(conn: &Connection, sql_text: &str) -> Result<i64> {
    let rows = parse_export_sql(sql_text)?;
    if rows.is_empty() {
        bail!("未能从 SQL 中解析出任何 Buff 集条目（请确认文件是 /api/buff-sets/export 导出的格式）");
    }
    repo::insert_buff_sets(conn, &rows)?;
    Ok(rows.len() as i64)
}

#[allow(unused)]
pub fn _exported_sample() -> &'static str {
    "INSERT INTO public.buff_sets (entity_type, entity_name, buff_name, scope, exclusive, condition, buff_set) VALUES"
}

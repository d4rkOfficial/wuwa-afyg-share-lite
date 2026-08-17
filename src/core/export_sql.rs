// Buff 集全量 SQL 导出（纯函数；对齐原版 src/lib/buff-snapshots/export-sql.ts）
// 生成可直接在 PostgreSQL / Supabase SQL Editor 执行的 INSERT 语句。

use crate::types::BuffSetRow;

fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}

pub struct ExportMeta {
    pub source: String,
    pub exported_at: String,
}

/// 生成 INSERT SQL；行数 > 0 时批量 VALUES，空数据时仅输出说明注释
pub fn buff_sets_to_sql(rows: &[BuffSetRow], meta: &ExportMeta) -> String {
    let mut header = Vec::new();
    header.push("-- ═══════════════════════════════════════════════════════════".to_string());
    header.push("-- 椰果工坊 · Buff 集全量导出".to_string());
    header.push(format!("-- 来源：{}", meta.source));
    header.push(format!("-- 导出时间：{}", meta.exported_at));
    header.push("-- 目标表：public.buff_sets (entity_type, entity_name, buff_name, scope, exclusive, condition, buff_set)".to_string());
    header.push("-- 可在 PostgreSQL / Supabase SQL Editor 直接执行".to_string());
    header.push("-- ═══════════════════════════════════════════════════════════".to_string());
    header.push(String::new());

    if rows.is_empty() {
        header.push("-- （当前无任何 Buff 集数据）".to_string());
        header.push(String::new());
        return header.join("\n");
    }

    let lines: Vec<String> = rows
        .iter()
        .map(|r| {
            let condition = match &r.condition {
                None => "NULL".to_string(),
                Some(c) => format!("'{}'::jsonb", sql_escape(&serde_json::to_string(c).unwrap_or_default())),
            };
            let buff_set = format!("'{}'::jsonb", sql_escape(&serde_json::to_string(&r.buff_set).unwrap_or_default()));
            format!(
                "('{}','{}','{}','{}',{}, {}, {})",
                sql_escape(&r.entity_type),
                sql_escape(&r.entity_name),
                sql_escape(&r.buff_name),
                sql_escape(&r.scope),
                if r.exclusive { "true" } else { "false" },
                condition,
                buff_set
            )
        })
        .collect();

    let mut out = header;
    out.push("BEGIN;".to_string());
    out.push("INSERT INTO public.buff_sets (entity_type, entity_name, buff_name, scope, exclusive, condition, buff_set) VALUES".to_string());
    out.push(lines.join(",\n") + ";");
    out.push("COMMIT;".to_string());
    out.push(String::new());
    out.join("\n")
}

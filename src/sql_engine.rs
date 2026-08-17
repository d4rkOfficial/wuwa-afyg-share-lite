// 共享 SQLite 执行器：管理员身份可读写，非管理员只读查询。
// 供 CLI `sql` 子命令与 TUI 脚本的 `sql` 动作复用。
//
// 权限判定来自调用方传入的 UserCtx：
//   - user.is_admin（含本机 root_admin）→ 可执行任意 SQL（UPDATE/DELETE/INSERT 等）
//   - 否则强制进入只读模式：仅允许 SELECT / PRAGMA / EXPLAIN 等无副作用语句

use anyhow::{bail, Result};
use rusqlite::Connection;
use std::time::Instant;

use crate::types::UserCtx;

/// SQL 执行结果（表格行）
#[derive(Debug)]
pub struct SqlResult {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// 受影响行数（对非 SELECT 语句可见；可能为 0）
    pub changed: u64,
    pub elapsed_ms: u128,
    /// 是否只读执行（因为用户非管理员被降级）
    pub readonly: bool,
}

/// 归一化：去掉前导空白与注释，返回首个有效关键字（大写）。
fn first_keyword(sql: &str) -> Option<String> {
    for tok in sql.split_whitespace() {
        let t = tok.trim();
        if t.is_empty() || t.starts_with("--") || t.starts_with("/*") {
            continue;
        }
        // 去掉可能的注释结尾
        let clean = t
            .split(';')
            .next()
            .unwrap_or("")
            .trim_start_matches(['(', '`', '"'])
            .trim_end_matches([')', '`', '"'])
            .to_uppercase();
        if !clean.is_empty() {
            return Some(clean);
        }
    }
    None
}

/// 判断语句是否只读（无副作用）。
fn is_query_sql(sql: &str) -> bool {
    matches!(
        first_keyword(sql).as_deref(),
        Some("SELECT") | Some("WITH") | Some("PRAGMA") | Some("EXPLAIN") | Some("VALUES")
    )
}

/// 执行一条 SQL；`user` 决定是否只读。
pub fn run_sql_on(user: &UserCtx, sql: &str, conn: &Connection) -> Result<SqlResult> {
    let readonly = !user.is_admin;
    if readonly && !is_query_sql(sql) {
        bail!("无权限：仅管理员可执行写操作（当前为非管理员，已限制为只读）");
    }

    let start = Instant::now();
    let is_query = is_query_sql(sql);

    if is_query {
        let mut stmt = conn.prepare(sql).map_err(|e| anyhow::anyhow!("SQL 准备失败：{}", e))?;
        let col_count = stmt.column_count();
        let mut headers = Vec::with_capacity(col_count);
        for i in 0..col_count {
            headers.push(stmt.column_name(i).unwrap_or("?").to_string());
        }
        let mut rows = Vec::new();
        if col_count > 0 {
            let mut query = stmt
                .query([])
                .map_err(|e| anyhow::anyhow!("SQL 执行失败：{}", e))?;
            while let Some(r) = query
                .next()
                .map_err(|e| anyhow::anyhow!("SQL 读取失败：{}", e))?
            {
                let mut row = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let v: rusqlite::types::ValueRef = r.get_ref(i)?;
                    row.push(match v {
                        rusqlite::types::ValueRef::Null => "(NULL)".into(),
                        rusqlite::types::ValueRef::Integer(i) => i.to_string(),
                        rusqlite::types::ValueRef::Real(f) => f.to_string(),
                        rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
                        rusqlite::types::ValueRef::Blob(b) => format!("<blob {} bytes>", b.len()),
                    });
                }
                rows.push(row);
            }
        }
        Ok(SqlResult {
            headers,
            rows,
            changed: 0,
            elapsed_ms: start.elapsed().as_millis(),
            readonly,
        })
    } else {
        // 写语句 / DDL
        let changed = conn
            .execute(sql, [])
            .map_err(|e| anyhow::anyhow!("SQL 执行失败：{}", e))? as u64;
        Ok(SqlResult {
            headers: Vec::new(),
            rows: Vec::new(),
            changed,
            elapsed_ms: start.elapsed().as_millis(),
            readonly,
        })
    }
}

/// 打印 SQL 结果（表格）
pub fn print_sql_result(r: &SqlResult) {
    if r.readonly {
        println!("（只读模式执行）");
    }
    if !r.headers.is_empty() {
        print_table(&r.headers, &r.rows);
        println!("（{} 行，用时 {} ms）", r.rows.len(), r.elapsed_ms);
    } else if r.changed > 0 {
        println!("（影响 {} 行，用时 {} ms）", r.changed, r.elapsed_ms);
    } else {
        println!("（执行完成，用时 {} ms）", r.elapsed_ms);
    }
}

fn print_table(headers: &[String], rows: &[Vec<String>]) {
    let mut widths = headers.iter().map(|h| h.chars().count()).collect::<Vec<_>>();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(cell.chars().count());
            }
        }
    }
    print_row(headers, &widths);
    let total = widths.iter().sum::<usize>() + widths.len().saturating_sub(1) + 4;
    println!("{}", "─".repeat(total));
    for row in rows {
        print_row(row, &widths);
    }
}

fn print_row(cells: &[String], widths: &[usize]) {
    for (i, c) in cells.iter().enumerate() {
        print!(
            "{}{:width$}",
            if i == 0 { "" } else { "│" },
            c,
            width = widths.get(i).copied().unwrap_or(0) + 2
        );
    }
    println!();
}

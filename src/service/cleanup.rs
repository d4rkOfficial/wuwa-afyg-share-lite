// 过期清理（对齐原版 pg_cron 每 5 分钟任务：删除已过期工程，保护工程豁免）

use anyhow::Result;
use rusqlite::Connection;

use crate::db::now_iso;
use crate::repo;

pub fn delete_expired_projects(conn: &Connection) -> Result<i64> {
    repo::delete_expired_projects(conn, &now_iso())
}

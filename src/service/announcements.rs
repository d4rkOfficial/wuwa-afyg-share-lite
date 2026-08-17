// 公告业务（对齐原版 src/lib/actions/announcements.ts）

use anyhow::{bail, Result};
use rusqlite::Connection;

use crate::db::trunc_chars;
use crate::repo;
use crate::types::AnnouncementRow;

pub const MAX_TITLE_LEN: usize = 120;
pub const MAX_LEN: usize = 2000;

pub fn list_announcements(conn: &Connection) -> Result<Vec<AnnouncementRow>> {
    repo::list_announcements(conn)
}

pub fn create_announcement(conn: &Connection, title: &str, content: &str) -> Result<()> {
    let t = trunc_chars(title.trim(), MAX_TITLE_LEN);
    let c = trunc_chars(content.trim(), MAX_LEN);
    if t.is_empty() {
        bail!("公告标题不能为空");
    }
    if c.is_empty() {
        bail!("公告详情不能为空");
    }
    repo::insert_announcement(conn, &t, &c)?;
    Ok(())
}

pub fn update_announcement(conn: &Connection, id: &str, title: &str, content: &str) -> Result<()> {
    let t = trunc_chars(title.trim(), MAX_TITLE_LEN);
    let c = trunc_chars(content.trim(), MAX_LEN);
    if t.is_empty() {
        bail!("公告标题不能为空");
    }
    if c.is_empty() {
        bail!("公告详情不能为空");
    }
    if repo::get_announcement(conn, id)?.is_none() {
        bail!("公告不存在");
    }
    repo::update_announcement(conn, id, &t, &c)?;
    Ok(())
}

pub fn delete_announcement(conn: &Connection, id: &str) -> Result<()> {
    if repo::get_announcement(conn, id)?.is_none() {
        bail!("公告不存在");
    }
    repo::delete_announcement(conn, id)?;
    Ok(())
}

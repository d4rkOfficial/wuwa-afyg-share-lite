// 外置可定制文件管理（web 页面 / TUI 脚本）
//
// 用户可修改的界面与脚本随数据一起存放在数据目录（~/.wuwa-afyg-share-lite/）：
//   - index.html       Web 页面（用户可自行修改；serve 时从文件读取）
//   - tui-script.json  TUI 脚本（JSON，可配置 SQL / HTTP / 命令动作）
// 二者在首次运行（或缺失）时从内嵌默认模板拷出；`wipe` 会一并清除；
// 可用 `cli reset-templates [--tui] [--web]` 恢复默认。

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// 内嵌默认 Web 页面模板（零构建原生 Web Components）
pub const DEFAULT_WEB: &str = include_str!("../web/index.html");

/// 内嵌默认 TUI 脚本模板（JSON）
pub const DEFAULT_TUI_SCRIPT: &str = include_str!("../defaults/tui-script.json");

/// 确保外置文件存在：缺失时从内嵌默认模板写入；已存在则保留（不覆盖用户修改）。
/// 返回文件是否被（重新）写入。
pub fn ensure_asset(path: &Path, default: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, default.as_bytes())?;
    Ok(true)
}

/// 读取外置文件内容；缺失时用默认模板（不落盘）。
pub fn read_asset(path: &Path, default: &str) -> Result<String> {
    if path.exists() {
        std::fs::read_to_string(path)
            .with_context(|| format!("读取外置文件失败：{}", path.display()))
    } else {
        Ok(default.to_string())
    }
}

/// 恢复默认：用内嵌默认模板覆盖文件。返回是否写入。
pub fn restore_default(path: &Path, default: &str) -> Result<bool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, default.as_bytes())?;
    Ok(true)
}

/// wipe：删除外置可定制文件（返回是否删除了某个）
pub fn delete_asset(path: &Path) -> Result<Option<String>> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(Some(path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("删除外置文件失败：{}", path.display())),
    }
}

/// 确保两处默认文件存在（serve / tui 首次运行时调用）
pub fn ensure_defaults(app_dir: &Path) -> Result<()> {
    let web = app_dir.join("index.html");
    let script = app_dir.join("tui-script.json");
    ensure_asset(&web, DEFAULT_WEB)?;
    ensure_asset(&script, DEFAULT_TUI_SCRIPT)?;
    Ok(())
}

/// web 页面读取（serve 用）
pub fn web_content(app_dir: &Path) -> Result<String> {
    read_asset(&app_dir.join("index.html"), DEFAULT_WEB)
}

/// tui 脚本读取（tui 用）
pub fn tui_script_content(app_dir: &Path) -> Result<String> {
    read_asset(&app_dir.join("tui-script.json"), DEFAULT_TUI_SCRIPT)
}

pub fn web_file_path(app_dir: &Path) -> PathBuf {
    app_dir.join("index.html")
}

pub fn tui_script_file_path(app_dir: &Path) -> PathBuf {
    app_dir.join("tui-script.json")
}

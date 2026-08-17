// 本地配置：服务器地址、登录 token、数据库路径（~/.wuwa-afyg-share-lite/config.json）

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_SERVER: &str = "http://localhost:3000";
pub const APP_DIR_NAME: &str = ".wuwa-afyg-share-lite";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_server")]
    pub server: String,
    #[serde(default)]
    pub token: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: default_server(),
            token: None,
        }
    }
}

fn default_server() -> String {
    DEFAULT_SERVER.to_string()
}

pub fn app_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(APP_DIR_NAME)
}

pub fn config_path() -> PathBuf {
    app_dir().join("config.json")
}

pub fn default_db_path() -> PathBuf {
    app_dir().join("share.db")
}

/// 读取配置：容忍 UTF-8 BOM（Windows PowerShell 写入的旧文件）与首尾空白
fn read_config_text(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut text = String::from_utf8(bytes).ok()?;
    if text.starts_with('\u{feff}') {
        text = text.trim_start_matches('\u{feff}').to_string();
    }
    Some(text.trim().to_string())
}

pub fn load_config() -> Config {
    let path = config_path();
    if let Some(text) = read_config_text(&path) {
        if let Ok(cfg) = serde_json::from_str::<Config>(&text) {
            return cfg;
        }
    }
    Config::default()
}

pub fn save_config(cfg: &Config) -> Result<()> {
    let dir = app_dir();
    std::fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(cfg)?;
    std::fs::write(config_path(), text)?;
    Ok(())
}

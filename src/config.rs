// 本地配置：服务器地址、登录 token、数据库路径（~/.wuwa-afyg-share-lite/config.json）

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_SERVER: &str = "http://localhost:3000";
pub const APP_DIR_NAME: &str = ".wuwa-afyg-share-lite";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_server")]
    pub server: String,
    #[serde(default)]
    pub token: Option<String>,
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

pub fn load_config() -> Config {
    let path = config_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
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

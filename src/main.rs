// wuwa-afyg-share-lite 入口
// 椰果工坊 SQLite 版：仅 TUI/CLI 界面；HTTP API 与 wuwa-afyg-share 一致
// （无 AI 工具 / bilibili-toy 功能）

mod cli;
mod client;
mod config;
mod core;
mod db;
mod http;
mod repo;
mod service;
mod tui;
mod types;
mod upstream;
mod web;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(e) = cli::run(&cli) {
        eprintln!("错误：{}", e);
        std::process::exit(1);
    }
}

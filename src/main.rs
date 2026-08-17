// wuwa-afyg-share-lite 入口
// 椰果工坊 SQLite 版：TUI/CLI + 单文件 Web；HTTP API 与 wuwa-afyg-share 一致
// （无 AI 工具 / bilibili-toy 功能）

mod assets;
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

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(e) = cli::run(&cli) {
        eprintln!("错误：{}", e);
        std::process::exit(1);
    }
}

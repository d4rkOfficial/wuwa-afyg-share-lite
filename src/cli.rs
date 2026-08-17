// CLI 命令定义与实现

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use dialoguer::{Password, Select};
use serde_json::{json, Value};

use crate::client::Client;
use crate::config;
use crate::core::format::{format_count, format_date, time_ago};

#[derive(Parser)]
#[command(
    name = "wuwa-afyg-share-lite",
    version,
    about = "椰果工坊 SQLite 版：仅 TUI/CLI 界面的《鸣潮》工程分享平台\nHTTP API 与 wuwa-afyg-share 一致（无 AI 工具 / bilibili-toy 功能）",
    arg_required_else_help = true
)]
pub struct Cli {
    /// 服务器地址（默认读取本地配置；可用环境变量 WUWA_AFYG_SHARE_SERVER）
    #[arg(long, global = true, env = "WUWA_AFYG_SHARE_SERVER")]
    pub server: Option<String>,
    /// 登录 token（默认读取本地配置）
    #[arg(long, global = true)]
    pub token: Option<String>,
    /// 输出原始 JSON
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 启动 HTTP API 服务器（SQLite 存储）
    Serve {
        /// 监听端口（默认 3000；可用环境变量 PORT）
        #[arg(long, env = "PORT")]
        port: Option<u16>,
        /// 监听地址（默认 0.0.0.0；可用环境变量 HOST）
        #[arg(long, env = "HOST")]
        host: Option<String>,
        /// 数据库文件路径（默认 ~/.wuwa-afyg-share-lite/share.db）
        #[arg(long, env = "WUWA_AFYG_SHARE_DB")]
        db: Option<PathBuf>,
        /// 分享链接前缀（默认取请求 Host）
        #[arg(long, env = "WUWA_AFYG_SHARE_SITE_URL")]
        site_url: Option<String>,
    },
    /// 注册账号（首个账号为根管理员；无邮件系统）
    Register {
        username: String,
        /// 密码（不传则交互输入）
        #[arg(long)]
        password: Option<String>,
    },
    /// 登录并保存 token
    Login {
        username: String,
        /// 密码（不传则交互输入）
        #[arg(long)]
        password: Option<String>,
    },
    /// 退出登录
    Logout,
    /// 查看当前登录用户
    Whoami,
    /// 浏览工程广场（公开）
    Browse {
        #[arg(long, default_value_t = 1)]
        page: i64,
        #[arg(long, default_value_t = 12)]
        per_page: i64,
        /// 搜索标题/作者
        #[arg(long)]
        q: Option<String>,
        /// 排序：newest 最新 / heat 最热
        #[arg(long, default_value = "newest")]
        sort: String,
        /// 只看登录用户工程
        #[arg(long)]
        exclude_anon: bool,
    },
    /// 查看分享详情（公开）
    Info { code: String },
    /// 下载工程 JSON（公开）
    Download {
        code: String,
        /// 保存到文件（默认输出到 stdout）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// 匿名分享工程文件（10 分钟有效）
    Share { file: PathBuf },
    /// 登录后发布工程
    Publish {
        file: PathBuf,
        /// 简介
        #[arg(long = "desc")]
        description: Option<String>,
        /// 标签（可多次）
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// 过期天数（不设则永久）
        #[arg(long)]
        days: Option<i64>,
        /// 指定过期时间（ISO 8601，优先于 days）
        #[arg(long)]
        expires: Option<String>,
    },
    /// 我的工程列表与统计
    Me,
    /// 我的工程管理（子命令）
    Project {
        id: String,
        #[command(subcommand)]
        action: ProjectAction,
    },
    /// 管理员操作（子命令）
    Admin {
        #[command(subcommand)]
        action: AdminAction,
    },
    /// Buff 集（子命令）
    Buff {
        #[command(subcommand)]
        action: BuffAction,
    },
    /// Buff 集快照（子命令，管理员）
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },
    /// 公告（子命令）
    Announcement {
        #[command(subcommand)]
        action: AnnouncementAction,
    },
    /// 上游名录（nanoka.cc 角色/武器/声骸/套装，公开）
    Catalog {
        #[command(subcommand)]
        action: CatalogAction,
    },
    /// 手动清理过期工程（管理员）
    Cleanup,
    /// 清空全部本地数据（仅本机 localhost 可用，需二次确认）
    Wipe {
        /// 跳过二次确认，直接清空
        #[arg(long)]
        force: bool,
    },
    /// 写入示例数据（管理员）
    Demo,
    /// 启动交互式 TUI
    Tui,
    /// 查看/修改本地配置
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// 查看详情
    Show,
    /// 编辑标题/简介/标签
    Update {
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// 设置过期时间（ISO 8601；传 --clear 清除到期时间）
    SetExpiry {
        #[arg(long)]
        at: Option<String>,
        #[arg(long)]
        clear: bool,
    },
    /// 换源：以新工程文件覆盖内容
    ReplaceFile { file: PathBuf },
    /// 重新生成分享码
    RegenerateCode,
    /// 发布/隐藏切换
    TogglePublish,
    /// 删除（保护中的工程不可删）
    Delete,
    /// 设置保护
    Protect,
    /// 解除保护
    Unprotect,
}

#[derive(Subcommand)]
pub enum AdminAction {
    /// 管理员列表（权限树）
    Users,
    /// 按用户名授权管理员
    Grant { username: String },
    /// 撤销管理员（仅授出者可收回，连坐收回子树）
    Revoke { username: String },
    /// 管理员工程列表
    Projects {
        #[arg(long)]
        q: Option<String>,
        #[arg(long, default_value_t = 1)]
        page: i64,
        #[arg(long, default_value_t = 20)]
        page_size: i64,
    },
    /// 删除某用户全部工程（保护工程豁免）
    DeleteUser { username: String },
    /// 查询某用户工程统计
    Summary { username: String },
}

#[derive(Subcommand)]
pub enum BuffAction {
    /// 列出 Buff 集（公开）
    List {
        #[arg(long)]
        entity_type: Option<String>,
        #[arg(long)]
        q: Option<String>,
    },
    /// 查看某实体的 Buff（公开）
    Get {
        entity_type: String,
        entity_name: String,
        /// 只显示指定 buff
        #[arg(long)]
        buff_name: Option<String>,
    },
    /// 导出全量 SQL（公开；优先快照，无快照用实时数据）
    Export {
        /// 保存到文件（默认输出到 stdout）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// 新增/更新单条 Buff（管理员）
    Upsert {
        entity_type: String,
        entity_name: String,
        buff_name: String,
        #[arg(long, default_value = "team")]
        scope: String,
        #[arg(long)]
        exclusive: bool,
        /// 生效条件 JSON，如 '{"chain":2}' / '{"elements":["热熔"]}'
        #[arg(long)]
        condition: Option<String>,
        /// 增益项，格式 zoneId=数值[;ref=引用区;pct=比例;override]，
        /// 如 "atkPct=12" 或 "atkPct=10;ref=totalAtk;pct=0.1;override"（可多次）
        #[arg(long = "zone")]
        zones: Vec<String>,
    },
    /// 删除单条 Buff（管理员）
    Delete {
        entity_type: String,
        entity_name: String,
        buff_name: String,
    },
    /// 整体替换实体（删除该实体全部行后写回；管理员）
    EntityUpdate {
        entity_type: String,
        entity_name: String,
        /// 输入 JSON 文件：[{"buffName","scope","exclusive","condition","zones"}]
        file: PathBuf,
    },
    /// 删除整个实体的全部 Buff（管理员）
    EntityDelete { entity_type: String, entity_name: String },
}

#[derive(Subcommand)]
pub enum SnapshotAction {
    /// 快照列表（根 + 版本链）
    List,
    /// 创建快照：无根→建根；有根→追加版本（无差异时报错）
    Create {
        #[arg(long)]
        note: Option<String>,
    },
    /// 对比当前 Buff 集与指定快照的差异
    Diff { id: String },
    /// 恢复到指定快照（级联删除比其新的版本）
    Restore { id: String },
    /// 删除版本快照（仅最新版本可删）
    Delete { id: String },
}

#[derive(Subcommand)]
pub enum AnnouncementAction {
    /// 公告列表（公开）
    List,
    /// 新建公告（管理员）
    Create { title: String, content: String },
    /// 更新公告（管理员）
    Update { id: String, title: String, content: String },
    /// 删除公告（管理员）
    Delete { id: String },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// 查看全部配置
    Get,
    /// 设置配置项（server / token）
    Set { key: String, value: String },
}

#[derive(Subcommand)]
pub enum CatalogAction {
    /// 列出名录（--type characters/weapons/echoes/sets，缺省全部）
    List {
        #[arg(long = "type")]
        entity_type: Option<String>,
        /// 按名称搜索
        #[arg(long)]
        q: Option<String>,
    },
    /// 查看上游版本与条目统计
    Version,
}

// ── 实现 ────────────────────────────────────────────────

fn make_client(cli: &Cli) -> Result<Client> {
    let cfg = config::load_config();
    let mut server = cli.server.clone().unwrap_or(cfg.server);
    if server.trim().is_empty() {
        server = config::DEFAULT_SERVER.to_string();
    }
    let token = cli.token.clone().or(cfg.token);
    Ok(Client::new(&server, token))
}

fn need_token(client: &Client) -> Result<String> {
    // 本机服务器（localhost）免登录，直接以 root_admin 身份操作
    if crate::client::is_loopback_url(&client.base) {
        return Ok(String::new());
    }
    client
        .token
        .clone()
        .ok_or_else(|| anyhow::anyhow!("请先登录：`wuwa-afyg-share-lite login <用户名>`"))
}

/// 输出原始 JSON（若开启 --json）；返回是否已输出（调用方应提前返回）
fn maybe_json(cli: &Cli, v: Value) -> bool {
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        true
    } else {
        false
    }
}

fn prompt_password(prompt: &str) -> Result<String> {
    Ok(Password::new().with_prompt(prompt).interact()?)
}

fn fmt_short_project(p: &Value, idx: i64) -> String {
    let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("?");
    let author = p.get("authorName").and_then(|v| v.as_str()).unwrap_or("?");
    let downloads = p.get("downloads").and_then(|v| v.as_i64()).unwrap_or(0);
    let created = p
        .get("createdAt")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| format_date(d.timestamp_millis()))
        .unwrap_or_default();
    let code = p.get("code").and_then(|v| v.as_str()).unwrap_or("");
    format!(
        "[{}] {} ｜ 作者：{} ｜ 下载：{} ｜ {} ｜ 码：{}",
        idx,
        title,
        author,
        format_count(downloads),
        created,
        code
    )
}

fn print_public_projects(v: &Value, page: i64, per_page: i64) {
    let projects = v.get("projects").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let total = v.get("total").and_then(|x| x.as_i64()).unwrap_or(0);
    if projects.is_empty() {
        println!("（暂无工程）");
        return;
    }
    let start = (page - 1) * per_page;
    for (i, p) in projects.iter().enumerate() {
        println!("{}", fmt_short_project(p, start + i as i64 + 1));
    }
    let pages = if per_page > 0 { (total + per_page - 1) / per_page } else { 1 };
    println!(
        "—— 第 {} / {} 页（共 {} 个工程）——",
        page,
        pages.max(1),
        total
    );
}

pub fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Serve { port, host, db, site_url } => run_serve(cli, *port, host.clone(), db.clone(), site_url.clone()),
        Command::Register { username, password } => run_register(cli, username, password.as_deref()),
        Command::Login { username, password } => run_login(cli, username, password.as_deref()),
        Command::Logout => run_logout(cli),
        Command::Whoami => run_whoami(cli),
        Command::Browse { page, per_page, q, sort, exclude_anon } => run_browse(cli, *page, *per_page, q.as_deref(), sort, *exclude_anon),
        Command::Info { code } => run_info(cli, code),
        Command::Download { code, output } => run_download(cli, code, output.as_ref()),
        Command::Share { file } => run_share(cli, file),
        Command::Publish { file, description, tags, days, expires } => run_publish(cli, file, description.as_deref(), tags, *days, expires.as_deref()),
        Command::Me => run_me(cli),
        Command::Project { id, action } => run_project(cli, id, action),
        Command::Admin { action } => run_admin(cli, action),
        Command::Buff { action } => run_buff(cli, action),
        Command::Snapshot { action } => run_snapshot(cli, action),
        Command::Announcement { action } => run_announcement(cli, action),
        Command::Catalog { action } => run_catalog(cli, action),
        Command::Cleanup => run_cleanup(cli),
        Command::Wipe { force } => run_wipe(cli, *force),
        Command::Demo => run_demo(cli),
        Command::Tui => crate::tui::run(&make_client(cli)?),
        Command::Config { action } => run_config(action),
    }
}

fn run_serve(cli: &Cli, port: Option<u16>, host: Option<String>, db: Option<PathBuf>, site_url: Option<String>) -> Result<()> {
    let _ = cli;
    let db_path = db
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| config::default_db_path().to_string_lossy().to_string());
    let host = host.unwrap_or_else(|| "0.0.0.0".to_string());
    let port = port.unwrap_or(3000);
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(crate::http::server::run_server(db_path, host, port, site_url))
}

fn run_register(cli: &Cli, username: &str, password: Option<&str>) -> Result<()> {
    let client = make_client(cli)?;
    let password = match password {
        Some(p) => p.to_string(),
        None => prompt_password("设置密码（至少 6 个字符）")?,
    };
    let resp = client.post("/api/auth/register", json!({ "username": username, "password": password }))?;
    let v = resp.ok_json()?;
    let token = v.get("token").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let mut cfg = config::load_config();
    cfg.server = cli.server.clone().unwrap_or(cfg.server);
    cfg.token = Some(token);
    config::save_config(&cfg)?;
    if maybe_json(cli, v.clone()) { return Ok(()); }
    let user = v.get("user").cloned().unwrap_or(Value::Null);
    let name = user.get("username").and_then(|x| x.as_str()).unwrap_or(username);
    let is_admin = user.get("isAdmin").and_then(|x| x.as_bool()).unwrap_or(false);
    println!("注册成功：{}（{}）", name, if is_admin { "根管理员" } else { "普通用户" });
    Ok(())
}

fn run_login(cli: &Cli, username: &str, password: Option<&str>) -> Result<()> {
    let client = make_client(cli)?;
    let password = match password {
        Some(p) => p.to_string(),
        None => prompt_password("密码")?,
    };
    let resp = client.post("/api/auth/login", json!({ "username": username, "password": password }))?;
    let v = resp.ok_json()?;
    let token = v.get("token").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let mut cfg = config::load_config();
    cfg.server = cli.server.clone().unwrap_or(cfg.server);
    cfg.token = Some(token);
    config::save_config(&cfg)?;
    if maybe_json(cli, v) { return Ok(()); }
    println!("登录成功：{}", username);
    Ok(())
}

fn run_logout(cli: &Cli) -> Result<()> {
    let client = make_client(cli)?;
    if let Some(_t) = &client.token {
        let _ = client.post("/api/auth/logout", json!({}));
    }
    let mut cfg = config::load_config();
    cfg.token = None;
    config::save_config(&cfg)?;
    println!("已退出登录");
    Ok(())
}

fn run_whoami(cli: &Cli) -> Result<()> {
    let client = make_client(cli)?;
    let resp = client.get("/api/auth/me", &[])?;
    if resp.status == 401 {
        println!("未登录");
        return Ok(());
    }
    let v = resp.ok_json()?;
    if maybe_json(cli, v.clone()) { return Ok(()); }
    let user = v.get("user").cloned().unwrap_or(Value::Null);
    let name = user.get("username").and_then(|x| x.as_str()).unwrap_or("?");
    let is_admin = user.get("isAdmin").and_then(|x| x.as_bool()).unwrap_or(false);
    println!(
        "当前用户：{}（{}）",
        name,
        if is_admin { "管理员" } else { "普通用户" }
    );
    Ok(())
}

fn run_browse(
    cli: &Cli,
    page: i64,
    per_page: i64,
    q: Option<&str>,
    sort: &str,
    exclude_anon: bool,
) -> Result<()> {
    let client = make_client(cli)?;
    let page_s = page.to_string();
    let per_page_s = per_page.to_string();
    let mut query: Vec<(&str, &str)> = vec![
        ("page", &page_s),
        ("perPage", &per_page_s),
        ("sort", sort),
    ];
    if let Some(q) = q {
        if !q.is_empty() {
            query.push(("q", q));
        }
    }
    if exclude_anon {
        query.push(("excludeAnon", "1"));
    }
    let resp = client.get("/api/public/projects", &query)?;
    let v = resp.ok_json()?;
    if maybe_json(cli, v.clone()) { return Ok(()); }
    print_public_projects(&v, page, per_page);
    Ok(())
}

fn run_info(cli: &Cli, code: &str) -> Result<()> {
    let client = make_client(cli)?;
    let resp = client.get(&format!("/api/public/projects/{}", code), &[])?;
    let v = resp.ok_json()?;
    if maybe_json(cli, v.clone()) { return Ok(()); }
    let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("?");
    let author = v.get("authorName").and_then(|x| x.as_str()).unwrap_or("?");
    let code = v.get("code").and_then(|x| x.as_str()).unwrap_or(code);
    let created = v
        .get("createdAt")
        .and_then(|x| x.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| format_date(d.timestamp_millis()))
        .unwrap_or_default();
    let downloads = v.get("downloads").and_then(|x| x.as_i64()).unwrap_or(0);
    let views = v.get("views").and_then(|x| x.as_i64()).unwrap_or(0);
    let expires = v.get("expiresAt").and_then(|x| x.as_str());
    println!("标题：{}", title);
    println!("作者：{}", author);
    println!("分享码：{}", code);
    println!("创建：{}", created);
    println!("浏览：{} ｜ 下载：{}", format_count(views), format_count(downloads));
    if let Some(names) = v.get("teamNames").and_then(|x| x.as_array()) {
        let names: Vec<&str> = names.iter().filter_map(|n| n.as_str()).collect();
        if !names.is_empty() {
            println!("配队：{}", names.join("、"));
        }
    }
    if let Some(tags) = v.get("tags").and_then(|x| x.as_array()) {
        let tags: Vec<String> = tags.iter().filter_map(|t| t.as_str().map(|s| format!("#{}", s))).collect();
        if !tags.is_empty() {
            println!("标签：{}", tags.join(" "));
        }
    }
    if let Some(desc) = v.get("description").and_then(|x| x.as_str()) {
        if !desc.is_empty() {
            println!("简介：{}", desc);
        }
    }
    match expires {
        Some(e) => println!("到期：{}", e),
        None => println!("到期：永久"),
    }
    println!("下载链接：{}/share/{}/download", client.base, code);
    Ok(())
}

fn run_download(cli: &Cli, code: &str, output: Option<&PathBuf>) -> Result<()> {
    let client = make_client(cli)?;
    let resp = client.get(&format!("/share/{}/download", code), &[])?;
    if !(200..300).contains(&resp.status) {
        bail!(resp.error_msg());
    }
    match output {
        Some(path) => {
            std::fs::write(path, &resp.text)?;
            println!("已保存到 {}", path.display());
        }
        None => print!("{}", resp.text),
    }
    Ok(())
}

fn read_file_text(path: &PathBuf) -> Result<String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("无法读取文件 {}：{}", path.display(), e))?;
    if text.trim().is_empty() {
        bail!("文件为空");
    }
    Ok(text)
}

fn run_share(cli: &Cli, file: &PathBuf) -> Result<()> {
    let client = make_client(cli)?;
    let file_text = read_file_text(file)?;
    let resp = client.post("/api/public/projects", json!({ "fileText": file_text }))?;
    let v = resp.ok_json()?;
    if maybe_json(cli, v.clone()) { return Ok(()); }
    let code = v.get("code").and_then(|x| x.as_str()).unwrap_or("?");
    let url = v.get("url").and_then(|x| x.as_str()).unwrap_or("");
    println!("匿名分享成功（10 分钟有效）：");
    println!("  分享码：{}", code);
    println!("  链接：  {}", url);
    Ok(())
}

fn run_publish(
    cli: &Cli,
    file: &PathBuf,
    description: Option<&str>,
    tags: &[String],
    days: Option<i64>,
    expires: Option<&str>,
) -> Result<()> {
    let client = make_client(cli)?;
    need_token(&client)?;
    let file_text = read_file_text(file)?;
    let body = json!({
        "fileText": file_text,
        "description": description.unwrap_or(""),
        "tags": tags,
        "expiresDays": days,
        "expiresAt": expires,
    });
    let resp = client.post("/api/me/projects", body)?;
    let v = resp.ok_json()?;
    if maybe_json(cli, v.clone()) { return Ok(()); }
    let code = v.get("code").and_then(|x| x.as_str()).unwrap_or("?");
    println!("发布成功：分享码 {} ｜ {}/share/{}", code, client.base, code);
    Ok(())
}

fn run_me(cli: &Cli) -> Result<()> {
    let client = make_client(cli)?;
    need_token(&client)?;
    let resp = client.get("/api/me", &[])?;
    let v = resp.ok_json()?;
    if maybe_json(cli, v.clone()) { return Ok(()); }
    let stats = v.get("stats").cloned().unwrap_or(Value::Null);
    let projects = v.get("projects").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    println!(
        "工程：{} ｜ 已过期：{} ｜ 总浏览：{} ｜ 总克隆：{}",
        stats.get("projects").and_then(|x| x.as_i64()).unwrap_or(0),
        stats.get("expiredCount").and_then(|x| x.as_i64()).unwrap_or(0),
        stats.get("totalViews").and_then(|x| x.as_i64()).unwrap_or(0),
        stats.get("totalClones").and_then(|x| x.as_i64()).unwrap_or(0),
    );
    if projects.is_empty() {
        println!("（还没有上传过工程）");
        return Ok(());
    }
    for (i, p) in projects.iter().enumerate() {
        let title = p.get("title").and_then(|x| x.as_str()).unwrap_or("?");
        let code = p.get("code").and_then(|x| x.as_str()).unwrap_or("");
        let published = p.get("published").and_then(|x| x.as_bool()).unwrap_or(true);
        let protected = p.get("protected").and_then(|x| x.as_bool()).unwrap_or(false);
        let clones = p.get("clone_count").and_then(|x| x.as_i64()).unwrap_or(0);
        let created = p
            .get("created_at")
            .and_then(|x| x.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| time_ago(d.timestamp_millis()))
            .unwrap_or_default();
        let id = p.get("id").and_then(|x| x.as_str()).unwrap_or("");
        println!(
            "[{}] {}（{}）{} {} ｜ 下载 {} ｜ id={}",
            i + 1,
            title,
            code,
            if published { "" } else { "已隐藏" },
            if protected { "已保护" } else { "" },
            clones,
            id
        );
        let _ = created;
    }
    Ok(())
}

fn run_project(cli: &Cli, id: &str, action: &ProjectAction) -> Result<()> {
    let client = make_client(cli)?;
    need_token(&client)?;
    let path = |suffix: &str| format!("/api/me/projects/{}{}", id, suffix);
    match action {
        ProjectAction::Show => {
            let resp = client.get(&path(""), &[])?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            Ok(())
        }
        ProjectAction::Update { title, description, tags } => {
            let mut body = json!({});
            if let Some(t) = title {
                body["title"] = json!(t);
            }
            if let Some(d) = description {
                body["description"] = json!(d);
            }
            if !tags.is_empty() {
                body["tags"] = json!(tags);
            }
            let resp = client.patch(&path(""), body)?;
            resp.ok_void()?;
            println!("已更新");
            Ok(())
        }
        ProjectAction::SetExpiry { at, clear } => {
            let expires_at: Option<String> = if *clear {
                None
            } else {
                Some(at.clone().ok_or_else(|| anyhow::anyhow!("请用 --at <ISO时间> 指定到期时间，或 --clear 清除"))?)
            };
            let resp = client.put(&path("/expiry"), json!({ "expiresAt": expires_at }))?;
            resp.ok_void()?;
            println!("已设置到期时间");
            Ok(())
        }
        ProjectAction::ReplaceFile { file } => {
            let file_text = read_file_text(file)?;
            let resp = client.put(&path("/file"), json!({ "fileText": file_text }))?;
            resp.ok_void()?;
            println!("换源成功");
            Ok(())
        }
        ProjectAction::RegenerateCode => {
            let resp = client.post(&path("/regenerate-code"), json!({}))?;
            let v = resp.ok_json()?;
            let code = v.get("code").and_then(|x| x.as_str()).unwrap_or("?");
            println!("新分享码：{}", code);
            Ok(())
        }
        ProjectAction::TogglePublish => {
            let resp = client.post(&path("/toggle-publish"), json!({}))?;
            let v = resp.ok_json()?;
            let published = v.get("published").and_then(|x| x.as_bool()).unwrap_or(false);
            println!("已{}", if published { "发布" } else { "隐藏" });
            Ok(())
        }
        ProjectAction::Delete => {
            // 二次确认（对齐原版 UI）
            let idx = Select::new()
                .with_prompt("确认删除该工程？此操作不可恢复")
                .items(&["取消", "确认删除"])
                .default(0)
                .interact()?;
            if idx != 1 {
                println!("已取消");
                return Ok(());
            }
            let resp = client.delete(&path(""))?;
            resp.ok_void()?;
            println!("已删除");
            Ok(())
        }
        ProjectAction::Protect => {
            let resp = client.put(&path("/protected"), json!({ "protected": true }))?;
            resp.ok_void()?;
            println!("已设置保护");
            Ok(())
        }
        ProjectAction::Unprotect => {
            let resp = client.put(&path("/protected"), json!({ "protected": false }))?;
            resp.ok_void()?;
            println!("已解除保护");
            Ok(())
        }
    }
}

fn run_admin(cli: &Cli, action: &AdminAction) -> Result<()> {
    let client = make_client(cli)?;
    need_token(&client)?;
    match action {
        AdminAction::Users => {
            let resp = client.get("/api/admin/users", &[])?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            let admins = v.get("admins").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            if admins.is_empty() {
                println!("（暂无管理员）");
                return Ok(());
            }
            for a in &admins {
                let name = a.get("username").and_then(|x| x.as_str()).unwrap_or("?");
                let granted_by = a.get("grantedBy").and_then(|x| x.as_str());
                let by_me = a.get("grantedByMe").and_then(|x| x.as_bool()).unwrap_or(false);
                match granted_by {
                    Some(g) => println!("{}（由 {} 授权{}）", name, g, if by_me { "，可撤销" } else { "" }),
                    None => println!("{}（根管理员，不可撤销）", name),
                }
            }
            Ok(())
        }
        AdminAction::Grant { username } => {
            let resp = client.post(&format!("/api/admin/users/{}/grant", username), json!({}))?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            println!("{}", v.get("message").and_then(|x| x.as_str()).unwrap_or("已授权"));
            Ok(())
        }
        AdminAction::Revoke { username } => {
            let resp = client.post(&format!("/api/admin/users/{}/revoke", username), json!({}))?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            println!("{}", v.get("message").and_then(|x| x.as_str()).unwrap_or("已撤销"));
            Ok(())
        }
        AdminAction::Projects { q, page, page_size } => {
            let page_s = page.to_string();
            let page_size_s = page_size.to_string();
            let mut query: Vec<(&str, &str)> = vec![
                ("page", &page_s),
                ("pageSize", &page_size_s),
            ];
            if let Some(q) = q {
                if !q.is_empty() {
                    query.push(("q", q));
                }
            }
            let resp = client.get("/api/admin/projects", &query)?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            let items = v.get("items").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let total = v.get("total").and_then(|x| x.as_i64()).unwrap_or(0);
            if items.is_empty() {
                println!("（暂无工程）");
                return Ok(());
            }
            let start = ((*page).max(1) - 1) * (*page_size).max(1);
            for (i, p) in items.iter().enumerate() {
                let title = p.get("title").and_then(|x| x.as_str()).unwrap_or("?");
                let author = p.get("author_name").and_then(|x| x.as_str()).unwrap_or("?");
                let code = p.get("code").and_then(|x| x.as_str()).unwrap_or("");
                let published = p.get("published").and_then(|x| x.as_bool()).unwrap_or(true);
                let id = p.get("id").and_then(|x| x.as_str()).unwrap_or("");
                println!(
                    "[{}] {}（{}）作者：{}{} ｜ id={}",
                    start + i as i64 + 1,
                    title,
                    code,
                    author,
                    if published { "" } else { "（已隐藏）" },
                    id
                );
            }
            println!("—— 共 {} 个工程 ——", total);
            Ok(())
        }
        AdminAction::DeleteUser { username } => {
            let resp = client.delete(&format!("/api/admin/users/{}/content", username))?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            println!(
                "已删除 {} 个工程",
                v.get("deletedProjects").and_then(|x| x.as_i64()).unwrap_or(0)
            );
            Ok(())
        }
        AdminAction::Summary { username } => {
            let resp = client.get(&format!("/api/admin/users/{}/summary", username), &[])?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            println!(
                "用户 {} 共有 {} 个工程",
                v.get("username").and_then(|x| x.as_str()).unwrap_or("?"),
                v.get("projectCount").and_then(|x| x.as_i64()).unwrap_or(0)
            );
            Ok(())
        }
    }
}

fn parse_zone(spec: &str) -> Result<(String, f64, Option<crate::types::BuffZoneRef>, Option<bool>)> {
    // 格式：zoneId=数值[;ref=引用区;pct=比例;threshold=..;discrete;override;refOwner=self|owner]
    let mut parts = spec.split(';');
    let first = parts.next().unwrap_or("");
    let (zone_id, value_str) = first
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("zone 格式错误：{}（应为 zoneId=数值）", spec))?;
    let value: f64 = value_str
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("zone 数值无效：{}", value_str))?;
    let mut r#ref: Option<crate::types::BuffZoneRef> = None;
    let mut r#override: Option<bool> = None;
    for part in parts {
        if part == "override" {
            r#override = Some(true);
            continue;
        }
        if part == "discrete" {
            if let Some(r) = &mut r#ref {
                r.discrete = Some(true);
            }
            continue;
        }
        if let Some((k, val)) = part.split_once('=') {
            match k.trim() {
                "ref" => {
                    let r = crate::types::BuffZoneRef {
                        target_zone_id: val.trim().to_string(),
                        pct: 0.0,
                        threshold: None,
                        lower: None,
                        upper: None,
                        discrete: None,
                        divisor: None,
                        multiplier: None,
                        ref_owner: None,
                    };
                    if let Some(rr) = &mut r#ref {
                        *rr = r;
                    } else {
                        r#ref = Some(r);
                    }
                }
                "pct" => {
                    if let Some(r) = &mut r#ref {
                        r.pct = val.trim().parse().unwrap_or(0.0);
                    }
                }
                "threshold" => {
                    if let Some(r) = &mut r#ref {
                        r.threshold = val.trim().parse().ok();
                    }
                }
                "lower" => {
                    if let Some(r) = &mut r#ref {
                        r.lower = val.trim().parse().ok();
                    }
                }
                "upper" => {
                    if let Some(r) = &mut r#ref {
                        r.upper = val.trim().parse().ok();
                    }
                }
                "divisor" => {
                    if let Some(r) = &mut r#ref {
                        r.divisor = val.trim().parse().ok();
                    }
                }
                "multiplier" => {
                    if let Some(r) = &mut r#ref {
                        r.multiplier = val.trim().parse().ok();
                    }
                }
                "refOwner" => {
                    if let Some(r) = &mut r#ref {
                        r.ref_owner = Some(val.trim().to_string());
                    }
                }
                _ => {}
            }
        }
    }
    Ok((zone_id.trim().to_string(), value, r#ref, r#override))
}

fn run_buff(cli: &Cli, action: &BuffAction) -> Result<()> {
    let client = make_client(cli)?;
    match action {
        BuffAction::List { entity_type, q } => {
            let mut query: Vec<(&str, &str)> = Vec::new();
            if let Some(t) = entity_type {
                query.push(("entity_type", t));
            }
            if let Some(q) = q {
                query.push(("q", q));
            }
            let resp = client.get("/api/buff-sets", &query)?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            let rows = v.get("buffSets").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            if rows.is_empty() {
                println!("（无匹配 Buff 集）");
                return Ok(());
            }
            let mut current = String::new();
            for r in &rows {
                let et = r.get("entity_type").and_then(|x| x.as_str()).unwrap_or("");
                let en = r.get("entity_name").and_then(|x| x.as_str()).unwrap_or("");
                let bn = r.get("buff_name").and_then(|x| x.as_str()).unwrap_or("");
                let scope = r.get("scope").and_then(|x| x.as_str()).unwrap_or("");
                let zones = r.get("buff_set").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
                let group = format!("{}｜{}", et, en);
                if group != current {
                    println!("[{}]", group);
                    current = group;
                }
                println!("  {}（scope={}，{} 项）", bn, scope, zones);
            }
            println!("—— 共 {} 条 ——", rows.len());
            Ok(())
        }
        BuffAction::Get { entity_type, entity_name, buff_name } => {
            let resp = client.get(
                "/api/buff-sets",
                &[("entity_type", entity_type), ("entity_name", entity_name)],
            )?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            let rows = v.get("buffSets").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            if rows.is_empty() {
                println!("（该实体暂无 Buff 数据）");
                return Ok(());
            }
            for r in &rows {
                let bn = r.get("buff_name").and_then(|x| x.as_str()).unwrap_or("");
                if let Some(filter) = buff_name {
                    if bn != filter {
                        continue;
                    }
                }
                let scope = r.get("scope").and_then(|x| x.as_str()).unwrap_or("");
                let exclusive = r.get("exclusive").and_then(|x| x.as_bool()).unwrap_or(false);
                let condition = r.get("condition").cloned().unwrap_or(Value::Null);
                println!("◆ {}（scope={}{}）", bn, scope, if exclusive { "，exclusive" } else { "" });
                if !condition.is_null() {
                    println!("  条件：{}", condition);
                }
                if let Some(zones) = r.get("buff_set").and_then(|x| x.as_array()) {
                    for z in zones {
                        let zid = z.get("zoneId").and_then(|x| x.as_str()).unwrap_or("");
                        let val = z.get("value").and_then(|x| x.as_f64()).unwrap_or(0.0);
                        let mut line = format!("    {} = {}", zid, val);
                        if let Some(rf) = z.get("ref") {
                            let target = rf.get("targetZoneId").and_then(|x| x.as_str()).unwrap_or("");
                            let pct = rf.get("pct").and_then(|x| x.as_f64()).unwrap_or(0.0);
                            line.push_str(&format!("（引用 {} × {}）", target, pct));
                        }
                        if z.get("override").and_then(|x| x.as_bool()).unwrap_or(false) {
                            line.push_str(" override");
                        }
                        println!("{}", line);
                    }
                }
            }
            Ok(())
        }
        BuffAction::Export { output } => {
            let resp = client.get("/api/buff-sets/export", &[])?;
            if !(200..300).contains(&resp.status) {
                bail!(resp.error_msg());
            }
            match output {
                Some(path) => {
                    std::fs::write(path, &resp.text)?;
                    println!("已导出到 {}", path.display());
                }
                None => print!("{}", resp.text),
            }
            Ok(())
        }
        BuffAction::Upsert { entity_type, entity_name, buff_name, scope, exclusive, condition, zones } => {
            need_token(&client)?;
            let mut zones_json = Vec::new();
            for z in zones {
                let (zone_id, value, r#ref, r#override) = parse_zone(z)?;
                let mut item = json!({ "zoneId": zone_id, "value": value });
                if let Some(rf) = r#ref {
                    item["ref"] = serde_json::to_value(rf)?;
                }
                if let Some(o) = r#override {
                    item["override"] = json!(o);
                }
                zones_json.push(item);
            }
            let condition_json = match condition {
                Some(c) => serde_json::from_str::<Value>(c)
                    .map_err(|_| anyhow::anyhow!("condition 不是合法 JSON：{}", c))?,
                None => Value::Null,
            };
            let body = json!({
                "entityType": entity_type,
                "entityName": entity_name,
                "buffName": buff_name,
                "scope": scope,
                "exclusive": exclusive,
                "condition": condition_json,
                "zones": zones_json,
            });
            let resp = client.post("/api/admin/buff-sets", body)?;
            resp.ok_void()?;
            println!("已保存 Buff");
            Ok(())
        }
        BuffAction::Delete { entity_type, entity_name, buff_name } => {
            need_token(&client)?;
            let resp = client.delete(&format!(
                "/api/admin/buff-sets/{}/{}/{}",
                entity_type, entity_name, buff_name
            ))?;
            resp.ok_void()?;
            println!("已删除");
            Ok(())
        }
        BuffAction::EntityUpdate { entity_type, entity_name, file } => {
            need_token(&client)?;
            let text = read_file_text(file)?;
            let buffs: Value = serde_json::from_str(&text)
                .map_err(|_| anyhow::anyhow!("输入文件不是合法 JSON 数组"))?;
            let resp = client.post(
                "/api/admin/buff-entities",
                json!({ "entityType": entity_type, "entityName": entity_name, "buffs": buffs }),
            )?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            println!(
                "已保存 {} 条 Buff",
                v.get("saved").and_then(|x| x.as_i64()).unwrap_or(0)
            );
            Ok(())
        }
        BuffAction::EntityDelete { entity_type, entity_name } => {
            need_token(&client)?;
            let resp = client.delete(&format!("/api/admin/buff-entities/{}/{}", entity_type, entity_name))?;
            resp.ok_void()?;
            println!("已删除该实体全部 Buff");
            Ok(())
        }
    }
}

fn run_snapshot(cli: &Cli, action: &SnapshotAction) -> Result<()> {
    let client = make_client(cli)?;
    need_token(&client)?;
    match action {
        SnapshotAction::List => {
            let resp = client.get("/api/admin/snapshots", &[])?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            let snaps = v.get("snapshots").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            if snaps.is_empty() {
                println!("（暂无快照）");
                return Ok(());
            }
            for s in &snaps {
                let id = s.get("id").and_then(|x| x.as_str()).unwrap_or("");
                let note = s.get("note").and_then(|x| x.as_str()).unwrap_or("");
                let created = s
                    .get("createdAt")
                    .and_then(|x| x.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| time_ago(d.timestamp_millis()))
                    .unwrap_or_default();
                let is_root = s.get("isRoot").and_then(|x| x.as_bool()).unwrap_or(false);
                let latest = s.get("isLatest").and_then(|x| x.as_bool()).unwrap_or(false);
                println!(
                    "[{}] {}{} {} ｜ {}（{}）",
                    id,
                    if is_root { "根快照" } else { "版本" },
                    if latest { "（最新）" } else { "" },
                    note,
                    created,
                    s.get("createdBy").and_then(|x| x.as_str()).unwrap_or("?")
                );
            }
            Ok(())
        }
        SnapshotAction::Create { note } => {
            let resp = client.post("/api/admin/snapshots", json!({ "note": note }))?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            let mode = v.get("mode").and_then(|x| x.as_str()).unwrap_or("");
            let rows = v.get("rows").and_then(|x| x.as_i64()).unwrap_or(0);
            println!(
                "已创建{}快照（{} 行）",
                if mode == "root" { "根" } else { "版本" },
                rows
            );
            Ok(())
        }
        SnapshotAction::Diff { id } => {
            let resp = client.get(&format!("/api/admin/snapshots/{}/diff", id), &[])?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            let target = v.get("target").cloned().unwrap_or(Value::Null);
            println!(
                "目标：{}（{}）{}",
                target.get("isRoot").and_then(|x| x.as_bool()).unwrap_or(false).then_some("根快照").unwrap_or("版本"),
                target.get("note").and_then(|x| x.as_str()).unwrap_or(""),
                target.get("createdAt").and_then(|x| x.as_str()).unwrap_or("")
            );
            let diff = v.get("diff").cloned().unwrap_or(Value::Null);
            let added = diff.get("added").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
            let modified = diff.get("modified").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
            let removed = diff.get("removed").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
            println!(
                "差异：新增 {}，修改 {}，删除 {}（当前共 {} 条）",
                added,
                modified,
                removed,
                v.get("currentCount").and_then(|x| x.as_i64()).unwrap_or(0)
            );
            if let Some(arr) = diff.get("added").and_then(|x| x.as_array()) {
                for a in arr {
                    println!(
                        "  + {}｜{}｜{}",
                        a.get("entity_type").and_then(|x| x.as_str()).unwrap_or(""),
                        a.get("entity_name").and_then(|x| x.as_str()).unwrap_or(""),
                        a.get("buff_name").and_then(|x| x.as_str()).unwrap_or("")
                    );
                }
            }
            if let Some(arr) = diff.get("removed").and_then(|x| x.as_array()) {
                for a in arr {
                    println!(
                        "  - {}｜{}｜{}",
                        a.get("old").and_then(|x| x.get("entity_type")).and_then(|x| x.as_str()).unwrap_or(""),
                        a.get("old").and_then(|x| x.get("entity_name")).and_then(|x| x.as_str()).unwrap_or(""),
                        a.get("old").and_then(|x| x.get("buff_name")).and_then(|x| x.as_str()).unwrap_or("")
                    );
                }
            }
            Ok(())
        }
        SnapshotAction::Restore { id } => {
            let resp = client.post(&format!("/api/admin/snapshots/{}/restore", id), json!({}))?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            println!(
                "已恢复 {} 条 Buff 集数据",
                v.get("restored").and_then(|x| x.as_i64()).unwrap_or(0)
            );
            Ok(())
        }
        SnapshotAction::Delete { id } => {
            let resp = client.delete(&format!("/api/admin/snapshots/{}", id))?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            println!("{}", v.get("message").and_then(|x| x.as_str()).unwrap_or("已删除"));
            Ok(())
        }
    }
}

fn run_announcement(cli: &Cli, action: &AnnouncementAction) -> Result<()> {
    let client = make_client(cli)?;
    match action {
        AnnouncementAction::List => {
            let resp = client.get("/api/announcements", &[])?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) { return Ok(()); }
            let rows = v.get("announcements").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            if rows.is_empty() {
                println!("（暂无公告）");
                return Ok(());
            }
            for a in &rows {
                println!(
                    "[{}] {}（{}）",
                    a.get("id").and_then(|x| x.as_str()).unwrap_or(""),
                    a.get("title").and_then(|x| x.as_str()).unwrap_or(""),
                    a.get("createdAt").and_then(|x| x.as_str()).unwrap_or("")
                );
                println!("    {}", a.get("content").and_then(|x| x.as_str()).unwrap_or(""));
            }
            Ok(())
        }
        AnnouncementAction::Create { title, content } => {
            need_token(&client)?;
            let resp = client.post("/api/admin/announcements", json!({ "title": title, "content": content }))?;
            resp.ok_void()?;
            println!("公告已创建");
            Ok(())
        }
        AnnouncementAction::Update { id, title, content } => {
            need_token(&client)?;
            let resp = client.patch(
                &format!("/api/admin/announcements/{}", id),
                json!({ "title": title, "content": content }),
            )?;
            resp.ok_void()?;
            println!("公告已更新");
            Ok(())
        }
        AnnouncementAction::Delete { id } => {
            need_token(&client)?;
            let resp = client.delete(&format!("/api/admin/announcements/{}", id))?;
            resp.ok_void()?;
            println!("公告已删除");
            Ok(())
        }
    }
}

fn run_catalog(cli: &Cli, action: &CatalogAction) -> Result<()> {
    let client = make_client(cli)?;
    match action {
        CatalogAction::List { entity_type, q } => {
            let path = match entity_type.as_deref() {
                Some(t) if t == "characters" || t == "weapons" || t == "echoes" || t == "sets" => {
                    format!("/api/catalog/{}", t)
                }
                Some(t) => bail!("无效的名录类型：{}（characters/weapons/echoes/sets）", t),
                None => "/api/catalog".to_string(),
            };
            let resp = client.get(&path, &[])?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) {
                return Ok(());
            }
            let version = v.get("version").and_then(|x| x.as_str()).unwrap_or("?");
            let source = v.get("source").and_then(|x| x.as_str()).unwrap_or("");
            let stale = v.get("stale").and_then(|x| x.as_bool()).unwrap_or(false);
            println!(
                "上游：{}（ww {}）{}",
                source,
                version,
                if stale { "【本地缓存】" } else { "" }
            );
            let q_filter = q.as_deref().map(|s| s.to_lowercase());
            let print_items = |key: &str, label: &str, fmt: &dyn Fn(&Value) -> String| {
                let Some(items) = v.get(key).and_then(|x| x.as_array()) else {
                    return;
                };
                let mut shown = 0;
                for item in items {
                    let name = item.get("name").and_then(|x| x.as_str()).unwrap_or("");
                    if let Some(qf) = &q_filter {
                        if !name.to_lowercase().contains(qf) {
                            continue;
                        }
                    }
                    println!("  {} {}", label, fmt(item));
                    shown += 1;
                }
                println!("  —— {} 共 {} 条（显示 {}）——", label, items.len(), shown);
            };
            print_items(
                "characters",
                "角色：",
                &|i| {
                    format!(
                        "{}（{}星 {} {}）",
                        i.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                        i.get("star").and_then(|x| x.as_i64()).unwrap_or(0),
                        i.get("element").and_then(|x| x.as_str()).unwrap_or(""),
                        i.get("weaponType").and_then(|x| x.as_str()).unwrap_or("")
                    )
                },
            );
            print_items(
                "weapons",
                "武器：",
                &|i| {
                    format!(
                        "{}（{}星 {}）",
                        i.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                        i.get("star").and_then(|x| x.as_i64()).unwrap_or(0),
                        i.get("weaponType").and_then(|x| x.as_str()).unwrap_or("")
                    )
                },
            );
            print_items(
                "echoes",
                "声骸：",
                &|i| {
                    let sets = i
                        .get("sets")
                        .and_then(|x| x.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|s| s.as_str().map(|x| x.to_string()))
                                .collect::<Vec<_>>()
                                .join("/")
                        })
                        .unwrap_or_default();
                    format!(
                        "{}（Cost {}）{}",
                        i.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                        i.get("cost").and_then(|x| x.as_i64()).unwrap_or(0),
                        if sets.is_empty() { String::new() } else { format!("【{}】", sets) }
                    )
                },
            );
            print_items(
                "sets",
                "套装：",
                &|i| {
                    let pieces = i
                        .get("pieces")
                        .and_then(|x| x.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|p| p.as_i64().map(|v| v.to_string()))
                                .collect::<Vec<_>>()
                                .join("/")
                        })
                        .unwrap_or_default();
                    format!(
                        "{}（{}件套）",
                        i.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                        pieces
                    )
                },
            );
            Ok(())
        }
        CatalogAction::Version => {
            let resp = client.get("/api/catalog", &[])?;
            let v = resp.ok_json()?;
            if maybe_json(cli, v.clone()) {
                return Ok(());
            }
            println!(
                "上游版本：ww {}（{}）{}",
                v.get("version").and_then(|x| x.as_str()).unwrap_or("?"),
                v.get("source").and_then(|x| x.as_str()).unwrap_or(""),
                if v.get("stale").and_then(|x| x.as_bool()).unwrap_or(false) {
                    "【本地缓存】"
                } else {
                    ""
                }
            );
            if let Some(counts) = v.get("counts") {
                println!(
                    "条目：角色 {} ｜ 武器 {} ｜ 声骸 {} ｜ 套装 {}",
                    counts.get("characters").and_then(|x| x.as_i64()).unwrap_or(0),
                    counts.get("weapons").and_then(|x| x.as_i64()).unwrap_or(0),
                    counts.get("echoes").and_then(|x| x.as_i64()).unwrap_or(0),
                    counts.get("sets").and_then(|x| x.as_i64()).unwrap_or(0),
                );
            }
            Ok(())
        }
    }
}

fn run_cleanup(cli: &Cli) -> Result<()> {
    let client = make_client(cli)?;
    need_token(&client)?;
    let resp = client.post("/api/admin/cleanup", json!({}))?;
    let v = resp.ok_json()?;
    println!(
        "已清理 {} 个过期工程",
        v.get("deleted").and_then(|x| x.as_i64()).unwrap_or(0)
    );
    Ok(())
}

/// 清空全部本地数据：仅本机（localhost）可用；--force 跳过二次确认
fn run_wipe(cli: &Cli, force: bool) -> Result<()> {
    let client = make_client(cli)?;
    if !crate::client::is_loopback_url(&client.base) {
        bail!(
            "仅本机（localhost）可清空本地数据：当前服务器为 {}",
            client.base
        );
    }
    if !force {
        let idx = Select::new()
            .with_prompt("确认清空全部本地数据？此操作不可恢复（工程/账号/会话/Buff 集/快照/公告/授权/上游缓存全部删除）")
            .items(&["取消", "确认清空"])
            .default(0)
            .interact()?;
        if idx != 1 {
            println!("已取消");
            return Ok(());
        }
    }
    let resp = client.post("/api/admin/wipe", json!({}))?;
    let v = resp.ok_json()?;
    let get = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    println!("已清空全部本地数据：");
    println!(
        "  工程 {} ｜ 账号 {} ｜ 会话 {} ｜ Buff 集 {} ｜ 快照 {} ｜ 公告 {} ｜ 授权 {} ｜ 上游缓存 {}",
        get("projects"),
        get("profiles"),
        get("sessions"),
        get("buffSets"),
        get("snapshots"),
        get("announcements"),
        get("grants"),
        get("meta")
    );
    println!("下次本机访问（CLI/TUI/Web）将自动重建 root_admin 账号");
    Ok(())
}

fn run_demo(cli: &Cli) -> Result<()> {
    let client = make_client(cli)?;
    need_token(&client)?;
    let resp = client.post("/api/admin/demo", json!({}))?;
    let v = resp.ok_json()?;
    println!("{}", v.get("message").and_then(|x| x.as_str()).unwrap_or("示例数据已写入"));
    Ok(())
}

fn run_config(action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Get => {
            let cfg = config::load_config();
            println!("server: {}", cfg.server);
            println!(
                "token: {}",
                cfg.token.as_deref().map(|_| "（已登录）").unwrap_or("（未登录）")
            );
            println!("db: {}", config::default_db_path().display());
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            let mut cfg = config::load_config();
            match key.as_str() {
                "server" => {
                    cfg.server = value.clone();
                }
                "token" => {
                    cfg.token = if value.is_empty() { None } else { Some(value.clone()) };
                }
                _ => bail!("仅支持配置项：server / token"),
            }
            config::save_config(&cfg)?;
            println!("已设置 {} = {}", key, value);
            Ok(())
        }
    }
}

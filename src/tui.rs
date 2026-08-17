// 交互式 TUI（dialoguer：方向键菜单 / 输入 / 密码 / 确认）

use anyhow::Result;
use dialoguer::{Confirm, Input, Password, Select};
use serde_json::{json, Value};

use crate::client::Client;
use crate::config;
use crate::core::format::{format_count, format_date, time_ago};

fn input<T>(prompt: &str, default: Option<T>) -> Result<T>
where
    T: std::str::FromStr + std::fmt::Display + Clone,
    T::Err: std::fmt::Display,
{
    let mut i = Input::<T>::new().with_prompt(prompt);
    if let Some(d) = default {
        i = i.default(d);
    }
    Ok(i.interact_text()?)
}

fn confirm(prompt: &str) -> Result<bool> {
    Ok(Confirm::new().with_prompt(prompt).default(false).interact()?)
}

fn select(prompt: &str, items: &[String], default: usize) -> Result<usize> {
    Ok(Select::new().with_prompt(prompt).items(items).default(default).interact()?)
}

fn wait_enter() -> Result<()> {
    let _ = input::<String>("（回车返回）", Some(String::new()))?;
    Ok(())
}

fn fmt_short(p: &Value, idx: i64) -> String {
    let title = p.get("title").and_then(|x| x.as_str()).unwrap_or("?");
    let author = p.get("authorName").and_then(|x| x.as_str()).unwrap_or("?");
    let downloads = p.get("downloads").and_then(|x| x.as_i64()).unwrap_or(0);
    let created = p
        .get("createdAt")
        .and_then(|x| x.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| format_date(d.timestamp_millis()))
        .unwrap_or_default();
    format!("[{}] {} ｜ {} ｜ 下载 {} ｜ {}", idx, title, author, format_count(downloads), created)
}

pub fn run(client: &Client) -> Result<()> {
    loop {
        let items = [
            "🌐 工程广场".to_string(),
            "👤 我的工程".to_string(),
            "📊 Buff 集".to_string(),
            "🛠 管理（管理员）".to_string(),
            "📢 公告".to_string(),
            "⬆ 上传工程".to_string(),
            "❌ 退出".to_string(),
        ];
        let sel = select("主菜单", &items, 0)?;
        match sel {
            0 => plaza(client)?,
            1 => my_projects(client)?,
            2 => buff_menu(client)?,
            3 => admin_menu(client)?,
            4 => announcements_menu(client)?,
            5 => upload_menu(client)?,
            _ => break,
        }
    }
    Ok(())
}

// ── 工程广场 ────────────────────────────────────────────

fn plaza(client: &Client) -> Result<()> {
    let mut page: i64 = 1;
    let mut sort = "newest".to_string();
    let mut q: Option<String> = None;
    loop {
        let page_s = page.to_string();
        let mut query: Vec<(&str, &str)> = vec![("page", &page_s), ("perPage", "12"), ("sort", &sort)];
        if let Some(qq) = &q {
            query.push(("q", qq));
        }
        let v = client.get("/api/public/projects", &query)?.ok_json()?;
        let projects = v.get("projects").and_then(|x| x.as_array()).cloned().unwrap_or_default();
        let total = v.get("total").and_then(|x| x.as_i64()).unwrap_or(0);
        let pages = ((total + 11) / 12).max(1);

        let mut items: Vec<String> = Vec::new();
        items.push(format!("⬅ 上一页（当前第 {} / {} 页）", page, pages));
        items.push(format!("➡ 下一页（共 {} 个工程）", total));
        items.push(format!(
            "🔍 搜索：{}",
            q.as_deref().unwrap_or("（无）")
        ));
        items.push(format!("↕ 排序：{}", if sort == "heat" { "最热" } else { "最新" }));
        for (i, p) in projects.iter().enumerate() {
            items.push(fmt_short(p, (page - 1) * 12 + i as i64 + 1));
        }
        items.push("🔙 返回".to_string());

        let sel = select("工程广场", &items, 4)?;
        match sel {
            0 => {
                if page > 1 {
                    page -= 1;
                }
            }
            1 => {
                if page < pages {
                    page += 1;
                }
            }
            2 => {
                let qq = input::<String>("搜索标题/作者（留空清除）", Some(q.clone().unwrap_or_default()))?;
                q = if qq.trim().is_empty() { None } else { Some(qq.trim().to_string()) };
                page = 1;
            }
            3 => {
                sort = if sort == "heat" { "newest".to_string() } else { "heat".to_string() };
                page = 1;
            }
            idx if idx >= 4 && idx < 4 + projects.len() => {
                let p = &projects[idx - 4];
                let code = p.get("code").and_then(|x| x.as_str()).unwrap_or("").to_string();
                project_detail(client, &code)?;
            }
            _ => break,
        }
    }
    Ok(())
}

fn project_detail(client: &Client, code: &str) -> Result<()> {
    let v = client
        .get(&format!("/api/public/projects/{}", code), &[])?
        .ok_json()?;
    println!("────────────────────────────");
    println!("标题：{}", v.get("title").and_then(|x| x.as_str()).unwrap_or("?"));
    println!("作者：{}", v.get("authorName").and_then(|x| x.as_str()).unwrap_or("?"));
    println!("码：{}", code);
    if let Some(names) = v.get("teamNames").and_then(|x| x.as_array()) {
        let names: Vec<&str> = names.iter().filter_map(|n| n.as_str()).collect();
        if !names.is_empty() {
            println!("配队：{}", names.join("、"));
        }
    }
    if let Some(desc) = v.get("description").and_then(|x| x.as_str()) {
        if !desc.is_empty() {
            println!("简介：{}", desc);
        }
    }
    println!("浏览：{} ｜ 下载：{}", format_count(v.get("views").and_then(|x| x.as_i64()).unwrap_or(0)), format_count(v.get("downloads").and_then(|x| x.as_i64()).unwrap_or(0)));
    println!("到期：{}", v.get("expiresAt").and_then(|x| x.as_str()).unwrap_or("永久"));
    println!("────────────────────────────");
    let items = [
        "⬇ 下载 JSON".to_string(),
        "🔙 返回".to_string(),
    ];
    let sel = select("操作", &items, 0)?;
    if sel == 0 {
        let default_name = format!(
            "{}.json",
            v.get("title").and_then(|x| x.as_str()).unwrap_or("project")
        );
        let path = input::<String>("保存到文件", Some(default_name))?;
        let resp = client.get(&format!("/share/{}/download", code), &[])?;
        if (200..300).contains(&resp.status) {
            std::fs::write(&path, &resp.text)?;
            println!("已保存到 {}", path);
        } else {
            println!("下载失败：{}", resp.error_msg());
        }
    }
    Ok(())
}

// ── 我的工程 ────────────────────────────────────────────

fn ensure_login(client: &Client) -> Result<bool> {
    // 本机服务器（localhost）免登录，直接以 root_admin 身份操作
    if client.token.is_some() || crate::client::is_loopback_url(&client.base) {
        return Ok(true);
    }
    let items = vec!["登录".to_string(), "注册".to_string(), "返回".to_string()];
    let sel = select("需要登录", &items, 0)?;
    match sel {
        0 => {
            let username = input::<String>("用户名", None::<String>)?;
            let password = Password::new().with_prompt("密码").interact()?;
            let v = client
                .post("/api/auth/login", json!({ "username": username, "password": password }))?
                .ok_json()?;
            save_token(client, &v)?;
            Ok(true)
        }
        1 => {
            let username = input::<String>("用户名（2-20 字符，中文/字母/数字/下划线）", None::<String>)?;
            let password = Password::new().with_prompt("密码（至少 6 位）").interact()?;
            let v = client
                .post("/api/auth/register", json!({ "username": username, "password": password }))?
                .ok_json()?;
            save_token(client, &v)?;
            println!("注册成功");
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn save_token(client: &Client, v: &Value) -> Result<()> {
    let token = v.get("token").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let mut cfg = config::load_config();
    cfg.server = client.base.clone();
    cfg.token = Some(token);
    config::save_config(&cfg)?;
    Ok(())
}

fn my_projects(client: &Client) -> Result<()> {
    if !ensure_login(client)? {
        return Ok(());
    }
    let mut client = client.clone();
    client.token = config::load_config().token.or(client.token);
    loop {
        let v = client.get("/api/me", &[])?;
        if v.status == 401 {
            println!("登录已过期，请重新登录");
            let mut cfg = config::load_config();
            cfg.token = None;
            config::save_config(&cfg)?;
            return Ok(());
        }
        let v = v.ok_json()?;
        let projects = v.get("projects").and_then(|x| x.as_array()).cloned().unwrap_or_default();
        let stats = v.get("stats").cloned().unwrap_or(Value::Null);
        let mut items: Vec<String> = Vec::new();
        items.push(format!(
            "📊 工程 {} ｜ 过期 {} ｜ 浏览 {} ｜ 克隆 {}",
            stats.get("projects").and_then(|x| x.as_i64()).unwrap_or(0),
            stats.get("expiredCount").and_then(|x| x.as_i64()).unwrap_or(0),
            stats.get("totalViews").and_then(|x| x.as_i64()).unwrap_or(0),
            stats.get("totalClones").and_then(|x| x.as_i64()).unwrap_or(0),
        ));
        for (i, p) in projects.iter().enumerate() {
            let title = p.get("title").and_then(|x| x.as_str()).unwrap_or("?");
            let code = p.get("code").and_then(|x| x.as_str()).unwrap_or("");
            let hidden = if p.get("published").and_then(|x| x.as_bool()).unwrap_or(true) { "" } else { "（已隐藏）" };
            let prot = if p.get("protected").and_then(|x| x.as_bool()).unwrap_or(false) { "🔒" } else { "" };
            let created = p
                .get("created_at")
                .and_then(|x| x.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| time_ago(d.timestamp_millis()))
                .unwrap_or_default();
            items.push(format!("[{}] {}{}{} ｜ {} ｜ {}", i + 1, title, hidden, prot, code, created));
        }
        items.push("🔙 返回".to_string());
        let sel = select("我的工程", &items, 0)?;
        if sel == 0 {
            continue;
        }
        if sel == items.len() - 1 {
            break;
        }
        let p = &projects[sel - 1];
        let id = p.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if my_project_actions(&mut client, &id)? {
            // 动作完成后回到列表
        }
    }
    Ok(())
}

fn my_project_actions(client: &mut Client, id: &str) -> Result<bool> {
    let v = client.get(&format!("/api/me/projects/{}", id), &[])?.ok_json()?;
    println!("────────────────────────────");
    println!("标题：{}", v.get("title").and_then(|x| x.as_str()).unwrap_or("?"));
    println!("码：{} ｜ 发布：{} ｜ 保护：{}", v.get("code").and_then(|x| x.as_str()).unwrap_or(""), v.get("published").and_then(|x| x.as_bool()).unwrap_or(true), v.get("protected").and_then(|x| x.as_bool()).unwrap_or(false));
    if let Some(desc) = v.get("description").and_then(|x| x.as_str()) {
        if !desc.is_empty() {
            println!("简介：{}", desc);
        }
    }
    println!("浏览 {} ｜ 下载 {} ｜ 创建 {}", format_count(v.get("view_count").and_then(|x| x.as_i64()).unwrap_or(0)), format_count(v.get("clone_count").and_then(|x| x.as_i64()).unwrap_or(0)), v.get("created_at").and_then(|x| x.as_str()).unwrap_or(""));
    println!("────────────────────────────");
    let items = [
        "✏ 编辑标题/简介/标签".to_string(),
        "⏰ 设置过期".to_string(),
        "🔁 换源（替换工程文件）".to_string(),
        "🎲 重新生成分享码".to_string(),
        "👁 发布/隐藏切换".to_string(),
        "🔒 保护/解除保护".to_string(),
        "🗑 删除".to_string(),
        "🔙 返回".to_string(),
    ];
    let sel = select("操作", &items, 7)?;
    match sel {
        0 => {
            let title = input::<String>("标题", Some(v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string()))?;
            let description = input::<String>("简介", Some(v.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string()))?;
            let tags_raw = input::<String>("标签（逗号分隔）", Some(String::new()))?;
            let tags: Vec<String> = tags_raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            client.patch(&format!("/api/me/projects/{}", id), json!({ "title": title, "description": description, "tags": tags }))?.ok_void()?;
            println!("已保存");
        }
        1 => {
            let days = input::<String>("过期天数（留空 = 永久）", Some(String::new()))?;
            let expires_at = if days.trim().is_empty() {
                None
            } else {
                let d: i64 = days.trim().parse().unwrap_or(0);
                let ts = chrono::Utc::now().timestamp_millis() + d * 86_400_000;
                Some(chrono::DateTime::from_timestamp_millis(ts).map(|x| x.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default())
            };
            client.put(&format!("/api/me/projects/{}/expiry", id), json!({ "expiresAt": expires_at }))?.ok_void()?;
            println!("已设置");
        }
        2 => {
            let path = input::<String>("工程 JSON 文件路径", None::<String>)?;
            let text = std::fs::read_to_string(&path).map_err(|e| anyhow::anyhow!("读取失败：{}", e))?;
            client.put(&format!("/api/me/projects/{}/file", id), json!({ "fileText": text }))?.ok_void()?;
            println!("换源成功");
        }
        3 => {
            let resp = client.post(&format!("/api/me/projects/{}/regenerate-code", id), json!({}))?;
            let v = resp.ok_json()?;
            println!("新分享码：{}", v.get("code").and_then(|x| x.as_str()).unwrap_or("?"));
        }
        4 => {
            client.post(&format!("/api/me/projects/{}/toggle-publish", id), json!({}))?.ok_void()?;
            println!("已切换");
        }
        5 => {
            let current = v.get("protected").and_then(|x| x.as_bool()).unwrap_or(false);
            client.put(&format!("/api/me/projects/{}/protected", id), json!({ "protected": !current }))?.ok_void()?;
            println!("已切换保护状态");
        }
        6 => {
            if confirm("确认删除该工程？此操作不可恢复")? {
                client.delete(&format!("/api/me/projects/{}", id))?.ok_void()?;
                println!("已删除");
                return Ok(true);
            }
        }
        _ => {}
    }
    Ok(false)
}

// ── Buff 集 ─────────────────────────────────────────────

const ENTITY_TYPES: [&str; 9] = ["全部", "character", "weapon", "echo", "1set", "2set", "3set", "4set", "5set"];

fn buff_menu(client: &Client) -> Result<()> {
    loop {
        let items = [
            "📦 Buff 集数据".to_string(),
            "🌐 上游名录（nanoka.cc）".to_string(),
            "🔙 返回".to_string(),
        ];
        let sel = select("Buff 集", &items, 0)?;
        match sel {
            0 => buff_data_menu(client)?,
            1 => catalog_menu(client)?,
            _ => return Ok(()),
        }
    }
}

fn buff_data_menu(client: &Client) -> Result<()> {
    loop {
        let sel = select("Buff 集数据", &ENTITY_TYPES.map(|s| s.to_string()), 0)?;
        if sel == 0 {
            buff_entity_list(client, None)?;
        } else if sel < ENTITY_TYPES.len() {
            buff_entity_list(client, Some(ENTITY_TYPES[sel]))?;
        } else {
            return Ok(());
        }
    }
}

/// 上游名录浏览（角色/武器/首位声骸/套装，来自 nanoka.cc）
fn catalog_menu(client: &Client) -> Result<()> {
    const TYPES: [&str; 4] = ["characters", "weapons", "echoes", "sets"];
    const LABELS: [&str; 4] = ["角色", "武器", "首位声骸", "套装"];
    loop {
        let mut type_items: Vec<String> = LABELS.iter().map(|l| l.to_string()).collect();
        type_items.push("🔙 返回".to_string());
        let sel = select("上游名录", &type_items, 0)?;
        if sel >= TYPES.len() {
            return Ok(());
        }
        let t = TYPES[sel];
        let q = input::<String>(
            &format!("搜索{}名称（留空 = 全部）", LABELS[sel]),
            Some(String::new()),
        )?;
        let resp = client.get(&format!("/api/catalog/{}", t), &[])?;
        let v = match resp.ok_json() {
            Ok(v) => v,
            Err(e) => {
                println!("获取失败：{}", e);
                wait_enter()?;
                continue;
            }
        };
        let items_arr = v.get(t).and_then(|x| x.as_array()).cloned().unwrap_or_default();
        let ql = q.trim().to_lowercase();
        let filtered: Vec<Value> = items_arr
            .into_iter()
            .filter(|i| {
                let name = i.get("name").and_then(|x| x.as_str()).unwrap_or("");
                ql.is_empty() || name.to_lowercase().contains(&ql)
            })
            .collect();
        if filtered.is_empty() {
            println!("（无匹配条目）");
            wait_enter()?;
            continue;
        }
        let mut menu: Vec<String> = filtered.iter().map(|i| catalog_item_label(t, i)).collect();
        menu.push("🔙 返回".to_string());
        let sel2 = select(
            &format!("{}名录（{} 条，ww {}）", LABELS[sel], filtered.len(), v.get("version").and_then(|x| x.as_str()).unwrap_or("?")),
            &menu,
            0,
        )?;
        if sel2 == menu.len() - 1 {
            continue;
        }
        let item = &filtered[sel2];
        println!("────────────────────────────");
        println!("{}", catalog_item_label(t, item));
        if t == "echoes" {
            if let Some(sets) = item.get("sets").and_then(|x| x.as_array()) {
                let names: Vec<&str> = sets.iter().filter_map(|s| s.as_str()).collect();
                if !names.is_empty() {
                    println!("所属套装：{}", names.join("、"));
                }
            }
        }
        if t == "sets" {
            if let Some(pieces) = item.get("pieces").and_then(|x| x.as_array()) {
                let ps: Vec<String> = pieces
                    .iter()
                    .filter_map(|p| p.as_i64().map(|v| v.to_string()))
                    .collect();
                if !ps.is_empty() {
                    println!("套装件数：{}", ps.join("/"));
                }
            }
        }
        println!("────────────────────────────");
        wait_enter()?;
    }
}

fn catalog_item_label(t: &str, i: &Value) -> String {
    let name = i.get("name").and_then(|x| x.as_str()).unwrap_or("");
    match t {
        "characters" => format!(
            "{}（{}星 {} {}）",
            name,
            i.get("star").and_then(|x| x.as_i64()).unwrap_or(0),
            i.get("element").and_then(|x| x.as_str()).unwrap_or(""),
            i.get("weaponType").and_then(|x| x.as_str()).unwrap_or("")
        ),
        "weapons" => format!(
            "{}（{}星 {}）",
            name,
            i.get("star").and_then(|x| x.as_i64()).unwrap_or(0),
            i.get("weaponType").and_then(|x| x.as_str()).unwrap_or("")
        ),
        "echoes" => {
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
                name,
                i.get("cost").and_then(|x| x.as_i64()).unwrap_or(0),
                if sets.is_empty() { String::new() } else { format!("【{}】", sets) }
            )
        }
        _ => format!(
            "{}（{}件套）",
            name,
            i.get("pieces")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p.as_i64().map(|v| v.to_string()))
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .unwrap_or_default()
        ),
    }
}

fn buff_entity_list(client: &Client, entity_type: Option<&str>) -> Result<()> {
    let mut query: Vec<(&str, &str)> = Vec::new();
    if let Some(t) = entity_type {
        query.push(("entity_type", t));
    }
    let v = client.get("/api/buff-sets", &query)?.ok_json()?;
    let rows = v.get("buffSets").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    // 按 实体类型｜实体名 分组
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for r in &rows {
        let key = format!(
            "{}｜{}",
            r.get("entity_type").and_then(|x| x.as_str()).unwrap_or(""),
            r.get("entity_name").and_then(|x| x.as_str()).unwrap_or("")
        );
        match groups.last_mut() {
            Some((k, list)) if *k == key => list.push(r.clone()),
            _ => groups.push((key, vec![r.clone()])),
        }
    }
    if groups.is_empty() {
        println!("（该类型暂无数据）");
        wait_enter()?;
        return Ok(());
    }
    let mut items: Vec<String> = groups.iter().map(|(k, list)| format!("{}（{} 条）", k, list.len())).collect();
    items.push("🔙 返回".to_string());
    let sel = select("选择实体", &items, 0)?;
    if sel >= groups.len() {
        return Ok(());
    }
    let (_, buffs) = &groups[sel];
    let mut buff_items: Vec<String> = buffs.iter().map(|b| {
        let name = b.get("buff_name").and_then(|x| x.as_str()).unwrap_or("?");
        let zones = b.get("buff_set").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
        format!("{}（{} 项增益）", name, zones)
    }).collect();
    buff_items.push("🔙 返回".to_string());
    let sel2 = select("选择 Buff", &buff_items, 0)?;
    if sel2 < buffs.len() {
        let b = &buffs[sel2];
        println!("────────────────────────────");
        println!("{}", b.get("buff_name").and_then(|x| x.as_str()).unwrap_or(""));
        println!("scope：{}", b.get("scope").and_then(|x| x.as_str()).unwrap_or(""));
        if let Some(c) = b.get("condition") {
            if !c.is_null() {
                println!("条件：{}", c);
            }
        }
        if let Some(zones) = b.get("buff_set").and_then(|x| x.as_array()) {
            for z in zones {
                let zid = z.get("zoneId").and_then(|x| x.as_str()).unwrap_or("");
                let val = z.get("value").and_then(|x| x.as_f64()).unwrap_or(0.0);
                println!("  {} = {}", zid, val);
            }
        }
        println!("────────────────────────────");
        wait_enter()?;
    }
    Ok(())
}

// ── 管理 ────────────────────────────────────────────────

fn admin_menu(client: &Client) -> Result<()> {
    if !ensure_login(client)? {
        return Ok(());
    }
    let mut client = client.clone();
    client.token = config::load_config().token.or(client.token);
    loop {
        let items = [
            "👥 管理员列表".to_string(),
            "➕ 授权管理员".to_string(),
            "➖ 撤销管理员".to_string(),
            "📋 工程管理".to_string(),
            "🧹 清空用户工程".to_string(),
            "📸 快照管理".to_string(),
            "📢 公告管理".to_string(),
            "⏰ 清理过期工程".to_string(),
            "🧪 写入示例数据".to_string(),
            "🔙 返回".to_string(),
        ];
        let sel = select("管理", &items, 9)?;
        match sel {
            0 => {
                let v = client.get("/api/admin/users", &[])?.ok_json()?;
                let admins = v.get("admins").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                for a in &admins {
                    let name = a.get("username").and_then(|x| x.as_str()).unwrap_or("?");
                    match a.get("grantedBy").and_then(|x| x.as_str()) {
                        Some(g) => println!("{}（由 {} 授权）", name, g),
                        None => println!("{}（根管理员）", name),
                    }
                }
                wait_enter()?;
            }
            1 => {
                let username = input::<String>("要授权的用户名", None::<String>)?;
                match client.post(&format!("/api/admin/users/{}/grant", username), json!({})) {
                    Ok(r) => {
                        let v = r.ok_json()?;
                        println!("{}", v.get("message").and_then(|x| x.as_str()).unwrap_or("已授权"));
                    }
                    Err(e) => println!("{}", e),
                }
                wait_enter()?;
            }
            2 => {
                let username = input::<String>("要撤销的用户名", None::<String>)?;
                match client.post(&format!("/api/admin/users/{}/revoke", username), json!({})) {
                    Ok(r) => {
                        let v = r.ok_json()?;
                        println!("{}", v.get("message").and_then(|x| x.as_str()).unwrap_or("已撤销"));
                    }
                    Err(e) => println!("{}", e),
                }
                wait_enter()?;
            }
            3 => admin_projects_menu(&mut client)?,
            4 => {
                let username = input::<String>("清空哪个用户的全部工程", None::<String>)?;
                if confirm("确认删除该用户全部工程（保护工程豁免）？")? {
                    match client.delete(&format!("/api/admin/users/{}/content", username)) {
                        Ok(r) => {
                            let v = r.ok_json()?;
                            println!("已删除 {} 个工程", v.get("deletedProjects").and_then(|x| x.as_i64()).unwrap_or(0));
                        }
                        Err(e) => println!("{}", e),
                    }
                }
                wait_enter()?;
            }
            5 => snapshots_menu(&mut client)?,
            6 => announcements_admin_menu(&mut client)?,
            7 => {
                let v = client.post("/api/admin/cleanup", json!({}))?.ok_json()?;
                println!("已清理 {} 个过期工程", v.get("deleted").and_then(|x| x.as_i64()).unwrap_or(0));
                wait_enter()?;
            }
            8 => {
                match client.post("/api/admin/demo", json!({})) {
                    Ok(r) => {
                        let v = r.ok_json()?;
                        println!("{}", v.get("message").and_then(|x| x.as_str()).unwrap_or("已写入"));
                    }
                    Err(e) => println!("{}", e),
                }
                wait_enter()?;
            }
            _ => break,
        }
    }
    Ok(())
}

fn admin_projects_menu(client: &mut Client) -> Result<()> {
    let mut page: i64 = 1;
    loop {
        let v = client.get("/api/admin/projects", &[("page", &page.to_string()), ("pageSize", "20")])?.ok_json()?;
        let items = v.get("items").and_then(|x| x.as_array()).cloned().unwrap_or_default();
        let total = v.get("total").and_then(|x| x.as_i64()).unwrap_or(0);
        let mut menu: Vec<String> = Vec::new();
        menu.push(format!("⬅ 上一页（共 {} 个工程）", total));
        menu.push("➡ 下一页".to_string());
        for (i, p) in items.iter().enumerate() {
            let title = p.get("title").and_then(|x| x.as_str()).unwrap_or("?");
            let author = p.get("author_name").and_then(|x| x.as_str()).unwrap_or("?");
            let code = p.get("code").and_then(|x| x.as_str()).unwrap_or("");
            menu.push(format!("[{}] {}（{}）作者：{}", (page - 1) * 20 + i as i64 + 1, title, code, author));
        }
        menu.push("🔙 返回".to_string());
        let sel = select("工程管理", &menu, 0)?;
        match sel {
            0 => {
                if page > 1 {
                    page -= 1;
                }
            }
            1 => page += 1,
            idx if idx >= 2 && idx < 2 + items.len() => {
                let p = &items[idx - 2];
                let id = p.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let title = input::<String>("改标题（留空不变）", Some(String::new()))?;
                let author = input::<String>("改作者名（留空不变）", Some(String::new()))?;
                let mut body = json!({});
                if !title.trim().is_empty() {
                    body["title"] = json!(title.trim());
                }
                if !author.trim().is_empty() {
                    body["authorName"] = json!(author.trim());
                }
                if !body.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                    client.patch(&format!("/api/admin/projects/{}", id), body)?.ok_void()?;
                    println!("已更新");
                }
                if confirm("删除该工程？")? {
                    match client.delete(&format!("/api/admin/projects/{}", id)) {
                        Ok(r) => r.ok_void()?,
                        Err(e) => println!("{}", e),
                    }
                }
            }
            _ => break,
        }
    }
    Ok(())
}

fn snapshots_menu(client: &mut Client) -> Result<()> {
    loop {
        let v = client.get("/api/admin/snapshots", &[])?.ok_json()?;
        let snaps = v.get("snapshots").and_then(|x| x.as_array()).cloned().unwrap_or_default();
        let mut menu: Vec<String> = Vec::new();
        menu.push("➕ 创建快照".to_string());
        for s in &snaps {
            let id = s.get("id").and_then(|x| x.as_str()).unwrap_or("");
            let note = s.get("note").and_then(|x| x.as_str()).unwrap_or("");
            let root = if s.get("isRoot").and_then(|x| x.as_bool()).unwrap_or(false) { "根" } else { "版" };
            let latest = if s.get("isLatest").and_then(|x| x.as_bool()).unwrap_or(false) { "（最新）" } else { "" };
            let created = s
                .get("createdAt")
                .and_then(|x| x.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| time_ago(d.timestamp_millis()))
                .unwrap_or_default();
            menu.push(format!("[{}] {}{} {} ｜ {}", root, note, latest, created, id));
        }
        menu.push("🔙 返回".to_string());
        let sel = select("快照管理", &menu, 0)?;
        if sel == 0 {
            let note = input::<String>("备注（可留空）", Some(String::new()))?;
            match client.post("/api/admin/snapshots", json!({ "note": note })) {
                Ok(r) => {
                    let v = r.ok_json()?;
                    println!("已创建{}快照（{} 行）", if v.get("mode").and_then(|x| x.as_str()).unwrap_or("") == "root" { "根" } else { "版本" }, v.get("rows").and_then(|x| x.as_i64()).unwrap_or(0));
                }
                Err(e) => println!("{}", e),
            }
            continue;
        }
        if sel == menu.len() - 1 {
            break;
        }
        let s = &snaps[sel - 1];
        let id = s.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let actions = [
            "📊 对比当前差异".to_string(),
            "♻ 恢复到该快照".to_string(),
            "🗑 删除（仅最新版本）".to_string(),
            "🔙 返回".to_string(),
        ];
        let a = select("快照操作", &actions, 3)?;
        match a {
            0 => {
                match client.get(&format!("/api/admin/snapshots/{}/diff", id), &[]) {
                    Ok(r) => {
                        let v = r.ok_json()?;
                        let d = v.get("diff").cloned().unwrap_or(Value::Null);
                        println!(
                            "差异：新增 {}，修改 {}，删除 {}",
                            d.get("added").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
                            d.get("modified").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
                            d.get("removed").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
                        );
                    }
                    Err(e) => println!("{}", e),
                }
                wait_enter()?;
            }
            1 => {
                if confirm("确认恢复到该快照？（将重写整个 Buff 集）")? {
                    match client.post(&format!("/api/admin/snapshots/{}/restore", id), json!({})) {
                        Ok(r) => {
                            let v = r.ok_json()?;
                            println!("已恢复 {} 条", v.get("restored").and_then(|x| x.as_i64()).unwrap_or(0));
                        }
                        Err(e) => println!("{}", e),
                    }
                }
            }
            2 => {
                match client.delete(&format!("/api/admin/snapshots/{}", id)) {
                    Ok(r) => {
                        let v = r.ok_json()?;
                        println!("{}", v.get("message").and_then(|x| x.as_str()).unwrap_or("已删除"));
                    }
                    Err(e) => println!("{}", e),
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn announcements_admin_menu(client: &mut Client) -> Result<()> {
    loop {
        let v = client.get("/api/announcements", &[])?.ok_json()?;
        let rows = v.get("announcements").and_then(|x| x.as_array()).cloned().unwrap_or_default();
        let mut menu: Vec<String> = Vec::new();
        menu.push("➕ 新建公告".to_string());
        for (i, a) in rows.iter().enumerate() {
            menu.push(format!(
                "[{}] {}（{}）",
                i + 1,
                a.get("title").and_then(|x| x.as_str()).unwrap_or(""),
                a.get("createdAt").and_then(|x| x.as_str()).unwrap_or("")
            ));
        }
        menu.push("🔙 返回".to_string());
        let sel = select("公告管理", &menu, 0)?;
        if sel == 0 {
            let title = input::<String>("标题", None::<String>)?;
            let content = input::<String>("内容", None::<String>)?;
            match client.post("/api/admin/announcements", json!({ "title": title, "content": content })) {
                Ok(r) => r.ok_void()?,
                Err(e) => println!("{}", e),
            }
            continue;
        }
        if sel == menu.len() - 1 {
            return Ok(());
        }
        let a = &rows[sel - 1];
        let id = a.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        println!("{}", a.get("content").and_then(|x| x.as_str()).unwrap_or(""));
        if confirm("删除该公告？")? {
            match client.delete(&format!("/api/admin/announcements/{}", id)) {
                Ok(r) => r.ok_void()?,
                Err(e) => println!("{}", e),
            }
        }
    }
}

// ── 公告（公开）─────────────────────────────────────────

fn announcements_menu(client: &Client) -> Result<()> {
    let v = client.get("/api/announcements", &[])?.ok_json()?;
    let rows = v.get("announcements").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    if rows.is_empty() {
        println!("（暂无公告）");
    }
    for (i, a) in rows.iter().enumerate() {
        println!("────────── [{}] {} ──────────", i + 1, a.get("title").and_then(|x| x.as_str()).unwrap_or(""));
        println!("{}", a.get("content").and_then(|x| x.as_str()).unwrap_or(""));
        println!("（{}）", a.get("createdAt").and_then(|x| x.as_str()).unwrap_or(""));
    }
    wait_enter()?;
    Ok(())
}

// ── 上传 ────────────────────────────────────────────────

fn upload_menu(client: &Client) -> Result<()> {
    let logged_in = client.token.is_some() || crate::client::is_loopback_url(&client.base);
    if !logged_in {
        let items = vec!["匿名分享（10 分钟有效）".to_string(), "登录后发布".to_string(), "返回".to_string()];
        let sel = select("未登录", &items, 0)?;
        match sel {
            0 => {}
            1 => {
                if !ensure_login(client)? {
                    return Ok(());
                }
            }
            _ => return Ok(()),
        }
    }
    let file = input::<String>("工程 JSON 文件路径", None::<String>)?;
    let text = match std::fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) => {
            println!("读取失败：{}", e);
            return Ok(());
        }
    };
    let description = input::<String>("简介（可留空）", Some(String::new()))?;
    let tags_raw = input::<String>("标签（逗号分隔，可留空）", Some(String::new()))?;
    let tags: Vec<String> = tags_raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let days_raw = input::<String>("过期天数（留空 = 永久）", Some(String::new()))?;
    let days: Option<i64> = days_raw.trim().parse().ok();

    let mut client2 = client.clone();
    client2.token = config::load_config().token.or(client2.token);

    let result = if client2.token.is_some() || crate::client::is_loopback_url(&client2.base) {
        client2.post(
            "/api/me/projects",
            json!({ "fileText": text, "description": description, "tags": tags, "expiresDays": days }),
        )
    } else {
        client2.post("/api/public/projects", json!({ "fileText": text }))
    };
    match result {
        Ok(r) => {
            let v = r.ok_json()?;
            let code = v.get("code").and_then(|x| x.as_str()).unwrap_or("?");
            let url = v.get("url").and_then(|x| x.as_str()).unwrap_or("");
            println!("分享成功：码 {} ｜ {}", code, url);
        }
        Err(e) => println!("分享失败：{}", e),
    }
    wait_enter()?;
    Ok(())
}

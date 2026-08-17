// TUI：JSON 脚本驱动（由数据目录 tui-script.json 定义）
//
// 脚本格式（参考 defaults/tui-script.json）：
// {
//   "title": "...", "author": "root_admin", "description": "...",
//   "menus": [
//     { "label": "...", "action": { "type": "sql", "sql": "...", "params": [...] } },
//     { "label": "...", "action": { "type": "http", "method": "GET", "path": "/api/..." } },
//     { "label": "...", "action": { "type": "command", "cmd": "..." } },
//     { "label": "退出", "exit": true }
//   ]
// }
//
// 字符串中的 {名称} 占位符会在动作执行前提示输入；留空=跳过该占位符。
// sql 动作直接对该项目的 SQLite 数据库运行；http 动作通过当前 Client 调用服务器；
// command 动作通过 std::process::Command 派生子进程（仅 Windows / 本机安全）。

use std::path::PathBuf;

use anyhow::{bail, Result};
use dialoguer::{Confirm, Input, Select};
use serde_json::Value;

use crate::assets;
use crate::client::{is_loopback_url, Client};

#[derive(Debug)]
struct MenuItem {
    label: String,
    exit: bool,
    action: Option<Action>,
}

#[derive(Debug, Clone)]
enum Action {
    Sql {
        sql: String,
        params: Vec<String>,
        description: Option<String>,
    },
    Http {
        method: String,
        path: String,
        body_template: Option<String>,
        params: Vec<String>,
        description: Option<String>,
    },
    Command {
        cmd: String,
        params: Vec<String>,
        description: Option<String>,
    },
    /// 通过「导出格式 SQL 文件」批量导入 Buff 集
    ImportBuff {
        params: Vec<String>,
        description: Option<String>,
    },
}

/// 占位符 {name} 提取，并按出现顺序去重
fn extract_placeholders(template: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while let Some(open) = template[i..].find('{') {
        let abs = i + open;
        if let Some(close) = template[abs..].find('}') {
            let name = &template[abs + 1..abs + close];
            let j = abs + close + 1;
            // 跳过 {{/}} 等（同 syntaxsql 简易处理：仅单层 {x}）
            i = j;
            // 去重保序
            if name != "_" && !out.contains(&name.to_string()) && !name.is_empty() {
                out.push(name.to_string());
            }
        } else {
            break;
        }
    }
    out
}

/// 询问用户输入占位符值，得到 fill: {占位名 => 值}
fn prompt_placeholders(names: Vec<String>) -> Result<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    for name in names {
        // 跳过 sql/http/command 关键字字段的形式名（避免误把 type=sql 等看作参数）。通常用户参数名非这些。
        let raw: String = Input::new()
            .with_prompt(&name)
            .default(String::new())
            .interact_text()?;
        if !raw.is_empty() {
            map.insert(name, raw);
        }
    }
    Ok(map)
}

/// 替换 `{name}` 为 map 中的值（简单的非贪婪匹配，重复名只替换一次）
fn apply_placeholders(template: &str, map: &std::collections::HashMap<String, String>) -> String {
    let mut out = template.to_string();
    // 反复扫描直到没有更多替换（处理 {a}{b}{a}）
    loop {
        let mut changed = false;
        for (k, v) in map.iter() {
            // 转义 curl/url 等可能含义的字符在调用方另行 URL encode
            let pat = format!("{{{}}}", k);
            if out.contains(&pat) {
                out = out.replace(&pat, v);
                changed = true;
            }
        }
        if !changed { break; }
    }
    out
}

/// 解析脚本
fn parse_script(json: &Value) -> Result<(String, String, Option<String>, Vec<MenuItem>)> {
    let title = json.get("title").and_then(|v| v.as_str()).unwrap_or("Lite TUI").to_string();
    let author = json.get("author").and_then(|v| v.as_str()).unwrap_or("root_admin").to_string();
    let description = json.get("description").and_then(|v| v.as_str()).map(String::from);
    let menus_v = json.get("menus").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut items = Vec::new();
    for m in &menus_v {
        let obj = m.as_object().ok_or_else(|| anyhow::anyhow!("菜单项必须为对象"))?;
        let label = obj
            .get("label")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("菜单项缺少 label"))?
            .to_string();
        let exit = obj.get("exit").and_then(|v| v.as_bool()).unwrap_or(false);
        let action = if obj.get("action").is_some() {
            let a = obj.get("action").unwrap();
            let typ = a.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let description = a.get("description").and_then(|v| v.as_str()).map(String::from);
            match typ.as_str() {
                "sql" => {
                    let sql = a
                        .get("sql")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("sql 动作缺少 sql 字段"))?
                        .to_string();
                    let mut params: Vec<String> = a
                        .get("params")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    // 额外提取 sql 中的 {name} 占位
                    for n in extract_placeholders(&sql) {
                        if !params.contains(&n) { params.push(n); }
                    }
                    Some(Action::Sql { sql, params, description })
                }
                "http" => {
                    let method = a
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("GET")
                        .to_uppercase();
                    let path = a
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("http 动作缺少 path 字段"))?
                        .to_string();
                    let body_template = a
                        .get("body")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let mut params: Vec<String> = a
                        .get("params")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    for n in extract_placeholders(&path) {
                        if !params.contains(&n) { params.push(n); }
                    }
                    if let Some(ref b) = body_template {
                        for n in extract_placeholders(b) {
                            if !params.contains(&n) { params.push(n); }
                        }
                    }
                    Some(Action::Http { method, path, body_template, params, description })
                }
                "command" => {
                    let cmd = a
                        .get("cmd")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("command 动作缺少 cmd 字段"))?
                        .to_string();
                    let mut params: Vec<String> = a
                        .get("params")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    for n in extract_placeholders(&cmd) {
                        if !params.contains(&n) { params.push(n); }
                    }
                    Some(Action::Command { cmd, params, description })
                }
                other => match typ.as_str() {
                    "import-buff-sql" => {
                        // 输入导出格式 SQL 文件路径，批量导入 Buff 集
                        let mut params: Vec<String> = a
                            .get("params")
                            .and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
                            .unwrap_or_default();
                        // 默认参数：先问文件路径
                        if params.is_empty() {
                            params.push("SQL文件路径".into());
                        }
                        Some(Action::ImportBuff { params, description })
                    }
                    _ => bail!("不支持的动作类型：{}（支持 sql / http / command / import-buff-sql）", other),
                }
            }
        } else { None };
        items.push(MenuItem { label, exit, action });
    }
    Ok((title, author, description, items))
}

/// 打开数据库连接（读写打开 app_dir 下的 share.db）。SQL 动作直接对它执行。
/// TUI 以脚本作者身份（默认 root_admin，或本机 Root）运行，管理员可写。
fn open_conn(app_dir: &std::path::Path) -> Result<rusqlite::Connection> {
    let path = app_dir.join("share.db");
    if !path.exists() {
        bail!("数据库文件不存在：{}（先启动过 `lite serve` 或 `lite tui` 创建）", path.display());
    }
    rusqlite::Connection::open(&path)
        .map_err(|e| anyhow::anyhow!("打开数据库失败：{}", e))
}

/// TUI 以脚本 author 身份作为当前用户；本机 / root_admin = 管理员。
fn script_user(author: &str) -> crate::types::UserCtx {
    crate::types::UserCtx {
        id: author.to_string(),
        username: author.to_string(),
        is_admin: author == "root_admin",
    }
}

/// 执行一条 SQL 并通过 sql_engine 打印结果。
fn sql_run_for(user: &crate::types::UserCtx, sql: &str, app_dir: &std::path::Path) -> Result<()> {
    let conn = open_conn(app_dir)?;
    let r = crate::sql_engine::run_sql_on(user, sql, &conn)?;
    crate::sql_engine::print_sql_result(&r);
    Ok(())
}

/// 执行一个动作（用户 = 脚本 author 身份，root_admin 可写）
fn run_action(client: &Client, app_dir: &std::path::Path, user: &crate::types::UserCtx, action: &Action) -> Result<()> {
    match action {
        Action::Sql { sql, params, description } => {
            if let Some(desc) = description {
                println!("—— {} ——", desc);
            }
            let map = if !params.is_empty() {
                prompt_placeholders(params.clone())?
            } else { std::collections::HashMap::new() };
            let final_sql = apply_placeholders(sql, &map);
            println!("SQL: {}", final_sql);
            sql_run_for(user, &final_sql, app_dir)
        }
        Action::ImportBuff { params, description } => {
            if let Some(desc) = description {
                println!("—— {} ——", desc);
            }
            if !user.is_admin {
                bail!("仅为管理员可导入 Buff 集（当前身份 {} 非管理员）", user.username);
            }
            let map = prompt_placeholders(params.clone())?;
            // 文件路径占位（默认任务名）
            let path_raw = map.get("SQL 文件路径").or_else(|| map.get("SQL文件路径")).or_else(|| map.values().next());
            let Some(path_raw) = path_raw else {
                bail!("请提供 Buff 集导出 SQL 文件路径");
            };
            let p = std::path::PathBuf::from(path_raw);
            if !p.exists() {
                bail!("SQL 文件不存在：{}", p.display());
            }
            let text = std::fs::read_to_string(&p)
                .map_err(|e| anyhow::anyhow!("读取 SQL 文件失败：{}", e))?;
            let conn = open_conn(app_dir)?;
            let n = crate::import_buff::import_from_sql(&conn, &text)?;
            println!("已批量导入 {} 条 Buff 集", n);
            Ok(())
        }
        Action::Http { method, path, body_template, params, description } => {
            if let Some(desc) = description {
                println!("—— {} ——", desc);
            }
            let map = if !params.is_empty() {
                prompt_placeholders(params.clone())?
            } else { std::collections::HashMap::new() };
            let final_path = apply_placeholders(path, &map);
            let final_body_str: Option<String> = body_template
                .as_ref()
                .map(|b| apply_placeholders(b, &map));
            let body_value: Option<Value> = final_body_str.as_ref().map(|s| {
                serde_json::from_str::<Value>(s).unwrap_or(Value::String(s.clone()))
            });
            println!("→ {} {}", method, final_path);
            let resp = match method.as_str() {
                "GET" => client.get(&final_path, &[])?,
                "DELETE" => client.delete(&final_path)?,
                "POST" | "PUT" | "PATCH" => {
                    let v = body_value.unwrap_or(Value::Object(Default::default()));
                    match method.as_str() {
                        "POST" => client.post(&final_path, v)?,
                        "PUT" => client.put(&final_path, v)?,
                        _ => client.patch(&final_path, v)?,
                    }
                }
                other => bail!("不支持的 HTTP 方法：{}", other),
            };
            println!("HTTP {} {}", resp.status, final_path);
            if !(200..300).contains(&resp.status) {
                println!("{}", resp.error_msg());
            } else {
                match resp.json() {
                    Some(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
                    None => println!("（无 JSON 响应）"),
                }
            }
            Ok(())
        }
        Action::Command { cmd, params, description } => {
            // command 仅允许本机连接
            if !is_loopback_url(&client.base) {
                bail!("command 动作仅允许本机（localhost）执行");
            }
            if let Some(desc) = description {
                println!("—— {} ——", desc);
            }
            let map = if !params.is_empty() {
                prompt_placeholders(params.clone())?
            } else { std::collections::HashMap::new() };
            let final_cmd = apply_placeholders(cmd, &map);
            println!("$ {}", final_cmd);
            #[cfg(windows)]
            let status = std::process::Command::new("cmd")
                .arg("/C")
                .arg(&final_cmd)
                .status()?;
            #[cfg(not(windows))]
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(&final_cmd)
                .status()?;
            println!("（命令退出状态 {:?}）", status.code());
            Ok(())
        }
    }
}

pub fn run(client: &Client, app_dir: PathBuf) -> Result<()> {
    // 首次运行：把默认模板拷到数据目录
    let _ = assets::ensure_defaults(&app_dir);

    let json_str = assets::tui_script_content(&app_dir)?;
    let root: Value = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("tui-script.json 解析失败：{}（用 `lite reset-templates` 恢复默认）", e))?;
    let (title, author, _description, items) = parse_script(&root)?;
    if items.is_empty() {
        bail!("tui-script.json 中没有菜单项");
    }
    // 以脚本 author 身份运行（root_admin = 管理员可写）
    let user = script_user(&author);

    loop {
        let labels: Vec<String> = items.iter().map(|m| m.label.clone()).collect();
        let sel = Select::new().with_prompt(&title).items(&labels).default(0).interact()?;
        let item = &items[sel];
        if item.exit {
            return Ok(());
        }
        if let Some(action) = &item.action {
            println!();
            if let Err(e) = run_action(client, &app_dir, &user, action) {
                println!("错误：{}", e);
            }
        } else {
            println!("（该菜单项没有动作）");
        }
        // 提示继续
        let _ = Confirm::new().with_prompt("回车继续...").default(true).interact();
    }
}

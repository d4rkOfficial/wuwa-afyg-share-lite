// CLI 的 HTTP 客户端（ureq；可对接本地服务或原版云端服务）

use anyhow::{bail, Result};
use serde_json::Value;

pub struct Resp {
    pub status: u16,
    pub text: String,
}

impl Resp {
    pub fn json(&self) -> Option<Value> {
        serde_json::from_str(&self.text).ok()
    }

    pub fn error_msg(&self) -> String {
        self.json()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| {
                if self.text.is_empty() {
                    format!("HTTP {}", self.status)
                } else {
                    self.text.clone()
                }
            })
    }

    /// 2xx 时返回 JSON；否则返回 {error} 消息
    pub fn ok_json(&self) -> Result<Value> {
        if (200..300).contains(&self.status) {
            self.json().ok_or_else(|| anyhow::anyhow!("响应不是合法 JSON"))
        } else {
            bail!(self.error_msg())
        }
    }

    pub fn ok_void(&self) -> Result<()> {
        if (200..300).contains(&self.status) {
            Ok(())
        } else {
            bail!(self.error_msg())
        }
    }
}

/// 判断服务器地址是否为本机（localhost / 回环 IP）
pub fn is_loopback_url(base: &str) -> bool {
    let u = base.trim_end_matches('/');
    let host = u.split("://").nth(1).unwrap_or(u);
    let host = host.split(['/', ':']).next().unwrap_or(host);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host == "::1" || host.starts_with("127.")
}

#[derive(Clone)]
pub struct Client {
    pub base: String,
    pub token: Option<String>,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(base: &str, token: Option<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(60))
            .build();
        Client {
            base: base.trim_end_matches('/').to_string(),
            token,
            agent,
        }
    }

    fn req(&self, method: &str, path: &str) -> ureq::Request {
        let mut req = self.agent.request(method, &format!("{}{}", self.base, path));
        if let Some(t) = &self.token {
            req = req.set("Authorization", &format!("Bearer {}", t));
        }
        req
    }

    /// 通用请求：query 为查询参数，body 为 JSON 请求体
    pub fn request(
        &self,
        method: &str,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Resp> {
        let mut req = self.req(method, path);
        for (k, v) in query {
            req = req.query(k, v);
        }
        let resp = match &body {
            Some(b) => req.send_json(b),
            None => req.call(),
        };
        match resp {
            Ok(r) => {
                let status = r.status();
                let text = r.into_string().unwrap_or_default();
                Ok(Resp { status, text })
            }
            Err(ureq::Error::Status(code, r)) => {
                let text = r.into_string().unwrap_or_default();
                Ok(Resp { status: code, text })
            }
            Err(ureq::Error::Transport(t)) => {
                bail!("无法连接服务器 {}：{}", self.base, t)
            }
        }
    }

    pub fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Resp> {
        self.request("GET", path, query, None)
    }

    pub fn post(&self, path: &str, body: Value) -> Result<Resp> {
        self.request("POST", path, &[], Some(body))
    }

    pub fn patch(&self, path: &str, body: Value) -> Result<Resp> {
        self.request("PATCH", path, &[], Some(body))
    }

    pub fn put(&self, path: &str, body: Value) -> Result<Resp> {
        self.request("PUT", path, &[], Some(body))
    }

    pub fn delete(&self, path: &str) -> Result<Resp> {
        self.request("DELETE", path, &[], None)
    }
}

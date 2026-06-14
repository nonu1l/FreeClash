use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use uuid::Uuid;

use crate::metrics::RuleMetrics;
use crate::mihomo::{group_name, render_config};
use crate::models::{
    AppConfig, AppRule, AppSnapshot, DelayResult, NodeInfo, RuleInput, RuleStats, RuntimeStatus,
};
use crate::proxy::start_meter_proxy;

#[derive(Clone)]
pub struct AppManager {
    paths: Arc<AppPaths>,
    inner: Arc<Mutex<InnerState>>,
    client: Client,
}

#[derive(Debug)]
struct AppPaths {
    runtime_dir: PathBuf,
    config_path: PathBuf,
    app_config_path: PathBuf,
    core_path: PathBuf,
}

struct InnerState {
    config: AppConfig,
    core: Option<Child>,
    meter_tasks: HashMap<String, JoinHandle<()>>,
    metrics: HashMap<String, Arc<StdMutex<RuleMetrics>>>,
    launched_apps: HashMap<String, std::process::Child>,
    nodes: Vec<NodeInfo>,
    core_version: Option<String>,
    status_message: Option<String>,
}

impl Drop for InnerState {
    fn drop(&mut self) {
        for (_, task) in self.meter_tasks.drain() {
            task.abort();
        }
        if let Some(mut child) = self.core.take() {
            let _ = child.start_kill();
        }
    }
}

impl AppManager {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from(".freeclash"));
        let runtime_dir = app_data_dir.join("runtime");
        let config_path = runtime_dir.join("config.yaml");
        let app_config_path = app_data_dir.join("freeclash.json");
        let core_path = resolve_core_path(app)?;

        fs::create_dir_all(&runtime_dir)?;
        fs::create_dir_all(runtime_dir.join("providers"))?;

        let config = load_config(&app_config_path)?;
        Ok(Self {
            paths: Arc::new(AppPaths {
                runtime_dir,
                config_path,
                app_config_path,
                core_path,
            }),
            inner: Arc::new(Mutex::new(InnerState {
                config,
                core: None,
                meter_tasks: HashMap::new(),
                metrics: HashMap::new(),
                launched_apps: HashMap::new(),
                nodes: vec![direct_node()],
                core_version: None,
                status_message: None,
            })),
            client: Client::builder().timeout(Duration::from_secs(12)).build()?,
        })
    }

    pub async fn initialize(&self) {
        if let Err(err) = self.apply_runtime().await {
            let mut inner = self.inner.lock().await;
            inner.status_message = Some(format!("初始化失败：{err:#}"));
        }
    }

    pub async fn get_state(&self) -> Result<AppSnapshot> {
        self.reap_finished_apps().await;
        let mut inner = self.inner.lock().await;
        reap_core_status(&mut inner)?;
        let stats = collect_stats(&mut inner);
        let config = inner.config.clone();
        let status = self.runtime_status(&inner);
        Ok(AppSnapshot {
            config,
            nodes: inner.nodes.clone(),
            stats,
            status,
        })
    }

    pub async fn set_subscription(&self, url: Option<String>) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            inner.config.subscription_url = url.filter(|value| !value.trim().is_empty());
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        self.apply_runtime().await
    }

    pub async fn refresh_nodes(&self) -> Result<Vec<NodeInfo>> {
        self.ensure_core_running().await?;
        let should_refresh = {
            let inner = self.inner.lock().await;
            inner.config.subscription_url.is_some()
        };
        if should_refresh {
            let _ = self
                .mihomo_request(reqwest::Method::PUT, "/providers/proxies/FreeClash", None)
                .await;
        }
        let nodes = self.fetch_nodes().await?;
        let mut inner = self.inner.lock().await;
        inner.nodes = nodes.clone();
        Ok(nodes)
    }

    pub async fn create_rule(&self, input: RuleInput) -> Result<AppRule> {
        validate_rule_input(&input)?;
        let rule = {
            let mut inner = self.inner.lock().await;
            let meter_port = allocate_port(&inner.config, None);
            let mihomo_port = allocate_port(&inner.config, Some(meter_port));
            let rule = AppRule {
                id: short_id(),
                name: input.name.trim().to_string(),
                app_path: input.app_path.trim().to_string(),
                args: input.args.trim().to_string(),
                working_dir: input.working_dir.trim().to_string(),
                selected_node: normalize_node(input.selected_node),
                enabled: input.enabled,
                meter_port,
                mihomo_port,
            };
            inner.config.rules.push(rule.clone());
            save_config(&self.paths.app_config_path, &inner.config)?;
            rule
        };
        self.apply_runtime().await?;
        Ok(rule)
    }

    pub async fn update_rule(&self, rule_id: &str, input: RuleInput) -> Result<AppRule> {
        validate_rule_input(&input)?;
        let updated = {
            let mut inner = self.inner.lock().await;
            let index = inner
                .config
                .rules
                .iter()
                .position(|rule| rule.id == rule_id)
                .ok_or_else(|| anyhow!("找不到规则 {rule_id}"))?;
            let rule = &mut inner.config.rules[index];
            rule.name = input.name.trim().to_string();
            rule.app_path = input.app_path.trim().to_string();
            rule.args = input.args.trim().to_string();
            rule.working_dir = input.working_dir.trim().to_string();
            rule.selected_node = normalize_node(input.selected_node);
            rule.enabled = input.enabled;
            let updated = rule.clone();
            if !updated.enabled {
                if let Some(mut child) = inner.launched_apps.remove(rule_id) {
                    let _ = child.kill();
                }
            }
            save_config(&self.paths.app_config_path, &inner.config)?;
            updated
        };
        self.apply_runtime().await?;
        Ok(updated)
    }

    pub async fn delete_rule(&self, rule_id: &str) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            if let Some(mut child) = inner.launched_apps.remove(rule_id) {
                let _ = child.kill();
            }
            inner.config.rules.retain(|rule| rule.id != rule_id);
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        self.apply_runtime().await
    }

    pub async fn set_rule_node(&self, rule_id: &str, node: String) -> Result<()> {
        let node = if node.trim().is_empty() {
            "DIRECT".to_string()
        } else {
            node.trim().to_string()
        };
        let enabled = {
            let mut inner = self.inner.lock().await;
            let rule = inner
                .config
                .rules
                .iter_mut()
                .find(|rule| rule.id == rule_id)
                .ok_or_else(|| anyhow!("找不到规则 {rule_id}"))?;
            rule.selected_node = Some(node.clone());
            let enabled = rule.enabled;
            save_config(&self.paths.app_config_path, &inner.config)?;
            enabled
        };
        if enabled {
            self.select_rule_node(rule_id, &node).await
        } else {
            Ok(())
        }
    }

    pub async fn start_rule_app(&self, rule_id: &str) -> Result<()> {
        self.ensure_core_running().await?;
        let rule = {
            let inner = self.inner.lock().await;
            inner
                .config
                .rules
                .iter()
                .find(|rule| rule.id == rule_id)
                .cloned()
                .ok_or_else(|| anyhow!("找不到规则 {rule_id}"))?
        };

        if !rule.enabled {
            bail!("规则已禁用");
        }

        let app_path = PathBuf::from(&rule.app_path);
        if !app_path.exists() {
            bail!("软件路径不存在：{}", rule.app_path);
        }

        let mut command = command_for_app_path(&rule.app_path);
        let args = split_args(&rule.args)?;
        command.args(args);

        if !rule.working_dir.trim().is_empty() {
            command.current_dir(&rule.working_dir);
        } else if let Some(parent) = app_path.parent() {
            command.current_dir(parent);
        }

        let proxy = format!("http://127.0.0.1:{}", rule.meter_port);
        command
            .env("HTTP_PROXY", &proxy)
            .env("HTTPS_PROXY", &proxy)
            .env("http_proxy", &proxy)
            .env("https_proxy", &proxy);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        let child = command
            .spawn()
            .with_context(|| format!("启动软件失败：{}", rule.app_path))?;
        let mut inner = self.inner.lock().await;
        inner.launched_apps.insert(rule_id.to_string(), child);
        Ok(())
    }

    pub async fn stop_rule_app(&self, rule_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let Some(mut child) = inner.launched_apps.remove(rule_id) else {
            return Ok(());
        };
        child.kill().ok();
        Ok(())
    }

    pub async fn restart_core(&self) -> Result<()> {
        self.apply_runtime().await
    }

    pub async fn test_node_delay(&self, node: String) -> Result<DelayResult> {
        if node.eq_ignore_ascii_case("DIRECT") {
            return Ok(DelayResult { node, delay: 0 });
        }

        self.ensure_core_running().await?;
        let path = format!(
            "/proxies/{}/delay?url={}&timeout=5000",
            urlencoding::encode(&node),
            urlencoding::encode("https://www.gstatic.com/generate_204")
        );
        let value = self
            .mihomo_request(reqwest::Method::GET, &path, None)
            .await
            .with_context(|| format!("测速失败：{node}"))?;
        let delay = value
            .get("delay")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32;
        let _ = self.refresh_nodes().await;
        Ok(DelayResult { node, delay })
    }

    pub async fn apply_runtime(&self) -> Result<()> {
        self.stop_core_and_meters().await;
        let result = async {
            self.write_mihomo_config().await?;
            self.start_meter_servers().await?;
            self.start_core().await?;
            self.wait_for_core().await?;
            self.sync_rule_selections().await;
            let nodes = self.fetch_nodes().await.unwrap_or_else(|_| vec![direct_node()]);
            let mut inner = self.inner.lock().await;
            inner.nodes = nodes;
            inner.status_message = Some("mihomo 核心已就绪".to_string());
            Ok::<(), anyhow::Error>(())
        }
        .await;

        if let Err(err) = result {
            self.stop_core_and_meters().await;
            let mut inner = self.inner.lock().await;
            inner.status_message = Some(format!("mihomo 启动失败：{err:#}"));
            return Err(err);
        }

        Ok(())
    }

    async fn ensure_core_running(&self) -> Result<()> {
        let running = {
            let mut inner = self.inner.lock().await;
            match inner.core.as_mut() {
                Some(child) => child.try_wait()?.is_none(),
                None => false,
            }
        };
        if running {
            Ok(())
        } else {
            self.apply_runtime().await
        }
    }

    async fn stop_core_and_meters(&self) {
        let child = {
            let mut inner = self.inner.lock().await;
            for (_, task) in inner.meter_tasks.drain() {
                task.abort();
            }
            inner.core.take()
        };

        if let Some(mut child) = child {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }

    async fn write_mihomo_config(&self) -> Result<()> {
        let config = {
            let inner = self.inner.lock().await;
            inner.config.clone()
        };
        fs::create_dir_all(&self.paths.runtime_dir)?;
        fs::create_dir_all(self.paths.runtime_dir.join("providers"))?;
        let yaml = render_config(&config)?;
        fs::write(&self.paths.config_path, yaml)?;
        Ok(())
    }

    async fn start_meter_servers(&self) -> Result<()> {
        let rules = {
            let inner = self.inner.lock().await;
            inner.config.rules.clone()
        };

        for rule in rules.into_iter().filter(|rule| rule.enabled) {
            let metrics = {
                let mut inner = self.inner.lock().await;
                inner
                    .metrics
                    .entry(rule.id.clone())
                    .or_insert_with(|| Arc::new(StdMutex::new(RuleMetrics::new(rule.id.clone()))))
                    .clone()
            };
            let task =
                start_meter_proxy(rule.name.clone(), rule.meter_port, rule.mihomo_port, metrics)
                    .await?;
            let mut inner = self.inner.lock().await;
            inner.meter_tasks.insert(rule.id.clone(), task);
        }
        Ok(())
    }

    async fn start_core(&self) -> Result<()> {
        if !self.paths.core_path.exists() {
            bail!("mihomo 核心不存在：{}", self.paths.core_path.display());
        }

        let mut command = Command::new(&self.paths.core_path);
        command
            .arg("-d")
            .arg(&self.paths.runtime_dir)
            .arg("-f")
            .arg(&self.paths.config_path)
            .current_dir(&self.paths.runtime_dir)
            .env("SAFE_PATHS", self.paths.runtime_dir.to_string_lossy().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        let child = command.spawn().context("启动 mihomo 核心失败")?;
        let mut inner = self.inner.lock().await;
        inner.core = Some(child);
        inner.status_message = Some("mihomo 核心启动中".to_string());
        Ok(())
    }

    async fn wait_for_core(&self) -> Result<()> {
        let mut last_error = None;
        for _ in 0..30 {
            match self
                .mihomo_request(reqwest::Method::GET, "/version", None)
                .await
            {
                Ok(value) => {
                    let version = value
                        .get("version")
                        .or_else(|| value.get("Version"))
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    let mut inner = self.inner.lock().await;
                    inner.core_version = Some(version);
                    return Ok(());
                }
                Err(err) => {
                    last_error = Some(err);
                    sleep(Duration::from_millis(250)).await;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("mihomo 核心健康检查超时")))
    }

    async fn sync_rule_selections(&self) {
        let selections = {
            let inner = self.inner.lock().await;
            inner
                .config
                .rules
                .iter()
                .filter(|rule| rule.enabled)
                .filter_map(|rule| {
                    rule.selected_node
                        .clone()
                        .map(|node| (rule.id.clone(), node))
                })
                .collect::<Vec<_>>()
        };

        for (rule_id, node) in selections {
            let _ = self.select_rule_node(&rule_id, &node).await;
        }
    }

    async fn select_rule_node(&self, rule_id: &str, node: &str) -> Result<()> {
        self.ensure_core_running().await?;
        let path = format!("/proxies/{}", urlencoding::encode(&group_name(rule_id)));
        self.mihomo_request(
            reqwest::Method::PUT,
            &path,
            Some(serde_json::json!({ "name": node })),
        )
        .await
        .map(|_| ())
        .with_context(|| format!("切换规则节点失败：{node}"))
    }

    async fn fetch_nodes(&self) -> Result<Vec<NodeInfo>> {
        let value = self
            .mihomo_request(reqwest::Method::GET, "/proxies", None)
            .await?;
        let proxies = value
            .get("proxies")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("mihomo /proxies 响应缺少 proxies 字段"))?;

        let mut nodes = vec![direct_node()];
        for (name, item) in proxies {
            if name.eq_ignore_ascii_case("DIRECT") || name.eq_ignore_ascii_case("REJECT") {
                continue;
            }
            if item.get("all").and_then(Value::as_array).is_some() {
                continue;
            }
            let node_type = item
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("Proxy")
                .to_string();
            if matches!(
                node_type.as_str(),
                "Direct" | "Reject" | "Selector" | "URLTest" | "Fallback" | "LoadBalance"
            ) {
                continue;
            }
            let delay = item
                .get("history")
                .and_then(Value::as_array)
                .and_then(|items| items.last())
                .and_then(|last| last.get("delay"))
                .and_then(Value::as_u64)
                .map(|delay| delay as u32)
                .filter(|delay| *delay > 0);
            nodes.push(NodeInfo {
                name: name.clone(),
                node_type,
                delay,
                is_builtin: false,
            });
        }
        nodes.sort_by(|a, b| a.is_builtin.cmp(&b.is_builtin).reverse().then(a.name.cmp(&b.name)));
        Ok(nodes)
    }

    async fn mihomo_request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let (url, secret) = {
            let inner = self.inner.lock().await;
            (
                format!("http://127.0.0.1:{}{}", inner.config.controller_port, path),
                inner.config.controller_secret.clone(),
            )
        };
        let mut request = self.client.request(method, url).bearer_auth(secret);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("mihomo API 返回 {status}: {text}");
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    async fn reap_finished_apps(&self) {
        let mut inner = self.inner.lock().await;
        inner.launched_apps.retain(|_, child| match child.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        });
    }

    fn runtime_status(&self, inner: &InnerState) -> RuntimeStatus {
        RuntimeStatus {
            core_path: self.paths.core_path.to_string_lossy().to_string(),
            config_path: self.paths.config_path.to_string_lossy().to_string(),
            runtime_dir: self.paths.runtime_dir.to_string_lossy().to_string(),
            controller_url: format!("http://127.0.0.1:{}", inner.config.controller_port),
            core_running: inner.core.is_some(),
            core_version: inner.core_version.clone(),
            message: inner.status_message.clone(),
        }
    }
}

fn collect_stats(inner: &mut InnerState) -> Vec<RuleStats> {
    inner
        .config
        .rules
        .iter()
        .filter_map(|rule| {
            let metrics = inner
                .metrics
                .entry(rule.id.clone())
                .or_insert_with(|| Arc::new(StdMutex::new(RuleMetrics::new(rule.id.clone()))))
                .clone();
            metrics.lock().ok().map(|mut guard| guard.snapshot())
        })
        .collect()
}

fn reap_core_status(inner: &mut InnerState) -> Result<()> {
    let status = match inner.core.as_mut() {
        Some(child) => child.try_wait()?,
        None => None,
    };

    if let Some(status) = status {
        inner.core = None;
        inner.core_version = None;
        inner.status_message = Some(format!("mihomo 核心已退出：{status}"));
    }
    Ok(())
}

fn load_config(path: &Path) -> Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_config(path: &Path, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

fn resolve_core_path(app: &AppHandle) -> Result<PathBuf> {
    let exe_name = "verge-mihomo.exe";
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidates = [
        current_dir.join("core").join(exe_name),
        current_dir
            .parent()
            .map(|parent| parent.join("core").join(exe_name))
            .unwrap_or_else(|| current_dir.join("core").join(exe_name)),
        app.path()
            .resource_dir()
            .unwrap_or_else(|_| current_dir.clone())
            .join("core")
            .join(exe_name),
    ];

    candidates
        .iter()
        .find(|path| path.exists())
        .cloned()
        .ok_or_else(|| anyhow!("未找到 mihomo 核心 core\\{exe_name}"))
}

fn validate_rule_input(input: &RuleInput) -> Result<()> {
    if input.name.trim().is_empty() {
        bail!("规则名不能为空");
    }
    if input.app_path.trim().is_empty() {
        bail!("软件路径不能为空");
    }
    Ok(())
}

fn normalize_node(node: Option<String>) -> Option<String> {
    node.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| Some("DIRECT".to_string()))
}

fn allocate_port(config: &AppConfig, extra_used: Option<u16>) -> u16 {
    let mut used = Vec::new();
    if let Some(port) = extra_used {
        used.push(port);
    }
    used.push(config.controller_port);
    for rule in &config.rules {
        used.push(rule.meter_port);
        used.push(rule.mihomo_port);
    }

    for port in config.port_range_start..=u16::MAX {
        if used.contains(&port) {
            continue;
        }
        if portpicker::is_free_tcp(port) {
            return port;
        }
    }
    config.port_range_start
}

fn short_id() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(12)
        .collect()
}

fn direct_node() -> NodeInfo {
    NodeInfo {
        name: "DIRECT".to_string(),
        node_type: "Builtin".to_string(),
        delay: None,
        is_builtin: true,
    }
}

fn command_for_app_path(app_path: &str) -> std::process::Command {
    let extension = Path::new(app_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat") {
        let mut command = std::process::Command::new("cmd");
        command.arg("/C").arg(app_path);
        command
    } else {
        std::process::Command::new(app_path)
    }
}

fn split_args(input: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        match ch {
            '"' | '\'' if quote == Some(ch) => quote = None,
            '"' | '\'' if quote.is_none() => quote = Some(ch),
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push('\\');
                }
            }
            ch if ch.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if quote.is_some() {
        bail!("启动参数中的引号没有闭合");
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_args() {
        let args = split_args(r#"--profile "work space" --flag"#).unwrap();
        assert_eq!(args, vec!["--profile", "work space", "--flag"]);
    }

    #[test]
    fn allocates_distinct_ports() {
        let mut config = AppConfig::default();
        config.port_range_start = 22000;
        config.rules.push(AppRule {
            id: "one".into(),
            name: "one".into(),
            app_path: "one.exe".into(),
            args: String::new(),
            working_dir: String::new(),
            selected_node: None,
            enabled: true,
            meter_port: 22000,
            mihomo_port: 22001,
        });
        let port = allocate_port(&config, None);
        assert!(port >= 22002);
    }

    #[test]
    fn wraps_cmd_launchers() {
        let command = command_for_app_path(r"C:\Users\me\AppData\Roaming\npm\codex.cmd");
        assert_eq!(command.get_program().to_string_lossy(), "cmd");
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(args[0], "/C");
        assert!(args[1].ends_with("codex.cmd"));
    }
}

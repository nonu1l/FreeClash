use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::TcpListener as StdTcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Method;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use uuid::Uuid;

use crate::metrics::PinMetrics;
use crate::mihomo::{group_name, provider_name, render_config};
use crate::models::{
    AppConfig, AppSnapshot, DelayResult, NodeInfo, PinRuntime, PinStats, PinnedNode, RuntimeStatus,
    Subscription, SubscriptionInput,
};
use crate::proxy::start_mixed_meter_proxy;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const DIRECT: &str = "DIRECT";
const REJECT: &str = "REJECT";
const CONFIG_FILE: &str = "freeclash.json";
const RUNTIME_CONFIG_FILE: &str = "mihomo.yaml";
const CORE_EXE: &str = "verge-mihomo.exe";

#[derive(Clone)]
pub struct AppManager {
    paths: Arc<AppPaths>,
    inner: Arc<StdMutex<InnerState>>,
    client: reqwest::Client,
}

struct AppPaths {
    data_dir: PathBuf,
    runtime_dir: PathBuf,
    config_path: PathBuf,
    runtime_config_path: PathBuf,
    core_path: PathBuf,
}

#[derive(Default)]
struct InnerState {
    config: AppConfig,
    core_child: Option<Child>,
    core_version: Option<String>,
    message: Option<String>,
    meter_tasks: HashMap<String, JoinHandle<()>>,
    metrics: HashMap<String, Arc<StdMutex<PinMetrics>>>,
    port_errors: HashMap<String, String>,
}

impl AppManager {
    pub fn new(app: &AppHandle) -> tauri::Result<Self> {
        let data_dir = app.path().app_data_dir()?;
        let runtime_dir = data_dir.join("runtime");
        let config_path = data_dir.join(CONFIG_FILE);
        let runtime_config_path = runtime_dir.join(RUNTIME_CONFIG_FILE);
        let core_path = find_core_path(app);

        Ok(Self {
            paths: Arc::new(AppPaths {
                data_dir,
                runtime_dir,
                config_path,
                runtime_config_path,
                core_path,
            }),
            inner: Arc::new(StdMutex::new(InnerState {
                config: AppConfig::default(),
                ..InnerState::default()
            })),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("reqwest client"),
        })
    }

    pub async fn initialize(&self) {
        let result = async {
            fs::create_dir_all(&self.paths.data_dir)?;
            fs::create_dir_all(&self.paths.runtime_dir)?;
            let config = load_config(&self.paths.config_path)?;
            {
                let mut inner = self.lock_inner()?;
                inner.config = config;
            }
            self.save_config()?;
            self.reload_runtime().await?;
            Ok::<_, anyhow::Error>(())
        }
        .await;

        if let Err(err) = result {
            self.set_message(Some(format!("初始化失败：{err:#}")));
        }
    }

    pub async fn shutdown(&self) {
        let (tasks, child) = {
            let Ok(mut inner) = self.lock_inner() else {
                return;
            };
            (
                inner.meter_tasks.drain().map(|(_, task)| task).collect::<Vec<_>>(),
                inner.core_child.take(),
            )
        };

        for task in tasks {
            task.abort();
        }
        stop_core_child(child).await;
    }

    pub async fn get_state(&self) -> Result<AppSnapshot> {
        let nodes = self.fetch_nodes().await.unwrap_or_default();
        let (config, status, pins) = {
            let mut inner = self.lock_inner()?;
            let config = inner.config.clone();
            let status = RuntimeStatus {
                core_path: self.paths.core_path.display().to_string(),
                config_path: self.paths.config_path.display().to_string(),
                runtime_dir: self.paths.runtime_dir.display().to_string(),
                controller_url: controller_url(&inner.config),
                core_running: inner.core_child.is_some(),
                core_version: inner.core_version.clone(),
                message: inner.message.clone(),
            };
            let pins = config
                .pinned_nodes
                .iter()
                .map(|pin| {
                    let stats = stats_for_pin(&mut inner, &pin.node_name);
                    let port_error = inner.port_errors.get(&pin.node_name).cloned();
                    PinRuntime {
                        node_name: pin.node_name.clone(),
                        port: pin.port,
                        port_available: port_error.is_none(),
                        port_error,
                        stats,
                    }
                })
                .collect();
            (config, status, pins)
        };

        Ok(AppSnapshot {
            config,
            nodes,
            pins,
            status,
        })
    }

    pub async fn set_subscription(&self, input: SubscriptionInput) -> Result<Subscription> {
        let url = input.url.trim();
        if url.is_empty() {
            bail!("订阅 URL 不能为空");
        }
        let subscription = Subscription {
            id: Uuid::new_v4().to_string(),
            name: if input.name.trim().is_empty() {
                subscription_name_from_url(url)
            } else {
                input.name.trim().to_string()
            },
            url: url.to_string(),
        };
        {
            let mut inner = self.lock_inner()?;
            inner.config.subscription = Some(subscription.clone());
        }
        self.save_config()?;
        self.reload_runtime().await?;
        let _ = self.refresh_subscription().await;
        Ok(subscription)
    }

    pub async fn refresh_subscription(&self) -> Result<Vec<NodeInfo>> {
        let subscription = {
            let inner = self.lock_inner()?;
            inner.config.subscription.clone()
        }
        .ok_or_else(|| anyhow!("请先设置订阅"))?;

        let path = format!(
            "/providers/proxies/{}",
            urlencoding::encode(&provider_name(&subscription.id))
        );
        self.mihomo_request(Method::PUT, &path, None).await?;
        self.fetch_nodes().await
    }

    pub async fn refresh_nodes(&self) -> Result<Vec<NodeInfo>> {
        match self.refresh_subscription().await {
            Ok(nodes) => Ok(nodes),
            Err(_) => self.fetch_nodes().await,
        }
    }

    pub async fn pin_node(&self, node_name: String) -> Result<PinnedNode> {
        let node_name = node_name.trim().to_string();
        if node_name.is_empty() {
            bail!("节点名称不能为空");
        }

        let pin = {
            let mut inner = self.lock_inner()?;
            if let Some(existing) = inner
                .config
                .pinned_nodes
                .iter()
                .find(|pin| pin.node_name == node_name)
                .cloned()
            {
                return Ok(existing);
            }

            let mut used = collect_used_ports(&inner.config);
            let port = allocate_available_port(inner.config.port_range_start, &mut used)?;
            let mihomo_http_port = allocate_available_port(inner.config.port_range_start, &mut used)?;
            let mihomo_socks_port =
                allocate_available_port(inner.config.port_range_start, &mut used)?;
            let pin = PinnedNode {
                node_name: node_name.clone(),
                port,
                mihomo_http_port,
                mihomo_socks_port,
            };
            inner.config.pinned_nodes.push(pin.clone());
            pin
        };

        self.save_config()?;
        self.reload_runtime().await?;
        Ok(pin)
    }

    pub async fn unpin_node(&self, node_name: String) -> Result<()> {
        {
            let mut inner = self.lock_inner()?;
            let before = inner.config.pinned_nodes.len();
            inner
                .config
                .pinned_nodes
                .retain(|pin| pin.node_name != node_name);
            if before == inner.config.pinned_nodes.len() {
                return Ok(());
            }
            inner.metrics.remove(&node_name);
            inner.port_errors.remove(&node_name);
        }
        self.save_config()?;
        self.reload_runtime().await
    }

    pub async fn update_pin_port(&self, node_name: String, port: u16) -> Result<PinnedNode> {
        validate_public_port(port)?;
        let updated = {
            let mut inner = self.lock_inner()?;
            let duplicate = inner
                .config
                .pinned_nodes
                .iter()
                .any(|pin| pin.node_name != node_name && pin.port == port);
            if duplicate || port == inner.config.controller_port {
                bail!("端口 {port} 已被 FreeClash 使用");
            }
            let pin = inner
                .config
                .pinned_nodes
                .iter_mut()
                .find(|pin| pin.node_name == node_name)
                .ok_or_else(|| anyhow!("节点未 Pin：{node_name}"))?;
            pin.port = port;
            pin.clone()
        };
        self.save_config()?;
        self.reload_runtime().await?;
        Ok(updated)
    }

    pub async fn test_node_delay(&self, node: String) -> Result<DelayResult> {
        let path = format!(
            "/proxies/{}/delay?url={}&timeout=5000",
            urlencoding::encode(&node),
            urlencoding::encode("https://www.gstatic.com/generate_204")
        );
        let value = self.mihomo_request(Method::GET, &path, None).await?;
        let delay = value
            .get("delay")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("mihomo 未返回延迟"))? as u32;
        Ok(DelayResult { node, delay })
    }

    pub async fn test_all_node_delays(&self) -> Result<Vec<DelayResult>> {
        let nodes = self.fetch_nodes().await?;
        let mut results = Vec::new();
        for node in nodes.into_iter().filter(|node| !node.is_builtin) {
            if let Ok(result) = self.test_node_delay(node.name).await {
                results.push(result);
            }
        }
        Ok(results)
    }

    async fn reload_runtime(&self) -> Result<()> {
        let (old_tasks, old_child) = {
            let mut inner = self.lock_inner()?;
            (
                inner.meter_tasks.drain().map(|(_, task)| task).collect::<Vec<_>>(),
                inner.core_child.take(),
            )
        };
        for task in old_tasks {
            task.abort();
        }
        stop_core_child(old_child).await;

        self.ensure_internal_ports()?;
        let config = {
            let inner = self.lock_inner()?;
            inner.config.clone()
        };
        fs::create_dir_all(self.paths.runtime_config_path.parent().unwrap())?;
        fs::write(&self.paths.runtime_config_path, render_config(&config)?)?;

        if !self.paths.core_path.exists() {
            bail!("找不到 mihomo 核心：{}", self.paths.core_path.display());
        }

        let mut command = Command::new(&self.paths.core_path);
        command
            .arg("-d")
            .arg(&self.paths.runtime_dir)
            .arg("-f")
            .arg(&self.paths.runtime_config_path)
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let child = command.spawn().context("启动 mihomo 失败")?;
        {
            let mut inner = self.lock_inner()?;
            inner.core_child = Some(child);
            inner.core_version = None;
            inner.message = Some("mihomo 启动中".to_string());
        }

        self.wait_for_core().await?;
        self.select_pinned_nodes().await;
        self.start_meter_servers().await?;
        self.set_message(Some("mihomo 核心已就绪".to_string()));
        Ok(())
    }

    fn ensure_internal_ports(&self) -> Result<()> {
        let mut inner = self.lock_inner()?;
        let mut used = HashSet::from([inner.config.controller_port]);
        for pin in &inner.config.pinned_nodes {
            used.insert(pin.port);
        }

        let start = inner.config.port_range_start;
        let mut changed = false;
        for pin in &mut inner.config.pinned_nodes {
            if pin.mihomo_http_port == 0 || !port_is_usable(pin.mihomo_http_port, &used) {
                pin.mihomo_http_port = allocate_available_port(start, &mut used)?;
                changed = true;
            } else {
                used.insert(pin.mihomo_http_port);
            }
            if pin.mihomo_socks_port == 0 || !port_is_usable(pin.mihomo_socks_port, &used) {
                pin.mihomo_socks_port = allocate_available_port(start, &mut used)?;
                changed = true;
            } else {
                used.insert(pin.mihomo_socks_port);
            }
        }
        drop(inner);

        if changed {
            self.save_config()?;
        }
        Ok(())
    }

    async fn wait_for_core(&self) -> Result<()> {
        let start = Instant::now();
        let mut last_error = None;
        while start.elapsed() < Duration::from_secs(8) {
            match self.mihomo_request(Method::GET, "/version", None).await {
                Ok(value) => {
                    let version = value
                        .get("version")
                        .or_else(|| value.get("Version"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let mut inner = self.lock_inner()?;
                    inner.core_version = version;
                    return Ok(());
                }
                Err(err) => {
                    last_error = Some(err);
                    sleep(Duration::from_millis(250)).await;
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow!("mihomo 启动超时")))
    }

    async fn select_pinned_nodes(&self) {
        let pins = {
            let inner = match self.lock_inner() {
                Ok(inner) => inner,
                Err(_) => return,
            };
            inner.config.pinned_nodes.clone()
        };
        for pin in pins {
            let path = format!(
                "/proxies/{}",
                urlencoding::encode(&group_name(&pin.node_name))
            );
            if let Err(err) = self
                .mihomo_request(Method::PUT, &path, Some(json!({ "name": pin.node_name })))
                .await
            {
                let mut inner = match self.lock_inner() {
                    Ok(inner) => inner,
                    Err(_) => return,
                };
                inner
                    .port_errors
                    .insert(pin.node_name, format!("节点选择失败：{err:#}"));
            }
        }
    }

    async fn start_meter_servers(&self) -> Result<()> {
        let pins = {
            let inner = self.lock_inner()?;
            inner.config.pinned_nodes.clone()
        };

        {
            let mut inner = self.lock_inner()?;
            inner.port_errors.clear();
        }

        let mut tasks = HashMap::new();
        for pin in pins {
            if let Err(err) = validate_public_port(pin.port) {
                self.remember_port_error(&pin.node_name, format!("{err:#}"));
                continue;
            }

            if !port_is_free(pin.port) {
                self.remember_port_error(&pin.node_name, format!("端口 {} 不可用", pin.port));
                continue;
            }

            let metrics = {
                let mut inner = self.lock_inner()?;
                inner
                    .metrics
                    .entry(pin.node_name.clone())
                    .or_insert_with(|| Arc::new(StdMutex::new(PinMetrics::new(pin.node_name.clone()))))
                    .clone()
            };

            match start_mixed_meter_proxy(
                pin.node_name.clone(),
                pin.port,
                pin.mihomo_http_port,
                pin.mihomo_socks_port,
                metrics,
            )
            .await
            {
                Ok(task) => {
                    tasks.insert(pin.node_name.clone(), task);
                }
                Err(err) => {
                    self.remember_port_error(&pin.node_name, format!("{err:#}"));
                }
            }
        }

        let mut inner = self.lock_inner()?;
        inner.meter_tasks = tasks;
        Ok(())
    }

    async fn fetch_nodes(&self) -> Result<Vec<NodeInfo>> {
        let mut nodes = Vec::new();
        if let Ok(value) = self.mihomo_request(Method::GET, "/proxies", None).await {
            if let Some(map) = value.get("proxies").and_then(Value::as_object) {
                for (name, proxy) in map {
                    if is_builtin_proxy(name) {
                        continue;
                    }
                    let node_type = proxy
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("Proxy")
                        .to_string();
                    if node_type.eq_ignore_ascii_case("Selector") {
                        continue;
                    }
                    nodes.push(NodeInfo {
                        name: name.clone(),
                        node_type,
                        delay: delay_from_proxy(proxy),
                        is_builtin: false,
                        provider_id: None,
                        provider_name: None,
                    });
                }
            }
        }

        let subscription = {
            let inner = self.lock_inner()?;
            inner.config.subscription.clone()
        };
        if let Some(subscription) = subscription {
            if let Ok(value) = self.mihomo_request(Method::GET, "/providers/proxies", None).await {
                if let Some(provider) = value
                    .get("providers")
                    .and_then(Value::as_object)
                    .and_then(|providers| providers.get(&provider_name(&subscription.id)))
                {
                    let mut provider_nodes = Vec::new();
                    if let Some(proxies) = provider.get("proxies").and_then(Value::as_array) {
                        for proxy in proxies {
                            let Some(name) = proxy.get("name").and_then(Value::as_str) else {
                                continue;
                            };
                            if is_builtin_proxy(name) {
                                continue;
                            }
                            provider_nodes.push(NodeInfo {
                                name: name.to_string(),
                                node_type: proxy
                                    .get("type")
                                    .and_then(Value::as_str)
                                    .unwrap_or("Proxy")
                                    .to_string(),
                                delay: delay_from_proxy(proxy),
                                is_builtin: false,
                                provider_id: Some(subscription.id.clone()),
                                provider_name: Some(subscription.name.clone()),
                            });
                        }
                    }
                    if !provider_nodes.is_empty() {
                        nodes = provider_nodes;
                    }
                }
            }
        }

        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        nodes.dedup_by(|a, b| a.name == b.name);
        Ok(nodes)
    }

    async fn mihomo_request(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let (port, secret) = {
            let inner = self.lock_inner()?;
            (
                inner.config.controller_port,
                inner.config.controller_secret.clone(),
            )
        };
        let url = format!("http://127.0.0.1:{port}{path}");
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
            Ok(json!({}))
        } else {
            Ok(serde_json::from_str(&text).unwrap_or_else(|_| json!({ "raw": text })))
        }
    }

    fn save_config(&self) -> Result<()> {
        let config = {
            let inner = self.lock_inner()?;
            inner.config.clone()
        };
        fs::create_dir_all(&self.paths.data_dir)?;
        fs::write(&self.paths.config_path, serde_json::to_string_pretty(&config)?)?;
        Ok(())
    }

    fn remember_port_error(&self, node_name: &str, error: String) {
        if let Ok(mut inner) = self.lock_inner() {
            inner.port_errors.insert(node_name.to_string(), error);
        }
    }

    fn set_message(&self, message: Option<String>) {
        if let Ok(mut inner) = self.lock_inner() {
            inner.message = message;
        }
    }

    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, InnerState>> {
        self.inner.lock().map_err(|_| anyhow!("应用状态锁已损坏"))
    }
}

async fn stop_core_child(child: Option<Child>) {
    let Some(mut child) = child else {
        return;
    };
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn load_config(path: &PathBuf) -> Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = fs::read_to_string(path)?;
    let config = serde_json::from_str(&raw).unwrap_or_default();
    Ok(config)
}

fn find_core_path(app: &AppHandle) -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("core").join(CORE_EXE));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("core").join(CORE_EXE));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("core").join(CORE_EXE));
    }
    candidates.push(PathBuf::from("D:\\my-project\\FreeClash\\core").join(CORE_EXE));

    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("core").join(CORE_EXE))
}

fn controller_url(config: &AppConfig) -> String {
    format!("http://127.0.0.1:{}", config.controller_port)
}

fn stats_for_pin(inner: &mut InnerState, node_name: &str) -> PinStats {
    inner
        .metrics
        .entry(node_name.to_string())
        .or_insert_with(|| Arc::new(StdMutex::new(PinMetrics::new(node_name.to_string()))))
        .lock()
        .map(|mut metrics| metrics.snapshot())
        .unwrap_or_else(|_| empty_stats(node_name))
}

fn empty_stats(node_name: &str) -> PinStats {
    PinStats {
        node_name: node_name.to_string(),
        upload_total: 0,
        download_total: 0,
        upload_speed: 0.0,
        download_speed: 0.0,
        active_connections: 0,
        recent_targets: Vec::new(),
    }
}

fn subscription_name_from_url(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "订阅".to_string())
}

fn validate_public_port(port: u16) -> Result<()> {
    if port < 1024 {
        bail!("端口必须大于等于 1024");
    }
    Ok(())
}

fn collect_used_ports(config: &AppConfig) -> HashSet<u16> {
    let mut used = HashSet::from([config.controller_port]);
    for pin in &config.pinned_nodes {
        used.insert(pin.port);
        used.insert(pin.mihomo_http_port);
        used.insert(pin.mihomo_socks_port);
    }
    used
}

fn allocate_available_port(start: u16, used: &mut HashSet<u16>) -> Result<u16> {
    for port in start..=u16::MAX {
        if port_is_usable(port, used) {
            used.insert(port);
            return Ok(port);
        }
    }
    bail!("没有可用端口")
}

fn port_is_usable(port: u16, used: &HashSet<u16>) -> bool {
    !used.contains(&port) && port_is_free(port)
}

fn port_is_free(port: u16) -> bool {
    StdTcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn is_builtin_proxy(name: &str) -> bool {
    name.eq_ignore_ascii_case(DIRECT) || name.eq_ignore_ascii_case(REJECT) || name.starts_with("fc-")
}

fn delay_from_proxy(proxy: &Value) -> Option<u32> {
    proxy
        .get("history")
        .and_then(Value::as_array)
        .and_then(|history| history.last())
        .and_then(|entry| entry.get("delay"))
        .and_then(Value::as_u64)
        .map(|delay| delay as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_unique_ports_for_pin() {
        let mut used = HashSet::new();
        used.insert(19100);
        let port = allocate_available_port(19100, &mut used).unwrap();
        assert_ne!(port, 19100);
        assert!(used.contains(&port));
    }

    #[test]
    fn rejects_reserved_public_ports() {
        assert!(validate_public_port(80).is_err());
        assert!(validate_public_port(19100).is_ok());
    }

    #[test]
    fn default_config_ignores_old_fields() {
        let raw = r#"{
            "global_proxy_enabled": false,
            "channels": [{"id":"old","name":"old","http_port":19100}],
            "subscription": {"id":"s","name":"Sub","url":"https://example.com/sub"},
            "pinned_nodes": [{"node_name":"HK","port":19100,"mihomo_http_port":19101,"mihomo_socks_port":19102}]
        }"#;
        let config: AppConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(config.pinned_nodes.len(), 1);
        assert_eq!(config.subscription.unwrap().name, "Sub");
    }
}

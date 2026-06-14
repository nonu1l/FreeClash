use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::TcpListener as StdTcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use uuid::Uuid;

use crate::metrics::ChannelMetrics;
use crate::mihomo::{group_name, provider_name, render_config};
use crate::models::{
    AppConfig, AppSnapshot, ChannelDiagnostics, ChannelInput, ChannelProxyTestResult, ChannelStats,
    DelayResult, NodeInfo, ProxyChannel, ProxyEndpointResult, ProxyProtocolTestResult,
    RuntimeStatus, Subscription, SubscriptionInput,
};
use crate::proxy::{start_meter_proxy, MeterProtocol};

#[derive(Clone)]
pub struct AppManager {
    paths: Arc<AppPaths>,
    inner: Arc<Mutex<InnerState>>,
    local_client: Client,
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
    http_shutdown: Option<oneshot::Sender<()>>,
    metrics: HashMap<String, Arc<StdMutex<ChannelMetrics>>>,
    nodes: Vec<NodeInfo>,
    core_version: Option<String>,
    status_message: Option<String>,
    last_errors: HashMap<String, String>,
}

impl Drop for InnerState {
    fn drop(&mut self) {
        for (_, task) in self.meter_tasks.drain() {
            task.abort();
        }
        if let Some(shutdown) = self.http_shutdown.take() {
            let _ = shutdown.send(());
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
                http_shutdown: None,
                metrics: HashMap::new(),
                nodes: vec![direct_node()],
                core_version: None,
                status_message: None,
                last_errors: HashMap::new(),
            })),
            local_client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(12))
                .build()?,
        })
    }

    pub fn global_proxy_enabled_blocking(&self) -> bool {
        self.inner
            .try_lock()
            .map(|inner| inner.config.global_proxy_enabled)
            .unwrap_or(true)
    }

    pub async fn initialize(&self) {
        if let Err(err) = self.apply_runtime().await {
            let mut inner = self.inner.lock().await;
            inner.status_message = Some(format!("初始化失败：{err:#}"));
        }
    }

    pub async fn shutdown(&self) {
        self.stop_http_api_server().await;
        self.stop_core_and_meters().await;
    }

    pub async fn start_http_api_from_config(&self) {
        if let Err(err) = self.apply_http_api_runtime().await {
            let mut inner = self.inner.lock().await;
            inner.status_message = Some(format!("HTTP API 启动失败：{err:#}"));
        }
    }

    pub async fn get_state(&self) -> Result<AppSnapshot> {
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
            inner.config.subscription_url = None;
            match url
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            {
                Some(url) => {
                    if let Some(subscription) = inner.config.subscriptions.first_mut() {
                        subscription.url = url;
                        if subscription.name.trim().is_empty() {
                            subscription.name = "默认订阅".to_string();
                        }
                    } else {
                        inner.config.subscriptions.push(Subscription {
                            id: "default".to_string(),
                            name: "默认订阅".to_string(),
                            url,
                        });
                    }
                }
                None => inner.config.subscriptions.clear(),
            }
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        self.apply_runtime().await
    }

    pub async fn create_subscription(&self, input: SubscriptionInput) -> Result<Subscription> {
        validate_subscription_input(&input)?;
        let subscription = {
            let mut inner = self.inner.lock().await;
            let subscription = Subscription {
                id: short_id(),
                name: input.name.trim().to_string(),
                url: input.url.trim().to_string(),
            };
            inner.config.subscriptions.push(subscription.clone());
            inner.config.subscription_url = None;
            save_config(&self.paths.app_config_path, &inner.config)?;
            subscription
        };
        self.apply_runtime().await?;
        Ok(subscription)
    }

    pub async fn delete_subscription(&self, subscription_id: &str) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            let before = inner.config.subscriptions.len();
            inner
                .config
                .subscriptions
                .retain(|subscription| subscription.id != subscription_id);
            if inner.config.subscriptions.len() == before {
                bail!("找不到订阅 {subscription_id}");
            }
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        self.apply_runtime().await
    }

    pub async fn refresh_subscription(&self, subscription_id: &str) -> Result<Vec<NodeInfo>> {
        self.ensure_core_running().await?;
        let subscription = {
            let inner = self.inner.lock().await;
            inner
                .config
                .subscriptions
                .iter()
                .find(|subscription| subscription.id == subscription_id)
                .cloned()
                .ok_or_else(|| anyhow!("找不到订阅 {subscription_id}"))?
        };
        self.refresh_provider(&subscription).await?;
        self.refresh_node_cache().await
    }

    pub async fn refresh_nodes(&self) -> Result<Vec<NodeInfo>> {
        self.ensure_core_running().await?;
        let subscriptions = {
            let inner = self.inner.lock().await;
            inner
                .config
                .subscriptions
                .iter()
                .cloned()
                .collect::<Vec<_>>()
        };
        for subscription in &subscriptions {
            self.refresh_provider(subscription).await?;
        }
        self.refresh_node_cache().await
    }

    pub async fn set_global_proxy_enabled(&self, enabled: bool) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            inner.config.global_proxy_enabled = enabled;
            inner.status_message = Some(if enabled {
                "全局代理链路已开启".to_string()
            } else {
                "全局代理链路已切换为 DIRECT".to_string()
            });
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        self.ensure_core_running().await?;
        self.sync_channel_selections().await;
        Ok(())
    }

    pub async fn set_http_api_config(&self, enabled: bool, port: u16) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            let current_server_owns_port =
                inner.http_shutdown.is_some() && inner.config.http_api_port == port;
            validate_http_api_port(&inner.config, port, enabled, current_server_owns_port)?;
            inner.config.http_api_enabled = enabled;
            inner.config.http_api_port = port;
            inner.status_message = Some(if enabled {
                format!("HTTP API 已开启：127.0.0.1:{port}")
            } else {
                "HTTP API 已关闭".to_string()
            });
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        self.apply_http_api_runtime().await
    }

    pub async fn toggle_global_proxy_enabled(&self) -> Result<bool> {
        let next = {
            let inner = self.inner.lock().await;
            !inner.config.global_proxy_enabled
        };
        self.set_global_proxy_enabled(next).await?;
        Ok(next)
    }

    pub async fn create_channel(&self, input: ChannelInput) -> Result<ProxyChannel> {
        validate_channel_input(&input)?;
        let channel = {
            let mut inner = self.inner.lock().await;
            let ports = allocate_channel_ports(&inner.config);
            let channel = ProxyChannel {
                id: short_id(),
                name: input.name.trim().to_string(),
                selected_node: normalize_node(input.selected_node),
                enabled: input.enabled,
                http_port: ports[0],
                socks_port: ports[1],
                mihomo_http_port: ports[2],
                mihomo_socks_port: ports[3],
            };
            inner.config.channels.push(channel.clone());
            save_config(&self.paths.app_config_path, &inner.config)?;
            channel
        };
        self.apply_runtime().await?;
        Ok(channel)
    }

    pub async fn update_channel(
        &self,
        channel_id: &str,
        input: ChannelInput,
    ) -> Result<ProxyChannel> {
        validate_channel_input(&input)?;
        let updated = {
            let mut inner = self.inner.lock().await;
            let index = inner
                .config
                .channels
                .iter()
                .position(|channel| channel.id == channel_id)
                .ok_or_else(|| anyhow!("找不到代理 {channel_id}"))?;
            let channel = &mut inner.config.channels[index];
            channel.name = input.name.trim().to_string();
            channel.selected_node = normalize_node(input.selected_node);
            channel.enabled = input.enabled;
            let updated = channel.clone();
            save_config(&self.paths.app_config_path, &inner.config)?;
            updated
        };
        self.ensure_core_running().await?;
        self.select_effective_channel_node(channel_id).await?;
        Ok(updated)
    }

    pub async fn delete_channel(&self, channel_id: &str) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            let before = inner.config.channels.len();
            inner
                .config
                .channels
                .retain(|channel| channel.id != channel_id);
            if inner.config.channels.len() == before {
                bail!("找不到代理 {channel_id}");
            }
            inner.metrics.remove(channel_id);
            inner.last_errors.remove(channel_id);
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        self.apply_runtime().await
    }

    pub async fn set_channel_enabled(
        &self,
        channel_id: &str,
        enabled: bool,
    ) -> Result<ProxyChannel> {
        let updated = {
            let mut inner = self.inner.lock().await;
            let channel = inner
                .config
                .channels
                .iter_mut()
                .find(|channel| channel.id == channel_id)
                .ok_or_else(|| anyhow!("找不到代理 {channel_id}"))?;
            channel.enabled = enabled;
            let updated = channel.clone();
            save_config(&self.paths.app_config_path, &inner.config)?;
            updated
        };
        self.ensure_core_running().await?;
        self.select_effective_channel_node(channel_id).await?;
        Ok(updated)
    }

    pub async fn duplicate_channel(&self, channel_id: &str) -> Result<ProxyChannel> {
        let duplicated = {
            let mut inner = self.inner.lock().await;
            let original = inner
                .config
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .cloned()
                .ok_or_else(|| anyhow!("找不到代理 {channel_id}"))?;
            let ports = allocate_channel_ports(&inner.config);
            let mut duplicated = original;
            duplicated.id = short_id();
            duplicated.name = next_duplicate_name(&inner.config.channels, &duplicated.name);
            duplicated.http_port = ports[0];
            duplicated.socks_port = ports[1];
            duplicated.mihomo_http_port = ports[2];
            duplicated.mihomo_socks_port = ports[3];
            inner.config.channels.push(duplicated.clone());
            save_config(&self.paths.app_config_path, &inner.config)?;
            duplicated
        };
        self.apply_runtime().await?;
        Ok(duplicated)
    }

    pub async fn set_channel_node(&self, channel_id: &str, node: String) -> Result<()> {
        let node = if node.trim().is_empty() {
            "DIRECT".to_string()
        } else {
            node.trim().to_string()
        };
        {
            let mut inner = self.inner.lock().await;
            let channel = inner
                .config
                .channels
                .iter_mut()
                .find(|channel| channel.id == channel_id)
                .ok_or_else(|| anyhow!("找不到代理 {channel_id}"))?;
            channel.selected_node = Some(node);
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        self.ensure_core_running().await?;
        self.select_effective_channel_node(channel_id).await
    }

    pub async fn diagnose_channel(&self, channel_id: &str) -> Result<ChannelDiagnostics> {
        let mut inner = self.inner.lock().await;
        reap_core_status(&mut inner)?;
        let channel = inner
            .config
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .cloned()
            .ok_or_else(|| anyhow!("找不到代理 {channel_id}"))?;
        let selected_node = selected_node_name(&channel);
        let effective_node = effective_node_for(&inner.config, &channel);
        let network_mode = network_mode_for(&effective_node);
        let stats = stats_for_channel(&mut inner, channel_id);
        let core_running = inner.core.is_some();
        Ok(ChannelDiagnostics {
            channel_id: channel.id,
            channel_name: channel.name,
            selected_node,
            effective_node,
            global_proxy_enabled: inner.config.global_proxy_enabled,
            channel_proxy_enabled: channel.enabled,
            core_running,
            network_mode,
            http_url: format!("http://127.0.0.1:{}", channel.http_port),
            socks_url: format!("socks5://127.0.0.1:{}", channel.socks_port),
            http_port: channel.http_port,
            socks_port: channel.socks_port,
            mihomo_http_port: channel.mihomo_http_port,
            mihomo_socks_port: channel.mihomo_socks_port,
            stats,
            last_error: inner.last_errors.get(channel_id).cloned(),
        })
    }

    pub async fn test_channel_proxy(&self, channel_id: &str) -> Result<ChannelProxyTestResult> {
        self.ensure_core_running().await?;
        self.select_effective_channel_node(channel_id).await?;
        let (http_port, socks_port, effective_node, network_mode) = {
            let inner = self.inner.lock().await;
            let channel = inner
                .config
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .cloned()
                .ok_or_else(|| anyhow!("找不到代理 {channel_id}"))?;
            let effective_node = effective_node_for(&inner.config, &channel);
            (
                channel.http_port,
                channel.socks_port,
                effective_node.clone(),
                network_mode_for(&effective_node),
            )
        };

        let started = Instant::now();
        let http_entry =
            run_proxy_protocol_test("HTTP", format!("http://127.0.0.1:{http_port}")).await?;
        let socks_entry =
            run_proxy_protocol_test("SOCKS5", format!("socks5h://127.0.0.1:{socks_port}")).await?;
        let elapsed_ms = started.elapsed().as_millis();

        let entries = vec![http_entry, socks_entry];
        let success = entries.iter().all(|entry| entry.success);
        let error = if success {
            None
        } else {
            Some(
                entries
                    .iter()
                    .filter(|entry| !entry.success)
                    .map(|entry| {
                        format!(
                            "{}: {}",
                            entry.protocol,
                            entry
                                .error
                                .clone()
                                .unwrap_or_else(|| "连接失败".to_string())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("；"),
            )
        };
        let result = ChannelProxyTestResult {
            channel_id: channel_id.to_string(),
            network_mode,
            effective_node,
            success,
            elapsed_ms,
            error,
            entries,
        };

        if result.success {
            self.clear_channel_error(channel_id).await;
        } else if let Some(error) = &result.error {
            self.remember_channel_error(channel_id, error.clone()).await;
        }
        Ok(result)
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
        let _ = self.refresh_node_cache().await;
        Ok(DelayResult { node, delay })
    }

    pub async fn apply_runtime(&self) -> Result<()> {
        self.stop_core_and_meters().await;
        let result = async {
            self.normalize_runtime_ports().await?;
            self.write_mihomo_config().await?;
            self.start_meter_servers().await?;
            self.start_core().await?;
            self.wait_for_core().await?;
            let nodes = match self.fetch_nodes().await {
                Ok(nodes) => {
                    self.sanitize_channel_selections(&nodes).await?;
                    nodes
                }
                Err(_) => vec![direct_node()],
            };
            self.sync_channel_selections().await;
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
            inner.core_version = None;
            inner.core.take()
        };

        if let Some(mut child) = child {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }

    async fn apply_http_api_runtime(&self) -> Result<()> {
        self.stop_http_api_server().await;
        let (enabled, port, token) = {
            let inner = self.inner.lock().await;
            (
                inner.config.http_api_enabled,
                inner.config.http_api_port,
                inner.config.http_api_token.clone(),
            )
        };

        if !enabled {
            return Ok(());
        }

        let shutdown = crate::http_api::start(self.clone(), port, token)?;
        let mut inner = self.inner.lock().await;
        inner.http_shutdown = Some(shutdown);
        inner.status_message = Some(format!("HTTP API 已监听 127.0.0.1:{port}"));
        Ok(())
    }

    async fn stop_http_api_server(&self) {
        let shutdown = {
            let mut inner = self.inner.lock().await;
            inner.http_shutdown.take()
        };
        if let Some(shutdown) = shutdown {
            let _ = shutdown.send(());
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
        let channels = {
            let inner = self.inner.lock().await;
            inner.config.channels.clone()
        };

        for channel in channels {
            let metrics = {
                let mut inner = self.inner.lock().await;
                inner
                    .metrics
                    .entry(channel.id.clone())
                    .or_insert_with(|| {
                        Arc::new(StdMutex::new(ChannelMetrics::new(channel.id.clone())))
                    })
                    .clone()
            };
            let http_task = start_meter_proxy(
                format!("{} HTTP", channel.name),
                MeterProtocol::Http,
                channel.http_port,
                channel.mihomo_http_port,
                metrics,
            )
            .await?;
            let socks_task = start_meter_proxy(
                format!("{} SOCKS5", channel.name),
                MeterProtocol::Socks5,
                channel.socks_port,
                channel.mihomo_socks_port,
                {
                    let mut inner = self.inner.lock().await;
                    inner
                        .metrics
                        .entry(channel.id.clone())
                        .or_insert_with(|| {
                            Arc::new(StdMutex::new(ChannelMetrics::new(channel.id.clone())))
                        })
                        .clone()
                },
            )
            .await?;
            let mut inner = self.inner.lock().await;
            inner
                .meter_tasks
                .insert(format!("{}:http", channel.id), http_task);
            inner
                .meter_tasks
                .insert(format!("{}:socks", channel.id), socks_task);
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
            .env(
                "SAFE_PATHS",
                self.paths.runtime_dir.to_string_lossy().to_string(),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
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
                    self.verify_channel_groups().await?;
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

    async fn refresh_provider(&self, subscription: &Subscription) -> Result<()> {
        let path = format!(
            "/providers/proxies/{}",
            urlencoding::encode(&provider_name(&subscription.id))
        );
        match self.mihomo_request(reqwest::Method::PUT, &path, None).await {
            Ok(_) => {}
            Err(first_error) if is_not_found_error(&first_error) => {
                self.apply_runtime().await?;
                self.mihomo_request(reqwest::Method::PUT, &path, None)
                    .await
                    .with_context(|| {
                        format!(
                            "刷新订阅失败：{}。核心重启后仍找不到 provider {}",
                            subscription.name,
                            provider_name(&subscription.id)
                        )
                    })?;
            }
            Err(error) => {
                return Err(error).with_context(|| format!("刷新订阅失败：{}", subscription.name));
            }
        }
        Ok(())
    }

    async fn normalize_runtime_ports(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let mut changed = false;
        let mut reserved = HashSet::new();

        if !port_is_usable(inner.config.controller_port, &reserved) {
            inner.config.controller_port =
                allocate_available_port(inner.config.port_range_start, &mut reserved);
            changed = true;
        } else {
            reserved.insert(inner.config.controller_port);
        }

        let port_range_start = inner.config.port_range_start;
        for channel in &mut inner.config.channels {
            if !port_is_usable(channel.http_port, &reserved) {
                channel.http_port = allocate_available_port(port_range_start, &mut reserved);
                changed = true;
            } else {
                reserved.insert(channel.http_port);
            }

            if !port_is_usable(channel.socks_port, &reserved) {
                channel.socks_port = allocate_available_port(port_range_start, &mut reserved);
                changed = true;
            } else {
                reserved.insert(channel.socks_port);
            }

            if !port_is_usable(channel.mihomo_http_port, &reserved) {
                channel.mihomo_http_port = allocate_available_port(port_range_start, &mut reserved);
                changed = true;
            } else {
                reserved.insert(channel.mihomo_http_port);
            }

            if !port_is_usable(channel.mihomo_socks_port, &reserved) {
                channel.mihomo_socks_port =
                    allocate_available_port(port_range_start, &mut reserved);
                changed = true;
            } else {
                reserved.insert(channel.mihomo_socks_port);
            }
        }

        if changed {
            inner.status_message = Some("检测到端口占用，已自动重新分配运行端口".to_string());
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        Ok(())
    }

    async fn verify_channel_groups(&self) -> Result<()> {
        let expected = {
            let inner = self.inner.lock().await;
            inner
                .config
                .channels
                .iter()
                .map(|channel| (channel.name.clone(), group_name(&channel.id)))
                .collect::<Vec<_>>()
        };
        if expected.is_empty() {
            return Ok(());
        }

        let value = self
            .mihomo_request(reqwest::Method::GET, "/proxies", None)
            .await?;
        let proxies = value
            .get("proxies")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("mihomo /proxies 响应缺少 proxies 字段"))?;

        let missing = expected
            .into_iter()
            .find(|(_, group)| !proxies.contains_key(group));
        if let Some((channel_name, group)) = missing {
            bail!("mihomo 当前配置缺少通道组 {group}（{channel_name}）");
        }
        Ok(())
    }

    async fn refresh_node_cache(&self) -> Result<Vec<NodeInfo>> {
        let nodes = self.fetch_nodes().await?;
        self.sanitize_channel_selections(&nodes).await?;
        self.sync_channel_selections().await;
        let mut inner = self.inner.lock().await;
        inner.nodes = nodes.clone();
        Ok(nodes)
    }

    async fn sanitize_channel_selections(&self, nodes: &[NodeInfo]) -> Result<()> {
        let mut inner = self.inner.lock().await;
        if sanitize_channel_selected_nodes(&mut inner.config, nodes) {
            inner.status_message = Some("部分代理节点已失效，已切换为 DIRECT".to_string());
            save_config(&self.paths.app_config_path, &inner.config)?;
        }
        Ok(())
    }

    async fn sync_channel_selections(&self) {
        let selections = {
            let inner = self.inner.lock().await;
            inner
                .config
                .channels
                .iter()
                .map(|channel| {
                    (
                        channel.id.clone(),
                        effective_node_for(&inner.config, channel),
                    )
                })
                .collect::<Vec<_>>()
        };

        for (channel_id, node) in selections {
            match self.select_channel_node(&channel_id, &node).await {
                Ok(()) => self.clear_channel_error(&channel_id).await,
                Err(err) => {
                    self.remember_channel_error(&channel_id, format!("{err:#}"))
                        .await;
                }
            }
        }
    }

    async fn select_effective_channel_node(&self, channel_id: &str) -> Result<()> {
        let node = {
            let inner = self.inner.lock().await;
            let channel = inner
                .config
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .ok_or_else(|| anyhow!("找不到代理 {channel_id}"))?;
            effective_node_for(&inner.config, channel)
        };
        let result = self.select_channel_node(channel_id, &node).await;
        match &result {
            Ok(()) => self.clear_channel_error(channel_id).await,
            Err(err) => {
                self.remember_channel_error(channel_id, format!("{err:#}"))
                    .await
            }
        }
        result
    }

    async fn select_channel_node(&self, channel_id: &str, node: &str) -> Result<()> {
        let path = format!("/proxies/{}", urlencoding::encode(&group_name(channel_id)));
        self.mihomo_request(
            reqwest::Method::PUT,
            &path,
            Some(serde_json::json!({ "name": node })),
        )
        .await
        .map(|_| ())
        .with_context(|| format!("切换通道节点失败：{node}"))
    }

    async fn fetch_nodes(&self) -> Result<Vec<NodeInfo>> {
        let provider_sources = self.fetch_provider_sources().await;
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
            let (provider_id, provider_name) =
                provider_sources.get(name).cloned().unwrap_or((None, None));
            nodes.push(NodeInfo {
                name: name.clone(),
                node_type,
                delay,
                is_builtin: false,
                provider_id,
                provider_name,
            });
        }
        nodes.sort_by(|a, b| {
            a.is_builtin
                .cmp(&b.is_builtin)
                .reverse()
                .then(a.provider_name.cmp(&b.provider_name))
                .then(a.name.cmp(&b.name))
        });
        Ok(nodes)
    }

    async fn fetch_provider_sources(&self) -> HashMap<String, (Option<String>, Option<String>)> {
        let (subscription_lookup, fallback_subscription) = {
            let inner = self.inner.lock().await;
            let lookup = inner
                .config
                .subscriptions
                .iter()
                .map(|subscription| {
                    (
                        provider_name(&subscription.id),
                        (subscription.id.clone(), subscription.name.clone()),
                    )
                })
                .collect::<HashMap<_, _>>();
            let fallback = if inner.config.subscriptions.len() == 1 {
                inner
                    .config
                    .subscriptions
                    .first()
                    .map(|subscription| (subscription.id.clone(), subscription.name.clone()))
            } else {
                None
            };
            (lookup, fallback)
        };

        let Ok(value) = self
            .mihomo_request(reqwest::Method::GET, "/providers/proxies", None)
            .await
        else {
            return HashMap::new();
        };

        let Some(providers) = value.get("providers").and_then(Value::as_object) else {
            return HashMap::new();
        };

        let mut sources = HashMap::new();
        for (provider_key, provider_value) in providers {
            let source = subscription_lookup
                .get(provider_key)
                .cloned()
                .or_else(|| fallback_subscription.clone());
            let Some((subscription_id, subscription_name)) = source else {
                continue;
            };
            let Some(proxies) = provider_value.get("proxies").and_then(Value::as_array) else {
                continue;
            };
            for proxy in proxies {
                if let Some(name) = proxy.get("name").and_then(Value::as_str) {
                    sources.insert(
                        name.to_string(),
                        (
                            Some(subscription_id.clone()),
                            Some(subscription_name.clone()),
                        ),
                    );
                }
            }
        }
        sources
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
        let mut request = self.local_client.request(method, url).bearer_auth(secret);
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

    async fn remember_channel_error(&self, channel_id: &str, error: String) {
        let mut inner = self.inner.lock().await;
        inner.last_errors.insert(channel_id.to_string(), error);
    }

    async fn clear_channel_error(&self, channel_id: &str) {
        let mut inner = self.inner.lock().await;
        inner.last_errors.remove(channel_id);
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

fn collect_stats(inner: &mut InnerState) -> Vec<ChannelStats> {
    inner
        .config
        .channels
        .clone()
        .iter()
        .map(|channel| stats_for_channel(inner, &channel.id))
        .collect()
}

fn stats_for_channel(inner: &mut InnerState, channel_id: &str) -> ChannelStats {
    let metrics = inner
        .metrics
        .entry(channel_id.to_string())
        .or_insert_with(|| Arc::new(StdMutex::new(ChannelMetrics::new(channel_id.to_string()))))
        .clone();
    metrics
        .lock()
        .map(|mut guard| guard.snapshot())
        .unwrap_or_else(|_| empty_stats(channel_id))
}

fn empty_stats(channel_id: &str) -> ChannelStats {
    ChannelStats {
        channel_id: channel_id.to_string(),
        upload_total: 0,
        download_total: 0,
        upload_speed: 0.0,
        download_speed: 0.0,
        active_connections: 0,
        recent_targets: Vec::new(),
    }
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
    let mut config = serde_json::from_str::<AppConfig>(&raw)?;
    let needs_http_api_defaults = !raw.contains("\"http_api_token\"")
        || !raw.contains("\"http_api_port\"")
        || !raw.contains("\"http_api_enabled\"");
    if migrate_config(&mut config) || needs_http_api_defaults {
        save_config(path, &config)?;
    }
    Ok(config)
}

fn migrate_config(config: &mut AppConfig) -> bool {
    let mut changed = false;
    if config.channels.is_empty() && !config.legacy_rules.is_empty() {
        let mut used = used_ports(config);
        for legacy in std::mem::take(&mut config.legacy_rules) {
            let http_port = if legacy.meter_port > 0 && !used.contains(&legacy.meter_port) {
                used.insert(legacy.meter_port);
                legacy.meter_port
            } else {
                allocate_available_port(config.port_range_start, &mut used)
            };
            let mihomo_http_port = if legacy.mihomo_port > 0 && !used.contains(&legacy.mihomo_port)
            {
                used.insert(legacy.mihomo_port);
                legacy.mihomo_port
            } else {
                allocate_available_port(config.port_range_start, &mut used)
            };
            let socks_port = allocate_available_port(config.port_range_start, &mut used);
            let mihomo_socks_port = allocate_available_port(config.port_range_start, &mut used);
            config.channels.push(ProxyChannel {
                id: legacy.id,
                name: legacy.name,
                selected_node: normalize_node(legacy.selected_node),
                enabled: legacy.enabled,
                http_port,
                socks_port,
                mihomo_http_port,
                mihomo_socks_port,
            });
        }
        changed = true;
    } else if !config.legacy_rules.is_empty() {
        config.legacy_rules.clear();
        changed = true;
    }

    if config.subscriptions.is_empty() {
        if let Some(url) = config
            .subscription_url
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            config.subscriptions.push(Subscription {
                id: "default".to_string(),
                name: "默认订阅".to_string(),
                url,
            });
            changed = true;
        }
    }
    if config.subscription_url.is_some() {
        config.subscription_url = None;
        changed = true;
    }
    changed
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

fn validate_subscription_input(input: &SubscriptionInput) -> Result<()> {
    if input.name.trim().is_empty() {
        bail!("订阅名称不能为空");
    }
    if input.url.trim().is_empty() {
        bail!("订阅地址不能为空");
    }
    Ok(())
}

fn validate_channel_input(input: &ChannelInput) -> Result<()> {
    if input.name.trim().is_empty() {
        bail!("代理名不能为空");
    }
    Ok(())
}

fn validate_http_api_port(
    config: &AppConfig,
    port: u16,
    enabled: bool,
    current_server_owns_port: bool,
) -> Result<()> {
    if port == 0 {
        bail!("HTTP API 端口无效");
    }
    if port == config.controller_port {
        bail!("HTTP API 端口不能与 mihomo 控制端口 {port} 相同");
    }
    for channel in &config.channels {
        if [
            channel.http_port,
            channel.socks_port,
            channel.mihomo_http_port,
            channel.mihomo_socks_port,
        ]
        .contains(&port)
        {
            bail!("HTTP API 端口 {port} 已被代理「{}」使用", channel.name);
        }
    }
    if enabled && !current_server_owns_port {
        let listener = StdTcpListener::bind(("127.0.0.1", port))
            .with_context(|| format!("HTTP API 端口 {port} 已被占用"))?;
        drop(listener);
    }
    Ok(())
}

fn normalize_node(node: Option<String>) -> Option<String> {
    node.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| Some("DIRECT".to_string()))
}

fn sanitize_channel_selected_nodes(config: &mut AppConfig, nodes: &[NodeInfo]) -> bool {
    let mut available = nodes
        .iter()
        .map(|node| node.name.as_str())
        .collect::<HashSet<_>>();
    available.insert("DIRECT");

    let mut changed = false;
    for channel in &mut config.channels {
        let selected = selected_node_name(channel);
        if !available.contains(selected.as_str()) {
            channel.selected_node = Some("DIRECT".to_string());
            changed = true;
        }
    }
    changed
}

fn selected_node_name(channel: &ProxyChannel) -> String {
    channel
        .selected_node
        .clone()
        .filter(|node| !node.trim().is_empty())
        .unwrap_or_else(|| "DIRECT".to_string())
}

fn effective_node_for(config: &AppConfig, channel: &ProxyChannel) -> String {
    if config.global_proxy_enabled && channel.enabled {
        selected_node_name(channel)
    } else {
        "DIRECT".to_string()
    }
}

fn network_mode_for(effective_node: &str) -> String {
    if effective_node.eq_ignore_ascii_case("DIRECT") {
        "Direct".to_string()
    } else {
        "Proxy".to_string()
    }
}

#[derive(Clone, Copy)]
enum TestExpectation {
    JsonIp,
    Reachable,
}

async fn run_proxy_protocol_test(
    protocol: &str,
    proxy_url: String,
) -> Result<ProxyProtocolTestResult> {
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .proxy(reqwest::Proxy::all(&proxy_url)?)
        .build()?;
    let started = Instant::now();
    let (ip_test, ip) = run_proxy_endpoint_test(
        &client,
        "出口 IP",
        "https://api.ipify.org?format=json",
        TestExpectation::JsonIp,
    )
    .await;
    let (google_test, _) = run_proxy_endpoint_test(
        &client,
        "Google",
        "https://www.google.com/generate_204",
        TestExpectation::Reachable,
    )
    .await;
    let (openai_test, _) = run_proxy_endpoint_test(
        &client,
        "OpenAI",
        "https://api.openai.com/v1/models",
        TestExpectation::Reachable,
    )
    .await;
    let elapsed_ms = started.elapsed().as_millis();
    let tests = vec![ip_test, google_test, openai_test];
    let success = tests.iter().all(|test| test.success);
    let error = if success {
        None
    } else {
        Some(
            tests
                .iter()
                .filter(|test| !test.success)
                .map(|test| {
                    format!(
                        "{}: {}",
                        test.name,
                        test.error.clone().unwrap_or_else(|| "连接失败".to_string())
                    )
                })
                .collect::<Vec<_>>()
                .join("；"),
        )
    };

    Ok(ProxyProtocolTestResult {
        protocol: protocol.to_string(),
        proxy_url,
        success,
        ip,
        elapsed_ms,
        error,
        tests,
    })
}

async fn run_proxy_endpoint_test(
    client: &Client,
    name: &str,
    url: &str,
    expectation: TestExpectation,
) -> (ProxyEndpointResult, Option<String>) {
    let started = Instant::now();
    let response = client.get(url).send().await;
    let elapsed_ms = started.elapsed().as_millis();
    match response {
        Ok(response) => {
            let status = response.status();
            let status_code = status.as_u16();
            let text = response.text().await.unwrap_or_default();
            match expectation {
                TestExpectation::JsonIp => {
                    let ip = parse_ip_response(&text);
                    let success = status.is_success() && ip.is_some();
                    (
                        ProxyEndpointResult {
                            name: name.to_string(),
                            url: url.to_string(),
                            success,
                            status: Some(status_code),
                            elapsed_ms,
                            error: if success {
                                None
                            } else {
                                Some(format!("出口 IP 响应无效：{status}"))
                            },
                        },
                        ip,
                    )
                }
                TestExpectation::Reachable => {
                    let success = status.is_success()
                        || status == reqwest::StatusCode::UNAUTHORIZED
                        || status == reqwest::StatusCode::FORBIDDEN;
                    (
                        ProxyEndpointResult {
                            name: name.to_string(),
                            url: url.to_string(),
                            success,
                            status: Some(status_code),
                            elapsed_ms,
                            error: if success {
                                None
                            } else {
                                Some(format!("HTTP {status}: {text}"))
                            },
                        },
                        None,
                    )
                }
            }
        }
        Err(err) => (
            ProxyEndpointResult {
                name: name.to_string(),
                url: url.to_string(),
                success: false,
                status: None,
                elapsed_ms,
                error: Some(format!("{err:#}")),
            },
            None,
        ),
    }
}

fn parse_ip_response(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value.get("ip").and_then(Value::as_str).map(str::to_string))
        .or_else(|| {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
}

fn is_not_found_error(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("404") || text.contains("Resource not found")
}

fn allocate_channel_ports(config: &AppConfig) -> [u16; 4] {
    let mut used = used_ports(config);
    [
        allocate_available_port(config.port_range_start, &mut used),
        allocate_available_port(config.port_range_start, &mut used),
        allocate_available_port(config.port_range_start, &mut used),
        allocate_available_port(config.port_range_start, &mut used),
    ]
}

fn used_ports(config: &AppConfig) -> HashSet<u16> {
    let mut used = HashSet::new();
    used.insert(config.controller_port);
    used.insert(config.http_api_port);
    for channel in &config.channels {
        used.insert(channel.http_port);
        used.insert(channel.socks_port);
        used.insert(channel.mihomo_http_port);
        used.insert(channel.mihomo_socks_port);
    }
    used
}

fn port_is_usable(port: u16, reserved: &HashSet<u16>) -> bool {
    !reserved.contains(&port) && portpicker::is_free_tcp(port)
}

fn allocate_available_port(start: u16, reserved: &mut HashSet<u16>) -> u16 {
    for port in start..=u16::MAX {
        if reserved.contains(&port) {
            continue;
        }
        if portpicker::is_free_tcp(port) {
            reserved.insert(port);
            return port;
        }
    }
    start
}

fn next_duplicate_name(channels: &[ProxyChannel], base: &str) -> String {
    let mut index = 2;
    let mut name = format!("{base} 副本");
    while channels.iter().any(|channel| channel.name == name) {
        name = format!("{base} 副本 {index}");
        index += 1;
    }
    name
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
        provider_id: None,
        provider_name: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_distinct_ports() {
        let mut config = AppConfig::default();
        config.port_range_start = 22000;
        config.channels.push(ProxyChannel {
            id: "one".into(),
            name: "one".into(),
            selected_node: None,
            enabled: true,
            http_port: 22000,
            socks_port: 22001,
            mihomo_http_port: 22002,
            mihomo_socks_port: 22003,
        });
        let ports = allocate_channel_ports(&config);
        assert_eq!(ports, [22004, 22005, 22006, 22007]);
    }

    #[test]
    fn switch_off_uses_direct_without_losing_selected_node() {
        let mut config = AppConfig::default();
        let mut channel = ProxyChannel {
            id: "one".into(),
            name: "one".into(),
            selected_node: Some("HK".into()),
            enabled: false,
            http_port: 22000,
            socks_port: 22001,
            mihomo_http_port: 22002,
            mihomo_socks_port: 22003,
        };
        assert_eq!(effective_node_for(&config, &channel), "DIRECT");
        channel.enabled = true;
        assert_eq!(effective_node_for(&config, &channel), "HK");
        config.global_proxy_enabled = false;
        assert_eq!(effective_node_for(&config, &channel), "DIRECT");
        assert_eq!(selected_node_name(&channel), "HK");
    }

    #[test]
    fn rejects_http_api_port_conflicts() {
        let mut config = AppConfig::default();
        config.controller_port = 25000;
        config.channels.push(ProxyChannel {
            id: "one".into(),
            name: "Chrome".into(),
            selected_node: Some("HK".into()),
            enabled: true,
            http_port: 25001,
            socks_port: 25002,
            mihomo_http_port: 25003,
            mihomo_socks_port: 25004,
        });

        assert!(validate_http_api_port(&config, 0, false, false).is_err());
        assert!(validate_http_api_port(&config, 25000, false, false).is_err());
        assert!(validate_http_api_port(&config, 25002, false, false).is_err());
        assert!(validate_http_api_port(&config, 25005, false, false).is_ok());
    }

    #[test]
    fn sanitizes_missing_selected_nodes() {
        let mut config = AppConfig::default();
        config.channels.push(ProxyChannel {
            id: "one".into(),
            name: "Chrome".into(),
            selected_node: Some("Removed Node".into()),
            enabled: true,
            http_port: 25001,
            socks_port: 25002,
            mihomo_http_port: 25003,
            mihomo_socks_port: 25004,
        });
        let nodes = vec![
            direct_node(),
            NodeInfo {
                name: "HK".into(),
                node_type: "Proxy".into(),
                delay: None,
                is_builtin: false,
                provider_id: Some("sub".into()),
                provider_name: Some("Sub".into()),
            },
        ];

        assert!(sanitize_channel_selected_nodes(&mut config, &nodes));
        assert_eq!(config.channels[0].selected_node.as_deref(), Some("DIRECT"));
        assert!(!sanitize_channel_selected_nodes(&mut config, &nodes));
    }

    #[test]
    fn migrates_legacy_subscription_url() {
        let mut config = AppConfig::default();
        config.subscription_url = Some("https://example.com/sub".into());
        assert!(migrate_config(&mut config));
        assert!(config.subscription_url.is_none());
        assert_eq!(config.subscriptions.len(), 1);
        assert_eq!(config.subscriptions[0].name, "默认订阅");
    }

    #[test]
    fn migrates_legacy_rules_to_channels() {
        let mut config = AppConfig::default();
        config.port_range_start = 23000;
        config.legacy_rules.push(crate::models::LegacyRule {
            id: "old".into(),
            name: "Chrome".into(),
            selected_node: Some("HK".into()),
            enabled: true,
            meter_port: 23000,
            mihomo_port: 23001,
        });
        assert!(migrate_config(&mut config));
        assert!(config.legacy_rules.is_empty());
        assert_eq!(config.channels.len(), 1);
        assert_eq!(config.channels[0].http_port, 23000);
        assert_eq!(config.channels[0].mihomo_http_port, 23001);
        assert_eq!(config.channels[0].socks_port, 23002);
        assert_eq!(config.channels[0].mihomo_socks_port, 23003);
    }
}


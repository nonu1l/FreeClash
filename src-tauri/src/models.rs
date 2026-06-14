use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_global_proxy_enabled")]
    pub global_proxy_enabled: bool,
    #[serde(default)]
    pub subscription_url: Option<String>,
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
    #[serde(default = "default_controller_port")]
    pub controller_port: u16,
    #[serde(default = "default_controller_secret")]
    pub controller_secret: String,
    #[serde(default = "default_port_range_start")]
    pub port_range_start: u16,
    #[serde(default)]
    pub http_api_enabled: bool,
    #[serde(default = "default_http_api_port")]
    pub http_api_port: u16,
    #[serde(default = "default_http_api_token")]
    pub http_api_token: String,
    #[serde(default)]
    pub channels: Vec<ProxyChannel>,
    #[serde(default, rename = "rules", skip_serializing)]
    pub legacy_rules: Vec<LegacyRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyChannel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub selected_node: Option<String>,
    #[serde(default = "default_channel_enabled")]
    pub enabled: bool,
    pub http_port: u16,
    pub socks_port: u16,
    pub mihomo_http_port: u16,
    pub mihomo_socks_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyRule {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub selected_node: Option<String>,
    #[serde(default = "default_channel_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub meter_port: u16,
    #[serde(default)]
    pub mihomo_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionInput {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInput {
    pub name: String,
    pub selected_node: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeInfo {
    pub name: String,
    pub node_type: String,
    pub delay: Option<u32>,
    pub is_builtin: bool,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelConnection {
    pub id: String,
    pub target: String,
    pub method: String,
    pub started_at: i64,
    pub upload: u64,
    pub download: u64,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelStats {
    pub channel_id: String,
    pub upload_total: u64,
    pub download_total: u64,
    pub upload_speed: f64,
    pub download_speed: f64,
    pub active_connections: usize,
    pub recent_targets: Vec<ChannelConnection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub core_path: String,
    pub config_path: String,
    pub runtime_dir: String,
    pub controller_url: String,
    pub core_running: bool,
    pub core_version: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSnapshot {
    pub config: AppConfig,
    pub nodes: Vec<NodeInfo>,
    pub stats: Vec<ChannelStats>,
    pub status: RuntimeStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct DelayResult {
    pub node: String,
    pub delay: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelDiagnostics {
    pub channel_id: String,
    pub channel_name: String,
    pub selected_node: String,
    pub effective_node: String,
    pub global_proxy_enabled: bool,
    pub channel_proxy_enabled: bool,
    pub core_running: bool,
    pub network_mode: String,
    pub http_url: String,
    pub socks_url: String,
    pub http_port: u16,
    pub socks_port: u16,
    pub mihomo_http_port: u16,
    pub mihomo_socks_port: u16,
    pub stats: ChannelStats,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelProxyTestResult {
    pub channel_id: String,
    pub network_mode: String,
    pub effective_node: String,
    pub success: bool,
    pub elapsed_ms: u128,
    pub error: Option<String>,
    pub entries: Vec<ProxyProtocolTestResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyProtocolTestResult {
    pub protocol: String,
    pub proxy_url: String,
    pub success: bool,
    pub ip: Option<String>,
    pub elapsed_ms: u128,
    pub error: Option<String>,
    pub tests: Vec<ProxyEndpointResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyEndpointResult {
    pub name: String,
    pub url: String,
    pub success: bool,
    pub status: Option<u16>,
    pub elapsed_ms: u128,
    pub error: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            global_proxy_enabled: default_global_proxy_enabled(),
            subscription_url: None,
            subscriptions: Vec::new(),
            controller_port: default_controller_port(),
            controller_secret: default_controller_secret(),
            port_range_start: default_port_range_start(),
            http_api_enabled: false,
            http_api_port: default_http_api_port(),
            http_api_token: default_http_api_token(),
            channels: Vec::new(),
            legacy_rules: Vec::new(),
        }
    }
}

fn default_global_proxy_enabled() -> bool {
    true
}

fn default_controller_port() -> u16 {
    19090
}

fn default_controller_secret() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_port_range_start() -> u16 {
    19100
}

fn default_http_api_port() -> u16 {
    19290
}

fn default_http_api_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_channel_enabled() -> bool {
    true
}

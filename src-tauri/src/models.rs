use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub subscription: Option<Subscription>,
    #[serde(default = "default_controller_port")]
    pub controller_port: u16,
    #[serde(default = "default_controller_secret")]
    pub controller_secret: String,
    #[serde(default = "default_port_range_start")]
    pub port_range_start: u16,
    #[serde(default)]
    pub pinned_nodes: Vec<PinnedNode>,
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
pub struct PinnedNode {
    pub node_name: String,
    pub port: u16,
    pub mihomo_http_port: u16,
    pub mihomo_socks_port: u16,
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
pub struct PinRuntime {
    pub node_name: String,
    pub port: u16,
    pub port_available: bool,
    pub port_error: Option<String>,
    pub stats: PinStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct PinConnection {
    pub id: String,
    pub target: String,
    pub method: String,
    pub started_at: i64,
    pub upload: u64,
    pub download: u64,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PinStats {
    pub node_name: String,
    pub upload_total: u64,
    pub download_total: u64,
    pub upload_speed: f64,
    pub download_speed: f64,
    pub active_connections: usize,
    pub recent_targets: Vec<PinConnection>,
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
    pub pins: Vec<PinRuntime>,
    pub status: RuntimeStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct DelayResult {
    pub node: String,
    pub delay: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            subscription: None,
            controller_port: default_controller_port(),
            controller_secret: default_controller_secret(),
            port_range_start: default_port_range_start(),
            pinned_nodes: Vec::new(),
        }
    }
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

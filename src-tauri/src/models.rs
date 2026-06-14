use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub subscription_url: Option<String>,
    pub controller_port: u16,
    pub controller_secret: String,
    pub port_range_start: u16,
    pub rules: Vec<AppRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRule {
    pub id: String,
    pub name: String,
    pub app_path: String,
    pub args: String,
    pub working_dir: String,
    pub selected_node: Option<String>,
    pub enabled: bool,
    pub meter_port: u16,
    pub mihomo_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleInput {
    pub name: String,
    pub app_path: String,
    pub args: String,
    pub working_dir: String,
    pub selected_node: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeInfo {
    pub name: String,
    pub node_type: String,
    pub delay: Option<u32>,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleConnection {
    pub id: String,
    pub target: String,
    pub method: String,
    pub started_at: i64,
    pub upload: u64,
    pub download: u64,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleStats {
    pub rule_id: String,
    pub upload_total: u64,
    pub download_total: u64,
    pub upload_speed: f64,
    pub download_speed: f64,
    pub active_connections: usize,
    pub recent_targets: Vec<RuleConnection>,
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
    pub stats: Vec<RuleStats>,
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
            subscription_url: None,
            controller_port: 19090,
            controller_secret: uuid::Uuid::new_v4().to_string(),
            port_range_start: 19100,
            rules: Vec::new(),
        }
    }
}


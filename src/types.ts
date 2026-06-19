export interface AppConfig {
  subscription: Subscription | null;
  controller_port: number;
  controller_secret: string;
  port_range_start: number;
  pinned_nodes: PinnedNode[];
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
}

export interface SubscriptionInput {
  name: string;
  url: string;
}

export interface PinnedNode {
  node_name: string;
  port: number;
  mihomo_http_port: number;
  mihomo_socks_port: number;
}

export interface NodeInfo {
  name: string;
  node_type: string;
  delay: number | null;
  is_builtin: boolean;
  provider_id: string | null;
  provider_name: string | null;
}

export interface PinConnection {
  id: string;
  target: string;
  method: string;
  started_at: number;
  upload: number;
  download: number;
  active: boolean;
}

export interface PinStats {
  node_name: string;
  upload_total: number;
  download_total: number;
  upload_speed: number;
  download_speed: number;
  active_connections: number;
  recent_targets: PinConnection[];
}

export interface PinRuntime {
  node_name: string;
  port: number;
  port_available: boolean;
  port_error: string | null;
  stats: PinStats;
}

export interface RuntimeStatus {
  core_path: string;
  config_path: string;
  runtime_dir: string;
  controller_url: string;
  core_running: boolean;
  core_version: string | null;
  message: string | null;
}

export interface AppSnapshot {
  config: AppConfig;
  nodes: NodeInfo[];
  pins: PinRuntime[];
  status: RuntimeStatus;
}

export interface DelayResult {
  node: string;
  delay: number;
}

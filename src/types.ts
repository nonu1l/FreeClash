export interface ProxyChannel {
  id: string;
  name: string;
  selected_node: string | null;
  enabled: boolean;
  http_port: number;
  socks_port: number;
  mihomo_http_port: number;
  mihomo_socks_port: number;
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
}

export interface SubscriptionInput {
  name: string;
  url: string;
  enabled: boolean;
}

export interface AppConfig {
  global_proxy_enabled: boolean;
  subscription_url: string | null;
  subscriptions: Subscription[];
  controller_port: number;
  controller_secret: string;
  port_range_start: number;
  http_api_enabled: boolean;
  http_api_port: number;
  http_api_token: string;
  channels: ProxyChannel[];
}

export interface NodeInfo {
  name: string;
  node_type: string;
  delay: number | null;
  is_builtin: boolean;
  provider_id: string | null;
  provider_name: string | null;
}

export interface ChannelConnection {
  id: string;
  target: string;
  method: string;
  started_at: number;
  upload: number;
  download: number;
  active: boolean;
}

export interface ChannelStats {
  channel_id: string;
  upload_total: number;
  download_total: number;
  upload_speed: number;
  download_speed: number;
  active_connections: number;
  recent_targets: ChannelConnection[];
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
  stats: ChannelStats[];
  status: RuntimeStatus;
}

export interface ChannelDraft {
  id?: string;
  name: string;
  selected_node: string | null;
  enabled: boolean;
}

export type ChannelInput = Omit<ChannelDraft, "id">;

export interface SubscriptionDraft {
  id?: string;
  name: string;
  url: string;
  enabled: boolean;
}

export interface DelayResult {
  node: string;
  delay: number;
}

export interface ChannelDiagnostics {
  channel_id: string;
  channel_name: string;
  selected_node: string;
  effective_node: string;
  global_proxy_enabled: boolean;
  channel_proxy_enabled: boolean;
  core_running: boolean;
  network_mode: string;
  http_url: string;
  socks_url: string;
  http_port: number;
  socks_port: number;
  mihomo_http_port: number;
  mihomo_socks_port: number;
  stats: ChannelStats;
  last_error: string | null;
}

export interface ChannelProxyTestResult {
  channel_id: string;
  network_mode: string;
  effective_node: string;
  success: boolean;
  elapsed_ms: number;
  error: string | null;
  entries: ProxyProtocolTestResult[];
}

export interface ProxyProtocolTestResult {
  protocol: string;
  proxy_url: string;
  success: boolean;
  ip: string | null;
  elapsed_ms: number;
  error: string | null;
  tests: ProxyEndpointResult[];
}

export interface ProxyEndpointResult {
  name: string;
  url: string;
  success: boolean;
  status: number | null;
  elapsed_ms: number;
  error: string | null;
}

export type NodeFilter = "all" | "available" | "untested" | "high";
export type ActiveView = "subscriptions" | "channels" | "connections" | "settings";

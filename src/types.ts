export interface AppRule {
  id: string;
  name: string;
  app_path: string;
  args: string;
  working_dir: string;
  selected_node: string | null;
  enabled: boolean;
  meter_port: number;
  mihomo_port: number;
}

export interface AppConfig {
  subscription_url: string | null;
  controller_port: number;
  controller_secret: string;
  port_range_start: number;
  rules: AppRule[];
}

export interface NodeInfo {
  name: string;
  node_type: string;
  delay: number | null;
  is_builtin: boolean;
}

export interface RuleConnection {
  id: string;
  target: string;
  method: string;
  started_at: number;
  upload: number;
  download: number;
  active: boolean;
}

export interface RuleStats {
  rule_id: string;
  upload_total: number;
  download_total: number;
  upload_speed: number;
  download_speed: number;
  active_connections: number;
  recent_targets: RuleConnection[];
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
  stats: RuleStats[];
  status: RuntimeStatus;
}

export interface RuleDraft {
  id?: string;
  name: string;
  app_path: string;
  args: string;
  working_dir: string;
  selected_node: string | null;
  enabled: boolean;
}


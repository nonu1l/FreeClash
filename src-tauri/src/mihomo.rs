use std::collections::BTreeMap;

use serde::Serialize;

use crate::models::AppConfig;

const PROVIDER_NAME: &str = "FreeClash";

#[derive(Debug, Serialize)]
struct MihomoConfig {
    #[serde(rename = "allow-lan")]
    allow_lan: bool,
    mode: String,
    #[serde(rename = "log-level")]
    log_level: String,
    #[serde(rename = "external-controller")]
    external_controller: String,
    secret: String,
    profile: Profile,
    #[serde(rename = "proxy-providers", skip_serializing_if = "Option::is_none")]
    proxy_providers: Option<BTreeMap<String, ProxyProvider>>,
    #[serde(rename = "proxy-groups")]
    proxy_groups: Vec<ProxyGroup>,
    listeners: Vec<Listener>,
    rules: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Profile {
    #[serde(rename = "store-selected")]
    store_selected: bool,
}

#[derive(Debug, Serialize)]
struct ProxyProvider {
    #[serde(rename = "type")]
    kind: String,
    url: String,
    interval: u32,
    path: String,
    #[serde(rename = "health-check")]
    health_check: HealthCheck,
}

#[derive(Debug, Serialize)]
struct HealthCheck {
    enable: bool,
    interval: u32,
    url: String,
}

#[derive(Debug, Serialize)]
struct ProxyGroup {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    proxies: Vec<String>,
    #[serde(rename = "use", skip_serializing_if = "Vec::is_empty")]
    use_providers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Listener {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    listen: String,
    port: u16,
    proxy: String,
}

pub fn group_name(rule_id: &str) -> String {
    format!("fc-rule-{rule_id}")
}

pub fn listener_name(rule_id: &str) -> String {
    format!("fc-listener-{rule_id}")
}

pub fn render_config(config: &AppConfig) -> anyhow::Result<String> {
    let has_subscription = config
        .subscription_url
        .as_ref()
        .map(|url| !url.trim().is_empty())
        .unwrap_or(false);

    let proxy_providers = if has_subscription {
        let mut providers = BTreeMap::new();
        providers.insert(
            PROVIDER_NAME.to_string(),
            ProxyProvider {
                kind: "http".to_string(),
                url: config.subscription_url.clone().unwrap_or_default(),
                interval: 3600,
                path: "./providers/freeclash.yaml".to_string(),
                health_check: HealthCheck {
                    enable: true,
                    interval: 600,
                    url: "https://www.gstatic.com/generate_204".to_string(),
                },
            },
        );
        Some(providers)
    } else {
        None
    };

    let proxy_groups = config
        .rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(|rule| ProxyGroup {
            name: group_name(&rule.id),
            kind: "select".to_string(),
            proxies: vec!["DIRECT".to_string()],
            use_providers: if has_subscription {
                vec![PROVIDER_NAME.to_string()]
            } else {
                Vec::new()
            },
        })
        .collect();

    let listeners = config
        .rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(|rule| Listener {
            name: listener_name(&rule.id),
            kind: "http".to_string(),
            listen: "127.0.0.1".to_string(),
            port: rule.mihomo_port,
            proxy: group_name(&rule.id),
        })
        .collect();

    let mihomo = MihomoConfig {
        allow_lan: false,
        mode: "rule".to_string(),
        log_level: "info".to_string(),
        external_controller: format!("127.0.0.1:{}", config.controller_port),
        secret: config.controller_secret.clone(),
        profile: Profile {
            store_selected: true,
        },
        proxy_providers,
        proxy_groups,
        listeners,
        rules: vec!["MATCH,DIRECT".to_string()],
    };

    Ok(serde_yaml::to_string(&mihomo)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppConfig, AppRule};

    #[test]
    fn renders_listener_and_provider() {
        let mut config = AppConfig::default();
        config.subscription_url = Some("https://example.com/sub".into());
        config.rules.push(AppRule {
            id: "abc".into(),
            name: "Codex".into(),
            app_path: "codex.exe".into(),
            args: String::new(),
            working_dir: String::new(),
            selected_node: Some("HK".into()),
            enabled: true,
            meter_port: 19100,
            mihomo_port: 19101,
        });

        let yaml = render_config(&config).unwrap();
        assert!(yaml.contains("proxy-providers"));
        assert!(yaml.contains("fc-rule-abc"));
        assert!(yaml.contains("port: 19101"));
    }
}


use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde::Serialize;

use crate::models::AppConfig;

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

pub fn group_name(node_name: &str) -> String {
    format!("fc-pin-{}", stable_key(node_name))
}

pub fn http_listener_name(node_name: &str) -> String {
    format!("fc-http-{}", stable_key(node_name))
}

pub fn socks_listener_name(node_name: &str) -> String {
    format!("fc-socks-{}", stable_key(node_name))
}

pub fn provider_name(subscription_id: &str) -> String {
    format!("fc-provider-{subscription_id}")
}

pub fn render_config(config: &AppConfig) -> anyhow::Result<String> {
    let mut providers = BTreeMap::new();
    if let Some(subscription) = config
        .subscription
        .as_ref()
        .filter(|subscription| !subscription.url.trim().is_empty())
    {
        let name = provider_name(&subscription.id);
        providers.insert(
            name.clone(),
            ProxyProvider {
                kind: "http".to_string(),
                url: subscription.url.trim().to_string(),
                interval: 3600,
                path: format!("./providers/{name}.yaml"),
                health_check: HealthCheck {
                    enable: true,
                    interval: 600,
                    url: "https://www.gstatic.com/generate_204".to_string(),
                },
            },
        );
    }

    let provider_keys = providers.keys().cloned().collect::<Vec<_>>();
    let proxy_providers = if providers.is_empty() {
        None
    } else {
        Some(providers)
    };

    let proxy_groups = config
        .pinned_nodes
        .iter()
        .map(|pin| ProxyGroup {
            name: group_name(&pin.node_name),
            kind: "select".to_string(),
            proxies: vec!["DIRECT".to_string()],
            use_providers: provider_keys.clone(),
        })
        .collect();

    let mut listeners = Vec::new();
    for pin in &config.pinned_nodes {
        listeners.push(Listener {
            name: http_listener_name(&pin.node_name),
            kind: "http".to_string(),
            listen: "127.0.0.1".to_string(),
            port: pin.mihomo_http_port,
            proxy: group_name(&pin.node_name),
        });
        listeners.push(Listener {
            name: socks_listener_name(&pin.node_name),
            kind: "socks".to_string(),
            listen: "127.0.0.1".to_string(),
            port: pin.mihomo_socks_port,
            proxy: group_name(&pin.node_name),
        });
    }

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

fn stable_key(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppConfig, PinnedNode, Subscription};

    #[test]
    fn renders_http_and_socks_listeners_and_provider_for_pins() {
        let mut config = AppConfig::default();
        config.subscription = Some(Subscription {
            id: "sub1".into(),
            name: "Sub 1".into(),
            url: "https://example.com/sub".into(),
        });
        config.pinned_nodes.push(PinnedNode {
            node_name: "HK 01".into(),
            port: 19100,
            mihomo_http_port: 19101,
            mihomo_socks_port: 19102,
        });

        let yaml = render_config(&config).unwrap();
        assert!(yaml.contains("proxy-providers"));
        assert!(yaml.contains("fc-provider-sub1"));
        assert!(yaml.contains(&group_name("HK 01")));
        assert!(yaml.contains(&http_listener_name("HK 01")));
        assert!(yaml.contains(&socks_listener_name("HK 01")));
        assert!(yaml.contains("port: 19101"));
        assert!(yaml.contains("port: 19102"));
        assert!(yaml.contains("type: socks"));
    }
}

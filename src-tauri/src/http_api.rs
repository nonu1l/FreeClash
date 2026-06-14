use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{anyhow, bail, Result};
use axum::extract::{Path, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::manager::AppManager;
use crate::models::ChannelInput;

#[derive(Clone)]
struct HttpApiState {
    manager: AppManager,
    token: String,
}

#[derive(Debug, Deserialize)]
struct InvokeRequest {
    #[serde(default)]
    payload: Value,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    enabled: bool,
    port: u16,
    subscriptions: usize,
    channels: usize,
    core_running: bool,
}

pub fn start(manager: AppManager, port: u16, token: String) -> Result<oneshot::Sender<()>> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let std_listener = std::net::TcpListener::bind(address)
        .map_err(|err| anyhow!("HTTP API 绑定失败 {address}: {err}"))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|err| anyhow!("HTTP API 设置非阻塞监听失败 {address}: {err}"))?;
    let listener = TcpListener::from_std(std_listener)
        .map_err(|err| anyhow!("HTTP API 初始化监听失败 {address}: {err}"))?;
    let state = HttpApiState { manager, token };
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/invoke/{command}", post(invoke_command))
        .layer(cors_layer())
        .with_state(state);

    tauri::async_runtime::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(err) = server.await {
            eprintln!("FreeClash HTTP API stopped with error: {err}");
        }
    });

    Ok(shutdown_tx)
}

async fn health(State(state): State<HttpApiState>) -> impl IntoResponse {
    match state.manager.get_state().await {
        Ok(snapshot) => Json(HealthResponse {
            ok: true,
            enabled: snapshot.config.http_api_enabled,
            port: snapshot.config.http_api_port,
            subscriptions: snapshot.config.subscriptions.len(),
            channels: snapshot.config.channels.len(),
            core_running: snapshot.status.core_running,
        })
        .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(format!("{error:#}"))),
        )
            .into_response(),
    }
}

async fn invoke_command(
    State(state): State<HttpApiState>,
    Path(command): Path<String>,
    headers: HeaderMap,
    Json(request): Json<InvokeRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("HTTP API token 无效".to_string())),
        )
            .into_response();
    }

    match dispatch(&state.manager, &command, request.payload).await {
        Ok(data) => Json(ApiResponse::success(data)).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(format!("{error:#}"))),
        )
            .into_response(),
    }
}

async fn dispatch(manager: &AppManager, command: &str, payload: Value) -> Result<Value> {
    match command {
        "get_state" => to_value(manager.get_state().await?),
        "set_subscription" => {
            manager
                .set_subscription(optional_field(&payload, "url")?)
                .await?;
            Ok(Value::Null)
        }
        "create_subscription" => to_value(
            manager
                .create_subscription(field(&payload, "input")?)
                .await?,
        ),
        "update_subscription" => to_value(
            manager
                .update_subscription(
                    &field::<String>(&payload, "subscriptionId")?,
                    field(&payload, "input")?,
                )
                .await?,
        ),
        "delete_subscription" => {
            manager
                .delete_subscription(&field::<String>(&payload, "subscriptionId")?)
                .await?;
            Ok(Value::Null)
        }
        "refresh_subscription" => to_value(
            manager
                .refresh_subscription(&field::<String>(&payload, "subscriptionId")?)
                .await?,
        ),
        "refresh_nodes" => to_value(manager.refresh_nodes().await?),
        "set_global_proxy_enabled" => {
            manager
                .set_global_proxy_enabled(field(&payload, "enabled")?)
                .await?;
            Ok(Value::Null)
        }
        "create_channel" => to_value(manager.create_channel(field(&payload, "input")?).await?),
        "update_channel" => to_value(
            manager
                .update_channel(
                    &field::<String>(&payload, "channelId")?,
                    field::<ChannelInput>(&payload, "input")?,
                )
                .await?,
        ),
        "delete_channel" => {
            manager
                .delete_channel(&field::<String>(&payload, "channelId")?)
                .await?;
            Ok(Value::Null)
        }
        "set_channel_enabled" => to_value(
            manager
                .set_channel_enabled(
                    &field::<String>(&payload, "channelId")?,
                    field(&payload, "enabled")?,
                )
                .await?,
        ),
        "duplicate_channel" => to_value(
            manager
                .duplicate_channel(&field::<String>(&payload, "channelId")?)
                .await?,
        ),
        "set_channel_node" => {
            manager
                .set_channel_node(
                    &field::<String>(&payload, "channelId")?,
                    field(&payload, "node")?,
                )
                .await?;
            Ok(Value::Null)
        }
        "get_channel_stats" => {
            let state = manager.get_state().await?;
            to_value(state.stats)
        }
        "list_channel_connections" => {
            let channel_id = field::<String>(&payload, "channelId")?;
            let state = manager.get_state().await?;
            let connections = state
                .stats
                .into_iter()
                .find(|stats| stats.channel_id == channel_id)
                .map(|stats| stats.recent_targets)
                .unwrap_or_default();
            to_value(connections)
        }
        "diagnose_channel" => to_value(
            manager
                .diagnose_channel(&field::<String>(&payload, "channelId")?)
                .await?,
        ),
        "test_channel_proxy" => to_value(
            manager
                .test_channel_proxy(&field::<String>(&payload, "channelId")?)
                .await?,
        ),
        "restart_core" => {
            manager.restart_core().await?;
            Ok(Value::Null)
        }
        "test_node_delay" => to_value(manager.test_node_delay(field(&payload, "node")?).await?),
        "set_http_api_config" => {
            manager
                .set_http_api_config(field(&payload, "enabled")?, field(&payload, "port")?)
                .await?;
            Ok(Value::Null)
        }
        _ => bail!("未知 HTTP API command: {command}"),
    }
}

fn field<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<T> {
    let value = payload
        .get(key)
        .cloned()
        .ok_or_else(|| anyhow!("缺少参数 {key}"))?;
    Ok(serde_json::from_value(value)?)
}

fn optional_field<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<Option<T>> {
    match payload.get(key).cloned() {
        Some(Value::Null) | None => Ok(None),
        Some(value) => Ok(Some(serde_json::from_value(value)?)),
    }
}

fn to_value<T: Serialize>(value: T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

fn is_authorized(headers: &HeaderMap, token: &str) -> bool {
    let expected = format!("Bearer {token}");
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(|value| value == expected)
        .unwrap_or(false)
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_origin(AllowOrigin::predicate(|origin, _| is_local_origin(origin)))
}

fn is_local_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    is_local_origin_str(origin)
}

fn is_local_origin_str(origin: &str) -> bool {
    origin.starts_with("http://127.0.0.1:")
        || origin.starts_with("http://localhost:")
        || origin == "http://127.0.0.1"
        || origin == "http://localhost"
}

impl ApiResponse {
    fn success(data: Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(error: String) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn local_origin_predicate_accepts_localhost() {
        assert!(is_local_origin_str("http://127.0.0.1:5173"));
        assert!(is_local_origin_str("http://localhost:1420"));
        assert!(!is_local_origin_str("https://example.com"));
    }

    #[test]
    fn auth_requires_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer abc"));
        assert!(is_authorized(&headers, "abc"));
        assert!(!is_authorized(&headers, "def"));
    }

    #[test]
    fn extracts_payload_fields() {
        let payload = json!({ "channelId": "r1", "enabled": true });
        assert_eq!(field::<String>(&payload, "channelId").unwrap(), "r1");
        assert!(field::<bool>(&payload, "enabled").unwrap());
        assert!(field::<String>(&payload, "missing").is_err());
    }
}

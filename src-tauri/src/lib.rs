mod manager;
mod metrics;
mod mihomo;
mod models;
mod proxy;

use manager::AppManager;
use models::{AppSnapshot, DelayResult, NodeInfo, RuleInput, RuleStats};
use tauri::State;

#[tauri::command]
async fn get_state(manager: State<'_, AppManager>) -> Result<AppSnapshot, String> {
    manager.get_state().await.map_err(display_error)
}

#[tauri::command]
async fn set_subscription(manager: State<'_, AppManager>, url: Option<String>) -> Result<(), String> {
    manager.set_subscription(url).await.map_err(display_error)
}

#[tauri::command]
async fn refresh_nodes(manager: State<'_, AppManager>) -> Result<Vec<NodeInfo>, String> {
    manager.refresh_nodes().await.map_err(display_error)
}

#[tauri::command]
async fn create_rule(manager: State<'_, AppManager>, input: RuleInput) -> Result<models::AppRule, String> {
    manager.create_rule(input).await.map_err(display_error)
}

#[tauri::command]
async fn update_rule(
    manager: State<'_, AppManager>,
    rule_id: String,
    input: RuleInput,
) -> Result<models::AppRule, String> {
    manager.update_rule(&rule_id, input).await.map_err(display_error)
}

#[tauri::command]
async fn delete_rule(manager: State<'_, AppManager>, rule_id: String) -> Result<(), String> {
    manager.delete_rule(&rule_id).await.map_err(display_error)
}

#[tauri::command]
async fn set_rule_node(manager: State<'_, AppManager>, rule_id: String, node: String) -> Result<(), String> {
    manager
        .set_rule_node(&rule_id, node)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn start_rule_app(manager: State<'_, AppManager>, rule_id: String) -> Result<(), String> {
    manager
        .start_rule_app(&rule_id)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn stop_rule_app(manager: State<'_, AppManager>, rule_id: String) -> Result<(), String> {
    manager
        .stop_rule_app(&rule_id)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn get_rule_stats(manager: State<'_, AppManager>) -> Result<Vec<RuleStats>, String> {
    manager
        .get_state()
        .await
        .map(|state| state.stats)
        .map_err(display_error)
}

#[tauri::command]
async fn list_rule_connections(
    manager: State<'_, AppManager>,
    rule_id: String,
) -> Result<Vec<models::RuleConnection>, String> {
    manager
        .get_state()
        .await
        .map(|state| {
            state
                .stats
                .into_iter()
                .find(|stats| stats.rule_id == rule_id)
                .map(|stats| stats.recent_targets)
                .unwrap_or_default()
        })
        .map_err(display_error)
}

#[tauri::command]
async fn restart_core(manager: State<'_, AppManager>) -> Result<(), String> {
    manager.restart_core().await.map_err(display_error)
}

#[tauri::command]
async fn test_node_delay(manager: State<'_, AppManager>, node: String) -> Result<DelayResult, String> {
    manager.test_node_delay(node).await.map_err(display_error)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let manager = AppManager::new(&app.handle())?;
            let initializer = manager.clone();
            app.manage(manager);
            tauri::async_runtime::spawn(async move {
                initializer.initialize().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_subscription,
            refresh_nodes,
            create_rule,
            update_rule,
            delete_rule,
            set_rule_node,
            start_rule_app,
            stop_rule_app,
            get_rule_stats,
            list_rule_connections,
            restart_core,
            test_node_delay
        ])
        .run(tauri::generate_context!())
        .expect("error while running FreeClash");
}

fn display_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}


mod http_api;
mod manager;
mod metrics;
mod mihomo;
mod models;
mod proxy;

use std::sync::atomic::{AtomicBool, Ordering};

use manager::AppManager;
use models::{
    AppSnapshot, ChannelDiagnostics, ChannelInput, ChannelProxyTestResult, ChannelStats,
    DelayResult, NodeInfo, SubscriptionInput,
};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent, State, WindowEvent};

#[derive(Default)]
struct AppExit {
    exiting: AtomicBool,
}

#[tauri::command]
async fn get_state(manager: State<'_, AppManager>) -> Result<AppSnapshot, String> {
    manager.get_state().await.map_err(display_error)
}

#[tauri::command]
async fn set_subscription(
    manager: State<'_, AppManager>,
    url: Option<String>,
) -> Result<(), String> {
    manager.set_subscription(url).await.map_err(display_error)
}

#[tauri::command]
async fn create_subscription(
    manager: State<'_, AppManager>,
    input: SubscriptionInput,
) -> Result<models::Subscription, String> {
    manager
        .create_subscription(input)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn delete_subscription(
    manager: State<'_, AppManager>,
    subscription_id: String,
) -> Result<(), String> {
    manager
        .delete_subscription(&subscription_id)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn refresh_subscription(
    manager: State<'_, AppManager>,
    subscription_id: String,
) -> Result<Vec<NodeInfo>, String> {
    manager
        .refresh_subscription(&subscription_id)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn refresh_nodes(manager: State<'_, AppManager>) -> Result<Vec<NodeInfo>, String> {
    manager.refresh_nodes().await.map_err(display_error)
}

#[tauri::command]
async fn set_global_proxy_enabled(
    manager: State<'_, AppManager>,
    enabled: bool,
) -> Result<(), String> {
    manager
        .set_global_proxy_enabled(enabled)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn set_http_api_config(
    manager: State<'_, AppManager>,
    enabled: bool,
    port: u16,
) -> Result<(), String> {
    manager
        .set_http_api_config(enabled, port)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn create_channel(
    manager: State<'_, AppManager>,
    input: ChannelInput,
) -> Result<models::ProxyChannel, String> {
    manager.create_channel(input).await.map_err(display_error)
}

#[tauri::command]
async fn update_channel(
    manager: State<'_, AppManager>,
    channel_id: String,
    input: ChannelInput,
) -> Result<models::ProxyChannel, String> {
    manager
        .update_channel(&channel_id, input)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn delete_channel(manager: State<'_, AppManager>, channel_id: String) -> Result<(), String> {
    manager
        .delete_channel(&channel_id)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn set_channel_enabled(
    manager: State<'_, AppManager>,
    channel_id: String,
    enabled: bool,
) -> Result<models::ProxyChannel, String> {
    manager
        .set_channel_enabled(&channel_id, enabled)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn duplicate_channel(
    manager: State<'_, AppManager>,
    channel_id: String,
) -> Result<models::ProxyChannel, String> {
    manager
        .duplicate_channel(&channel_id)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn set_channel_node(
    manager: State<'_, AppManager>,
    channel_id: String,
    node: String,
) -> Result<(), String> {
    manager
        .set_channel_node(&channel_id, node)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn get_channel_stats(manager: State<'_, AppManager>) -> Result<Vec<ChannelStats>, String> {
    manager
        .get_state()
        .await
        .map(|state| state.stats)
        .map_err(display_error)
}

#[tauri::command]
async fn diagnose_channel(
    manager: State<'_, AppManager>,
    channel_id: String,
) -> Result<ChannelDiagnostics, String> {
    manager
        .diagnose_channel(&channel_id)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn test_channel_proxy(
    manager: State<'_, AppManager>,
    channel_id: String,
) -> Result<ChannelProxyTestResult, String> {
    manager
        .test_channel_proxy(&channel_id)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn test_node_delay(
    manager: State<'_, AppManager>,
    node: String,
) -> Result<DelayResult, String> {
    manager.test_node_delay(node).await.map_err(display_error)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppExit::default())
        .setup(|app| {
            let manager = AppManager::new(&app.handle())?;
            setup_tray(app, manager.global_proxy_enabled_blocking())?;
            let initializer = manager.clone();
            let http_initializer = manager.clone();
            app.manage(manager);
            tauri::async_runtime::spawn(async move {
                initializer.initialize().await;
            });
            tauri::async_runtime::spawn(async move {
                http_initializer.start_http_api_from_config().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_subscription,
            create_subscription,
            delete_subscription,
            refresh_subscription,
            refresh_nodes,
            set_global_proxy_enabled,
            set_http_api_config,
            create_channel,
            update_channel,
            delete_channel,
            set_channel_enabled,
            duplicate_channel,
            set_channel_node,
            get_channel_stats,
            diagnose_channel,
            test_channel_proxy,
            test_node_delay
        ])
        .build(tauri::generate_context!())
        .expect("error while building FreeClash")
        .run(|app_handle, event| {
            if let RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } = event
            {
                let exiting = app_handle.state::<AppExit>().exiting.load(Ordering::SeqCst);
                if !exiting {
                    api.prevent_close();
                    if let Some(window) = app_handle.get_webview_window(&label) {
                        let _ = window.hide();
                    }
                }
            }
        });
}

fn display_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}

fn setup_tray(app: &tauri::App, global_proxy_enabled: bool) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show_window", "显示窗口", true, None::<&str>)?;
    let toggle_proxy = CheckMenuItem::with_id(
        app,
        "toggle_proxy",
        "全局代理",
        true,
        global_proxy_enabled,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &toggle_proxy, &separator, &quit])?;

    let toggle_proxy_item = toggle_proxy.clone();
    let mut tray = TrayIconBuilder::new()
        .tooltip("FreeClash")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show_window" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "toggle_proxy" => {
                let manager = app.state::<AppManager>().inner().clone();
                let item = toggle_proxy_item.clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(enabled) = manager.toggle_global_proxy_enabled().await {
                        let _ = item.set_checked(enabled);
                    }
                });
            }
            "quit" => {
                app.state::<AppExit>().exiting.store(true, Ordering::SeqCst);
                let manager = app.state::<AppManager>().inner().clone();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    manager.shutdown().await;
                    app_handle.exit(0);
                });
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    Ok(())
}

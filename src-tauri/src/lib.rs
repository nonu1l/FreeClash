mod manager;
mod metrics;
mod mihomo;
mod models;
mod proxy;

use std::sync::atomic::{AtomicBool, Ordering};

use manager::AppManager;
use models::{AppSnapshot, DelayResult, NodeInfo, PinnedNode, Subscription, SubscriptionInput};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
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
    input: SubscriptionInput,
) -> Result<Subscription, String> {
    manager.set_subscription(input).await.map_err(display_error)
}

#[tauri::command]
async fn refresh_subscription(manager: State<'_, AppManager>) -> Result<Vec<NodeInfo>, String> {
    manager.refresh_subscription().await.map_err(display_error)
}

#[tauri::command]
async fn refresh_nodes(manager: State<'_, AppManager>) -> Result<Vec<NodeInfo>, String> {
    manager.refresh_nodes().await.map_err(display_error)
}

#[tauri::command]
async fn pin_node(
    manager: State<'_, AppManager>,
    node_name: String,
) -> Result<PinnedNode, String> {
    manager.pin_node(node_name).await.map_err(display_error)
}

#[tauri::command]
async fn unpin_node(manager: State<'_, AppManager>, node_name: String) -> Result<(), String> {
    manager.unpin_node(node_name).await.map_err(display_error)
}

#[tauri::command]
async fn update_pin_port(
    manager: State<'_, AppManager>,
    node_name: String,
    port: u16,
) -> Result<PinnedNode, String> {
    manager
        .update_pin_port(node_name, port)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn test_node_delay(
    manager: State<'_, AppManager>,
    node_name: String,
) -> Result<DelayResult, String> {
    manager
        .test_node_delay(node_name)
        .await
        .map_err(display_error)
}

#[tauri::command]
async fn test_all_node_delays(manager: State<'_, AppManager>) -> Result<Vec<DelayResult>, String> {
    manager.test_all_node_delays().await.map_err(display_error)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .manage(AppExit::default())
        .setup(|app| {
            let manager = AppManager::new(&app.handle())?;
            setup_tray(app)?;
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
            refresh_subscription,
            refresh_nodes,
            pin_node,
            unpin_node,
            update_pin_port,
            test_node_delay,
            test_all_node_delays
        ])
        .build(tauri::generate_context!())
        .expect("error while building FreeClash")
        .run(|app_handle, event| match event {
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } => {
                let exiting = app_handle.state::<AppExit>().exiting.load(Ordering::SeqCst);
                if !exiting {
                    api.prevent_close();
                    if let Some(window) = app_handle.get_webview_window(&label) {
                        let _ = window.hide();
                    }
                }
            }
            RunEvent::ExitRequested { api, .. } => {
                let exiting = app_handle
                    .state::<AppExit>()
                    .exiting
                    .swap(true, Ordering::SeqCst);
                if !exiting {
                    api.prevent_exit();
                    let manager = app_handle.state::<AppManager>().inner().clone();
                    let app_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        manager.shutdown().await;
                        app_handle.exit(0);
                    });
                }
            }
            _ => {}
        });
}

fn display_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show_window", "显示窗口", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit])?;

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

    if let Ok(icon) = Image::from_bytes(include_bytes!("../icons/tray.ico")) {
        tray = tray.icon(icon);
    } else if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    Ok(())
}

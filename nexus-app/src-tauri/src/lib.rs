// Project Nexus — Tauri Backend
// Phase 1: Basic message handling (mock) + System Tray
// Phase 2: Claude Code CLI integration

use std::process::Command;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

/// Send a message to Claude Code CLI and return the response.
/// Runs in a blocking thread via Tauri's async runtime to keep UI responsive.
#[tauri::command]
async fn send_message(message: String) -> Result<String, String> {
    // Run CLI in a blocking thread so the UI doesn't freeze
    tauri::async_runtime::spawn_blocking(move || {
        // Use full path to npm's claude.cmd to avoid picking up
        // the Claude Desktop app (claude.exe) which sits earlier in PATH.
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        let claude_cmd = format!(r"{}\AppData\Roaming\npm\claude.cmd", home);

        let output = Command::new("cmd")
            .args(["/c", &claude_cmd, "-p", &message])
            .output()
            .map_err(|e| format!("Claude Code CLIの起動に失敗: {}", e))?;

        if output.status.success() {
            let response = String::from_utf8_lossy(&output.stdout).to_string();
            if response.trim().is_empty() {
                Ok("(空の応答が返されました)".to_string())
            } else {
                Ok(response)
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("Claude Code CLIエラー: {}", stderr))
        }
    })
    .await
    .map_err(|e| format!("スレッドエラー: {}", e))?
}

/// Get the status of connected machines.
#[tauri::command]
fn get_machine_status() -> Vec<MachineStatus> {
    vec![
        MachineStatus {
            name: "OMEN".to_string(),
            role: "Commander".to_string(),
            online: true,
        },
        MachineStatus {
            name: "SIGMA".to_string(),
            role: "Remote".to_string(),
            online: false,
        },
        MachineStatus {
            name: "Precision".to_string(),
            role: "Remote".to_string(),
            online: false,
        },
    ]
}

#[derive(serde::Serialize)]
struct MachineStatus {
    name: String,
    role: String,
    online: bool,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            send_message,
            get_machine_status,
        ])
        .setup(|app| {
            // Build tray menu
            let show = MenuItemBuilder::with_id("show", "表示").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "終了").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            // Build tray icon
            TrayIconBuilder::new()
                .icon(tauri::include_image!("icons/32x32.png"))
                .menu(&menu)
                .tooltip("Project Nexus")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        // Hide window first, then give WebView2 a brief
                        // moment to release GPU/audio resources before exit.
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                        }
                        std::thread::spawn(|| {
                            std::thread::sleep(std::time::Duration::from_millis(300));
                            std::process::exit(0);
                        });
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        // Hide window instead of closing (minimize to tray)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

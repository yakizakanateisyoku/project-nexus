// Project Nexus — Tauri Backend
// Phase 2: Anthropic API Direct Integration
// - HTTP direct call (no subprocess, no audio glitch)
// - Conversation history management
// - Model switching support

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, State, WindowEvent,
};

// ========================================
// Anthropic API Types
// ========================================

#[derive(Serialize, Clone)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ApiMessage>,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    error: Option<ApiErrorDetail>,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: Option<String>,
}

// ========================================
// App State
// ========================================

struct ChatState {
    history: Vec<ApiMessage>,
    model: String,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            model: "claude-sonnet-4-5-20250929".to_string(),
        }
    }
}

const MAX_HISTORY: usize = 20; // 直近20メッセージを保持
const API_URL: &str = "https://api.anthropic.com/v1/messages";

// ========================================
// Tauri Commands
// ========================================

/// Send a message via Anthropic API (non-streaming)
#[tauri::command]
async fn send_message(
    message: String,
    state: State<'_, Mutex<ChatState>>,
) -> Result<String, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY 環境変数が設定されていません".to_string())?;

    // Build messages list from history
    let messages = {
        let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;

        // Add user message to history
        chat.history.push(ApiMessage {
            role: "user".to_string(),
            content: message,
        });

        // Trim history to last N messages
        if chat.history.len() > MAX_HISTORY {
            let drain_count = chat.history.len() - MAX_HISTORY;
            chat.history.drain(..drain_count);
        }

        chat.history.clone()
    };

    let model = {
        let chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
        chat.model.clone()
    };

    // Call Anthropic API
    let client = reqwest::Client::new();
    let body = ApiRequest {
        model,
        max_tokens: 4096,
        messages,
    };

    let response = client
        .post(API_URL)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API接続エラー: {}", e))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("レスポンス読み取りエラー: {}", e))?;

    if !status.is_success() {
        // Try to parse error message
        if let Ok(err) = serde_json::from_str::<ApiError>(&response_text) {
            if let Some(detail) = err.error {
                return Err(format!(
                    "API Error ({}): {}",
                    status,
                    detail.message.unwrap_or_default()
                ));
            }
        }
        return Err(format!("API Error ({}): {}", status, response_text));
    }

    // Parse success response
    let api_resp: ApiResponse = serde_json::from_str(&response_text)
        .map_err(|e| format!("レスポンスパースエラー: {}", e))?;

    let assistant_text = api_resp
        .content
        .into_iter()
        .filter_map(|block| block.text)
        .collect::<Vec<_>>()
        .join("");

    if assistant_text.is_empty() {
        return Ok("(空の応答が返されました)".to_string());
    }

    // Add assistant response to history
    {
        let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
        chat.history.push(ApiMessage {
            role: "assistant".to_string(),
            content: assistant_text.clone(),
        });
    }

    Ok(assistant_text)
}

/// Clear conversation history
#[tauri::command]
fn clear_history(state: State<'_, Mutex<ChatState>>) -> Result<(), String> {
    let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    chat.history.clear();
    Ok(())
}

/// Switch model
#[tauri::command]
fn set_model(model_id: String, state: State<'_, Mutex<ChatState>>) -> Result<String, String> {
    let valid_models = [
        "claude-sonnet-4-5-20250929",
        "claude-haiku-4-5-20251001",
    ];

    if !valid_models.contains(&model_id.as_str()) {
        return Err(format!("無効なモデル: {}", model_id));
    }

    let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    chat.model = model_id.clone();
    Ok(format!("モデルを {} に変更しました", model_id))
}

/// Get current model info
#[tauri::command]
fn get_current_model(state: State<'_, Mutex<ChatState>>) -> Result<String, String> {
    let chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    Ok(chat.model.clone())
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

#[derive(Serialize)]
struct MachineStatus {
    name: String,
    role: String,
    online: bool,
}

// ========================================
// App Entry
// ========================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(ChatState::default()))
        .invoke_handler(tauri::generate_handler![
            send_message,
            clear_history,
            set_model,
            get_current_model,
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
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

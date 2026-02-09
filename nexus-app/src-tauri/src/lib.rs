// Project Nexus — Tauri Backend
// Phase 3: Context Monitoring + Token Tracking
// - HTTP direct call (no subprocess, no audio glitch)
// - Conversation history management
// - Model switching support
// - Real token usage tracking from API response
// - Cost estimation and context warnings

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
    usage: Option<UsageInfo>,
}

#[derive(Deserialize, Serialize, Clone, Default)]
struct UsageInfo {
    input_tokens: u64,
    output_tokens: u64,
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

#[derive(Serialize, Clone, Default)]
struct TokenStats {
    last_input_tokens: u64,
    last_output_tokens: u64,
    total_input_tokens: u64,
    total_output_tokens: u64,
    request_count: u32,
}

struct ChatState {
    history: Vec<ApiMessage>,
    model: String,
    token_stats: TokenStats,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            model: "claude-sonnet-4-5-20250929".to_string(),
            token_stats: TokenStats::default(),
        }
    }
}

const MAX_HISTORY: usize = 20; // 直近20メッセージを保持
const API_URL: &str = "https://api.anthropic.com/v1/messages";

// ========================================
// Tauri Commands
// ========================================

/// Response from send_message including token usage
#[derive(Serialize)]
struct SendMessageResponse {
    text: String,
    token_stats: TokenStats,
}

/// Send a message via Anthropic API (non-streaming)
#[tauri::command]
async fn send_message(
    message: String,
    state: State<'_, Mutex<ChatState>>,
) -> Result<SendMessageResponse, String> {
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
        return Ok(SendMessageResponse {
            text: "(空の応答が返されました)".to_string(),
            token_stats: TokenStats::default(),
        });
    }

    // Update token stats and add assistant response to history
    let current_stats = {
        let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;

        // Update token stats from usage info
        if let Some(usage) = &api_resp.usage {
            chat.token_stats.last_input_tokens = usage.input_tokens;
            chat.token_stats.last_output_tokens = usage.output_tokens;
            chat.token_stats.total_input_tokens += usage.input_tokens;
            chat.token_stats.total_output_tokens += usage.output_tokens;
            chat.token_stats.request_count += 1;
        }

        chat.history.push(ApiMessage {
            role: "assistant".to_string(),
            content: assistant_text.clone(),
        });

        chat.token_stats.clone()
    };

    Ok(SendMessageResponse {
        text: assistant_text,
        token_stats: current_stats,
    })
}

/// Clear conversation history and reset token stats
#[tauri::command]
fn clear_history(state: State<'_, Mutex<ChatState>>) -> Result<(), String> {
    let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    chat.history.clear();
    chat.token_stats = TokenStats::default();
    Ok(())
}

/// Get current token usage statistics
#[tauri::command]
fn get_token_stats(state: State<'_, Mutex<ChatState>>) -> Result<TokenStats, String> {
    let chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    Ok(chat.token_stats.clone())
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
    // Load .env file (API keys etc.) — GUI起動時に環境変数が見えない問題の対策
    // 1. カレントディレクトリの.envを試行
    if dotenvy::dotenv().is_err() {
        // 2. 実行ファイルと同階層の.envを試行
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let env_path = exe_dir.join(".env");
                let _ = dotenvy::from_path(&env_path);
            }
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(ChatState::default()))
        .invoke_handler(tauri::generate_handler![
            send_message,
            clear_history,
            set_model,
            get_current_model,
            get_machine_status,
            get_token_stats,
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

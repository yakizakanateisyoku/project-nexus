// Project Nexus — Tauri Backend
// Phase 3-B: Tool Use Integration
// - HTTP direct call (no subprocess, no audio glitch)
// - Conversation history management
// - Model switching support
// - Real token usage tracking from API response
// - Cost estimation and context warnings
// - Tool Use: Claude が自律的にSSHコマンドを実行

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager, State, WindowEvent,
};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

/// バイト列をUTF-8として解釈し、失敗したらShift-JIS→EUC-JPの順で試行
fn decode_bytes(bytes: &[u8]) -> String {
    // まずUTF-8を試す
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // Shift-JIS (CP932) を試す
    let (decoded, _, had_errors) = encoding_rs::SHIFT_JIS.decode(bytes);
    if !had_errors {
        return decoded.to_string();
    }
    // EUC-JP を試す
    let (decoded, _, had_errors) = encoding_rs::EUC_JP.decode(bytes);
    if !had_errors {
        return decoded.to_string();
    }
    // 全部ダメならlossyで
    String::from_utf8_lossy(bytes).to_string()
}

// ========================================
// Anthropic API Types (Tool Use対応)
// ========================================

/// 履歴用メッセージ（テキストのみ保持、トークン節約）
#[derive(Serialize, Clone)]
struct HistoryMessage {
    role: String,
    content: String,
}

/// API送信用リクエスト（tools / system 対応）
#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

/// APIレスポンス
#[derive(Deserialize, Debug)]
struct ApiResponse {
    content: Vec<serde_json::Value>,
    usage: Option<UsageInfo>,
    stop_reason: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
struct UsageInfo {
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Deserialize)]
struct ApiError {
    error: Option<ApiErrorDetail>,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: Option<String>,
}

/// ツール実行結果（フロントエンドに返す）
#[derive(Serialize, Clone, Debug)]
struct ToolExecution {
    machine_name: String,
    command: String,
    stdout: String,
    stderr: String,
    success: bool,
}

/// ツール実行中イベント（Tauriイベント経由でフロントへ）
#[derive(Serialize, Clone, Debug)]
struct ToolExecutingEvent {
    machine_name: String,
    command: String,
}

/// ツール実行完了イベント
#[derive(Serialize, Clone, Debug)]
struct ToolCompletedEvent {
    machine_name: String,
    command: String,
    success: bool,
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
    history: Vec<HistoryMessage>,
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
const MAX_TOOL_LOOPS: usize = 5; // Tool Use最大ループ回数（暴走防止）
const API_URL: &str = "https://api.anthropic.com/v1/messages";

// ========================================
// Tauri Commands
// ========================================

/// Response from send_message including token usage and tool executions
#[derive(Serialize)]
struct SendMessageResponse {
    text: String,
    token_stats: TokenStats,
    tool_executions: Vec<ToolExecution>,
}

// ========================================
// Tool Use — ヘルパー関数
// ========================================

/// 利用可能なマシンからツール定義を動的生成
fn build_tools(machines: &[SshMachineConfig]) -> Vec<serde_json::Value> {
    let machine_names: Vec<String> = machines
        .iter()
        .filter(|m| m.enabled && m.role != "Commander")
        .map(|m| m.name.clone())
        .collect();

    if machine_names.is_empty() {
        return vec![];
    }

    vec![serde_json::json!({
        "name": "execute_remote_command",
        "description": "リモートマシンでシェルコマンドを実行する。ディスク容量、プロセス確認、サービス状態など、システム情報の取得や管理タスクに使用。",
        "input_schema": {
            "type": "object",
            "properties": {
                "machine_name": {
                    "type": "string",
                    "description": format!("対象マシン名。利用可能: {}", machine_names.join(", ")),
                    "enum": machine_names
                },
                "command": {
                    "type": "string",
                    "description": "実行するシェルコマンド（例: df -h, free -m, systemctl status nginx）"
                }
            },
            "required": ["machine_name", "command"]
        }
    })]
}

/// システムプロンプト生成（マシン情報を注入）
fn build_system_prompt(machines: &[SshMachineConfig]) -> String {
    let machine_info: Vec<String> = machines
        .iter()
        .map(|m| {
            let status = if m.role == "Commander" {
                "ローカル（自分自身）"
            } else if m.enabled {
                "SSH接続可能"
            } else {
                "無効"
            };
            format!("- {}: {} [{}]", m.name, m.role, status)
        })
        .collect();

    format!(
        "あなたはProject Nexusのシステム管理アシスタントです。\n\
         以下のマシンをSSH経由でリモート管理できます：\n{}\n\n\
         ユーザーの指示に応じて execute_remote_command ツールで適切なコマンドを実行し、\n\
         結果を日本語で分かりやすく説明してください。\n\
         コマンド実行が不要な質問には通常通り回答してください。",
        machine_info.join("\n")
    )
}

/// ツール実行（SSH経由）
async fn execute_tool_ssh(
    machine_name: &str,
    command: &str,
    machines: &[SshMachineConfig],
) -> ToolExecution {
    let machine = machines
        .iter()
        .find(|m| m.name == machine_name && m.enabled && m.role != "Commander");

    let Some(machine) = machine else {
        return ToolExecution {
            machine_name: machine_name.to_string(),
            command: command.to_string(),
            stdout: String::new(),
            stderr: format!("マシン '{}' が見つからないか無効です", machine_name),
            success: false,
        };
    };

    let result = timeout(
        Duration::from_secs(30),
        TokioCommand::new("ssh")
            .args([
                "-o", "BatchMode=yes",
                "-o", "ConnectTimeout=5",
                &machine.host,
                command,
            ])
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => ToolExecution {
            machine_name: machine_name.to_string(),
            command: command.to_string(),
            stdout: decode_bytes(&output.stdout),
            stderr: decode_bytes(&output.stderr),
            success: output.status.success(),
        },
        Ok(Err(e)) => ToolExecution {
            machine_name: machine_name.to_string(),
            command: command.to_string(),
            stdout: String::new(),
            stderr: format!("SSH実行エラー: {}", e),
            success: false,
        },
        Err(_) => ToolExecution {
            machine_name: machine_name.to_string(),
            command: command.to_string(),
            stdout: String::new(),
            stderr: "タイムアウト（30秒）".to_string(),
            success: false,
        },
    }
}

/// Anthropic API呼び出し（共通）
async fn call_anthropic(
    api_key: &str,
    model: &str,
    system: &str,
    tools: &[serde_json::Value],
    messages: &[serde_json::Value],
) -> Result<ApiResponse, String> {
    let client = reqwest::Client::new();

    let body = ApiRequest {
        model: model.to_string(),
        max_tokens: 4096,
        system: Some(system.to_string()),
        messages: messages.to_vec(),
        tools: if tools.is_empty() {
            None
        } else {
            Some(tools.to_vec())
        },
    };

    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
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

    serde_json::from_str(&response_text)
        .map_err(|e| format!("レスポンスパースエラー: {} / body: {}", e, &response_text[..200.min(response_text.len())]))
}

// ========================================
// Tauri Commands
// ========================================

/// Send a message via Anthropic API with Tool Use support
#[tauri::command]
async fn send_message(
    message: String,
    state: State<'_, Mutex<ChatState>>,
    ssh_state: State<'_, Mutex<SshState>>,
    app_handle: tauri::AppHandle,
) -> Result<SendMessageResponse, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY 環境変数が設定されていません".to_string())?;

    // マシン情報からツール定義とシステムプロンプトを生成
    let (tools, system_prompt, machines) = {
        let ssh = ssh_state.lock().map_err(|e| format!("Lock error: {}", e))?;
        (
            build_tools(&ssh.machines),
            build_system_prompt(&ssh.machines),
            ssh.machines.clone(),
        )
    };

    // 履歴からAPIメッセージ配列を構築
    let mut api_messages: Vec<serde_json::Value> = {
        let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;

        // ユーザーメッセージを履歴に追加
        chat.history.push(HistoryMessage {
            role: "user".to_string(),
            content: message.clone(),
        });

        // 履歴をトリム
        if chat.history.len() > MAX_HISTORY {
            let drain_count = chat.history.len() - MAX_HISTORY;
            chat.history.drain(..drain_count);
        }

        // 履歴を API メッセージ形式に変換
        chat.history
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect()
    };

    let model = {
        let chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
        chat.model.clone()
    };

    // ========================================
    // Tool Use ループ
    // ========================================
    let mut all_text_parts: Vec<String> = Vec::new();
    let mut all_tool_executions: Vec<ToolExecution> = Vec::new();
    let mut total_usage = UsageInfo::default();
    let mut last_call_input_tokens: u64 = 0; // コンテキスト使用率計算用（最後のAPIコールのみ）

    for loop_count in 0..MAX_TOOL_LOOPS {
        let api_resp =
            call_anthropic(&api_key, &model, &system_prompt, &tools, &api_messages).await?;

        // トークン使用量を累積
        if let Some(usage) = &api_resp.usage {
            total_usage.input_tokens += usage.input_tokens;
            total_usage.output_tokens += usage.output_tokens;
            last_call_input_tokens = usage.input_tokens; // 最新のAPIコールのinput_tokensを記録
        }

        // レスポンスのcontentブロックを解析
        let mut tool_uses: Vec<(String, String, serde_json::Value)> = Vec::new(); // (id, name, input)

        for block in &api_resp.content {
            if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            all_text_parts.push(text.to_string());
                        }
                    }
                    "tool_use" => {
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let input = block
                            .get("input")
                            .cloned()
                            .unwrap_or(serde_json::json!({}));
                        tool_uses.push((id, name, input));
                    }
                    _ => {}
                }
            }
        }

        // アシスタント応答をメッセージ配列に追加（tool_useブロック含む）
        api_messages.push(serde_json::json!({
            "role": "assistant",
            "content": api_resp.content
        }));

        // ツール呼び出しがなければ終了
        if tool_uses.is_empty() || api_resp.stop_reason.as_deref() != Some("tool_use") {
            break;
        }

        // ループ上限チェック
        if loop_count >= MAX_TOOL_LOOPS - 1 {
            all_text_parts
                .push("\n⚠️ ツール実行回数が上限に達しました。".to_string());
            break;
        }

        // ツール実行
        let mut tool_results: Vec<serde_json::Value> = Vec::new();

        for (tool_id, tool_name, tool_input) in &tool_uses {
            let machine_name = tool_input
                .get("machine_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let command = tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // フロントエンドに実行中イベントを送信
            let _ = app_handle.emit(
                "tool-executing",
                ToolExecutingEvent {
                    machine_name: machine_name.to_string(),
                    command: command.to_string(),
                },
            );

            if tool_name == "execute_remote_command" {
                let exec_result = execute_tool_ssh(machine_name, command, &machines).await;

                // 実行完了イベント
                let _ = app_handle.emit(
                    "tool-completed",
                    ToolCompletedEvent {
                        machine_name: machine_name.to_string(),
                        command: command.to_string(),
                        success: exec_result.success,
                    },
                );

                // tool_resultの content を構築
                let result_text = if exec_result.success {
                    if exec_result.stdout.is_empty() {
                        "(コマンド成功・出力なし)".to_string()
                    } else {
                        exec_result.stdout.clone()
                    }
                } else {
                    format!(
                        "エラー: {}{}",
                        exec_result.stderr,
                        if !exec_result.stdout.is_empty() {
                            format!("\nstdout: {}", exec_result.stdout)
                        } else {
                            String::new()
                        }
                    )
                };

                tool_results.push(serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": tool_id,
                    "content": result_text,
                    "is_error": !exec_result.success
                }));

                all_tool_executions.push(exec_result);
            } else {
                // 未知のツール
                tool_results.push(serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": tool_id,
                    "content": format!("未知のツール: {}", tool_name),
                    "is_error": true
                }));
            }
        }

        // ツール結果をuserメッセージとして追加
        api_messages.push(serde_json::json!({
            "role": "user",
            "content": tool_results
        }));
    }

    // 最終テキスト
    let final_text = all_text_parts.join("");
    let final_text = if final_text.is_empty() {
        "(空の応答が返されました)".to_string()
    } else {
        final_text
    };

    // 履歴とトークン統計を更新（最終テキストのみ保存）
    let current_stats = {
        let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;

        chat.token_stats.last_input_tokens = last_call_input_tokens; // コンテキスト%用: 最後のAPIコールのみ
        chat.token_stats.last_output_tokens = total_usage.output_tokens;
        chat.token_stats.total_input_tokens += total_usage.input_tokens; // コスト計算用: 全ループ合計
        chat.token_stats.total_output_tokens += total_usage.output_tokens;
        chat.token_stats.request_count += 1;

        // アシスタント応答を履歴に追加（テキストのみ）
        chat.history.push(HistoryMessage {
            role: "assistant".to_string(),
            content: final_text.clone(),
        });

        chat.token_stats.clone()
    };

    Ok(SendMessageResponse {
        text: final_text,
        token_stats: current_stats,
        tool_executions: all_tool_executions,
    })
}

/// Clear conversation history (コスト累計は保持)
#[tauri::command]
fn clear_history(state: State<'_, Mutex<ChatState>>) -> Result<(), String> {
    let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    chat.history.clear();
    // コンテキスト関連のみリセット、コスト累計は保持
    chat.token_stats.last_input_tokens = 0;
    chat.token_stats.last_output_tokens = 0;
    Ok(())
}

/// コスト累計をリセット
#[tauri::command]
fn reset_cost(state: State<'_, Mutex<ChatState>>) -> Result<(), String> {
    let mut chat = state.lock().map_err(|e| format!("State lock error: {}", e))?;
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

#[derive(Serialize)]
struct MachineStatus {
    name: String,
    role: String,
    online: bool,
}

// ========================================
// SSH Remote Management (Phase 3-A)
// ========================================

const SSH_TIMEOUT_SECS: u64 = 5;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SshMachineConfig {
    name: String,
    host: String,       // ~/.ssh/config の Host名 or IPアドレス
    role: String,       // "Commander" | "Remote"
    enabled: bool,      // 接続試行するか
}

impl Default for SshMachineConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            role: "Remote".to_string(),
            enabled: true,
        }
    }
}

struct SshState {
    machines: Vec<SshMachineConfig>,
}

impl Default for SshState {
    fn default() -> Self {
        Self {
            machines: vec![
                SshMachineConfig {
                    name: "OMEN".to_string(),
                    host: "localhost".to_string(),
                    role: "Commander".to_string(),
                    enabled: false, // 自分自身なので不要
                },
                SshMachineConfig {
                    name: "SIGMA".to_string(),
                    host: "sigma".to_string(),  // ~/.ssh/config の Host名
                    role: "Remote".to_string(),
                    enabled: true,
                },
                SshMachineConfig {
                    name: "Precision".to_string(),
                    host: "precision".to_string(),
                    role: "Remote".to_string(),
                    enabled: true,
                },
            ],
        }
    }
}

/// SSH接続テスト（ssh.exe経由、軽量）
async fn ssh_check_alive(host: &str) -> bool {
    let result = timeout(
        Duration::from_secs(SSH_TIMEOUT_SECS),
        TokioCommand::new("ssh")
            .args([
                "-o", "BatchMode=yes",
                "-o", "ConnectTimeout=3",
                "-o", "StrictHostKeyChecking=accept-new",
                host,
                "echo", "nexus-ping",
            ])
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains("nexus-ping")
        }
        _ => false,
    }
}

/// 全マシンのステータスを実SSH接続で取得
#[tauri::command]
async fn get_machine_status(
    ssh_state: State<'_, Mutex<SshState>>,
) -> Result<Vec<MachineStatus>, String> {
    let machines = {
        let state = ssh_state.lock().map_err(|e| format!("Lock error: {}", e))?;
        state.machines.clone()
    };

    let mut statuses = Vec::new();

    for machine in &machines {
        let online = if machine.role == "Commander" {
            true // OMEN（自分自身）は常にオンライン
        } else if machine.enabled {
            ssh_check_alive(&machine.host).await
        } else {
            false
        };

        statuses.push(MachineStatus {
            name: machine.name.clone(),
            role: machine.role.clone(),
            online,
        });
    }

    Ok(statuses)
}

/// リモートPCでコマンドを実行
#[tauri::command]
async fn execute_remote_command(
    machine_name: String,
    command: String,
    ssh_state: State<'_, Mutex<SshState>>,
) -> Result<RemoteCommandResult, String> {
    let machine = {
        let state = ssh_state.lock().map_err(|e| format!("Lock error: {}", e))?;
        state
            .machines
            .iter()
            .find(|m| m.name == machine_name)
            .cloned()
            .ok_or_else(|| format!("マシン '{}' が見つかりません", machine_name))?
    };

    if machine.role == "Commander" {
        return Err("OMENへのリモート実行はサポートされていません".to_string());
    }

    if !machine.enabled {
        return Err(format!("マシン '{}' は無効化されています", machine_name));
    }

    let result = timeout(
        Duration::from_secs(30), // コマンド実行は長めのタイムアウト
        TokioCommand::new("ssh")
            .args([
                "-o", "BatchMode=yes",
                "-o", "ConnectTimeout=5",
                &machine.host,
                &command,
            ])
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => Ok(RemoteCommandResult {
            success: output.status.success(),
            stdout: decode_bytes(&output.stdout),
            stderr: decode_bytes(&output.stderr),
            exit_code: output.status.code().unwrap_or(-1),
        }),
        Ok(Err(e)) => Err(format!("SSH実行エラー: {}", e)),
        Err(_) => Err("タイムアウト: コマンド実行が30秒を超えました".to_string()),
    }
}

#[derive(Serialize)]
struct RemoteCommandResult {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: i32,
}

/// SSH設定一覧を取得
#[tauri::command]
fn get_ssh_config(
    ssh_state: State<'_, Mutex<SshState>>,
) -> Result<Vec<SshMachineConfig>, String> {
    let state = ssh_state.lock().map_err(|e| format!("Lock error: {}", e))?;
    Ok(state.machines.clone())
}

/// SSH設定を更新（マシンのhost/enabled変更）
#[tauri::command]
fn update_ssh_config(
    machine_name: String,
    host: Option<String>,
    enabled: Option<bool>,
    ssh_state: State<'_, Mutex<SshState>>,
) -> Result<String, String> {
    let mut state = ssh_state.lock().map_err(|e| format!("Lock error: {}", e))?;
    let machine = state
        .machines
        .iter_mut()
        .find(|m| m.name == machine_name)
        .ok_or_else(|| format!("マシン '{}' が見つかりません", machine_name))?;

    if let Some(h) = host {
        machine.host = h;
    }
    if let Some(e) = enabled {
        machine.enabled = e;
    }

    Ok(format!("マシン '{}' の設定を更新しました", machine_name))
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
        .manage(Mutex::new(SshState::default()))
        .invoke_handler(tauri::generate_handler![
            send_message,
            clear_history,
            reset_cost,
            set_model,
            get_current_model,
            get_machine_status,
            get_token_stats,
            execute_remote_command,
            get_ssh_config,
            update_ssh_config,
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

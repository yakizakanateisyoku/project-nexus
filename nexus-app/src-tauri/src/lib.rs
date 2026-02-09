// Project Nexus — Tauri Backend
// Phase 1: Basic message handling (mock)
// Phase 2: Claude Code CLI integration

/// Send a message and get a response.
/// Currently returns a mock echo response.
/// Will be replaced with Claude Code CLI integration in Phase 2.
#[tauri::command]
fn send_message(message: String) -> Result<String, String> {
    // TODO Phase 2: Spawn Claude Code CLI process and pipe message
    Ok(format!(
        "[Echo] {}\n\n— Claude Code CLI integration coming in Phase 2.",
        message
    ))
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

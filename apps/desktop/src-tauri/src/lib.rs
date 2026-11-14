//! The desktop editor's native side. It holds no project state and no editing logic: it is a
//! client of the runtime, exactly like the MCP server. The UI sends commands through
//! [`runtime_call`], receives every change (its own, the agent's) as `runtime-event`s, and
//! fetches frame pixels with [`runtime_render`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};
use sprite_commands::{CommandError, ErrorCode};
use sprite_runtime::protocol::RequestBody;
use sprite_runtime::{CallResult, ClientHello, RuntimeClient};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

struct Shared {
    client: Mutex<Option<Arc<RuntimeClient>>>,
    endpoint: String,
    psm: PathBuf,
}

type AppState = Arc<Shared>;

/// Finds the `psm` executable: `PSM_EXE`, next to this app (bundled sidecar), or the cargo
/// target folder during development.
fn find_psm() -> PathBuf {
    if let Some(p) = std::env::var_os("PSM_EXE") {
        return PathBuf::from(p);
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(PathBuf::from).unwrap_or_default();
    let name = if cfg!(windows) { "psm.exe" } else { "psm" };
    let candidates = [dir.join(name), dir.join(format!("psm-{}{}", target_triple(), if cfg!(windows) { ".exe" } else { "" }))];
    candidates.iter().find(|p| p.exists()).cloned().unwrap_or_else(|| dir.join(name))
}

fn target_triple() -> &'static str {
    if cfg!(all(windows, target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        "aarch64-pc-windows-msvc"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else {
        "x86_64-unknown-linux-gnu"
    }
}

fn hello() -> ClientHello {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    ClientHello { kind: "desktop".into(), name: "Editor".into(), workspace: Some(home.display().to_string()), allowed_dirs: vec![] }
}

/// Connects (starting the runtime if needed) and forwards events to the UI. Reconnects forever.
async fn connection_loop(app: AppHandle, state: AppState) {
    loop {
        match RuntimeClient::connect_or_spawn(&state.endpoint, hello(), &state.psm).await {
            Ok(client) => {
                let client = Arc::new(client);
                match client.subscribe().await {
                    Ok(mut events) => {
                        *state.client.lock().await = Some(client.clone());
                        let _ = app.emit("runtime-status", json!({ "connected": true, "endpoint": state.endpoint }));
                        loop {
                            match tokio::time::timeout(Duration::from_secs(2), events.recv()).await {
                                Ok(Ok(ev)) => {
                                    let _ = app.emit("runtime-event", ev);
                                }
                                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                                    let _ = app.emit("runtime-event", json!({ "event": "resync" }));
                                }
                                Ok(Err(_)) => break,
                                Err(_) => {
                                    if !client.is_alive() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("assinatura de eventos falhou: {}", e.message),
                }
                *state.client.lock().await = None;
                let _ = app.emit("runtime-status", json!({ "connected": false, "endpoint": state.endpoint }));
            }
            Err(e) => {
                let _ = app.emit("runtime-status", json!({ "connected": false, "endpoint": state.endpoint, "error": e.to_string() }));
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn client(state: &AppState) -> Result<Arc<RuntimeClient>, CommandError> {
    for _ in 0..50 {
        if let Some(c) = state.client.lock().await.clone()
            && c.is_alive()
        {
            return Ok(c);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(CommandError::new(ErrorCode::Internal, "sem conexão com o runtime"))
}

/// Runs any editor command (the same commands the MCP server exposes).
#[tauri::command]
async fn runtime_call(
    state: State<'_, AppState>,
    project: Option<String>,
    name: String,
    args: Value,
    expected_revision: Option<u64>,
) -> Result<CallResult, CommandError> {
    client(&state).await?.call(project, &name, args, expected_revision).await
}

/// Model, revision, history and path of a project.
#[tauri::command]
async fn runtime_state(state: State<'_, AppState>, project: Option<String>) -> Result<Value, CommandError> {
    client(&state).await?.request(RequestBody::State { project }).await
}

#[tauri::command]
async fn runtime_select(state: State<'_, AppState>, project: String) -> Result<Value, CommandError> {
    client(&state).await?.request(RequestBody::Select { project }).await
}

/// Raw pixels of a frame: 8-byte header (width, height as little-endian u32) + RGBA.
#[tauri::command]
async fn runtime_render(
    state: State<'_, AppState>,
    project: Option<String>,
    sprite: Option<u32>,
    frame: u32,
    layer: Option<u32>,
    all_layers: Option<bool>,
) -> Result<Response, CommandError> {
    let v = client(&state)
        .await?
        .request(RequestBody::Render { project, sprite, frame, layer, all_layers: all_layers.unwrap_or(false) })
        .await?;
    let (w, h) = (v["width"].as_u64().unwrap_or(0) as u32, v["height"].as_u64().unwrap_or(0) as u32);
    let rgba = base64::engine::general_purpose::STANDARD
        .decode(v["rgba"].as_str().unwrap_or_default())
        .map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?;
    let mut out = Vec::with_capacity(8 + rgba.len());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&rgba);
    Ok(Response::new(out))
}

/// Raw pixels of a stored image (reference images), same layout as `runtime_render`.
#[tauri::command]
async fn runtime_render_image(state: State<'_, AppState>, project: Option<String>, image: u32) -> Result<Response, CommandError> {
    let v = client(&state).await?.request(RequestBody::RenderImage { project, image }).await?;
    let (w, h) = (v["width"].as_u64().unwrap_or(0) as u32, v["height"].as_u64().unwrap_or(0) as u32);
    let rgba = base64::engine::general_purpose::STANDARD
        .decode(v["rgba"].as_str().unwrap_or_default())
        .map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?;
    let mut out = Vec::with_capacity(8 + rgba.len());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&rgba);
    Ok(Response::new(out))
}

/// Connection info and the configuration snippet for Claude Code.
#[tauri::command]
async fn runtime_info(state: State<'_, AppState>) -> Result<Value, CommandError> {
    let connected = state.client.lock().await.as_ref().is_some_and(|c| c.is_alive());
    Ok(json!({
        "connected": connected,
        "endpoint": state.endpoint,
        "psm": state.psm.display().to_string(),
        "psm_exists": state.psm.exists(),
        "claude_code_config": {
            "mcpServers": { "pixel-studio-maker": { "type": "stdio", "command": state.psm.display().to_string(), "args": ["mcp"] } }
        },
        "claude_code_command": format!("claude mcp add pixel-studio-maker -- \"{}\" mcp", state.psm.display()),
        "tools": sprite_commands::specs().len(),
    }))
}

pub fn run() {
    let state: AppState = Arc::new(Shared { client: Mutex::new(None), endpoint: sprite_runtime::default_endpoint(), psm: find_psm() });
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            let st = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(connection_loop(handle, st));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            runtime_call,
            runtime_state,
            runtime_select,
            runtime_render,
            runtime_render_image,
            runtime_info
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o Pixel Studio Maker");
}

use crate::config::{AuthMethod, TunnelConfig, TunnelType};
use chrono::Utc;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelStatus {
    Stopped,
    Starting,
    Running,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelState {
    pub id: String,
    pub status: TunnelStatus,
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: Option<String>,
    #[serde(rename = "reconnectCount")]
    pub reconnect_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    #[serde(rename = "tunnelId")]
    pub tunnel_id: String,
    #[serde(rename = "tunnelName")]
    pub tunnel_name: String,
    pub level: String,
    pub message: String,
}

pub struct TunnelProcess {
    pub child: Child,
    pub state: TunnelState,
    pub config: TunnelConfig,
}

pub type TunnelManager = Arc<Mutex<HashMap<String, TunnelProcess>>>;

pub fn new_manager() -> TunnelManager {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn build_plink_args(tunnel: &TunnelConfig, plink_path: &str) -> (String, Vec<String>) {
    let mut args = vec![
        "-N".to_string(),        // no shell
        "-batch".to_string(),    // non-interactive
        "-ssh".to_string(),      // force SSH
    ];

    // Port
    if tunnel.port != 22 {
        args.push("-P".to_string());
        args.push(tunnel.port.to_string());
    }

    // Auth
    match &tunnel.auth_method {
        AuthMethod::Key => {
            if let Some(ref key) = tunnel.key_path {
                args.push("-i".to_string());
                args.push(key.clone());
            }
        }
        AuthMethod::Password => {
            // plink will prompt — but in batch mode this will fail
            // User should use key-based auth for unattended tunnels
        }
    }

    // Tunnel forwarding
    match tunnel.tunnel_type {
        TunnelType::Local => {
            args.push("-L".to_string());
            args.push(format!(
                "{}:{}:{}",
                tunnel.local_port, tunnel.remote_host, tunnel.remote_port
            ));
        }
        TunnelType::Remote => {
            args.push("-R".to_string());
            args.push(format!(
                "{}:{}:{}",
                tunnel.remote_port, tunnel.remote_host, tunnel.local_port
            ));
        }
        TunnelType::Dynamic => {
            args.push("-D".to_string());
            args.push(tunnel.local_port.to_string());
        }
    }

    // user@host
    args.push(format!("{}@{}", tunnel.username, tunnel.host));

    (plink_path.to_string(), args)
}

pub async fn start_tunnel(
    manager: &TunnelManager,
    tunnel: &TunnelConfig,
    plink_path: &str,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let (cmd, args) = build_plink_args(tunnel, plink_path);

    info!("Starting tunnel '{}': {} {}", tunnel.name, cmd, args.join(" "));

    let mut child = Command::new(&cmd)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to start plink: {}. Is '{}' in PATH?", e, cmd))?;

    let state = TunnelState {
        id: tunnel.id.clone(),
        status: TunnelStatus::Running,
        last_error: None,
        started_at: Some(Utc::now().to_rfc3339()),
        reconnect_count: 0,
    };

    // Stream stderr to logs
    let tunnel_id = tunnel.id.clone();
    let tunnel_name = tunnel.name.clone();
    let handle = app_handle.clone();
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        tokio::spawn(async move {
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let entry = LogEntry {
                    timestamp: Utc::now().to_rfc3339(),
                    tunnel_id: tunnel_id.clone(),
                    tunnel_name: tunnel_name.clone(),
                    level: "info".to_string(),
                    message: line,
                };
                let _ = handle.emit("tunnel-log", &entry);
            }
        });
    }

    let mut mgr = manager.lock().await;
    mgr.insert(
        tunnel.id.clone(),
        TunnelProcess {
            child,
            state,
            config: tunnel.clone(),
        },
    );

    // Emit status update
    let _ = app_handle.emit("tunnel-status", &get_all_states_inner(&mgr));

    Ok(())
}

pub async fn stop_tunnel(
    manager: &TunnelManager,
    tunnel_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    let mut mgr = manager.lock().await;
    if let Some(process) = mgr.get_mut(tunnel_id) {
        info!("Stopping tunnel '{}'", process.config.name);
        let _ = process.child.kill().await;
        process.state.status = TunnelStatus::Stopped;
        process.state.last_error = None;

        let _ = app_handle.emit("tunnel-status", &get_all_states_inner(&mgr));
    }
    mgr.remove(tunnel_id);
    Ok(())
}

pub async fn get_all_states(manager: &TunnelManager) -> Vec<TunnelState> {
    let mgr = manager.lock().await;
    get_all_states_inner(&mgr)
}

fn get_all_states_inner(mgr: &HashMap<String, TunnelProcess>) -> Vec<TunnelState> {
    mgr.values().map(|p| p.state.clone()).collect()
}

pub async fn check_tunnel_health(manager: &TunnelManager) -> Vec<String> {
    let mut dead_tunnels = Vec::new();
    let mut mgr = manager.lock().await;

    for (id, process) in mgr.iter_mut() {
        if process.state.status == TunnelStatus::Running {
            match process.child.try_wait() {
                Ok(Some(exit)) => {
                    warn!(
                        "Tunnel '{}' exited with status: {:?}",
                        process.config.name, exit
                    );
                    process.state.status = TunnelStatus::Error;
                    process.state.last_error =
                        Some(format!("Process exited with code: {:?}", exit.code()));
                    dead_tunnels.push(id.clone());
                }
                Ok(None) => {} // still running
                Err(e) => {
                    error!("Error checking tunnel '{}': {}", process.config.name, e);
                    process.state.status = TunnelStatus::Error;
                    process.state.last_error = Some(format!("Health check error: {}", e));
                    dead_tunnels.push(id.clone());
                }
            }
        }
    }

    dead_tunnels
}

use crate::config::{load_config, AppConfig};
use crate::tunnel::{self, TunnelManager, TunnelStatus};
use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::Mutex;
use tokio::time::sleep;

pub struct MonitorState {
    pub running: bool,
    pub reconnect_attempts: std::collections::HashMap<String, u32>,
}

pub type Monitor = Arc<Mutex<MonitorState>>;

pub fn new_monitor() -> Monitor {
    Arc::new(Mutex::new(MonitorState {
        running: false,
        reconnect_attempts: std::collections::HashMap::new(),
    }))
}

pub async fn start_monitor(
    manager: TunnelManager,
    monitor: Monitor,
    app_handle: tauri::AppHandle,
) {
    {
        let mut mon = monitor.lock().await;
        if mon.running {
            return;
        }
        mon.running = true;
    }

    info!("Tunnel monitor started");

    loop {
        {
            let mon = monitor.lock().await;
            if !mon.running {
                break;
            }
        }

        sleep(Duration::from_secs(3)).await;

        // Check health
        let dead = tunnel::check_tunnel_health(&manager).await;

        if dead.is_empty() {
            continue;
        }

        // Try to reconnect dead tunnels
        let config: AppConfig = load_config();

        for tunnel_id in &dead {
            let tunnel_config = config.tunnels.iter().find(|t| &t.id == tunnel_id);

            let tunnel_config = match tunnel_config {
                Some(t) if t.auto_connect && t.enabled => t,
                _ => continue,
            };

            let attempts = {
                let mut mon = monitor.lock().await;
                let count = mon.reconnect_attempts.entry(tunnel_id.clone()).or_insert(0);
                *count += 1;
                *count
            };

            // Max attempts check (0 = unlimited)
            if config.settings.max_reconnect_attempts > 0
                && attempts > config.settings.max_reconnect_attempts
            {
                warn!(
                    "Tunnel '{}' exceeded max reconnect attempts ({})",
                    tunnel_config.name, config.settings.max_reconnect_attempts
                );

                if config.settings.notify_on_disconnect {
                    let _ = app_handle.emit(
                        "notification",
                        serde_json::json!({
                            "title": "OpenTunnel",
                            "body": format!("Tunnel '{}' failed after {} attempts", tunnel_config.name, attempts),
                            "type": "error"
                        }),
                    );
                }
                continue;
            }

            // Exponential backoff: base_delay * 2^(attempts-1), max 300s
            let delay = std::cmp::min(
                config.settings.reconnect_delay_sec * 2u64.pow(attempts.saturating_sub(1)),
                300,
            );

            info!(
                "Reconnecting tunnel '{}' in {}s (attempt {})",
                tunnel_config.name, delay, attempts
            );

            // Update status to reconnecting
            {
                let mut mgr = manager.lock().await;
                if let Some(process) = mgr.get_mut(tunnel_id) {
                    process.state.status = TunnelStatus::Reconnecting;
                    process.state.reconnect_count = attempts;
                }
            }

            let _ = app_handle.emit(
                "tunnel-status",
                &tunnel::get_all_states(&manager).await,
            );

            sleep(Duration::from_secs(delay)).await;

            // Remove dead process before restarting
            {
                let mut mgr = manager.lock().await;
                mgr.remove(tunnel_id);
            }

            // Restart
            match tunnel::start_tunnel(
                &manager,
                tunnel_config,
                &config.settings.plink_path,
                app_handle.clone(),
            )
            .await
            {
                Ok(_) => {
                    info!("Tunnel '{}' reconnected successfully", tunnel_config.name);
                    // Reset attempts on success
                    let mut mon = monitor.lock().await;
                    mon.reconnect_attempts.remove(tunnel_id);

                    if config.settings.notify_on_reconnect {
                        let _ = app_handle.emit(
                            "notification",
                            serde_json::json!({
                                "title": "OpenTunnel",
                                "body": format!("Tunnel '{}' reconnected", tunnel_config.name),
                                "type": "success"
                            }),
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to reconnect '{}': {}", tunnel_config.name, e);

                    if config.settings.notify_on_disconnect {
                        let _ = app_handle.emit(
                            "notification",
                            serde_json::json!({
                                "title": "OpenTunnel",
                                "body": format!("Tunnel '{}' reconnect failed: {}", tunnel_config.name, e),
                                "type": "error"
                            }),
                        );
                    }
                }
            }
        }
    }
}

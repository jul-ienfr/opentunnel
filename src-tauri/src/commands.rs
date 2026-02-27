use crate::config::{self, AppConfig, TunnelConfig};
use crate::tunnel::{self, TunnelManager, TunnelState};
use log::info;
use uuid::Uuid;

#[cfg(windows)]
use crate::putty_import;

// ── Tunnel CRUD ──

#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    Ok(config::load_config())
}

#[tauri::command]
pub async fn save_settings(settings: config::Settings) -> Result<(), String> {
    let mut cfg = config::load_config();
    cfg.settings = settings;
    config::save_config(&cfg)
}

#[tauri::command]
pub async fn add_tunnel(mut tunnel: TunnelConfig) -> Result<TunnelConfig, String> {
    if tunnel.id.is_empty() {
        tunnel.id = Uuid::new_v4().to_string();
    }
    let mut cfg = config::load_config();
    cfg.tunnels.push(tunnel.clone());
    config::save_config(&cfg)?;
    Ok(tunnel)
}

#[tauri::command]
pub async fn update_tunnel(tunnel: TunnelConfig) -> Result<(), String> {
    let mut cfg = config::load_config();
    if let Some(existing) = cfg.tunnels.iter_mut().find(|t| t.id == tunnel.id) {
        *existing = tunnel;
        config::save_config(&cfg)?;
        Ok(())
    } else {
        Err("Tunnel not found".to_string())
    }
}

#[tauri::command]
pub async fn delete_tunnel(
    id: String,
    manager: tauri::State<'_, TunnelManager>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Stop if running
    tunnel::stop_tunnel(&manager, &id, &app_handle).await?;

    let mut cfg = config::load_config();
    cfg.tunnels.retain(|t| t.id != id);
    config::save_config(&cfg)
}

// ── Tunnel Control ──

#[tauri::command]
pub async fn start_tunnel_cmd(
    id: String,
    manager: tauri::State<'_, TunnelManager>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let cfg = config::load_config();
    let tunnel_cfg = cfg
        .tunnels
        .iter()
        .find(|t| t.id == id)
        .ok_or("Tunnel not found")?;

    tunnel::start_tunnel(&manager, tunnel_cfg, &cfg.settings.plink_path, app_handle).await
}

#[tauri::command]
pub async fn stop_tunnel_cmd(
    id: String,
    manager: tauri::State<'_, TunnelManager>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    tunnel::stop_tunnel(&manager, &id, &app_handle).await
}

#[tauri::command]
pub async fn start_all_tunnels(
    manager: tauri::State<'_, TunnelManager>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let cfg = config::load_config();
    for tunnel_cfg in &cfg.tunnels {
        if tunnel_cfg.enabled {
            let _ = tunnel::start_tunnel(
                &manager,
                tunnel_cfg,
                &cfg.settings.plink_path,
                app_handle.clone(),
            )
            .await;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn stop_all_tunnels(
    manager: tauri::State<'_, TunnelManager>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let ids: Vec<String> = {
        let mgr = manager.lock().await;
        mgr.keys().cloned().collect()
    };
    for id in ids {
        tunnel::stop_tunnel(&manager, &id, &app_handle).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_tunnel_states(
    manager: tauri::State<'_, TunnelManager>,
) -> Result<Vec<TunnelState>, String> {
    Ok(tunnel::get_all_states(&manager).await)
}

// ── PuTTY Import ──

#[tauri::command]
pub async fn import_putty_sessions() -> Result<Vec<TunnelConfig>, String> {
    #[cfg(windows)]
    {
        putty_import::import_sessions()
    }
    #[cfg(not(windows))]
    {
        Err("PuTTY import is only available on Windows".to_string())
    }
}

// ── Auto-start ──

#[tauri::command]
pub async fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                KEY_SET_VALUE | KEY_READ,
            )
            .map_err(|e| format!("Failed to open registry: {}", e))?;

        if enabled {
            let exe_path = std::env::current_exe()
                .map_err(|e| format!("Failed to get exe path: {}", e))?;
            run_key
                .set_value("OpenTunnel", &exe_path.to_string_lossy().to_string())
                .map_err(|e| format!("Failed to set autostart: {}", e))?;
            info!("Autostart enabled");
        } else {
            let _ = run_key.delete_value("OpenTunnel");
            info!("Autostart disabled");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("Autostart is only available on Windows".to_string())
    }
}

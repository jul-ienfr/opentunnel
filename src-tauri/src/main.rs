#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod monitor;
mod putty_import;
mod tunnel;

use config::load_config;
use log::info;

fn main() {
    env_logger::init();

    let manager = tunnel::new_manager();
    let mon = monitor::new_monitor();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(manager.clone())
        .manage(mon.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_settings,
            commands::add_tunnel,
            commands::update_tunnel,
            commands::delete_tunnel,
            commands::start_tunnel_cmd,
            commands::stop_tunnel_cmd,
            commands::start_all_tunnels,
            commands::stop_all_tunnels,
            commands::get_tunnel_states,
            commands::import_putty_sessions,
            commands::set_autostart,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let mgr = manager.clone();
            let monitor_state = mon.clone();

            // Start monitor thread
            tauri::async_runtime::spawn(async move {
                monitor::start_monitor(mgr.clone(), monitor_state, handle.clone()).await;
            });

            // Auto-connect tunnels
            let mgr2 = manager.clone();
            let handle2 = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let cfg = load_config();
                for t in &cfg.tunnels {
                    if t.auto_connect && t.enabled {
                        info!("Auto-connecting tunnel '{}'", t.name);
                        let _ = tunnel::start_tunnel(
                            &mgr2,
                            t,
                            &cfg.settings.plink_path,
                            handle2.clone(),
                        )
                        .await;
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running OpenTunnel");
}

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(rename = "authMethod")]
    pub auth_method: AuthMethod,
    #[serde(rename = "keyPath", skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    #[serde(rename = "type")]
    pub tunnel_type: TunnelType,
    #[serde(rename = "localPort")]
    pub local_port: u16,
    #[serde(rename = "remoteHost")]
    pub remote_host: String,
    #[serde(rename = "remotePort")]
    pub remote_port: u16,
    #[serde(rename = "autoConnect", default)]
    pub auto_connect: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    Password,
    Key,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelType {
    Local,
    Remote,
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(rename = "plinkPath", default = "default_plink_path")]
    pub plink_path: String,
    #[serde(rename = "startWithWindows", default)]
    pub start_with_windows: bool,
    #[serde(rename = "startMinimized", default = "default_true")]
    pub start_minimized: bool,
    #[serde(rename = "reconnectDelaySec", default = "default_reconnect_delay")]
    pub reconnect_delay_sec: u64,
    #[serde(rename = "maxReconnectAttempts", default)]
    pub max_reconnect_attempts: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(rename = "notifyOnDisconnect", default = "default_true")]
    pub notify_on_disconnect: bool,
    #[serde(rename = "notifyOnReconnect", default = "default_true")]
    pub notify_on_reconnect: bool,
}

fn default_plink_path() -> String {
    "plink.exe".to_string()
}

fn default_reconnect_delay() -> u64 {
    5
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            plink_path: default_plink_path(),
            start_with_windows: false,
            start_minimized: true,
            reconnect_delay_sec: default_reconnect_delay(),
            max_reconnect_attempts: 0,
            theme: default_theme(),
            notify_on_disconnect: true,
            notify_on_reconnect: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub tunnels: Vec<TunnelConfig>,
    pub settings: Settings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tunnels: Vec::new(),
            settings: Settings::default(),
        }
    }
}

impl TunnelConfig {
    #[allow(dead_code)]
    pub fn new(name: String, host: String, username: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            host,
            port: 22,
            username,
            auth_method: AuthMethod::Key,
            key_path: None,
            tunnel_type: TunnelType::Local,
            local_port: 0,
            remote_host: "127.0.0.1".to_string(),
            remote_port: 0,
            auto_connect: false,
            enabled: true,
        }
    }
}

pub fn config_dir() -> PathBuf {
    let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(".opentunnel")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    if !path.exists() {
        return AppConfig::default();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
    let json =
        serde_json::to_string_pretty(config).map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(config_path(), json).map_err(|e| format!("Failed to write config: {}", e))?;
    Ok(())
}

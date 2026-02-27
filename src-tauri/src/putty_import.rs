#[cfg(windows)]
use crate::config::{AuthMethod, TunnelConfig, TunnelType};
#[cfg(windows)]
use uuid::Uuid;

#[cfg(windows)]
pub fn import_sessions() -> Result<Vec<TunnelConfig>, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let sessions_key = hkcu
        .open_subkey(r"Software\SimonTatham\PuTTY\Sessions")
        .map_err(|_| "PuTTY sessions not found in registry. Is PuTTY installed?".to_string())?;

    let mut tunnels = Vec::new();

    for session_name in sessions_key.enum_keys().filter_map(|k| k.ok()) {
        let session_key = match sessions_key.open_subkey(&session_name) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let host: String = session_key.get_value("HostName").unwrap_or_default();
        let port: u32 = session_key.get_value("PortNumber").unwrap_or(22);
        let username: String = session_key.get_value("UserName").unwrap_or_default();
        let key_path: String = session_key.get_value("PublicKeyFile").unwrap_or_default();
        let port_fwds: String = session_key.get_value("PortForwardings").unwrap_or_default();

        if host.is_empty() || port_fwds.is_empty() {
            continue;
        }

        // Parse PuTTY port forwarding format: "L8080=localhost:80,R9090=remote:90,D1080="
        for fwd in port_fwds.split(',') {
            let fwd = fwd.trim();
            if fwd.is_empty() {
                continue;
            }

            let (tunnel_type, rest) = match fwd.chars().next() {
                Some('L') => (TunnelType::Local, &fwd[1..]),
                Some('R') => (TunnelType::Remote, &fwd[1..]),
                Some('D') => (TunnelType::Dynamic, &fwd[1..]),
                _ => continue,
            };

            let parts: Vec<&str> = rest.splitn(2, '=').collect();
            if parts.is_empty() {
                continue;
            }

            let local_port: u16 = parts[0].parse().unwrap_or(0);
            if local_port == 0 {
                continue;
            }

            let (remote_host, remote_port) = if tunnel_type == TunnelType::Dynamic {
                ("127.0.0.1".to_string(), 0u16)
            } else if parts.len() > 1 {
                let dest_parts: Vec<&str> = parts[1].rsplitn(2, ':').collect();
                if dest_parts.len() == 2 {
                    (
                        dest_parts[1].to_string(),
                        dest_parts[0].parse().unwrap_or(0),
                    )
                } else {
                    continue;
                }
            } else {
                continue;
            };

            let decoded_name = urlencoding_decode(&session_name);

            tunnels.push(TunnelConfig {
                id: Uuid::new_v4().to_string(),
                name: format!("{} ({}:{})", decoded_name, remote_host, remote_port),
                host: host.clone(),
                port: port as u16,
                username: username.clone(),
                auth_method: if key_path.is_empty() {
                    AuthMethod::Password
                } else {
                    AuthMethod::Key
                },
                key_path: if key_path.is_empty() {
                    None
                } else {
                    Some(key_path.clone())
                },
                tunnel_type,
                local_port,
                remote_host,
                remote_port,
                auto_connect: false,
                enabled: true,
            });
        }
    }

    Ok(tunnels)
}

#[cfg(windows)]
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else {
            result.push(c);
        }
    }
    result
}

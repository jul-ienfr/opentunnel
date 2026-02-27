# OpenTunnel

Modern SSH tunnel manager for Windows — a MyEnTunnel replacement built with Tauri + Rust.

## Features

- **Multi-tunnel management** — Create, edit, delete multiple SSH tunnels
- **System tray** — Runs minimized in the Windows tray with status indicators
- **Auto-reconnect** — Automatically reconnects dropped tunnels with exponential backoff
- **Real-time monitoring** — Live status and logs for each tunnel
- **PuTTY import** — Import existing PuTTY sessions and their port forwardings
- **Windows notifications** — Toast alerts on disconnect/reconnect events
- **Start with Windows** — Optional autostart via registry
- **Dark theme** — Modern dark UI

## Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+)
- [plink.exe](https://www.chiark.greenend.org.uk/~sgtatham/putty/latest.html) (from PuTTY) — must be in PATH or configured in settings

## Development

```bash
# Install Tauri CLI
cargo install tauri-cli

# Run in development mode
cargo tauri dev

# Build release
cargo tauri build
```

The release build produces:
- `src-tauri/target/release/opentunnel.exe` — standalone executable
- NSIS installer in `src-tauri/target/release/bundle/nsis/`
- MSI installer in `src-tauri/target/release/bundle/msi/`

## Configuration

Config is stored at `%USERPROFILE%\.opentunnel\config.json`.

### Tunnel types

| Type | Flag | Description |
|------|------|-------------|
| Local | `-L` | Forward local port to remote host |
| Remote | `-R` | Forward remote port to local host |
| Dynamic | `-D` | SOCKS proxy |

### Example config

```json
{
  "tunnels": [
    {
      "id": "...",
      "name": "My Web Server",
      "host": "192.168.1.100",
      "port": 22,
      "username": "admin",
      "authMethod": "key",
      "keyPath": "C:\\Users\\me\\.ssh\\id_rsa",
      "type": "local",
      "localPort": 8080,
      "remoteHost": "127.0.0.1",
      "remotePort": 80,
      "autoConnect": true,
      "enabled": true
    }
  ],
  "settings": {
    "plinkPath": "plink.exe",
    "startWithWindows": false,
    "startMinimized": true,
    "reconnectDelaySec": 5,
    "maxReconnectAttempts": 0
  }
}
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+N` | Add new tunnel |
| `Escape` | Close modal |

## Architecture

```
src-tauri/src/
  main.rs          — Tauri app entry, system tray, auto-connect
  config.rs        — JSON config persistence
  tunnel.rs        — plink process management (spawn/kill/health)
  monitor.rs       — Auto-reconnect with exponential backoff
  commands.rs      — Tauri commands (frontend API)
  putty_import.rs  — Import PuTTY sessions from Windows registry
```

## License

MIT

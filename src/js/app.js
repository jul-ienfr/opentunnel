const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ── State ──

let config = { tunnels: [], settings: {} };
let tunnelStates = new Map();
let logs = [];
const MAX_LOGS = 1000;

// ── Init ──

document.addEventListener('DOMContentLoaded', async () => {
    await loadConfig();
    renderTunnels();
    setupEventListeners();
    setupTauriListeners();
    setupKeyboard();
});

async function loadConfig() {
    try {
        config = await invoke('get_config');
    } catch (e) {
        addLog('system', 'OpenTunnel', 'error', `Failed to load config: ${e}`);
    }
}

// ── Tauri Event Listeners ──

async function setupTauriListeners() {
    await listen('tunnel-status', (event) => {
        const states = event.payload;
        tunnelStates.clear();
        for (const s of states) {
            tunnelStates.set(s.id, s);
        }
        renderTunnels();
    });

    await listen('tunnel-log', (event) => {
        const entry = event.payload;
        addLog(entry.tunnelId, entry.tunnelName, entry.level, entry.message);
    });

    await listen('notification', (event) => {
        const n = event.payload;
        addLog('system', 'OpenTunnel', n.type, n.body);
    });

    // Fetch initial states
    try {
        const states = await invoke('get_tunnel_states');
        for (const s of states) {
            tunnelStates.set(s.id, s);
        }
        renderTunnels();
    } catch (_) {}
}

// ── Render ──

function renderTunnels() {
    const list = document.getElementById('tunnel-list');
    const empty = document.getElementById('empty-state');

    if (config.tunnels.length === 0) {
        list.innerHTML = '';
        list.appendChild(empty);
        empty.style.display = '';
        return;
    }

    empty.style.display = 'none';
    list.innerHTML = config.tunnels.map(t => {
        const state = tunnelStates.get(t.id);
        const status = state?.status || 'stopped';
        const typeLabel = t.type === 'local' ? 'L' : t.type === 'remote' ? 'R' : 'D';
        const detail = t.type === 'dynamic'
            ? `${typeLabel} :${t.localPort} via ${t.username}@${t.host}`
            : `${typeLabel} :${t.localPort} -> ${t.remoteHost}:${t.remotePort} via ${t.username}@${t.host}`;

        const isRunning = status === 'running' || status === 'starting' || status === 'reconnecting';
        const toggleBtn = isRunning
            ? `<button class="btn btn-sm btn-danger" onclick="stopTunnel('${t.id}')" title="Stop">&#9632;</button>`
            : `<button class="btn btn-sm btn-success" onclick="startTunnel('${t.id}')" title="Start">&#9654;</button>`;

        const reconnectInfo = state?.reconnectCount > 0
            ? ` <span style="color:var(--warning)">(retry #${state.reconnectCount})</span>`
            : '';

        const errorInfo = state?.lastError
            ? ` <span style="color:var(--danger)" title="${state.lastError}">&#9888;</span>`
            : '';

        return `
            <div class="tunnel-card" data-id="${t.id}">
                <div class="tunnel-status status-${status}" title="${status}"></div>
                <div class="tunnel-info">
                    <div class="tunnel-name">${escapeHtml(t.name)}${reconnectInfo}${errorInfo}</div>
                    <div class="tunnel-detail">${escapeHtml(detail)}</div>
                </div>
                <div class="tunnel-actions">
                    ${toggleBtn}
                    <button class="btn-icon" onclick="editTunnel('${t.id}')" title="Edit">&#9998;</button>
                    <button class="btn-icon" onclick="deleteTunnel('${t.id}')" title="Delete">&#128465;</button>
                </div>
            </div>
        `;
    }).join('');
}

// ── Tunnel Actions ──

window.startTunnel = async function(id) {
    try {
        await invoke('start_tunnel_cmd', { id });
        addLog(id, getTunnelName(id), 'success', 'Tunnel started');
    } catch (e) {
        addLog(id, getTunnelName(id), 'error', `Failed to start: ${e}`);
    }
};

window.stopTunnel = async function(id) {
    try {
        await invoke('stop_tunnel_cmd', { id });
        addLog(id, getTunnelName(id), 'info', 'Tunnel stopped');
    } catch (e) {
        addLog(id, getTunnelName(id), 'error', `Failed to stop: ${e}`);
    }
};

window.editTunnel = function(id) {
    const tunnel = config.tunnels.find(t => t.id === id);
    if (!tunnel) return;
    openTunnelModal(tunnel);
};

window.deleteTunnel = async function(id) {
    const tunnel = config.tunnels.find(t => t.id === id);
    if (!tunnel) return;
    if (!confirm(`Delete tunnel "${tunnel.name}"?`)) return;

    try {
        await invoke('delete_tunnel', { id });
        config.tunnels = config.tunnels.filter(t => t.id !== id);
        tunnelStates.delete(id);
        renderTunnels();
        addLog('system', 'OpenTunnel', 'info', `Tunnel "${tunnel.name}" deleted`);
    } catch (e) {
        addLog('system', 'OpenTunnel', 'error', `Failed to delete: ${e}`);
    }
};

// ── Modal: Tunnel ──

function openTunnelModal(tunnel = null) {
    const modal = document.getElementById('modal-tunnel');
    const title = document.getElementById('modal-title');

    if (tunnel) {
        title.textContent = 'Edit Tunnel';
        document.getElementById('tunnel-id').value = tunnel.id;
        document.getElementById('tunnel-name').value = tunnel.name;
        document.getElementById('tunnel-host').value = tunnel.host;
        document.getElementById('tunnel-port').value = tunnel.port;
        document.getElementById('tunnel-username').value = tunnel.username;
        document.getElementById('tunnel-auth').value = tunnel.authMethod;
        document.getElementById('tunnel-keypath').value = tunnel.keyPath || '';
        document.getElementById('tunnel-type').value = tunnel.type;
        document.getElementById('tunnel-localport').value = tunnel.localPort;
        document.getElementById('tunnel-remotehost').value = tunnel.remoteHost;
        document.getElementById('tunnel-remoteport').value = tunnel.remotePort;
        document.getElementById('tunnel-autoconnect').checked = tunnel.autoConnect;
    } else {
        title.textContent = 'Add Tunnel';
        document.getElementById('tunnel-form').reset();
        document.getElementById('tunnel-id').value = '';
        document.getElementById('tunnel-port').value = 22;
        document.getElementById('tunnel-remotehost').value = '127.0.0.1';
    }

    updateFormVisibility();
    modal.style.display = '';
    document.getElementById('tunnel-name').focus();
}

function closeTunnelModal() {
    document.getElementById('modal-tunnel').style.display = 'none';
}

function updateFormVisibility() {
    const auth = document.getElementById('tunnel-auth').value;
    const type = document.getElementById('tunnel-type').value;

    document.getElementById('key-path-group').style.display = auth === 'key' ? '' : 'none';
    document.getElementById('remote-group').style.display = type === 'dynamic' ? 'none' : '';
}

async function saveTunnel(e) {
    e.preventDefault();

    const id = document.getElementById('tunnel-id').value;
    const tunnel = {
        id: id || '',
        name: document.getElementById('tunnel-name').value.trim(),
        host: document.getElementById('tunnel-host').value.trim(),
        port: parseInt(document.getElementById('tunnel-port').value) || 22,
        username: document.getElementById('tunnel-username').value.trim(),
        authMethod: document.getElementById('tunnel-auth').value,
        keyPath: document.getElementById('tunnel-auth').value === 'key'
            ? document.getElementById('tunnel-keypath').value.trim() || null
            : null,
        type: document.getElementById('tunnel-type').value,
        localPort: parseInt(document.getElementById('tunnel-localport').value),
        remoteHost: document.getElementById('tunnel-remotehost').value.trim() || '127.0.0.1',
        remotePort: parseInt(document.getElementById('tunnel-remoteport').value) || 0,
        autoConnect: document.getElementById('tunnel-autoconnect').checked,
        enabled: true,
    };

    try {
        if (id) {
            await invoke('update_tunnel', { tunnel });
            const idx = config.tunnels.findIndex(t => t.id === id);
            if (idx >= 0) config.tunnels[idx] = tunnel;
        } else {
            const saved = await invoke('add_tunnel', { tunnel });
            config.tunnels.push(saved);
        }
        renderTunnels();
        closeTunnelModal();
        addLog('system', 'OpenTunnel', 'success', `Tunnel "${tunnel.name}" saved`);
    } catch (e) {
        addLog('system', 'OpenTunnel', 'error', `Failed to save: ${e}`);
    }
}

// ── Modal: Settings ──

function openSettings() {
    const s = config.settings;
    document.getElementById('settings-plink').value = s.plinkPath || 'plink.exe';
    document.getElementById('settings-reconnect').value = s.reconnectDelaySec || 5;
    document.getElementById('settings-maxretry').value = s.maxReconnectAttempts || 0;
    document.getElementById('settings-autostart').checked = s.startWithWindows || false;
    document.getElementById('settings-minimized').checked = s.startMinimized !== false;
    document.getElementById('settings-notify-disconnect').checked = s.notifyOnDisconnect !== false;
    document.getElementById('settings-notify-reconnect').checked = s.notifyOnReconnect !== false;
    document.getElementById('modal-settings').style.display = '';
}

function closeSettings() {
    document.getElementById('modal-settings').style.display = 'none';
}

async function saveSettings(e) {
    e.preventDefault();

    const settings = {
        plinkPath: document.getElementById('settings-plink').value.trim(),
        startWithWindows: document.getElementById('settings-autostart').checked,
        startMinimized: document.getElementById('settings-minimized').checked,
        reconnectDelaySec: parseInt(document.getElementById('settings-reconnect').value) || 5,
        maxReconnectAttempts: parseInt(document.getElementById('settings-maxretry').value) || 0,
        theme: 'dark',
        notifyOnDisconnect: document.getElementById('settings-notify-disconnect').checked,
        notifyOnReconnect: document.getElementById('settings-notify-reconnect').checked,
    };

    try {
        await invoke('save_settings', { settings });
        config.settings = settings;

        // Handle autostart
        await invoke('set_autostart', { enabled: settings.startWithWindows });

        closeSettings();
        addLog('system', 'OpenTunnel', 'success', 'Settings saved');
    } catch (e) {
        addLog('system', 'OpenTunnel', 'error', `Failed to save settings: ${e}`);
    }
}

// ── Import PuTTY ──

async function importPuTTY() {
    try {
        const imported = await invoke('import_putty_sessions');
        if (imported.length === 0) {
            addLog('system', 'OpenTunnel', 'info', 'No PuTTY tunnels found to import');
            return;
        }

        for (const t of imported) {
            const saved = await invoke('add_tunnel', { tunnel: t });
            config.tunnels.push(saved);
        }

        renderTunnels();
        addLog('system', 'OpenTunnel', 'success', `Imported ${imported.length} tunnel(s) from PuTTY`);
    } catch (e) {
        addLog('system', 'OpenTunnel', 'error', `PuTTY import failed: ${e}`);
    }
}

// ── Logs ──

function addLog(tunnelId, tunnelName, level, message) {
    const now = new Date().toLocaleTimeString('fr-FR', { hour12: false });
    const entry = { timestamp: now, tunnelId, tunnelName, level, message };

    logs.push(entry);
    if (logs.length > MAX_LOGS) logs.shift();

    const content = document.getElementById('log-content');
    const cls = level === 'error' ? ' error' : level === 'success' ? ' success' : '';
    content.innerHTML += `<div class="log-entry${cls}"><span class="timestamp">[${now}]</span> <span class="tunnel-tag">[${escapeHtml(tunnelName)}]</span> ${escapeHtml(message)}</div>`;
    content.scrollTop = content.scrollHeight;
}

function clearLogs() {
    logs = [];
    document.getElementById('log-content').innerHTML = '';
}

function toggleLogs() {
    document.getElementById('log-panel').classList.toggle('collapsed');
}

// ── Event Listeners ──

function setupEventListeners() {
    document.getElementById('btn-add').addEventListener('click', () => openTunnelModal());
    document.getElementById('btn-settings').addEventListener('click', openSettings);
    document.getElementById('btn-import').addEventListener('click', importPuTTY);

    document.getElementById('btn-start-all').addEventListener('click', async () => {
        try {
            await invoke('start_all_tunnels');
            addLog('system', 'OpenTunnel', 'success', 'All tunnels started');
        } catch (e) {
            addLog('system', 'OpenTunnel', 'error', `Start all failed: ${e}`);
        }
    });

    document.getElementById('btn-stop-all').addEventListener('click', async () => {
        try {
            await invoke('stop_all_tunnels');
            addLog('system', 'OpenTunnel', 'info', 'All tunnels stopped');
        } catch (e) {
            addLog('system', 'OpenTunnel', 'error', `Stop all failed: ${e}`);
        }
    });

    // Tunnel modal
    document.getElementById('tunnel-form').addEventListener('submit', saveTunnel);
    document.getElementById('btn-modal-close').addEventListener('click', closeTunnelModal);
    document.getElementById('btn-cancel').addEventListener('click', closeTunnelModal);
    document.getElementById('tunnel-auth').addEventListener('change', updateFormVisibility);
    document.getElementById('tunnel-type').addEventListener('change', updateFormVisibility);

    // Settings modal
    document.getElementById('settings-form').addEventListener('submit', saveSettings);
    document.getElementById('btn-settings-close').addEventListener('click', closeSettings);
    document.getElementById('btn-settings-cancel').addEventListener('click', closeSettings);

    // Logs
    document.getElementById('btn-clear-logs').addEventListener('click', clearLogs);
    document.getElementById('btn-toggle-logs').addEventListener('click', toggleLogs);

    // Close modals on backdrop click
    document.querySelectorAll('.modal').forEach(modal => {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) modal.style.display = 'none';
        });
    });
}

function setupKeyboard() {
    document.addEventListener('keydown', (e) => {
        // Ctrl+N: New tunnel
        if (e.ctrlKey && e.key === 'n') {
            e.preventDefault();
            openTunnelModal();
        }
        // Escape: Close modals
        if (e.key === 'Escape') {
            document.querySelectorAll('.modal').forEach(m => m.style.display = 'none');
        }
    });
}

// ── Helpers ──

function getTunnelName(id) {
    const t = config.tunnels.find(t => t.id === id);
    return t?.name || id;
}

function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

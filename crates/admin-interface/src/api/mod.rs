pub mod config_viewer;
pub mod errors;
pub mod halt;
pub mod health;
pub mod identity;
pub mod logs;
pub mod permissions;
pub mod wheelhouse;

use axum::response::Html;

/// Dashboard page — main admin view with HTMX live panels.
pub async fn dashboard() -> Html<String> {
    Html(render_page(
        "Dashboard",
        r#"
        <div class="grid">
            <div class="card" hx-get="/api/health" hx-trigger="load, every 10s" hx-swap="innerHTML">
                <h3>Service Health</h3>
                <p class="loading">Loading...</p>
            </div>
            <div class="card" hx-get="/api/services" hx-trigger="load, every 30s" hx-swap="innerHTML">
                <h3>Registered Services</h3>
                <p class="loading">Loading...</p>
            </div>
        </div>
        <div class="grid" style="margin-top:1.5rem">
            <div class="card" id="halt-panel" hx-get="/api/halt" hx-trigger="load, every 10s" hx-swap="innerHTML">
                <h3>Halt Controls</h3>
                <p class="loading">Loading...</p>
            </div>
            <div class="card" id="permissions-panel" hx-get="/api/permissions" hx-trigger="load, every 30s" hx-swap="innerHTML">
                <h3>Permissions</h3>
                <p class="loading">Loading...</p>
            </div>
        </div>
        <div style="margin-top:1.5rem">
            <div class="card" hx-get="/api/config" hx-trigger="load" hx-swap="innerHTML">
                <h3>Service Configuration</h3>
                <p class="loading">Loading...</p>
            </div>
        </div>
        <div class="grid" style="margin-top:1.5rem">
            <div class="card" hx-get="/api/wheelhouse" hx-trigger="load, every 10s" hx-swap="innerHTML">
                <h3>Wheelhouse Agents</h3>
                <p class="loading">Loading...</p>
            </div>
            <div class="card" hx-get="/api/identity" hx-trigger="load, every 30s" hx-swap="innerHTML">
                <h3>SMS Identity</h3>
                <p class="loading">Loading...</p>
            </div>
        </div>
        <div style="margin-top:1.5rem">
            <div class="card" hx-get="/api/logs?limit=15" hx-trigger="load, every 5s" hx-swap="innerHTML">
                <h3>Recent Logs (WARN+)</h3>
                <p class="loading">Loading...</p>
            </div>
        </div>
        <div style="margin-top:1.5rem">
            <div class="card" hx-get="/api/errors/recent?limit=10" hx-trigger="load, every 10s" hx-swap="innerHTML">
                <h3>Recent Errors</h3>
                <p class="loading">Loading...</p>
            </div>
        </div>
        "#,
    ))
}

/// Full-page log viewer with filters.
pub async fn logs_page() -> Html<String> {
    Html(render_page(
        "Logs",
        r##"
        <h2>System Logs</h2>
        <div class="filter-bar">
            <select id="log-service" name="service" hx-get="/api/logs" hx-trigger="change" hx-target="#log-results" hx-include="[id^='log-']">
                <option value="">All services</option>
                <option value="cloak">Cloak</option>
                <option value="cortex">Cortex</option>
                <option value="gateway">Gateway</option>
                <option value="datastore">Datastore</option>
                <option value="wheelhouse">Wheelhouse</option>
                <option value="admin-interface">Admin</option>
                <option value="analog-communications">Analog</option>
            </select>
            <select id="log-level" name="level" hx-get="/api/logs" hx-trigger="change" hx-target="#log-results" hx-include="[id^='log-']">
                <option value="">All levels</option>
                <option value="ERROR">ERROR</option>
                <option value="WARN">WARN</option>
            </select>
            <input id="log-error_code" name="error_code" placeholder="Error code (e.g. CLOAK)" hx-get="/api/logs" hx-trigger="keyup changed delay:300ms" hx-target="#log-results" hx-include="[id^='log-']" />
            <input type="hidden" id="log-limit" name="limit" value="100" />
        </div>
        <div id="log-results" hx-get="/api/logs?limit=100" hx-trigger="load, every 5s" hx-swap="innerHTML">
            <p class="loading">Loading...</p>
        </div>
        "##,
    ))
}

/// Full-page error report viewer.
pub async fn errors_page() -> Html<String> {
    Html(render_page(
        "Errors",
        r##"
        <h2>Error Reports</h2>
        <div class="filter-bar">
            <select id="err-service" name="service" hx-get="/api/errors/summary" hx-trigger="change" hx-target="#error-summary" hx-include="[id^='err-']">
                <option value="">All services</option>
                <option value="cloak">Cloak</option>
                <option value="cortex">Cortex</option>
                <option value="gateway">Gateway</option>
                <option value="wheelhouse">Wheelhouse</option>
                <option value="analog-communications">Analog</option>
            </select>
            <select id="err-severity" name="severity" hx-get="/api/errors/summary" hx-trigger="change" hx-target="#error-summary" hx-include="[id^='err-']">
                <option value="">All severities</option>
                <option value="critical">Critical</option>
                <option value="error">Error</option>
                <option value="warning">Warning</option>
                <option value="info">Info</option>
            </select>
            <input id="err-code" name="code" placeholder="Code prefix (e.g. GW)" hx-get="/api/errors/summary" hx-trigger="keyup changed delay:300ms" hx-target="#error-summary" hx-include="[id^='err-']" />
        </div>
        <h3>By Error Code</h3>
        <div id="error-summary" hx-get="/api/errors/summary" hx-trigger="load, every 15s" hx-swap="innerHTML">
            <p class="loading">Loading...</p>
        </div>
        <h3 style="margin-top:1.5rem">Recent Instances</h3>
        <div id="error-recent" hx-get="/api/errors/recent?limit=30" hx-trigger="load, every 10s" hx-swap="innerHTML">
            <p class="loading">Loading...</p>
        </div>
        "##,
    ))
}

fn render_page(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head>
<title>{title} — Panorama Admin</title>
<script src="https://unpkg.com/htmx.org@2.0.4"></script>
<style>
:root {{ --bg: #1a1a2e; --card: #16213e; --accent: #e94560; --text: #e0e0e0; --muted: #888; --ok: #4caf50; --warn: #ff9800; --err: #f44336; }}
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{ font-family: system-ui, -apple-system, sans-serif; background: var(--bg); color: var(--text); }}
nav {{ background: var(--card); padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid var(--accent); }}
nav h1 {{ font-size: 1.2rem; }}
nav a {{ color: var(--text); text-decoration: none; margin-left: 1.5rem; font-size: 0.95rem; }}
nav a:hover {{ color: var(--accent); }}
main {{ padding: 2rem; max-width: 1400px; margin: 0 auto; }}
h2 {{ margin-bottom: 1rem; color: var(--accent); }}
h3 {{ margin-bottom: 0.75rem; color: var(--accent); font-size: 1rem; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 1.5rem; }}
.card {{ background: var(--card); border-radius: 8px; padding: 1.5rem; }}
.card h3 {{ margin-bottom: 1rem; }}
.loading {{ color: var(--muted); }}
.status-ok {{ color: var(--ok); }}
.status-degraded {{ color: var(--warn); }}
.status-down {{ color: var(--err); }}
table {{ width: 100%; border-collapse: collapse; font-size: 0.9rem; }}
th, td {{ text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid #2a2a4a; }}
th {{ color: var(--muted); font-weight: 600; }}
code {{ background: #0d1117; padding: 0.15rem 0.4rem; border-radius: 3px; font-size: 0.85rem; }}
.suggestion {{ color: var(--muted); font-size: 0.85rem; font-style: italic; }}
.filter-bar {{ display: flex; gap: 0.75rem; margin-bottom: 1.5rem; flex-wrap: wrap; }}
.filter-bar select, .filter-bar input {{ background: var(--card); color: var(--text); border: 1px solid #333; border-radius: 4px; padding: 0.5rem 0.75rem; font-size: 0.9rem; }}
.filter-bar input {{ min-width: 200px; }}
.btn {{ padding: 0.5rem 1rem; border: none; border-radius: 4px; cursor: pointer; font-size: 0.85rem; font-weight: 600; }}
.btn-halt {{ background: var(--err); color: white; }}
.btn-halt:hover {{ background: #d32f2f; }}
.btn-halt-sm {{ background: #5c2020; color: var(--err); border: 1px solid var(--err); padding: 0.3rem 0.6rem; font-size: 0.8rem; margin-right: 0.3rem; margin-bottom: 0.3rem; }}
.btn-halt-sm:hover {{ background: var(--err); color: white; }}
.btn-resume {{ background: var(--ok); color: white; }}
.btn-resume:hover {{ background: #388e3c; }}
.btn-add {{ background: var(--accent); color: white; }}
.btn-add:hover {{ opacity: 0.9; }}
.btn-delete {{ background: transparent; color: var(--err); border: 1px solid var(--err); padding: 0.2rem 0.5rem; font-size: 0.8rem; cursor: pointer; border-radius: 3px; }}
.btn-delete:hover {{ background: var(--err); color: white; }}
.halt-status {{ padding: 0.75rem 1rem; border-radius: 6px; font-weight: 700; margin-bottom: 1rem; }}
.halt-active {{ background: #5c2020; color: var(--err); border: 1px solid var(--err); }}
.halt-ok {{ background: #1b3a1b; color: var(--ok); border: 1px solid var(--ok); }}
.perm-form {{ display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 0.5rem; }}
.perm-form input, .perm-form select {{ background: var(--bg); color: var(--text); border: 1px solid #333; border-radius: 4px; padding: 0.4rem 0.6rem; font-size: 0.85rem; }}
.perm-form input {{ flex: 1; min-width: 120px; }}
.pool-summary {{ display: flex; gap: 1.5rem; margin-bottom: 1rem; }}
.pool-summary .stat {{ font-size: 0.95rem; }}
</style>
</head><body>
<nav>
    <h1>Panorama Admin</h1>
    <div>
        <a href="/">Dashboard</a>
        <a href="/logs">Logs</a>
        <a href="/errors">Errors</a>
    </div>
</nav>
<main>{content}</main>
</body></html>"#,
    )
}

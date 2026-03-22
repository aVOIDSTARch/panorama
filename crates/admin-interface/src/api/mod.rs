pub mod health;

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
        "#,
    ))
}

fn render_page(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head>
<title>{title} — Panorama Admin</title>
<script src="https://unpkg.com/htmx.org@2.0.4"></script>
<style>
:root {{ --bg: #1a1a2e; --card: #16213e; --accent: #e94560; --text: #e0e0e0; --muted: #888; }}
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{ font-family: system-ui, -apple-system, sans-serif; background: var(--bg); color: var(--text); }}
nav {{ background: var(--card); padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid var(--accent); }}
nav h1 {{ font-size: 1.2rem; }}
nav a {{ color: var(--text); text-decoration: none; margin-left: 1.5rem; }}
nav a:hover {{ color: var(--accent); }}
main {{ padding: 2rem; max-width: 1200px; margin: 0 auto; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 1.5rem; }}
.card {{ background: var(--card); border-radius: 8px; padding: 1.5rem; }}
.card h3 {{ margin-bottom: 1rem; color: var(--accent); }}
.loading {{ color: var(--muted); }}
.status-ok {{ color: #4caf50; }}
.status-degraded {{ color: #ff9800; }}
.status-down {{ color: #f44336; }}
table {{ width: 100%; border-collapse: collapse; }}
th, td {{ text-align: left; padding: 0.5rem; border-bottom: 1px solid #333; }}
</style>
</head><body>
<nav>
    <h1>Panorama Admin</h1>
    <div>
        <a href="/">Dashboard</a>
        <a href="/services">Services</a>
        <a href="/logs">Logs</a>
    </div>
</nav>
<main>{content}</main>
</body></html>"#,
    )
}

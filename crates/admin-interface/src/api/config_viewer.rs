use axum::response::Html;

/// HTMX fragment: service configuration from cortex-manifest.toml.
pub async fn config_panel() -> Html<String> {
    let manifest_path =
        std::env::var("CORTEX_MANIFEST").unwrap_or_else(|_| "cortex-manifest.toml".into());

    let content = match std::fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            return Html(format!(
                "<p class='status-down'>Cannot read manifest at {}: {e}</p>",
                html_escape(&manifest_path)
            ));
        }
    };

    let manifest: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(e) => {
            return Html(format!(
                "<p class='status-down'>Invalid TOML: {e}</p>"
            ));
        }
    };

    let mut html = String::new();

    // Parse services section
    if let Some(services) = manifest.get("services").and_then(|s| s.as_table()) {
        html.push_str(
            "<table><thead><tr>\
             <th>Service</th><th>Name</th><th>Base URL</th><th>Health Path</th>\
             <th>Timeout</th><th>Queue TTL</th>\
             </tr></thead><tbody>",
        );

        let mut service_keys: Vec<&String> = services.keys().collect();
        service_keys.sort();

        for key in service_keys {
            let svc = &services[key];
            let name = svc.get("name").and_then(|v| v.as_str()).unwrap_or(key);
            let base_url = svc
                .get("base_url")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let health = svc
                .get("health_path")
                .and_then(|v| v.as_str())
                .unwrap_or("/health");
            let timeout = svc
                .get("timeout_ms")
                .and_then(|v| v.as_integer())
                .unwrap_or(5000);
            let queue_ttl = svc
                .get("queue_ttl_s")
                .and_then(|v| v.as_integer())
                .unwrap_or(30);

            html.push_str(&format!(
                "<tr>\
                 <td><code>{}</code></td>\
                 <td>{}</td>\
                 <td><code>{}</code></td>\
                 <td><code>{}</code></td>\
                 <td>{timeout}ms</td>\
                 <td>{queue_ttl}s</td>\
                 </tr>",
                html_escape(key),
                html_escape(name),
                html_escape(base_url),
                html_escape(health),
            ));
        }

        html.push_str("</tbody></table>");
    } else {
        html.push_str("<p class='loading'>No services defined in manifest</p>");
    }

    // Show any other top-level sections
    let known_keys = ["services"];
    let extra_keys: Vec<&String> = manifest
        .as_table()
        .map(|t| {
            t.keys()
                .filter(|k| !known_keys.contains(&k.as_str()))
                .collect()
        })
        .unwrap_or_default();

    if !extra_keys.is_empty() {
        html.push_str("<h4 style='margin-top:1rem;color:var(--muted)'>Other Config</h4><pre><code>");
        for key in extra_keys {
            if let Some(val) = manifest.get(key) {
                html.push_str(&html_escape(&format!("[{key}]\n{val}\n\n")));
            }
        }
        html.push_str("</code></pre>");
    }

    Html(html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

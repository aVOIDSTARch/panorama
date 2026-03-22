use crate::alerts::{sms, webhook};
use crate::config::AlertsConfig;
use crate::types::{Alert, AlertLevel};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

pub struct AlertRouter {
    config: AlertsConfig,
    last_sms: Mutex<HashMap<AlertLevel, Instant>>,
    sms_count_hour: Mutex<(u32, Instant)>,
    http_client: reqwest::Client,
}

impl AlertRouter {
    pub fn new(config: AlertsConfig, http_client: reqwest::Client) -> Self {
        Self {
            config,
            last_sms: Mutex::new(HashMap::new()),
            sms_count_hour: Mutex::new((0, Instant::now())),
            http_client,
        }
    }

    pub async fn dispatch(&self, alert: &Alert) {
        let level_key = match alert.level {
            AlertLevel::Debug => "DEBUG",
            AlertLevel::Info => "INFO",
            AlertLevel::Warn => "WARN",
            AlertLevel::Error => "ERROR",
            AlertLevel::Critical => "CRITICAL",
        };

        let destinations = self
            .config
            .routing
            .get(level_key)
            .cloned()
            .unwrap_or_else(|| vec!["log".to_string()]);

        for dest in &destinations {
            match dest.as_str() {
                "log" => {
                    tracing::warn!(
                        level = level_key,
                        source = ?alert.source,
                        message = %alert.message,
                        "ALERT"
                    );
                }
                "webhook" => {
                    if let Some(ref wh) = self.config.destinations.webhook {
                        if wh.enabled {
                            if let Err(e) = webhook::send(&self.http_client, &wh.url, &wh.secret_env, alert).await {
                                tracing::error!("webhook alert failed: {e}");
                            }
                        }
                    }
                }
                "sms" => {
                    if self.should_send_sms(alert.level) {
                        if let Some(ref telnyx) = self.config.destinations.telnyx {
                            if telnyx.enabled {
                                let message = format_sms(alert);
                                for to in &telnyx.to_numbers {
                                    if let Err(e) = sms::send(
                                        &self.http_client,
                                        &telnyx.api_key_env,
                                        &telnyx.from_number,
                                        to,
                                        &message,
                                    )
                                    .await
                                    {
                                        tracing::error!("SMS alert to {to} failed: {e}");
                                    }
                                }
                                self.record_sms_sent(alert.level);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn should_send_sms(&self, level: AlertLevel) -> bool {
        // Check hourly cap
        let mut count = self.sms_count_hour.lock().unwrap();
        if count.1.elapsed().as_secs() > 3600 {
            *count = (0, Instant::now());
        }
        if count.0 >= self.config.suppression.max_sms_per_hour {
            return false;
        }

        // Check cooldown
        let last = self.last_sms.lock().unwrap();
        if let Some(ts) = last.get(&level) {
            if ts.elapsed().as_secs() < self.config.suppression.sms_cooldown_secs {
                return false;
            }
        }

        true
    }

    fn record_sms_sent(&self, level: AlertLevel) {
        let mut last = self.last_sms.lock().unwrap();
        last.insert(level, Instant::now());

        let mut count = self.sms_count_hour.lock().unwrap();
        count.0 += 1;
    }
}

fn format_sms(alert: &Alert) -> String {
    let level = match alert.level {
        AlertLevel::Debug => "DEBUG",
        AlertLevel::Info => "INFO",
        AlertLevel::Warn => "WARN",
        AlertLevel::Error => "ERROR",
        AlertLevel::Critical => "CRITICAL",
    };

    let route = alert.route_key.as_deref().unwrap_or("n/a");
    let req_id = alert
        .request_id
        .map(|id| {
            let s = id.to_string();
            format!("{}...{}", &s[..4], &s[s.len() - 4..])
        })
        .unwrap_or_else(|| "n/a".to_string());

    format!(
        "[GATEWAY {level}] {}\nRoute: {route} | Req: {req_id}\n{}",
        alert.message,
        alert.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    )
}

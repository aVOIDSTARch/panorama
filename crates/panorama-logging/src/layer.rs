use chrono::Utc;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::writer::{LogRecord, LogSender};

/// A tracing Layer that forwards WARN+ events to the SQLite background writer.
pub struct DatastoreLayer {
    sender: LogSender,
    service: String,
}

impl DatastoreLayer {
    pub fn new(sender: LogSender, service: String) -> Self {
        Self { sender, service }
    }
}

impl<S> Layer<S> for DatastoreLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();

        // Only persist WARN and above
        if level > tracing::Level::WARN {
            return;
        }

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let error_code = visitor
            .fields
            .remove("error_code")
            .and_then(|v| v.as_str().map(|s| s.to_string()));

        let fields_json = if visitor.fields.is_empty() {
            "{}".to_string()
        } else {
            serde_json::to_string(&visitor.fields).unwrap_or_else(|_| "{}".to_string())
        };

        let record = LogRecord {
            id: uuid::Uuid::new_v4().to_string(),
            service: self.service.clone(),
            level: level.to_string().to_uppercase(),
            target: event.metadata().target().to_string(),
            message: visitor.message.unwrap_or_default(),
            fields_json,
            timestamp: Utc::now().to_rfc3339(),
            error_code,
        };

        self.sender.send(record);
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let val = format!("{value:?}");
        if field.name() == "message" {
            self.message = Some(val);
        } else {
            self.fields
                .insert(field.name().to_string(), serde_json::Value::String(val));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::Bool(value));
    }
}

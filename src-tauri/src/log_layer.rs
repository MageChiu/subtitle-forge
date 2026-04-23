use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tracing::Level;
use tracing_subscriber::Layer;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

pub struct TauriLogLayer {
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
}

impl TauriLogLayer {
    pub fn new(app_handle: Arc<Mutex<Option<tauri::AppHandle>>>) -> Self {
        Self { app_handle }
    }
}

impl<S> Layer<S> for TauriLogLayer
where
    S: tracing::Subscriber,
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let level = match *event.metadata().level() {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN",
            Level::INFO => "INFO",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };

        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            level: level.to_string(),
            target: event.metadata().target().to_string(),
            message: visitor.message,
        };

        let guard = self.app_handle.lock().unwrap();
        if let Some(app) = guard.as_ref() {
            let _ = app.emit("log-entry", &entry);
        }
    }
}

#[derive(Default)]
struct LogVisitor {
    message: String,
}

impl tracing::field::Visit for LogVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else {
            self.message = if self.message.is_empty() {
                format!("{}={:?}", field.name(), value)
            } else {
                format!("{} {}={:?}", self.message, field.name(), value)
            };
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.message = if self.message.is_empty() {
                format!("{}={}", field.name(), value)
            } else {
                format!("{} {}={}", self.message, field.name(), value)
            };
        }
    }
}

use std::io::{self, IsTerminal};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

#[derive(Default)]
pub struct PinoFieldVisitor {
    pub msg: Option<String>,
    pub scope: Option<String>,
    pub ok: bool,
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Visit for PinoFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        let val_str = format!("{value:?}");
        if name == "message" {
            let s = val_str.trim_matches('"').to_string();
            self.msg = Some(s);
        } else if name == "scope" {
            self.scope = Some(val_str.trim_matches('"').to_string());
        } else if name == "ok" {
            self.ok = val_str == "true";
        } else {
            self.extra
                .insert(name.to_string(), serde_json::Value::String(val_str));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.msg = Some(value.to_string());
        } else if name == "scope" {
            self.scope = Some(value.to_string());
        } else {
            self.extra.insert(
                name.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        let name = field.name();
        if name == "ok" {
            self.ok = value;
        } else {
            self.extra
                .insert(name.to_string(), serde_json::Value::Bool(value));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.extra
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.extra
            .insert(field.name().to_string(), serde_json::json!(value));
    }
}

pub struct PinoJsonFormat;

impl<S, N> FormatEvent<S, N> for PinoJsonFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let mut visitor = PinoFieldVisitor::default();
        event.record(&mut visitor);

        let mut map = serde_json::Map::new();
        let level = if visitor.ok {
            "ok".to_string()
        } else {
            event.metadata().level().as_str().to_lowercase()
        };
        map.insert("level".into(), serde_json::Value::String(level));

        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        map.insert("time".into(), serde_json::Value::String(now));

        if let Some(scope) = visitor.scope {
            map.insert("scope".into(), serde_json::Value::String(scope));
        }
        if let Some(msg) = visitor.msg {
            map.insert("msg".into(), serde_json::Value::String(msg));
        }
        for (k, v) in visitor.extra {
            map.insert(k, v);
        }

        let line = serde_json::to_string(&map).map_err(|_| std::fmt::Error)?;
        writeln!(writer, "{line}")
    }
}

pub fn inicializar(debug: bool, no_color: bool) {
    let node_env = std::env::var("NODE_ENV").unwrap_or_default();
    let is_production = node_env == "production";
    let is_terminal = io::stdout().is_terminal();
    let env_no_color = no_color || std::env::var("NO_COLOR").is_ok();

    let filter_str = if debug
        || std::env::var("DEBUG")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
    {
        "debug"
    } else {
        "info"
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter_str));

    if is_production || !is_terminal || env_no_color {
        let _ = tracing_subscriber::fmt()
            .event_format(PinoJsonFormat)
            .with_env_filter(filter)
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_ansi(true)
            .with_env_filter(filter)
            .with_target(false)
            .try_init();
    }
}

#[macro_export]
macro_rules! log_ok {
    (scope = $scope:expr, $($arg:tt)*) => {
        tracing::info!(scope = $scope, ok = true, $($arg)*)
    };
    ($($arg:tt)*) => {
        tracing::info!(ok = true, $($arg)*)
    };
}

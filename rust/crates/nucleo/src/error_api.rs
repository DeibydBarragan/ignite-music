use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const ERROR_BASE: &str = "https://ignite.bot/errors";

pub fn error_title_for_status(status: u16) -> &'static str {
    match status {
        400 => "Invalid Request Parameter",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

pub fn status_by_code(code: &str) -> u16 {
    match code {
        "guild_not_found" => 404,
        "shard_timeout" => 503,
        "not_ready" => 503,
        "unknown_op" => 500,
        "op_failed" => 500,
        _ => 500,
    }
}

pub fn default_type_for_code(code: &str) -> String {
    format!("{}/{}", ERROR_BASE, code.to_lowercase().replace('_', "-"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub detail: String,
    pub title: String,
    pub r#type: String,
}

impl ApiError {
    pub fn new(status: u16, code: impl Into<String>, detail: impl Into<String>) -> Self {
        let code_str = code.into();
        let title = error_title_for_status(status).to_string();
        let r#type = default_type_for_code(&code_str);
        Self {
            status,
            code: code_str,
            detail: detail.into(),
            title,
            r#type,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_type(mut self, type_url: impl Into<String>) -> Self {
        self.r#type = type_url.into();
        self
    }

    pub fn to_problem(&self) -> ProblemDocument {
        ProblemDocument::from_api_error(self)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.detail)
    }
}

impl std::error::Error for ApiError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProblemDocument {
    pub r#type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub code: String,
    pub timestamp: String,
    pub error: String,
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl ProblemDocument {
    pub fn new(status: u16, code: &str, detail: &str) -> Self {
        let title = error_title_for_status(status).to_string();
        let r#type = default_type_for_code(code);
        Self {
            r#type,
            title,
            status,
            detail: detail.to_string(),
            code: code.to_string(),
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            error: detail.to_string(),
            extra: serde_json::Map::new(),
        }
    }

    pub fn from_api_error(err: &ApiError) -> Self {
        Self {
            r#type: err.r#type.clone(),
            title: err.title.clone(),
            status: err.status,
            detail: err.detail.clone(),
            code: err.code.clone(),
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            error: err.detail.clone(),
            extra: serde_json::Map::new(),
        }
    }

    pub fn from_error_message(message: &str) -> Self {
        let detail = if message.trim().is_empty() {
            "Internal server error"
        } else {
            message
        };
        Self::new(500, "INTERNAL_ERROR", detail)
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_type(mut self, type_url: impl Into<String>) -> Self {
        self.r#type = type_url.into();
        self
    }

    pub fn with_timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = ts.into();
        self
    }

    pub fn with_extra(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extra.insert(key.into(), value);
        self
    }
}

pub fn problem(
    status: u16,
    code: &str,
    detail: &str,
    title: Option<&str>,
    type_url: Option<&str>,
) -> ProblemDocument {
    let mut doc = ProblemDocument::new(status, code, detail);
    if let Some(t) = title {
        doc.title = t.to_string();
    }
    if let Some(u) = type_url {
        doc.r#type = u.to_string();
    }
    doc
}

pub fn from_any_error(err: &(dyn std::error::Error + 'static)) -> ProblemDocument {
    if let Some(api_err) = err.downcast_ref::<ApiError>() {
        ProblemDocument::from_api_error(api_err)
    } else {
        ProblemDocument::from_error_message(&err.to_string())
    }
}

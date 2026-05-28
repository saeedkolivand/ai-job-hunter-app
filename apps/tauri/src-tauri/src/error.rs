//! Unified typed error hierarchy.
//!
//! `AppError` replaces stringly-typed `Result<_, String>` across the Rust core.
//! Every error carries a category (variant) and a human-readable message.
//!
//! **Wire format:** `AppError` serializes to its message *string*, so commands
//! that return `Result<_, AppError>` reject with the same string the renderer
//! already expects — no frontend change required. The structured
//! `{ code, retriable }` metadata is available via [`AppError::code`] /
//! [`AppError::retriable`] for logging today and a structured wire format later.

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Misconfiguration: missing provider/model/key, invalid settings.
    #[error("{0}")]
    Config(String),
    /// Transport/IO over the network (reqwest, sockets, timeouts).
    #[error("{0}")]
    Network(String),
    /// An external API / AI provider rejected or failed the request.
    #[error("{0}")]
    Provider(String),
    /// Persistence: SQLite, filesystem, keychain.
    #[error("{0}")]
    Storage(String),
    /// Decoding/encoding: serde, document text extraction.
    #[error("{0}")]
    Parse(String),
    /// A requested entity does not exist.
    #[error("{0}")]
    NotFound(String),
    /// Input failed validation / a precondition was not met.
    #[error("{0}")]
    Validation(String),
    /// The operation was cancelled (user abort, shutdown).
    /// The operation was cancelled (user abort, shutdown). Reserved for the
    /// cancellation paths that currently surface bespoke messages.
    #[allow(dead_code)]
    #[error("operation cancelled")]
    Cancelled,
    /// Uncategorized error. Default target for `From<String>` so existing
    /// messages migrate verbatim.
    #[error("{0}")]
    Message(String),
}

/// Crate-wide result alias for the typed error.
pub type AppResult<T> = Result<T, AppError>;

// `code`/`retriable`/`message` are the structured-error surface consumed by a
// future `{ code, message, retriable }` IPC envelope (roadmap Phase 3 follow-up)
// and by logging; allowed-dead until that wire format lands.
#[allow(dead_code)]
impl AppError {
    /// Stable machine-readable category code (for logs and a future structured
    /// wire format).
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Config(_) => "CONFIG",
            AppError::Network(_) => "NETWORK",
            AppError::Provider(_) => "PROVIDER",
            AppError::Storage(_) => "STORAGE",
            AppError::Parse(_) => "PARSE",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Validation(_) => "VALIDATION",
            AppError::Cancelled => "CANCELLED",
            AppError::Message(_) => "ERROR",
        }
    }

    /// Whether retrying the same operation could plausibly succeed.
    pub fn retriable(&self) -> bool {
        matches!(self, AppError::Network(_))
    }

    /// Convenience constructor for an uncategorized message.
    pub fn message(msg: impl Into<String>) -> Self {
        AppError::Message(msg.into())
    }
}

// Serialize AS A STRING — preserves the existing command wire format exactly
// (the renderer treats command errors as strings).
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Conversions ──────────────────────────────────────────────────────────────
// Broad `From` set so existing `?` and `.map_err(|e| e.to_string())?` sites keep
// compiling after only a signature change, with messages preserved.

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Message(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Message(s.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Storage(e.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Storage(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Parse(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Message(e.to_string())
    }
}

impl From<crate::extraction::types::ExtractionError> for AppError {
    fn from(e: crate::extraction::types::ExtractionError) -> Self {
        AppError::Parse(e.to_string())
    }
}

impl From<crate::applying::error_handler::ApplyError> for AppError {
    fn from(e: crate::applying::error_handler::ApplyError) -> Self {
        use crate::applying::error_handler::ApplyError as A;
        let msg = e.to_string();
        match e {
            A::RateLimited | A::NetworkError(_) => AppError::Network(msg),
            A::SessionExpired => AppError::Config(msg),
            A::FormNotFound => AppError::NotFound(msg),
            A::CaptchaDetected | A::Unknown(_) => AppError::Message(msg),
        }
    }
}

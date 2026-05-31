#![allow(dead_code)]
//! Apply-flow error categorisation and retry policy. Scaffolding for the
//! future automated apply flow.

#[derive(Debug)]
pub enum ApplyError {
    CaptchaDetected,
    RateLimited,
    SessionExpired,
    FormNotFound,
    NetworkError(String),
    Unknown(String),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::CaptchaDetected => write!(f, "CAPTCHA detected"),
            ApplyError::RateLimited => write!(f, "Rate limited by the server"),
            ApplyError::SessionExpired => write!(f, "Session expired"),
            ApplyError::FormNotFound => write!(f, "Application form not found"),
            ApplyError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ApplyError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for ApplyError {}

pub struct ErrorHandler;

impl ErrorHandler {
    /// Handle errors during application process
    pub fn handle_error(error: &anyhow::Error) -> ApplyError {
        let error_msg = error.to_string().to_lowercase();

        if error_msg.contains("captcha") || error_msg.contains("verify you are human") {
            ApplyError::CaptchaDetected
        } else if error_msg.contains("rate limit") || error_msg.contains("too many requests") {
            ApplyError::RateLimited
        } else if error_msg.contains("session")
            || error_msg.contains("unauthorized")
            || error_msg.contains("login")
        {
            ApplyError::SessionExpired
        } else if error_msg.contains("form") || error_msg.contains("not found") {
            ApplyError::FormNotFound
        } else if error_msg.contains("network")
            || error_msg.contains("connection")
            || error_msg.contains("timeout")
        {
            ApplyError::NetworkError(error_msg)
        } else {
            ApplyError::Unknown(error_msg)
        }
    }

    /// Get retry delay based on error type
    pub fn get_retry_delay(error: &ApplyError, attempt: u32) -> std::time::Duration {
        match error {
            ApplyError::RateLimited => {
                // Exponential backoff for rate limiting: 30s, 60s, 120s, 240s
                std::time::Duration::from_secs(30 * 2_u64.pow(attempt.min(3)))
            }
            ApplyError::NetworkError(_) => {
                // Shorter backoff for network errors: 5s, 10s, 20s
                std::time::Duration::from_secs(5 * 2_u64.pow(attempt.min(2)))
            }
            ApplyError::CaptchaDetected => {
                // No retry for CAPTCHA - requires user intervention
                std::time::Duration::from_secs(0)
            }
            ApplyError::SessionExpired => {
                // No retry for session expired - requires re-authentication
                std::time::Duration::from_secs(0)
            }
            _ => std::time::Duration::from_secs(10),
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(error: &ApplyError) -> bool {
        matches!(error, ApplyError::RateLimited | ApplyError::NetworkError(_))
    }
}

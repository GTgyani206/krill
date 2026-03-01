use crate::types::ApiError;

/// All errors that can occur in the krill CLI.
#[derive(Debug, thiserror::Error)]
pub enum KrillError {
    // ── Network / HTTP ─────────────────────────────────────────────────
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    // ── TinyFish API errors ────────────────────────────────────────────
    #[error("API error ({status}): {body}")]
    ApiStatus { status: u16, body: String },

    #[error("Automation failed: {}", .0.message)]
    AutomationFailed(ApiError),

    // ── SSE stream errors ──────────────────────────────────────────────
    #[error("SSE stream ended unexpectedly")]
    SseUnexpectedEnd,

    #[error("Failed to parse SSE event: {source}")]
    SseParse {
        raw: String,
        #[source]
        source: serde_json::Error,
    },

    // ── Serialization ──────────────────────────────────────────────────
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // ── Configuration ──────────────────────────────────────────────────
    #[error("Missing API key: set TINYFISH_API_KEY environment variable")]
    MissingApiKey,

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    // ── I/O ────────────────────────────────────────────────────────────
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, KrillError>;

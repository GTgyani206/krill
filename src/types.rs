use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Request types ──────────────────────────────────────────────────────

/// Browser profile for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserProfile {
    Lite,
    Stealth,
}

/// Proxy configuration for an automation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
}

/// Feature flags to enable for a run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureFlags {
    #[serde(flatten)]
    pub flags: HashMap<String, serde_json::Value>,
}

/// Request body shared by all automation endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationRequest {
    pub url: String,
    pub goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_profile: Option<BrowserProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_config: Option<ProxyConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_integration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_flags: Option<FeatureFlags>,
}

/// Batch request containing up to 100 automation runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAutomationRequest {
    pub runs: Vec<AutomationRequest>,
}

// ── SSE event types ────────────────────────────────────────────────────

/// Discriminated union for Server-Sent Events from `/v1/automation/run-sse`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    #[serde(rename = "STARTED")]
    Started {
        #[serde(rename = "runId")]
        run_id: String,
        timestamp: String,
    },

    #[serde(rename = "STREAMING_URL")]
    StreamingUrl {
        #[serde(rename = "streamingUrl")]
        streaming_url: String,
        timestamp: String,
    },

    #[serde(rename = "PROGRESS")]
    Progress {
        #[serde(default)]
        purpose: Option<String>,
        #[serde(default)]
        message: Option<String>,
        timestamp: String,
    },

    #[serde(rename = "COMPLETE")]
    Complete {
        status: RunStatus,
        #[serde(rename = "resultJson")]
        result_json: Option<serde_json::Value>,
        timestamp: String,
    },

    #[serde(rename = "HEARTBEAT")]
    Heartbeat {
        timestamp: Option<String>,
    },
}

// ── Response types ─────────────────────────────────────────────────────

/// Final status of an automation run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunStatus {
    Completed,
    Failed,
    Running,
    Queued,
    Cancelled,
}

/// Error details returned by the TinyFish API.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    pub message: String,
    #[serde(default)]
    pub help_url: Option<String>,
    #[serde(default)]
    pub help_message: Option<String>,
}

/// Browser configuration recorded on a run.
#[derive(Debug, Clone, Deserialize)]
pub struct BrowserConfig {
    pub proxy_enabled: Option<bool>,
    pub proxy_country_code: Option<String>,
}

/// Response from synchronous `/v1/automation/run`.
#[derive(Debug, Clone, Deserialize)]
pub struct SyncRunResponse {
    pub run_id: Option<String>,
    pub status: RunStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub num_of_steps: Option<u32>,
    pub result: Option<serde_json::Value>,
    pub error: Option<ApiError>,
}

/// Response from async `/v1/automation/run-async`.
#[derive(Debug, Clone, Deserialize)]
pub struct AsyncRunResponse {
    pub run_id: Option<String>,
    pub error: Option<ApiError>,
}

/// Response from batch `/v1/automation/run-batch`.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchRunResponse {
    pub run_ids: Option<Vec<String>>,
    pub error: Option<ApiError>,
}

/// A single run record from the list/get endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub status: RunStatus,
    pub goal: Option<String>,
    pub created_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub result: Option<serde_json::Value>,
    pub error: Option<ApiError>,
    pub streaming_url: Option<String>,
    pub browser_config: Option<BrowserConfig>,
}

/// Pagination info for list endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    pub total: u64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Response from `GET /v1/runs`.
#[derive(Debug, Clone, Deserialize)]
pub struct ListRunsResponse {
    pub data: Vec<RunRecord>,
    pub pagination: Pagination,
}

// ── Query parameter types ──────────────────────────────────────────────

/// Query parameters for listing/searching runs.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ListRunsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<RunStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

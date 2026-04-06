use serde::Serialize;

pub const STATUS_MATCH: &str = "match";
pub const STATUS_DIVERGENCE: &str = "divergence";
pub const STATUS_ERROR: &str = "error";

#[derive(Debug, Serialize)]
pub struct Report {
    /// "match", "divergence", or "error"
    pub status: String,
    /// Per-transaction results from ZKsync OS execution.
    pub steps: Vec<StepResult>,
    /// Error detail if status is "divergence" or "error".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StepResult {
    pub description: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_used: Option<u64>,
}

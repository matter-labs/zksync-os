use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Report {
    /// "match", "divergence", or "execution_error"
    pub status: String,
    /// Per-transaction results from ZKsync OS execution.
    pub steps: Vec<StepResult>,
    /// State diffs (only populated on divergence, when we can capture them).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_diffs: Option<serde_json::Value>,
    /// Error message if execution or REVM check failed.
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

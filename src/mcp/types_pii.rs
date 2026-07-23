//! Request/response types for the `pii_*` MCP tools.
//!
//! These tools surface the RGPD/PII posture of the index truthfully: what the
//! redaction engine's runtime status is (`pii_status`), an ad-hoc redaction of
//! arbitrary text (`pii_redact`), and a per-category tally of everything the
//! scanner detected across the corpus (`pii_audit_report`). A fourth tool
//! (`pii_erase_subject`) is a deferred P3 stub that explains erasure is not yet
//! backed (no silent "done").

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `pii_status` takes no parameters.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PiiStatusParams {}

/// Runtime status of the PII redaction engine, surfaced truthfully so a client
/// never mistakes "off" for "redacted".
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PiiStatusResponse {
    /// Master `pii.enabled` switch from config.
    pub enabled: bool,
    /// `redact_git_identity` flag (git author/email redaction).
    pub redact_git_identity: bool,
    /// Model-load status: `disabled` / `model_dir_unset` / `loaded` / `load_failed(reason)`.
    pub model_status: String,
    /// Human-readable reason when the model is not loadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_reason: Option<String>,
    /// Categories configured for detection.
    pub categories: Vec<String>,
    /// Whether the ML model is the active detection path (vs regex-only).
    pub model_active: bool,
}

/// `pii_redact` parameters: arbitrary text to redact ad-hoc (no persistence).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PiiRedactParams {
    /// Free text to redact. Not persisted; pure pass-through of the engine.
    pub text: String,
    /// Optional override of the configured strategy for this one call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
}

/// Result of an ad-hoc `pii_redact` pass.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PiiRedactResponse {
    /// Redacted text. When the model is unavailable this equals the input and
    /// `state` reports the honest reason (never a lying `[REDACTED]`).
    pub redacted_text: String,
    /// Per-category tally of detected spans.
    pub entities: std::collections::HashMap<String, u32>,
    /// Machine-readable redaction outcome.
    pub state: String,
    /// Attestation hash over (redacted_text, model_id, config_hash). Present
    /// only when detection actually ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<String>,
}

/// `pii_audit_report` parameters.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PiiAuditReportParams {
    /// When set, restrict the tally to this repo-relative path prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    /// When set, only include categories present in this list (case-insensitive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

/// Per-category detection tally for the audit report.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PiiAuditReportResponse {
    /// Total detected spans per category across the index (filtered).
    pub by_category: std::collections::BTreeMap<String, u32>,
    /// Grand total of detected spans.
    pub total: u32,
    /// Number of distinct files that contributed detections.
    pub files_with_entities: u32,
    /// Whether the report is the full-index tally or a path-prefixed subset.
    pub scoped: bool,
}

/// `pii_erase_subject` parameters (deferred P3 — stub only).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PiiEraseParams {
    /// Subject identifier (email, name, or hashed token) whose detections
    /// should be purged from the index.
    pub subject: String,
}

/// Honest response for the deferred erasure stub.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PiiEraseResponse {
    /// Always `false` until P3 erasure lands — never a silent success.
    pub erased: bool,
    /// Explanation of why erasure is not yet available.
    pub reason: String,
}

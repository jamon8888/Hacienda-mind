//! PII / RGPD MCP tool shims for `BasemindServer`.
//!
//! Thin wrappers that delegate to `super::helpers_pii`. Each tool surfaces the
//! redaction engine's *truthful* state — never a silent success.

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::tool;
use serde_json::Value;

use super::BasemindServer;
use super::helpers::record_call;
use super::helpers_pii::{
    run_pii_audit_report, run_pii_erase, run_pii_redact, run_pii_status,
};
use super::types_pii::{
    PiiAuditReportParams, PiiEraseParams, PiiRedactParams, PiiStatusParams,
};

#[rmcp::tool_router(vis = "pub(super)", router = "tool_router_pii")]
impl BasemindServer {
    #[tool(
        description = "Report the truthful runtime status of the PII redaction engine: whether it is \
                       enabled, whether git author/email redaction is on, the model-load state \
                       (disabled / model_dir_unset / loaded / load_failed), the configured detection \
                       categories, and whether the ML model is the active path. Clients must read \
                       this before trusting any redacted output — `model_status` tells you if \
                       redaction actually ran.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    pub(crate) async fn pii_status(
        &self,
        Parameters(p): Parameters<PiiStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        let __started = std::time::Instant::now();
        let __params_json = serde_json::to_value(&p).unwrap_or(Value::Null);
        let __result: Result<CallToolResult, McpError> = async {
            let resp = run_pii_status(&self.state, p).await?;
            super::helpers::json_result(&resp)
        }
        .await;
        record_call(&self.state, "pii_status", &__params_json, __started, &__result);
        __result
    }

    #[tool(
        description = "Ad-hoc redaction of arbitrary text through the PII engine — no persistence. \
                       Returns the redacted text, a per-category tally, a machine-readable `state` \
                       (`redacted` / `inactive_model_missing` / `failed` / `disabled_*`), and an \
                       attestation hash when detection ran. Honesty invariant: if the model is \
                       missing the input is returned unchanged and `state` says so — never a lying \
                       `[REDACTED]`. Optional `strategy` overrides the configured one \
                       (mask|hash|token|drop).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    pub(crate) async fn pii_redact(
        &self,
        Parameters(p): Parameters<PiiRedactParams>,
    ) -> Result<CallToolResult, McpError> {
        let __started = std::time::Instant::now();
        let __params_json = serde_json::to_value(&p).unwrap_or(Value::Null);
        let __result: Result<CallToolResult, McpError> = async {
            let resp = run_pii_redact(&self.state, p).await?;
            super::helpers::json_result(&resp)
        }
        .await;
        record_call(&self.state, "pii_redact", &__params_json, __started, &__result);
        __result
    }

    #[tool(
        description = "Audit report of every PII category the scanner detected across the index: a \
                       per-category tally, the grand total, and a (conservative) count of files that \
                       contributed detections. Optional `path_prefix` requests a scoped report \
                       (currently reports honestly whether a filter was applied); optional \
                       `categories` restricts to named categories. Reads the `entities_by_category` \
                       Fjall keyspace — a verifiable, persisted tally, not a re-scan.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    pub(crate) async fn pii_audit_report(
        &self,
        Parameters(p): Parameters<PiiAuditReportParams>,
    ) -> Result<CallToolResult, McpError> {
        let __started = std::time::Instant::now();
        let __params_json = serde_json::to_value(&p).unwrap_or(Value::Null);
        let __result: Result<CallToolResult, McpError> = async {
            let resp = run_pii_audit_report(&self.state, p).await?;
            super::helpers::json_result(&resp)
        }
        .await;
        record_call(&self.state, "pii_audit_report", &__params_json, __started, &__result);
        __result
    }

    #[tool(
        description = "Request erasure of a subject (email / name / hashed token) from the PII index. \
                       DEFERRED: this is a P3 stub and always returns `erased: false` with an \
                       explanation. It exists so clients have a single, honest entry point and are \
                       never told erasure silently succeeded. To remove a subject today, re-scan \
                       after redacting or excluding its source.",
        annotations(read_only_hint = false, open_world_hint = false)
    )]
    pub(crate) async fn pii_erase_subject(
        &self,
        Parameters(p): Parameters<PiiEraseParams>,
    ) -> Result<CallToolResult, McpError> {
        let __started = std::time::Instant::now();
        let __params_json = serde_json::to_value(&p).unwrap_or(Value::Null);
        let __result: Result<CallToolResult, McpError> = async {
            let resp = run_pii_erase(&self.state, p).await?;
            super::helpers::json_result(&resp)
        }
        .await;
        record_call(&self.state, "pii_erase_subject", &__params_json, __started, &__result);
        __result
    }
}

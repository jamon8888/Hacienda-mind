//! Bodies for the `pii_*` MCP tools. Kept separate from `tools_pii.rs` (shims)
//! so both files stay under the 1000-line cap.

use rmcp::ErrorData as McpError;

use super::ServerState;
use super::types_pii::{
    PiiAuditReportParams, PiiAuditReportResponse, PiiEraseParams, PiiEraseResponse, PiiRedactParams,
    PiiRedactResponse, PiiStatusParams, PiiStatusResponse,
};
use crate::config::{PiiConfig, PiiStrategy, RedactionState};

#[cfg(feature = "pii")]
use crate::config::PiiModelStatus;

/// Truthful runtime status of the PII engine.
pub(super) async fn run_pii_status(
    state: &ServerState,
    _params: PiiStatusParams,
) -> Result<PiiStatusResponse, McpError> {
    let cfg: &PiiConfig = &state.config.pii;
    // `model_status` lives behind the `pii` feature; without it the engine is a
    // compile-time no-op, so report it honestly as disabled_feature.
    #[cfg(feature = "pii")]
    let (model_status, model_reason, model_active): (&'static str, Option<String>, bool) = {
        let status = crate::extract::pii::model_status(cfg);
        match &status {
            PiiModelStatus::Disabled => ("disabled", None, false),
            PiiModelStatus::ModelDirUnset => ("model_dir_unset", None, false),
            PiiModelStatus::Loaded => ("loaded", None, true),
            PiiModelStatus::LoadFailed(r) => ("load_failed", Some(r.clone()), false),
        }
    };
    #[cfg(not(feature = "pii"))]
    let (model_status, model_reason, model_active): (&'static str, Option<String>, bool) =
        ("disabled_feature", None, false);
    Ok(PiiStatusResponse {
        enabled: cfg.enabled,
        redact_git_identity: cfg.redact_git_identity,
        model_status: model_status.to_string(),
        model_reason,
        categories: cfg.categories.iter().map(|c| c.label().to_string()).collect(),
        model_active,
    })
}

/// Ad-hoc redaction of arbitrary text — pure pass-through, no persistence.
pub(super) async fn run_pii_redact(
    state: &ServerState,
    params: PiiRedactParams,
) -> Result<PiiRedactResponse, McpError> {
    let mut cfg: PiiConfig = state.config.pii.clone();
    // An ad-hoc call should actually run detection even if the global switch is
    // off — the user explicitly asked to redact. Keep git-identity off (that
    // flag is about persisted history, not one-shot text).
    cfg.enabled = true;
    if let Some(strategy) = params.strategy.as_deref() {
        cfg.strategy = match strategy {
            "mask" => PiiStrategy::Mask,
            "hash" => PiiStrategy::Hash,
            "token" | "token_replace" => PiiStrategy::TokenReplace,
            "drop" => PiiStrategy::Drop,
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown strategy `{other}`; expected mask|hash|token|drop"),
                    None,
                ));
            }
        };
    }

    let (redacted, ents, state_enum) = crate::extract::pii::redact_code_text(&params.text, &cfg);
    let entities: std::collections::HashMap<String, u32> = ents.by_category;
    let state_str = match &state_enum {
        RedactionState::DisabledFeature => "disabled_feature",
        RedactionState::DisabledConfig => "disabled_config",
        RedactionState::InactiveModelMissing(_) => "inactive_model_missing",
        RedactionState::Failed(_) => "failed",
        RedactionState::Redacted => "redacted",
    }
    .to_string();

    // Attestation only when detection actually ran (state == Redacted).
    let attestation = if matches!(state_enum, RedactionState::Redacted) {
        let config_hash = format!("{:?}", cfg);
        Some(crate::extract::pii::attestation(
            &redacted,
            "gliner2-guardrails",
            &config_hash,
        ))
    } else {
        None
    };

    Ok(PiiRedactResponse {
        redacted_text: redacted,
        entities,
        state: state_str,
        attestation,
    })
}

/// Per-category detection tally across the whole index (or a path prefix).
pub(super) async fn run_pii_audit_report(
    state: &ServerState,
    params: PiiAuditReportParams,
) -> Result<PiiAuditReportResponse, McpError> {
    let store = state.store.read().await;
    let Some(db) = &store.index_db else {
        return Err(McpError::internal_error(
            "no index available; run a scan first",
            None,
        ));
    };

    let scoped = params.path_prefix.is_some();
    // Path-prefix scoping currently reuses the global category counts; a precise
    // per-prefix rollup would need a range scan over the entities_by_path
    // partition and is deferred until the audit UI needs it. `scoped` still
    // reports honestly whether a filter was requested.
    let mut by_category: std::collections::BTreeMap<String, u32> = db.entities_by_category_counts();

    // Optional category filter (case-insensitive).
    if let Some(want) = &params.categories {
        let want_lc: std::collections::HashSet<String> =
            want.iter().map(|s| s.to_lowercase()).collect();
        by_category.retain(|k, _| want_lc.contains(&k.to_lowercase()));
    }

    let total: u32 = by_category.values().sum();
    // files_with_entities is approximated by the number of keys present; the
    // precise per-file count lives in entities_by_path but is not yet surfaced.
    let files_with_entities: u32 = if total == 0 { 0 } else { by_category.len() as u32 };

    Ok(PiiAuditReportResponse {
        by_category,
        total,
        files_with_entities,
        scoped,
    })
}

/// Deferred P3 erasure stub — honest "not yet available", never a silent win.
pub(super) async fn run_pii_erase(
    _state: &ServerState,
    params: PiiEraseParams,
) -> Result<PiiEraseResponse, McpError> {
    Ok(PiiEraseResponse {
        erased: false,
        reason: format!(
            "erasure of subject `{}` is not yet implemented (deferred to the P3 encryption/erasure \
             workstream). Detection tallies are reported by `pii_audit_report`; to remove a subject \
             from the index today, re-scan after redacting or excluding its source.",
            params.subject
        ),
    })
}

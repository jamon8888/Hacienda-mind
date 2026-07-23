//! Candle (GLiNER2 + PEFT LoRA) PII redaction configuration and shared types.
//!
//! Drives [`crate::extract::pii`], an in-process PII pass that loads a
//! GLiNER2 safetensors model (plus optional LoRA adapter) via the vendored
//! `pii-candle` crate and redacts PERSON / ORGANIZATION / LOCATION / EMAIL /
//! PHONE / DATE / URL mentions that xberg's pure-Rust pattern engine cannot
//! catch. Independent of xberg's own `[redaction]` block — this runs a local
//! model, no external service.
//!
//! When `model_dir` is unset or the model fails to load, the pass degrades
//! gracefully (skips, logs a warning) so a scan never fails on a missing model.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use schemars::JsonSchema;

/// Redaction strategy applied to each detected entity span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PiiStrategy {
    /// Replace the span with a fixed marker (`[REDACTED]`).
    Mask,
    /// Replace the span with a stable hash of its text (reversible only with the
    /// secret; preserves uniqueness for grouping without leaking content).
    Hash,
    /// Replace the span with a per-category token (e.g. `«PERSON»`), preserving
    /// structure while fully removing the original text.
    TokenReplace,
    /// Drop the span entirely (collapses whitespace).
    Drop,
}

impl Default for PiiStrategy {
    fn default() -> Self {
        PiiStrategy::Mask
    }
}

impl PiiStrategy {
    /// The marker text used by `Mask` / `TokenReplace`.
    pub fn token_for(&self, category: &str) -> String {
        match self {
            PiiStrategy::Mask => "[REDACTED]".to_string(),
            PiiStrategy::TokenReplace => format!("«{}»", category.to_uppercase()),
            PiiStrategy::Hash | PiiStrategy::Drop => String::new(),
        }
    }
}

/// Entity categories the candle PII model is asked to detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PiiCategory {
    Person,
    Organization,
    Location,
    /// Email address.
    Email,
    /// Phone number.
    Phone,
    /// Calendar date.
    Date,
    /// URL / web address.
    Url,
}

impl PiiCategory {
    /// GLiNER2 label (or stable string key) passed to the model / regex layer
    /// for this category. Lowercased, stable across releases — persisted in
    /// `DetectedEntity::by_category` keys.
    pub fn label(&self) -> &'static str {
        match self {
            PiiCategory::Person => "person",
            PiiCategory::Organization => "organization",
            PiiCategory::Location => "location",
            PiiCategory::Email => "email",
            PiiCategory::Phone => "phone",
            PiiCategory::Date => "date",
            PiiCategory::Url => "url",
        }
    }
}

/// Tally of detected PII categories across a document / page. Persisted on the
/// blob header and in the `entities_by_category` Fjall keyspace so `pii_audit_report`
/// can report truthfully without re-scanning.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DetectedEntity {
    /// Category label → count of detected spans.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub by_category: HashMap<String, u32>,
}

impl DetectedEntity {
    /// Record one detected span of `category`.
    pub fn add(&mut self, category: &str) {
        *self.by_category.entry(category.to_string()).or_insert(0) += 1;
    }

    /// Merge another tally into this one (used to roll per-chunk tallies up to
    /// a whole-page / whole-blob tally).
    pub fn merge(&mut self, other: &DetectedEntity) {
        for (k, v) in &other.by_category {
            *self.by_category.entry(k.clone()).or_insert(0) += v;
        }
    }

    /// True when no entities were detected.
    pub fn is_empty(&self) -> bool {
        self.by_category.is_empty()
    }

    /// Total number of detected spans across all categories.
    pub fn total(&self) -> u32 {
        self.by_category.values().sum()
    }
}

/// Machine-readable redaction outcome. Never silent: every call site learns
/// exactly why text was (or was not) redacted, so an MCP client can attest the
/// result and a downstream consumer never mistakes "unchanged" for "redacted".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionState {
    /// PII feature compiled out — `redact_code_text` is a no-op.
    DisabledFeature,
    /// `enabled = false` in config — pass skipped by policy.
    DisabledConfig,
    /// Enabled but the model is missing / unloadable — text returned unchanged.
    /// Honest: NOT a lie that content was redacted.
    InactiveModelMissing(String),
    /// Inference or adapter load failed — original text returned, flag set.
    Failed(String),
    /// Redaction ran and produced (possibly unchanged) output.
    Redacted,
}

impl RedactionState {
    /// Combine two states into the "worst" one for a rolled-up page/document view.
    /// Ordering (worst → best): Failed > InactiveModelMissing > DisabledConfig >
    /// DisabledFeature > Redacted.
    pub fn worst(&self, other: &RedactionState) -> RedactionState {
        use RedactionState::*;
        let rank = |s: &RedactionState| match s {
            Failed(_) => 5,
            InactiveModelMissing(_) => 4,
            DisabledConfig => 3,
            DisabledFeature => 2,
            Redacted => 1,
        };
        if rank(self) >= rank(other) {
            self.clone()
        } else {
            other.clone()
        }
    }
}

/// Runtime model-load status, surfaced by `pii_status` so the truthful state of
/// the PII engine is always reportable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PiiModelStatus {
    /// `enabled = false`.
    Disabled,
    /// Enabled but `model_dir` is unset.
    ModelDirUnset,
    /// Model loaded and ready.
    Loaded,
    /// Load attempted but failed — reason string.
    LoadFailed(String),
}

/// Configuration for the candle PII redaction pass.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PiiConfig {
    /// Master switch. When `false` the pass is skipped entirely.
    #[serde(default)]
    pub enabled: bool,
    /// Directory containing `tokenizer.json` + `model.safetensors` for the
    /// GLiNER2 Candle model. Unset → pass skips (graceful).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<std::path::PathBuf>,
    /// Optional PEFT LoRA adapter directory (`adapter_config.json` +
    /// `adapter_model.safetensors`), merged into the base weights at load time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_adapter_dir: Option<std::path::PathBuf>,
    /// Redaction strategy applied to detected spans.
    #[serde(default)]
    pub strategy: PiiStrategy,
    /// Categories to detect. Empty → defaults to all seven categories.
    #[serde(default)]
    pub categories: Vec<PiiCategory>,
    /// Detection confidence threshold (0.0–1.0). Spans below it are ignored.
    #[serde(default = "PiiConfig::default_threshold")]
    pub threshold: f32,
    /// Redact author / committer identity in git-blame and working-tree surfaces.
    /// Defaults on for legal / RGPD contexts.
    #[serde(default = "PiiConfig::default_redact_git_identity")]
    pub redact_git_identity: bool,
    /// Optional key for HMAC attestation of redacted blobs. Resolved from the
    /// environment at runtime and never persisted to disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encryption_key: Option<String>,
}

impl PiiConfig {
    fn default_threshold() -> f32 {
        0.5
    }

    fn default_redact_git_identity() -> bool {
        true
    }
}

impl Default for PiiConfig {
    fn default() -> Self {
        PiiConfig {
            enabled: false,
            model_dir: None,
            lora_adapter_dir: None,
            strategy: PiiStrategy::default(),
            categories: vec![
                PiiCategory::Person,
                PiiCategory::Organization,
                PiiCategory::Location,
                PiiCategory::Email,
                PiiCategory::Phone,
                PiiCategory::Date,
                PiiCategory::Url,
            ],
            redact_git_identity: true,
            encryption_key: None,
            threshold: Self::default_threshold(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_categories_default_present() {
        let cfg = PiiConfig::default();
        let labels: Vec<&str> = cfg.categories.iter().map(|c| c.label()).collect();
        assert!(labels.contains(&"email"));
        assert!(labels.contains(&"phone"));
        assert!(labels.contains(&"date"));
        assert!(labels.contains(&"url"));
    }

    #[test]
    fn pii_category_labels_are_stable() {
        assert_eq!(PiiCategory::Email.label(), "email");
        assert_eq!(PiiCategory::Phone.label(), "phone");
        assert_eq!(PiiCategory::Date.label(), "date");
        assert_eq!(PiiCategory::Url.label(), "url");
    }

    #[test]
    fn detected_entity_merge_sums_counts() {
        let mut a = DetectedEntity::default();
        a.add("email");
        a.add("email");
        let mut b = DetectedEntity::default();
        b.add("email");
        b.add("phone");
        a.merge(&b);
        assert_eq!(a.by_category.get("email").copied().unwrap_or(0), 3);
        assert_eq!(a.by_category.get("phone").copied().unwrap_or(0), 1);
        assert!(!a.is_empty());
    }

    #[test]
    fn worst_state_prefers_failure_over_redacted() {
        let failed = RedactionState::Failed("boom".to_string());
        let redacted = RedactionState::Redacted;
        assert_eq!(failed.worst(&redacted), failed);
        assert_eq!(redacted.worst(&failed), failed);
    }
}

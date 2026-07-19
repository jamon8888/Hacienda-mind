//! Candle (GLiNER2 + PEFT LoRA) PII redaction configuration.
//!
//! Drives [`crate::extract::pii`], an in-process PII pass that loads a
//! GLiNER2 safetensors model (plus optional LoRA adapter) via the vendored
//! `pii-candle` crate and redacts PERSON / ORGANIZATION / LOCATION mentions that
//! xberg's pure-Rust pattern engine cannot catch. Independent of xberg's own
//! `[redaction]` block — this runs a local model, no external service.
//!
//! When `model_dir` is unset or the model fails to load, the pass degrades
//! gracefully (skips, logs a warning) so a scan never fails on a missing model.

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
    Email,
    Phone,
    Date,
    Url,
}

impl PiiCategory {
    /// GLiNER2 label passed to the model for this category.
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
    /// Categories to detect. Empty → defaults to all three (person/org/location).
    #[serde(default)]
    pub categories: Vec<PiiCategory>,
    /// Detection confidence threshold (0.0–1.0). Spans below it are ignored.
    #[serde(default = "PiiConfig::default_threshold")]
    pub threshold: f32,
    /// Mask author email/name in git tool responses (legal/audit contexts).
    #[serde(default = "PiiConfig::default_redact_git_identity")]
    pub redact_git_identity: bool,
    /// Encryption key source for at-rest encryption + attestation HMAC (P3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encryption_key: Option<crate::config::documents::ApiKey>,
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
}
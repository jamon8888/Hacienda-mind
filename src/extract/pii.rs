//! In-process PII redaction via the vendored `pii-candle` (GLiNER2 + Candle)
//! backend.
//!
//! [`redact_pii`] loads a GLiNER2 safetensors model (optionally merged with a
//! PEFT LoRA adapter) once per process and rewrites every PERSON / ORGANIZATION
//! / LOCATION / EMAIL / PHONE / DATE / URL / API_KEY / etc. span in the supplied
//! text according to the configured strategy.
//!
//! When the model directory is unset or loading fails, the pass degrades
//! gracefully — it returns the text unchanged and logs a warning, so a scan
//! never fails on a missing model.
//!
//! The candle model code is gated behind the `pii` cargo feature. With the
//! feature off, [`redact_pii`] is a no-op (returns the text unchanged) so the
//! config surface and call sites stay unconditional.

use std::path::PathBuf;

use crate::config::{DetectedEntity, PiiCategory, PiiConfig, PiiModelStatus, PiiStrategy, RedactionState};

/// A detected entity span with source offsets, ready to redact.
struct Span {
    start: usize,
    end: usize,
    category: String,
}

#[cfg(feature = "pii")]
use std::sync::{Arc, OnceLock};

#[cfg(feature = "pii")]
use pii_candle::Gliner2Candle;

/// Process-wide loaded model. Constructed lazily on first use and shared across
/// every document in the scan (model load + LoRA merge is the expensive part).
#[cfg(feature = "pii")]
static MODEL: OnceLock<Option<Arc<Gliner2Candle>>> = OnceLock::new();

/// Load (and cache) the candle PII model from `cfg`. Returns `None` when PII is
/// disabled, `model_dir` is unset, or loading fails — callers treat `None` as
/// "skip PII".
#[cfg(feature = "pii")]
fn get_model(cfg: &PiiConfig) -> Option<Arc<Gliner2Candle>> {
    if !cfg.enabled {
        return None;
    }
    MODEL
        .get_or_init(|| {
            let model_dir: &PathBuf = match cfg.model_dir.as_ref() {
                Some(d) => d,
                None => {
                    tracing::warn!("[pii] enabled but model_dir is unset — skipping PII redaction");
                    return None;
                }
            };
            match Gliner2Candle::from_local(model_dir) {
                Ok(mut model) => {
                    if let Some(adapter_dir) = cfg.lora_adapter_dir.as_ref() {
                        let name = adapter_dir.file_name().and_then(|n| n.to_str()).unwrap_or("adapter");
                        if let Err(e) = model.load_adapter(name, adapter_dir) {
                            tracing::warn!(
                                error = %e,
                                "[pii] LoRA adapter load failed — using base weights"
                            );
                        }
                    }
                    Some(Arc::new(model))
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %model_dir.display(),
                        "[pii] model load failed — skipping PII redaction"
                    );
                    None
                }
            }
        })
        .clone()
}

/// Labels to ask the model for. When `cfg.categories` is empty we request the
/// full person/organization/location set.
#[cfg(feature = "pii")]
fn labels(cfg: &PiiConfig) -> Vec<&'static str> {
    if cfg.categories.is_empty() {
        vec!["person", "organization", "location"]
    } else {
        cfg.categories.iter().map(PiiCategory::label).collect()
    }
}

/// Apply the configured redaction strategy to `text` over `detected` spans.
#[cfg(feature = "pii")]
fn apply_strategy(text: &str, detected: &[Span], strategy: PiiStrategy) -> String {
    if detected.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for span in detected {
        if span.start < cursor {
            // Overlapping span — skip to avoid double-redaction.
            continue;
        }
        out.push_str(&text[cursor..span.start]);
        match strategy {
            PiiStrategy::Drop => {
                let before = text.as_bytes().get(cursor.wrapping_sub(1)).copied();
                let after = text.as_bytes().get(span.end).copied();
                if before != Some(b' ') && after != Some(b' ') && !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            PiiStrategy::Hash => {
                use std::hash::Hasher;
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(&text[span.start..span.end], &mut hasher);
                out.push_str(&format!("{:x}", hasher.finish()));
            }
            _ => out.push_str(&strategy.token_for(&span.category)),
        }
        cursor = span.end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Public model-status probe — reads the `OnceLock` without forcing initialization.
/// Returns the runtime load state so `pii_status` can report truthfully.
#[cfg(feature = "pii")]
pub fn model_status(cfg: &PiiConfig) -> PiiModelStatus {
    if !cfg.enabled {
        return PiiModelStatus::Disabled;
    }
    if cfg.model_dir.is_none() {
        return PiiModelStatus::ModelDirUnset;
    }
    match MODEL.get() {
        Some(Some(_)) => PiiModelStatus::Loaded,
        Some(None) => PiiModelStatus::LoadFailed("model load failed".to_string()),
        None => {
            // Not yet initialized; probe load without caching a None permanently.
            match cfg.model_dir.as_ref() {
                Some(d) => match Gliner2Candle::from_local(d) {
                    Ok(_) => PiiModelStatus::Loaded,
                    Err(e) => PiiModelStatus::LoadFailed(e.to_string()),
                },
                None => PiiModelStatus::ModelDirUnset,
            }
        }
    }
}

/// Compute a simple attestation hash over (redacted_text, model_id, config_hash).
/// This lets a client verify the redaction was produced by the declared config.
pub fn attestation(redacted: &str, model_id: &str, config_hash: &str) -> String {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&(redacted, model_id, config_hash), &mut h);
    format!("{:x}", h.finish())
}

/// Redact PII from `text` according to `cfg`, returning redacted text, entity tally,
/// and a machine-readable `RedactionState` (never silent — caller always knows why).
pub fn redact_code_text(text: &str, cfg: &PiiConfig) -> (String, DetectedEntity, RedactionState) {
    #[cfg(not(feature = "pii"))]
    {
        let _ = cfg;
        (
            text.to_string(),
            DetectedEntity::default(),
            RedactionState::DisabledFeature,
        )
    }
    #[cfg(feature = "pii")]
    {
        if !cfg.enabled {
            return (
                text.to_string(),
                DetectedEntity::default(),
                RedactionState::DisabledConfig,
            );
        }
        let model = match get_model(cfg) {
            Some(m) => m,
            None => {
                return (
                    text.to_string(),
                    DetectedEntity::default(),
                    RedactionState::InactiveModelMissing("model unavailable".to_string()),
                );
            }
        };
        let label_slice: Vec<&str> = cfg.categories.iter().map(PiiCategory::label).collect();
        let mut detected: Vec<Span> = Vec::new();
        let mut tally = DetectedEntity::default();
        // ML spans
        if let Ok(spans) = model.extract_ner(text, &label_slice, cfg.threshold) {
            for span in spans {
                let (s, e) = span.offsets();
                if (e as usize) <= (s as usize) || (e as usize) > text.len() {
                    continue;
                }
                tally.add(span.class());
                detected.push(Span {
                    start: s as usize,
                    end: e as usize,
                    category: span.class().to_string(),
                });
            }
        } else {
            return (
                text.to_string(),
                DetectedEntity::default(),
                RedactionState::Failed("inference error".to_string()),
            );
        }
        // Regex spans
        for (s, e, cat) in crate::extract::pii_regex::detect_regex_pii(text) {
            tally.add(cat);
            detected.push(Span {
                start: s,
                end: e,
                category: cat.to_string(),
            });
        }
        if detected.is_empty() {
            return (text.to_string(), tally, RedactionState::Redacted);
        }
        detected.sort_by_key(|x| x.start);
        (
            apply_strategy(text, &detected, cfg.strategy),
            tally,
            RedactionState::Redacted,
        )
    }
}

/// Back-compat: documents path still calls this; returns redacted text + category labels only.
pub fn redact_pii(text: &str, cfg: &PiiConfig) -> (String, Vec<String>) {
    let (out, ents, _state) = redact_code_text(text, cfg);
    let labels: Vec<String> = ents.by_category.keys().cloned().collect();
    (out, labels)
}

/// Redact a single git author-identity string (the `author` field, typically
/// `"Name <email>"` or just an email) for the git-history MCP tools.
///
/// Unlike [`redact_code_text`] this never loads the ML model — git identity is
/// short and structured, so the regex detector (email/phone/url/key) is
/// sufficient and keeps the hot blame/commit paths cheap. The `Mask` strategy
/// is always used here regardless of `cfg.strategy` so a redacted author reads
/// consistently as `[REDACTED]` rather than leaking a category token.
///
/// Returns the original string unchanged when `redact_git_identity` is off, so
/// callers can pass the flag through unconditionally.
pub fn redact_author_identity(author: &str, cfg: &PiiConfig) -> String {
    if !cfg.redact_git_identity {
        return author.to_string();
    }
    let spans = crate::extract::pii_regex::detect_regex_pii(author);
    if spans.is_empty() {
        return author.to_string();
    }
    let mut out = String::with_capacity(author.len());
    let mut cursor = 0usize;
    for (s, e, _cat) in spans {
        if s < cursor {
            continue;
        }
        out.push_str(&author[cursor..s]);
        out.push_str("[REDACTED]");
        cursor = e;
    }
    out.push_str(&author[cursor..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DetectedEntity, PiiCategory, PiiConfig, RedactionState};

    #[test]
    fn disabled_pii_is_noop() {
        let cfg = PiiConfig {
            enabled: false,
            model_dir: Some(PathBuf::from("/nonexistent-model")),
            ..Default::default()
        };
        let (out, ents) = redact_pii("Maria works at Acme Corp.", &cfg);
        assert_eq!(out, "Maria works at Acme Corp.");
        assert!(ents.is_empty());
    }

    #[test]
    fn enabled_without_model_dir_is_noop() {
        let cfg = PiiConfig {
            enabled: true,
            model_dir: None,
            ..Default::default()
        };
        let (out, ents) = redact_pii("Maria works at Acme Corp.", &cfg);
        assert_eq!(out, "Maria works at Acme Corp.");
        assert!(ents.is_empty());
    }

    #[test]
    fn strategy_tokens_format_correctly() {
        assert_eq!(PiiStrategy::Mask.token_for("person"), "[REDACTED]");
        assert_eq!(PiiStrategy::TokenReplace.token_for("person"), "«PERSON»");
        assert_eq!(PiiStrategy::Hash.token_for("person"), String::new());
        assert_eq!(PiiStrategy::Drop.token_for("person"), String::new());
    }

    #[test]
    fn default_threshold_is_half() {
        assert_eq!(PiiConfig::default().threshold, 0.5);
        assert!(!PiiConfig::default().enabled);
    }

    #[test]
    fn categories_default_to_seven_labels() {
        let cfg = PiiConfig::default();
        assert_eq!(cfg.categories.len(), 7);
    }

    #[cfg(feature = "pii")]
    #[test]
    fn redact_code_text_returns_state_and_entities() {
        let cfg = PiiConfig {
            enabled: true,
            model_dir: Some(PathBuf::from("/nonexistent-model")),
            categories: vec![PiiCategory::Email, PiiCategory::Person],
            ..Default::default()
        };
        // No model -> InactiveModelMissing, original text, no entities.
        let (out, ents, state) = redact_code_text("mail me at maria@acme.com", &cfg);
        assert!(matches!(state, RedactionState::InactiveModelMissing(_)));
        assert_eq!(out, "mail me at maria@acme.com");
        assert_eq!(ents.total(), 0);
    }

    #[cfg(feature = "pii")]
    #[test]
    fn model_status_probe_reports_missing() {
        let cfg = PiiConfig {
            enabled: true,
            model_dir: None,
            ..Default::default()
        };
        assert_eq!(model_status(&cfg), crate::config::PiiModelStatus::ModelDirUnset);
    }

    #[test]
    fn disabled_feature_returns_disabled_feature() {
        #[cfg(not(feature = "pii"))]
        {
            let cfg = PiiConfig::default();
            let (out, ents, state) = redact_code_text("test", &cfg);
            assert_eq!(out, "test");
            assert_eq!(ents.total(), 0);
            assert_eq!(state, RedactionState::DisabledFeature);
        }
    }

    #[cfg(feature = "pii")]
    #[test]
    fn redact_author_identity_masks_email_and_respects_flag() {
        // With redact_git_identity on, an email in the identity is masked.
        let on = PiiConfig {
            redact_git_identity: true,
            ..Default::default()
        };
        let masked = redact_author_identity("Maria Lopez <maria@acme.com>", &on);
        assert!(
            !masked.contains("maria@acme.com"),
            "email must be masked, got: {masked}"
        );
        assert!(masked.contains("Maria Lopez"), "display name without PII stays");

        // With the flag off, the identity is returned verbatim.
        let off = PiiConfig {
            redact_git_identity: false,
            ..Default::default()
        };
        assert_eq!(
            redact_author_identity("Maria Lopez <maria@acme.com>", &off),
            "Maria Lopez <maria@acme.com>"
        );

        // No PII in the string -> unchanged even when on.
        assert_eq!(redact_author_identity("Linus Torvalds", &on), "Linus Torvalds");
    }
}

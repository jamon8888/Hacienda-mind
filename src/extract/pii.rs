//! In-process PII redaction via the vendored `pii-candle` (GLiNER2 + Candle)
//! backend.
//!
//! [`redact_pii`] loads a GLiNER2 safetensors model (optionally merged with a
//! PEFT LoRA adapter) once per process and rewrites every PERSON / ORGANIZATION
//! / LOCATION span in the supplied text according to the configured strategy.
//! When the model directory is unset or loading fails, the pass degrades
//! gracefully — it returns the text unchanged and logs a warning, so a scan
//! never fails on a missing model.
//!
//! The candle model code is gated behind the `pii` cargo feature. With the
//! feature off, [`redact_pii`] is a no-op (returns the text unchanged) so the
//! config surface and call sites stay unconditional.

use std::path::PathBuf;

use crate::config::{PiiCategory, PiiConfig, PiiStrategy};

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
                        let name = adapter_dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("adapter");
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

/// Redact PII from `text` in place according to `cfg`.
///
/// With the `pii` cargo feature disabled (no candle model compiled in), or when
/// PII is disabled / the model is unavailable, returns the original text
/// unchanged with an empty entity list.
pub fn redact_pii(text: &str, cfg: &PiiConfig) -> (String, Vec<String>) {
    #[cfg(not(feature = "pii"))]
    {
        let _ = cfg;
        (text.to_string(), Vec::new())
    }
    #[cfg(feature = "pii")]
    {
        let model = match get_model(cfg) {
            Some(m) => m,
            None => return (text.to_string(), Vec::new()),
        };

        let label_slice = labels(cfg);
        let spans = match model.extract_ner(text, &label_slice, cfg.threshold) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "[pii] inference failed — returning text unchanged");
                return (text.to_string(), Vec::new());
            }
        };

        let mut entities: Vec<String> = Vec::new();
        let mut detected: Vec<Span> = Vec::new();
        for span in spans {
            let (start, end) = span.offsets();
            if (end as usize) <= (start as usize) || (end as usize) > text.len() {
                continue;
            }
            entities.push(span.class().to_string());
            detected.push(Span {
                start: start as usize,
                end: end as usize,
                category: span.class().to_string(),
            });
        }

        if detected.is_empty() {
            return (text.to_string(), Vec::new());
        }

        detected.sort_by_key(|s| s.start);
        (apply_strategy(text, &detected, cfg.strategy), entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn categories_default_to_three_labels() {
        // labels() is feature-gated behind candle; test the config surface instead.
        let cfg = PiiConfig::default();
        assert!(cfg.categories.is_empty());
        let cfg2 = PiiConfig {
            categories: vec![PiiCategory::Person, PiiCategory::Location],
            ..Default::default()
        };
        assert_eq!(cfg2.categories.len(), 2);
    }
}


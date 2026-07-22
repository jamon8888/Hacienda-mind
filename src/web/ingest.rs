//! Shared chunk + embed + LanceDB write path for web ingestion.
//!
//! Called by both `web_scrape` (single page) and `web_crawl` (each page) so
//! the LanceDB row shape stays identical to documents indexed from disk.
//!
//! The flow mirrors `scanner_docs::extract_and_persist_doc`:
//!  1. chunk the page text via `xberg::chunking::chunk_text`,
//!  2. embed each chunk via the shared `SharedEmbedder`,
//!  3. write the rows to LanceDB through `LanceStore::replace_document`.
//!
//! Errors during embed / write are returned to the caller rather than logged
//! and swallowed — the MCP tool wants to report `chunks_indexed = 0` plus the
//! reason to the agent, not silently succeed.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use xberg::chunking::{ChunkingConfig, chunk_text};

use crate::config::{DetectedEntity, DocumentsConfig, PiiConfig, RedactionState};
use crate::embeddings::SharedEmbedder;
use crate::extract::pii;
use crate::lance::{DocumentRow, LanceStore};

/// Outcome of indexing a single fetched page.
#[derive(Debug, Clone)]
pub struct IndexedPage {
    /// Number of chunks written to LanceDB.
    pub chunks_indexed: usize,
    /// Source byte length before chunking. Zero when no body / non-text content.
    pub bytes: usize,
    /// Worst redaction outcome across all chunks (None when `pii` disabled / absent).
    pub redaction: Option<RedactionState>,
    /// Merged tally of detected PII categories across all chunks.
    pub redacted_entities: DetectedEntity,
    /// Attestation hash over the concatenated redacted chunk text (verifiable by clients).
    pub attestation: Option<String>,
}

/// Chunk `body`, embed each chunk, replace all rows for `(scope, path)` in
/// LanceDB, return the count. Empty body short-circuits with `chunks_indexed=0`.
///
/// `documents_cfg` controls chunk sizing (max_characters, overlap) so web
/// chunking matches disk chunking — agents see consistent retrieval behaviour
/// across both sources. When `pii.enabled`, each chunk's text is redacted
/// (via [`crate::extract::pii::redact_code_text`]) *before* embedding and storing,
/// so neither the vector nor the stored `text` carries raw PII/secrets. The worst
/// redaction state + merged tally + attestation are surfaced in [`IndexedPage`]
/// for the MCP tool to echo truthfully.
pub fn index_page(
    lance: &LanceStore,
    embedder: &Arc<SharedEmbedder>,
    documents_cfg: &DocumentsConfig,
    pii_cfg: &PiiConfig,
    scope: &str,
    path: &str,
    mime_type: &str,
    body: &str,
) -> Result<IndexedPage> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        lance
            .replace_document(scope, path, Vec::new())
            .context("clear stale rows for empty body")?;
        return Ok(IndexedPage {
            chunks_indexed: 0,
            bytes: 0,
            redaction: None,
            redacted_entities: DetectedEntity::default(),
            attestation: None,
        });
    }

    let chunking_cfg = ChunkingConfig {
        max_characters: documents_cfg.max_characters,
        overlap: documents_cfg.overlap,
        ..Default::default()
    };
    let chunked = chunk_text(body, &chunking_cfg, None).context("chunk_text on web page body")?;

    if chunked.chunks.is_empty() {
        lance
            .replace_document(scope, path, Vec::new())
            .context("clear stale rows when chunker yielded zero chunks")?;
        return Ok(IndexedPage {
            chunks_indexed: 0,
            bytes: body.len(),
            redaction: None,
            redacted_entities: DetectedEntity::default(),
            attestation: None,
        });
    }

    let dim = embedder.dim();
    if lance.dim() != dim {
        return Err(anyhow!(
            "LanceStore dim {} disagrees with embedder dim {}",
            lance.dim(),
            dim
        ));
    }

    // Redact every chunk's text before embedding + storing. Track the worst state
    // and merge the per-chunk tallies so the IndexedPage reflects the whole page.
    let (redacted_chunks, worst_state, merged_tally) =
        redact_page_chunks(&chunked.chunks, pii_cfg);

    let mut rows: Vec<DocumentRow> = Vec::with_capacity(redacted_chunks.len());
    for (idx, chunk) in chunked.chunks.iter().enumerate() {
        let redacted = &redacted_chunks[idx];
        let embedding = embedder
            .embed(redacted)
            .with_context(|| format!("embed chunk {idx} of {path}"))?;
        if embedding.len() != usize::from(dim) {
            return Err(anyhow!(
                "embedder returned vector of length {} but dim is {}",
                embedding.len(),
                dim
            ));
        }
        let byte_start = u32::try_from(chunk.metadata.byte_start).unwrap_or(u32::MAX);
        let byte_end = u32::try_from(chunk.metadata.byte_end).unwrap_or(u32::MAX);
        rows.push(DocumentRow {
            scope: scope.to_string(),
            path: path.to_string(),
            chunk_idx: u32::try_from(idx).unwrap_or(u32::MAX),
            mime_type: mime_type.to_string(),
            text: redacted.clone(),
            byte_start,
            byte_end,
            embedding,
        });
    }

    let count = rows.len();
    lance
        .replace_document(scope, path, rows)
        .with_context(|| format!("write {count} chunks to LanceDB for {path}"))?;

    // Attestation over the concatenated redacted text — lets clients verify the
    // stored rows were produced by the redaction pass (not raw).
    let concatted = redacted_chunks.concat();
    let attestation = if merged_tally.is_empty() {
        None
    } else {
        let config_hash = format!("{:?}", pii_cfg);
        Some(pii::attestation(&concatted, "gliner2-guardrails", &config_hash))
    };

    Ok(IndexedPage {
        chunks_indexed: count,
        bytes: body.len(),
        redaction: Some(worst_state),
        redacted_entities: merged_tally,
        attestation,
    })
}

/// Redact each chunk's text via [`crate::extract::pii::redact_code_text`] before it is
/// embedded or persisted. Returns the redacted text per chunk (index-aligned with `chunks`)
/// plus the worst [`RedactionState`] and the merged [`DetectedEntity`] tally across the page.
///
/// Split out from [`index_page`] so the core redaction behaviour is unit-testable without a
/// LanceDB store or an embedding model. Honesty invariant: when the model is missing the worst
/// state is `InactiveModelMissing` (never a lying `Redacted`).
fn redact_page_chunks(
    chunks: &[xberg::Chunk],
    pii_cfg: &PiiConfig,
) -> (Vec<String>, RedactionState, DetectedEntity) {
    let mut worst_state = RedactionState::Redacted;
    let mut merged_tally = DetectedEntity::default();
    let mut out: Vec<String> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let (redacted, ents, state) = pii::redact_code_text(&chunk.content, pii_cfg);
        worst_state = worst_state.worst(&state);
        merged_tally.merge(&ents);
        out.push(redacted);
    }
    (out, worst_state, merged_tally)
}

/// Default scope tag for web content when the caller does not override it.
/// Falls back to `"web:unknown"` when the URL has no host (which the `Url`
/// newtype's parser does not actually permit for http/https — kept as a
/// defence-in-depth string).
pub fn default_scope(url: &crate::url::Url) -> String {
    let host = url.host_str().unwrap_or("unknown");
    format!("web:{host}")
}

#[cfg(test)]
mod tests {
    use super::default_scope;
    use crate::url::Url;

    #[test]
    fn default_scope_uses_host_for_simple_url() {
        let u = Url::parse("https://example.com/page").unwrap();
        assert_eq!(default_scope(&u), "web:example.com");
    }

    #[test]
    fn default_scope_distinguishes_subdomains() {
        let a = Url::parse("https://docs.rs/rmcp/").unwrap();
        let b = Url::parse("https://github.com/Goldziher/hacienda-mcp").unwrap();
        assert_eq!(default_scope(&a), "web:docs.rs");
        assert_eq!(default_scope(&b), "web:github.com");
        assert_ne!(default_scope(&a), default_scope(&b));
    }

    #[test]
    fn default_scope_strips_port_and_path() {
        let a = Url::parse("https://example.com:8443/a").unwrap();
        let b = Url::parse("https://example.com/b?q=1").unwrap();
        assert_eq!(default_scope(&a), default_scope(&b));
        assert_eq!(default_scope(&a), "web:example.com");
    }

    #[test]
    fn default_scope_preserves_case_as_parsed() {
        let u = Url::parse("https://EXAMPLE.com/").unwrap();
        assert_eq!(default_scope(&u), "web:example.com");
    }

    const ALLOW_ENV: &str = "HACIENDA_MCP_ALLOW_PRIVATE_HOSTS";

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::url::PRIVATE_HOSTS_ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn rfc1918_host_rejected_by_default() {
        let _g = env_lock();
        unsafe { std::env::remove_var(ALLOW_ENV) };
        assert!(
            Url::parse("http://192.168.1.1/").is_err(),
            "192.168.1.1 must be rejected without the override"
        );
    }

    #[test]
    fn ipv6_loopback_host_rejected_by_default() {
        let _g = env_lock();
        unsafe { std::env::remove_var(ALLOW_ENV) };
        assert!(
            Url::parse("http://[::1]/").is_err(),
            "[::1] must be rejected without the override"
        );
    }

    #[test]
    fn default_scope_handles_private_hosts_under_override() {
        let _g = env_lock();
        unsafe { std::env::set_var(ALLOW_ENV, "1") };
        let v4 = Url::parse("http://192.168.1.1/");
        let v6 = Url::parse("http://[::1]/");
        unsafe { std::env::remove_var(ALLOW_ENV) };

        let v4 = v4.expect("override must allow RFC1918 host");
        assert_eq!(default_scope(&v4), "web:192.168.1.1");
        let v6 = v6.expect("override must allow IPv6 loopback host");
        let scope = default_scope(&v6);
        assert!(
            scope.starts_with("web:") && scope.contains(":1"),
            "ipv6 scope should contain the address; got {scope}"
        );
    }

    #[cfg(feature = "pii")]
    #[test]
    fn redact_page_chunks_reports_honest_inactive_state_without_model() {
        use super::redact_page_chunks;
        use crate::config::{DetectedEntity, PiiConfig, RedactionState};
        use xberg::Chunk;

        // Two chunks: one with an email, one clean. No model dir + enabled = the
        // pass degrades to InactiveModelMissing. The honesty invariant: state is
        // NEVER a lying `Redacted` — and since no model ran, the text is returned
        // unchanged (the email is NOT silently masked). A client reading the state
        // knows redaction did not happen.
        let chunks = vec![
            Chunk {
                content: "contact maria@acme.com for details".to_string(),
                chunk_type: xberg::ChunkType::Unknown,
                embedding: None,
                metadata: xberg::ChunkMetadata {
                    byte_start: 0,
                    byte_end: 30,
                    token_count: None,
                    chunk_index: 0,
                    total_chunks: 2,
                    first_page: None,
                    last_page: None,
                    heading_context: None,
                    heading_path: Vec::new(),
                    image_indices: Vec::new(),
                },
            },
            Chunk {
                content: "no secrets here".to_string(),
                chunk_type: xberg::ChunkType::Unknown,
                embedding: None,
                metadata: xberg::ChunkMetadata {
                    byte_start: 31,
                    byte_end: 45,
                    token_count: None,
                    chunk_index: 1,
                    total_chunks: 2,
                    first_page: None,
                    last_page: None,
                    heading_context: None,
                    heading_path: Vec::new(),
                    image_indices: Vec::new(),
                },
            },
        ];
        let cfg = PiiConfig {
            enabled: true,
            model_dir: None,
            ..Default::default()
        };
        let (redacted, state, tally) = redact_page_chunks(&chunks, &cfg);
        // Honesty: model missing -> InactiveModelMissing, not Redacted.
        assert!(
            matches!(state, RedactionState::InactiveModelMissing(_)),
            "missing model must report InactiveModelMissing, got {state:?}"
        );
        // No model ran, so the raw email is preserved verbatim (no silent masking).
        assert!(
            redacted[0].contains("maria@acme.com"),
            "without a model the text must be returned unchanged, got: {}",
            redacted[0]
        );
        // Tally is empty because no detection ran.
        assert!(tally.is_empty(), "no detection without a model");
        // Clean chunk is unchanged.
        assert_eq!(redacted[1], "no secrets here");
    }
}

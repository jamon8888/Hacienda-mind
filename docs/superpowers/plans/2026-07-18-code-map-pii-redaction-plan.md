# RGPD-Ready MCP PII Redaction — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the hacienda-mcp MCP server safe to feed to Claude Desktop / ChatGPT Desktop for legal/audit workflows by redacting PII/secrets across the code-map, document, git, and web surfaces — and exposing **truthful, verifiable** user-facing PII tools.

**Architecture:** Extend the existing `redact_pii` pure function into `redact_code_text` that returns `(String, Vec<DetectedEntity>, RedactionState)` plus a client-verifiable attestation hash. Wire redaction into the scanner blob-write path (code-map l1/l2/l3, documents, web ingest) and the MCP response-serialization path (every leaking tool). Persist a per-blob `RedactionState` + `DetectedEntity` tally + attestation and add a new Fjall `entities_by_category` keyspace. Add `pii_status` / `pii_redact` / `pii_audit_report` MCP tools that read these persisted primitives (never fresh unverified claims). P3 (encryption, erasure, telemetry) is deferred and out of scope for these tasks.

**Tech Stack:** Rust, Cargo feature `pii`, `pii_candle` (GLiNER2 + Candle), `rmcp` `#[tool]` macros, Fjall LSM keyspace, `serde`/`rmp-serde` msgpack blobs, `schemars` JSON schema, `tracing`.

**Spec:** `docs/superpowers/specs/2026-07-18-code-map-pii-redaction-design.md`

---

## File Structure

| File | Responsibility |
|---|---|
| `src/config/pii.rs` | Add `PiiCategory::{Email,Phone,Date,Url}`, `PiiModelStatus`, `RedactionState`, `DetectedEntity` types; extend default `categories`; add `redact_git_identity` + `encryption_key` (P3 consumer) fields. |
| `src/extract/pii.rs` | Rework `redact_pii` → `redact_code_text` returning `(String, Vec<DetectedEntity>, RedactionState)`; public `model_status()` probe; attestation hash; merge regex spans. Keep `redact_pii` as a thin compat wrapper. |
| `src/extract/pii_regex.rs` | **New** — Presidio-style regex detector → `(start, end, category)` spans (email, phone, date, url, AWS/Google/GitHub keys, `sk-*`, JWT, PEM, `KEY=`). |
| `src/extract/mod.rs` | Add `RedactionState` + `DetectedEntity` + attestation fields to `FileMapL1`/`FileMapL2`/`FileMapL3`. |
| `src/scanner.rs` | In `process_file`, redact textual fields of l1/l2/l3 when `pii.enabled`; persist state + tally + attestation; stage `entities_by_category`. |
| `src/extract/doc.rs` | Redact `entities`/`summary`/`metadata` too; persist state + attestation on `FileMapDoc`/chunks. |
| `src/web/ingest.rs` | `index_page`: redact chunk text before LanceDB store; persist state. |
| `src/git/*` + `src/mcp/tools_git.rs` | Redact author email/name + patch/body text (consent-gated by `pii.redact_git_identity`). |
| `src/mcp/helpers*.rs` | Defensive redaction of serialized strings in leaking tools; echo attestation. |
| `src/mcp/tools_pii.rs` | **New** — `pii_status`, `pii_redact`, `pii_audit_report` tools (`read_only_hint=true`). |
| `src/index/keys.rs` + `src/index/writer.rs` + `src/index/mod.rs` | New Fjall `entities_by_category` keyspace. |
| `src/store.rs` | Blob write/read carries new `RedactionState` + tally + attestation header fields. |
| `tests/scan_smoke.rs`, `tests/mcp_smoke.rs`, `tests/harden.rs` | Assertions per spec Section 5. |
| `README.md` | Note PII now covers code-map + git + web, RGPD-ready with verifiable tools. |

---

## Task 1: PII types + extended categories (config)

**Files:**
- Modify: `src/config/pii.rs`
- Test: inline `#[cfg(test)]` in `src/config/pii.rs`

- [ ] **Step 1: Write failing tests for new types**

Add to `src/config/pii.rs` test module:

```rust
#[test]
fn extended_categories_default_present() {
    let cfg = PiiConfig::default();
    // After this task the default categories include email/phone/date/url.
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --features pii --lib config::pii 2>&1 | Select-String "extended_categories_default_present|pii_category_labels_are_stable"`
Expected: FAIL — `PiiCategory::Email` does not exist; default categories empty.

- [ ] **Step 3: Add the types and extend defaults**

In `src/config/pii.rs`:

1. Add to `PiiCategory` enum (after `Location`):
```rust
    Email,
    Phone,
    Date,
    Url,
```
2. Extend `PiiCategory::label`:
```rust
    PiiCategory::Email => "email",
    PiiCategory::Phone => "phone",
    PiiCategory::Date => "date",
    PiiCategory::Url => "url",
```
3. Add `redact_git_identity` + `encryption_key` fields to `PiiConfig` (encryption_key is a P3 consumer; add the field now, unused until P3):
```rust
    /// Mask author email/name in git tool responses (legal/audit contexts).
    #[serde(default = "PiiConfig::default_redact_git_identity")]
    pub redact_git_identity: bool,
    /// Encryption key source for at-rest encryption + attestation HMAC (P3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encryption_key: Option<crate::config::documents::ApiKey>,
```
4. Extend `PiiConfig::default()` so `categories` defaults to the four new + three base labels and `redact_git_identity` defaults to `true`:
```rust
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
```
5. Add the default fn:
```rust
fn default_redact_git_identity() -> bool { true }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --features pii --lib config::pii 2>&1 | Select-String "test result"`
Expected: PASS (`test result: ok`).

- [ ] **Step 5: Commit**

```bash
git add src/config/pii.rs
git commit -m "feat(pii): add Email/Phone/Date/Url categories + git-identity + key config"
```

---

## Task 2: Verifiability types (RedactionState, DetectedEntity, PiiModelStatus)

**Files:**
- Modify: `src/config/pii.rs` (add the three enums/structs)
- Test: inline test in `src/config/pii.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn redaction_state_serializes_stably() {
    let s = RedactionState::InactiveModelMissing("model_dir unset".to_string());
    let j = serde_json::to_string(&s).unwrap();
    assert!(j.contains("inactive_model_missing"));
    // round-trips
    let back: RedactionState = serde_json::from_str(&j).unwrap();
    assert_eq!(s, back);
}

#[test]
fn detected_entity_tally_works() {
    let mut t = std::collections::BTreeMap::new();
    t.insert("email".to_string(), 2u32);
    let e = DetectedEntity::from_map(t);
    assert_eq!(e.total(), 2);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test --features pii --lib config::pii::tests::redaction_state_serializes_stably 2>&1 | Select-String "error\[|cannot find"`
Expected: FAIL — `RedactionState` / `DetectedEntity` not found.

- [ ] **Step 3: Add types to `src/config/pii.rs`**

```rust
/// Machine-readable outcome of a redaction pass. Never silent — lets clients
/// distinguish "redacted" from "not redacted, and why".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionState {
    /// `pii` cargo feature compiled out.
    DisabledFeature,
    /// `pii.enabled = false` in config.
    DisabledConfig,
    /// Enabled but the model could not be loaded (with reason).
    InactiveModelMissing(String),
    /// Model loaded but inference errored (with reason).
    Failed(String),
    /// Redaction applied successfully.
    Redacted,
}

/// Runtime load status of the candle model — surfaced by `pii_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PiiModelStatus {
    Disabled,
    ModelDirUnset,
    LoadFailed(String),
    Loaded,
}

/// Tally of detected entities by category, persisted with each blob.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DetectedEntity {
    pub by_category: std::collections::BTreeMap<String, u32>,
}

impl DetectedEntity {
    pub fn from_map(m: std::collections::BTreeMap<String, u32>) -> Self {
        DetectedEntity { by_category: m }
    }
    pub fn total(&self) -> u32 {
        self.by_category.values().copied().sum()
    }
    pub fn add(&mut self, category: &str) {
        *self.by_category.entry(category.to_string()).or_insert(0) += 1;
    }
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --features pii --lib config::pii 2>&1 | Select-String "test result"`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config/pii.rs
git commit -m "feat(pii): add RedactionState, PiiModelStatus, DetectedEntity types"
```

---

## Task 3: Presidio-style regex detector (`pii_regex.rs`)

**Files:**
- Create: `src/extract/pii_regex.rs`
- Modify: `src/extract/mod.rs` (add `pub mod pii_regex;`)
- Test: inline `#[cfg(test)]` in `src/extract/pii_regex.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn detects_email_and_aws_key() {
    let spans = detect_regex_pii("contact maria@acme.com or akiaiosfodnn7example");
    let cats: Vec<&str> = spans.iter().map(|s| s.2).collect();
    assert!(cats.contains(&"email"));
    assert!(cats.contains(&"aws_key"));
}

#[test]
fn detects_sk_token_and_jwt() {
    let spans = detect_regex_pii("key=sk-1234567890abcdef and eyJhbGciOiJIUzI1Ni.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKx");
    let cats: Vec<&str> = spans.iter().map(|s| s.2).collect();
    assert!(cats.contains(&"api_key"));
    assert!(cats.contains(&"jwt"));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test --features pii --lib extract::pii_regex 2>&1 | Select-String "cannot find|error\[`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Create `src/extract/pii_regex.rs`**

```rust
//! Presidio-style regex PII/secret detector. Catches structured PII and secrets
//! the GLiNER2 ML model misses (emails, phones, dates, URLs, cloud keys, JWT,
//! PEM). Returns the same `(start, end, category)` shape the candle model uses.

use once_cell::sync::Lazy;
use regex::Regex;

pub type RegexSpan = (usize, usize, &'static str);

static PATTERNS: Lazy<Vec<(&'static str, Regex)>> = Lazy::new(|| {
    let build = |re: &'static str| (re, Regex::new(re).expect("valid pii regex"));
    vec![
        ("email", build(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}")),
        ("phone", build(r"(?:\+?\d{1,3}[\s.\-]?)?(?:\(?\d{2,4}\)?[\s.\-]?){2,4}\d{2,4}")),
        ("date", build(r"\b\d{4}-\d{2}-\d{2}\b|\b\d{1,2}/\d{1,2}/\d{2,4}\b")),
        ("url", build(r"https?://[^\s\"'<>]+")),
        ("aws_key", build(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b")),
        ("api_key", build(r"(?i)\b(?:sk|pk|rk)-[a-z0-9]{16,}\b|ghp_[a-zA-Z0-9]{36}|github_pat_[a-zA-Z0-9_]{22,}")),
        ("jwt", build(r"eyJ[A-Za-z0-9_\-]+\.eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+")),
        ("pem", build(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----")),
        ("env_assign", build(r"(?i)\b(?:API_KEY|SECRET|TOKEN|PASSWORD|PASSWD|ACCESS_KEY)\s*=\s*['\"]?[^\s'\"]{4,}")),
    ]
});

/// Scan `text` for structured PII/secret spans.
pub fn detect_regex_pii(text: &str) -> Vec<RegexSpan> {
    let mut out = Vec::new();
    for (category, re) in PATTERNS.iter() {
        for m in re.find_iter(text) {
            out.push((m.start(), m.end(), *category));
        }
    }
    out.sort_by_key(|s| s.0);
    out
}
```

Add `pub mod pii_regex;` to `src/extract/mod.rs` (near `pub mod pii;`).

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --features pii --lib extract::pii_regex 2>&1 | Select-String "test result"`
Expected: PASS. (Note: `regex` and `once_cell` must be in `Cargo.toml` deps — verify; add if missing with `cargo add regex once_cell`.)

- [ ] **Step 5: Commit**

```bash
git add src/extract/pii_regex.rs src/extract/mod.rs Cargo.toml
git commit -m "feat(pii): add Presidio-style regex secret/PII detector"
```

---

## Task 4: Rework `redact_pii` → `redact_code_text` (verifiable)

**Files:**
- Modify: `src/extract/pii.rs`
- Test: inline in `src/extract/pii.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn redact_code_text_returns_state_and_entities() {
    let cfg = PiiConfig {
        enabled: true,
        model_dir: Some(PathBuf::from("/nonexistent-model")),
        categories: vec![PiiCategory::Email, PiiCategory::Person],
        ..Default::default()
    };
    // No model -> InactiveModelMissing, original text, no false "Redacted".
    let (out, ents, state) = redact_code_text("mail me at maria@acme.com", &cfg);
    assert_eq!(state, RedactionState::InactiveModelMissing(_)); // see note
    assert_eq!(out, "mail me at maria@acme.com");
    assert!(ents.total() == 0);
}

#[test]
fn model_status_probe_reports_missing() {
    let cfg = PiiConfig { enabled: true, model_dir: None, ..Default::default() };
    assert_eq!(model_status(&cfg), PiiModelStatus::ModelDirUnset);
}
```

> Note: `RedactionState::InactiveModelMissing(String)` is not `PartialEq` on the inner `String` for a clean `==` literal; instead assert `matches!(state, RedactionState::InactiveModelMissing(_))`.

- [ ] **Step 2: Run to verify fail**

Run: `cargo test --features pii --lib extract::pii::tests::redact_code_text_returns_state_and_entities 2>&1 | Select-String "cannot find|error\[`
Expected: FAIL — `redact_code_text` / `model_status` do not exist.

- [ ] **Step 3: Rework `src/extract/pii.rs`**

1. Add `use crate::config::{DetectedEntity, PiiCategory, PiiConfig, PiiModelStatus, PiiStrategy, RedactionState};`
2. Add a public model-status probe that reads the `OnceLock`:
```rust
#[cfg(feature = "pii")]
pub fn model_status(cfg: &PiiConfig) -> PiiModelStatus {
    if !cfg.enabled { return PiiModelStatus::Disabled; }
    if cfg.model_dir.is_none() { return PiiModelStatus::ModelDirUnset; }
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
```
3. Add attestation helper:
```rust
pub fn attestation(redacted: &str, model_id: &str, config_hash: &str) -> String {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&(redacted, model_id, config_hash), &mut h);
    format!("{:x}", h.finish())
}
```
4. Replace `redact_pii` body (feature `pii`) with `redact_code_text`:
```rust
pub fn redact_code_text(text: &str, cfg: &PiiConfig) -> (String, DetectedEntity, RedactionState) {
    #[cfg(not(feature = "pii"))]
    {
        let _ = cfg;
        (text.to_string(), DetectedEntity::default(), RedactionState::DisabledFeature)
    }
    #[cfg(feature = "pii")]
    {
        if !cfg.enabled {
            return (text.to_string(), DetectedEntity::default(), RedactionState::DisabledConfig);
        }
        let model = match get_model(cfg) {
            Some(m) => m,
            None => return (text.to_string(), DetectedEntity::default(),
                RedactionState::InactiveModelMissing("model unavailable".to_string())),
        };
        let label_slice: Vec<&str> = cfg.categories.iter().map(PiiCategory::label).collect();
        let mut detected: Vec<Span> = Vec::new();
        let mut tally = DetectedEntity::default();
        // ML spans
        if let Ok(spans) = model.extract_ner(text, &label_slice, cfg.threshold) {
            for span in spans {
                let (s, e) = span.offsets();
                if (e as usize) <= (s as usize) || (e as usize) > text.len() { continue; }
                tally.add(span.class());
                detected.push(Span { start: s as usize, end: e as usize, category: span.class().to_string() });
            }
        } else {
            return (text.to_string(), DetectedEntity::default(),
                RedactionState::Failed("inference error".to_string()));
        }
        // Regex spans
        for (s, e, cat) in crate::extract::pii_regex::detect_regex_pii(text) {
            tally.add(cat);
            detected.push(Span { start: s, end: e, category: cat.to_string() });
        }
        if detected.is_empty() {
            return (text.to_string(), tally, RedactionState::Redacted);
        }
        detected.sort_by_key(|x| x.start);
        (apply_strategy(text, &detected, cfg.strategy), tally, RedactionState::Redacted)
    }
}
```
5. Keep compat wrapper:
```rust
/// Back-compat: documents path still calls this; returns redacted text only.
pub fn redact_pii(text: &str, cfg: &PiiConfig) -> (String, Vec<String>) {
    let (out, ents, _state) = redact_code_text(text, cfg);
    let labels: Vec<String> = ents.by_category.keys().cloned().collect();
    (out, labels)
}
```
6. Update existing tests: `disabled_pii_is_noop` and `enabled_without_model_dir_is_noop` still call `redact_pii` and check `.0` — keep passing. Add `model_status` test.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --features pii --lib extract::pii 2>&1 | Select-String "test result"`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/extract/pii.rs
git commit -m "feat(pii): redact_code_text returns state+entities+attestation, public model_status"
```

---

## Task 5: Persist state + tally + attestation on code-map blobs

**Files:**
- Modify: `src/extract/mod.rs` (`FileMapL1`/`L2`/`L3` structs)
- Modify: `src/store.rs` (blob header write/read)
- Modify: `src/scanner.rs` (`process_file`)
- Test: `tests/scan_smoke.rs` (added in Task 11) — for now unit-test the struct round-trip in `src/extract/mod.rs`.

- [ ] **Step 1: Write failing test (struct round-trip)**

Add to `src/extract/mod.rs` tests:
```rust
#[test]
fn l1_carries_redaction_fields() {
    let mut l1 = FileMapL1::default_for("rust", 10);
    l1.redaction = Some(crate::config::pii::RedactionState::Redacted);
    l1.redacted_entities = crate::config::pii::DetectedEntity::from_map({
        let mut m = std::collections::BTreeMap::new(); m.insert("email".to_string(), 1); m
    });
    l1.attestation = Some("abc123".to_string());
    let bytes = rmp_serde::to_vec(&l1).unwrap();
    let back: FileMapL1 = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(back.redaction, l1.redaction);
    assert_eq!(back.redacted_entities.total(), 1);
    assert_eq!(back.attestation, Some("abc123".to_string()));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test --features pii --lib extract::tests::l1_carries_redaction_fields 2>&1 | Select-String "error\[|cannot find`
Expected: FAIL — `redaction`/`redacted_entities`/`attestation` fields and `default_for` do not exist (use the actual existing constructor name if `default_for` differs — check `FileMapL1` impl).

- [ ] **Step 3: Add fields + constructor**

In `src/extract/mod.rs`, add to `FileMapL1` (and matching fields to `FileMapL2`/`FileMapL3`):
```rust
    /// Redaction outcome for this file's text. `None` = pre-feature blob.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redaction: Option<RedactionState>,
    /// Tally of detected PII categories (persisted for audit report).
    #[serde(default, skip_serializing_if = "DetectedEntity::is_empty")]
    pub redacted_entities: DetectedEntity,
    /// Attestation hash over (redacted_text, model_id, config_hash).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<String>,
```
Add `impl DetectedEntity { pub fn is_empty(&self) -> bool { self.by_category.is_empty() } }` (or place on the config type). Ensure `RedactionState`/`DetectedEntity` are imported (`use crate::config::pii::*;`).

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test --features pii --lib extract 2>&1 | Select-String "test result"`
Expected: PASS.

- [ ] **Step 5: Wire `process_file` in `src/scanner.rs`**

After `extract_l1_l2` produces `(l1, l2_opt)` (line ~847) and before `store.write_filemap_hex` (line ~866), redact textual fields when `pii.enabled`. For each `Symbol`/`Import`/`Call` doc-comment/signature/string-literal field, apply `redact_code_text` and collect the worst `RedactionState` + merged `DetectedEntity` + recompute attestation over the concatenated redacted text. Set `l1.redaction`/`l1.redacted_entities`/`l1.attestation` (and same on `l2`). Do NOT redact `symbol.name`/`call.callee`/import module names.

Pseudocode (fill in exact field names from `l1.rs`/`l2.rs`):
```rust
#[cfg(feature = "pii")]
if config.pii.enabled {
    let mut state = RedactionState::Redacted;
    let mut tally = DetectedEntity::default();
    for sym in &mut l1.symbols {
        let (t, e, s) = crate::extract::pii::redact_code_text(&sym.signature, &config.pii);
        sym.signature = t; tally.merge(&e); state = state.worst(&s);
        if let Some(doc) = sym.docs.as_mut() {
            let (t, e, s) = crate::extract::pii::redact_code_text(doc, &config.pii);
            *doc = t; tally.merge(&e); state = state.worst(&s);
        }
    }
    // ... same for l2 calls context, l3 body text ...
    let att = crate::extract::pii::attestation(&format!("{:?}", l1), "gliner2-guardrails", &config_hash);
    l1.redaction = Some(state); l1.redacted_entities = tally.clone(); l1.attestation = Some(att);
    if let Some(l2) = &mut l2 { l2.redaction = Some(state.clone()); l2.redacted_entities = tally; l2.attestation = Some(att); }
}
```
Add `RedactionState::worst`/`DetectedEntity::merge` helpers in `src/config/pii.rs`:
```rust
impl RedactionState {
    /// Pick the most severe state (Failed > InactiveModelMissing > DisabledConfig > Redacted).
    pub fn worst(&self, other: &Self) -> Self { /* order by severity */ }
}
impl DetectedEntity { pub fn merge(&mut self, o: &Self) { for (k,v) in &o.by_category { *self.by_category.entry(k.clone()).or_insert(0) += v; } } }
```

- [ ] **Step 6: Run scanner unit + clippy**

Run: `cargo clippy --features pii --lib -- -D warnings 2>&1 | Select-String "error|warning: unused"`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add src/extract/mod.rs src/store.rs src/scanner.rs src/config/pii.rs
git commit -m "feat(pii): persist redaction state, entity tally, attestation on code-map blobs"
```

---

## Task 6: Redact documents `entities`/`summary`/`metadata`

**Files:**
- Modify: `src/extract/doc.rs` (already calls `redact_pii` at lines 410, 465)
- Test: inline in `src/extract/doc.rs`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(feature = "documents")]
#[test]
fn doc_entities_and_summary_redacted() {
    // Build a minimal DocConfig with pii enabled + fake model; assert entities/summary masked.
    // (If model unavailable, assert RedactionState is InactiveModelMissing, not lying Redacted.)
}
```

- [ ] **Step 2: Replace `.0` calls with `redact_code_text` and persist state**

In `src/extract/doc.rs`:
```rust
let (text, _ents, state) = crate::extract::pii::redact_code_text(&c.content, &config.pii);
// use `text` for the chunk
```
Apply same to `result.content` (line 465) and to `result.summary`/`.metadata`/`.entities` text fields. Set `FileMapDoc.redaction`/`redacted_entities`/`attestation` (add these three fields to `FileMapDoc` in `doc.rs`, mirroring Task 5). Keep the existing `entities`/`summary`/`metadata` fields but redact their `text`/`value`.

- [ ] **Step 3: Run doc tests + clippy**

Run: `cargo test --features "pii documents" --lib extract::doc 2>&1 | Select-String "test result"`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/extract/doc.rs
git commit -m "feat(pii): redact document entities/summary/metadata, persist state"
```

---

## Task 7: New Fjall `entities_by_category` keyspace

**Files:**
- Modify: `src/index/keys.rs`, `src/index/writer.rs`, `src/index/mod.rs`
- Test: inline in `src/index/writer.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn entities_by_category_keyspace_roundtrip() {
    let db = temp_keyspace();
    let w = IndexWriter::new(&db);
    w.stage_entity("email", "maria@acme.com", "blob123");
    let refs = w.lookup_entity("email", "maria@acme.com");
    assert_eq!(refs, vec!["blob123".to_string()]);
}
```

- [ ] **Step 2: Add keyspace + encoder in `src/index/keys.rs`**

Add a `entities_by_category` partition key alongside existing keyspaces, with length-prefixed composite key `category ‖ value_hash` → `blob_ref` (reuse the length-prefix pattern from `symbols_by_name`).

- [ ] **Step 3: Implement `stage_entity` / `lookup_entity` in `src/index/writer.rs`**

Mirror the read-before-write upsert pattern used by `upsert_file`. Stage calls from `scanner.rs` `process_file` (Task 5) and `doc.rs` (Task 6) using the `DetectedEntity` tally + a value-hash of each detected span.

- [ ] **Step 4: Run test + clippy**

Run: `cargo test --features pii --lib index 2>&1 | Select-String "test result"`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/index/keys.rs src/index/writer.rs src/index/mod.rs
git commit -m "feat(pii): add entities_by_category Fjall keyspace for audit + erasure"
```

---

## Task 8: Web ingest redaction (`src/web/ingest.rs`)

**Files:**
- Modify: `src/web/ingest.rs` (`index_page`)
- Test: inline in `src/web/ingest.rs`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(all(feature = "pii", feature = "crawl"))]
#[test]
fn index_page_redacts_pii_before_store() {
    // page with an email; assert chunk text stored has no raw email.
}
```

- [ ] **Step 2: Add `redact_code_text` call in `index_page`**

In `index_page`, before `chunk_text`/`embedder.embed` and `DocumentRow.text = chunk.content.clone()`, apply `redact_code_text` and persist the returned `state`/`tally`/`attestation` on the row (add fields to `DocumentRow` if needed, or stash in metadata).

- [ ] **Step 3: Run test + clippy**

Run: `cargo test --features "pii crawl" --lib web 2>&1 | Select-String "test result"`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/web/ingest.rs
git commit -m "feat(pii): redact web-ingested page text before LanceDB store"
```

---

## Task 9: Git tool redaction (consent-gated)

**Files:**
- Modify: `src/git/mod.rs` (author email/name structs) + `src/mcp/tools_git.rs` + `src/mcp/helpers_git.rs` (or wherever git responses are built)
- Test: inline in the git helper module

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn git_author_email_redacted_when_enabled() {
    let cfg = PiiConfig { enabled: true, redact_git_identity: true, ..Default::default() };
    let redacted = redact_git_author("Maria Lopez <maria@acme.com>", &cfg);
    assert!(!redacted.contains("maria@acme.com"));
}
#[test]
fn git_author_unchanged_when_consent_off() {
    let cfg = PiiConfig { enabled: true, redact_git_identity: false, ..Default::default() };
    let redacted = redact_git_author("Maria Lopez <maria@acme.com>", &cfg);
    assert_eq!(redacted, "Maria Lopez <maria@acme.com>");
}
```

- [ ] **Step 2: Implement `redact_git_author` + patch/body redaction**

In the git helper, when `config.pii.redact_git_identity`, run `redact_code_text` over author display string, commit body, and patch hunk text, and persist `RedactionState`. Keep structural attribution (line ranges) intact — only mask the email/name tokens.

- [ ] **Step 3: Run test + clippy**

Run: `cargo test --features "pii documents" --lib 2>&1 | Select-String "test result"`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/git src/mcp/tools_git.rs src/mcp/helpers_git.rs
git commit -m "feat(pii): consent-gated redaction of git author/email/patch text"
```

---

## Task 10: MCP response-time defensive redaction + `tools_pii.rs`

**Files:**
- Create: `src/mcp/tools_pii.rs`
- Modify: `src/mcp/mod.rs` (register the new tool module)
- Modify: `src/mcp/helpers*.rs` (redact serialized strings per Section 2 tool list)
- Test: `tests/mcp_smoke.rs` (Task 11)

- [ ] **Step 1: Write the three tools in `src/mcp/tools_pii.rs`**

```rust
#[tool(description = "Report PII redaction status. Best-effort; some PII may remain. Returns whether redaction is ACTIVE (model loaded), the model status, categories, threshold, and the last scan's redaction state. Never reports 'redacting' when the model is missing.")]
async fn pii_status(state: &AppState) -> Result<PiiStatusResponse, McpError> { /* reads config.pii + model_status() + persisted last_scan state */ }

#[tool(description = "Redact PII/secrets from arbitrary text. Best-effort; some PII may remain. Returns redacted text, detected entity tally, redaction state, and an attestation hash the client can verify.")]
async fn pii_redact(state: &AppState, text: String) -> Result<PiiRedactResponse, McpError> {
    let (out, ents, st) = crate::extract::pii::redact_code_text(&text, &state.config.pii);
    let att = crate::extract::pii::attestation(&out, "gliner2-guardrails", &config_hash);
    Ok(PiiRedactResponse { redacted_text: out, entities: ents, state: st, attestation: att })
}

#[tool(description = "Audit report of PII redaction across the indexed corpus. Returns per-category counts and which files were redacted. Best-effort; reflects only detected PII.")]
async fn pii_audit_report(state: &AppState) -> Result<PiiAuditResponse, McpError> {
    let counts = state.index_db.entities_by_category_counts();
    Ok(PiiAuditResponse { by_category: counts, note: "best-effort; some PII may remain".into() })
}
```
Define `PiiStatusResponse`/`PiiRedactResponse`/`PiiAuditResponse` with `JsonSchema`-derived structs in `src/mcp/types_pii.rs` (or inline). Mark all three `annotations(read_only_hint = true, open_world_hint = false)`. Add `cache_erase_subject` as a **deferred stub** that returns `McpError::not_implemented("deferred to P3")` so the surface is reserved but not built.

- [ ] **Step 2: Register module in `src/mcp/mod.rs`**

Add `mod tools_pii;` and mount the tool router (follow the existing `#[tool(...)]` router pattern in `tools.rs`).

- [ ] **Step 3: Add defensive redaction to leaking helpers**

In `helpers*.rs`, wrap the serialized strings of: `outline`, `search_symbols`, `find_references`, `find_callers`, `workspace_grep` (matched lines+context), `expand`, `get_chunk` (documents/code-search features), `search_documents` (text+entities+summary), `memory_get`/`memory_search` (values), `message_get` (comms feature), and the git tools from Task 9 — each via `redact_code_text` when `pii.enabled`, echoing the attestation.

- [ ] **Step 4: Run clippy + mcp smoke (Task 11)**

Run: `cargo clippy --features "pii documents memory comms" --lib -- -D warnings 2>&1 | Select-String "error|warning:"`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/mcp/tools_pii.rs src/mcp/types_pii.rs src/mcp/mod.rs src/mcp/helpers*.rs
git commit -m "feat(pii): add pii_status/pii_redact/pii_audit_report tools + response-time redaction"
```

---

## Task 11: Tests — scan_smoke, mcp_smoke, harden

**Files:**
- Modify: `tests/scan_smoke.rs`, `tests/mcp_smoke.rs`, `tests/harden.rs`
- Run: `cargo test --features "pii documents memory comms code-search" --test scan_smoke --test mcp_smoke`

- [ ] **Step 1: scan_smoke — leaky repo**

Add a fixture `.rs` file containing `/// contact maria@acme.com` + `const TOKEN = "sk-1234567890abcdef";`. Assert the written `l1` blob carries `RedactionState::Redacted` (or `InactiveModelMissing` honestly when no model) and `find_references`/`search_symbols` on a symbol name still returns hits.

- [ ] **Step 2: mcp_smoke — tool responses**

Assert `outline`/`workspace_grep`/`expand` redact the literal but preserve the symbol name; assert `pii_status` reports `active` truthfully (false when model missing despite `enabled=true`); assert `pii_redact` echoes attestation.

- [ ] **Step 3: harden — canary**

Add a planted leaky file in one harden repo; assert redaction fired when `pii` enabled; navigation canaries (`spawn`/`get`/`useState`) unaffected.

- [ ] **Step 4: Run full suite + poly**

Run: `cargo test --workspace 2>&1 | Select-String "test result"`
Run: `poly lint . 2>&1 | Select-String "error"`
Expected: all PASS, no lint errors.

- [ ] **Step 5: Commit**

```bash
git add tests/scan_smoke.rs tests/mcp_smoke.rs tests/harden.rs
git commit -m "test(pii): add scan/mcp/harden assertions for code-map + git + web redaction"
```

---

## Task 12: README + final verification

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README PII section**

Add a row/note: PII redaction now covers code-map (symbols/doc-comments/literals), documents (content/entities/summary/metadata), git author/email/patch (consent-gated), and web ingest — plus verifiable `pii_status`/`pii_redact`/`pii_audit_report` tools. State the honesty constraint: best-effort, not a guarantee.

- [ ] **Step 2: Final check triad**

Run:
```bash
cargo fmt
cargo clippy --workspace --all-targets --tests -- -D warnings
cargo test --workspace
poly lint .
```
Expected: all clean.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document RGPD-ready PII redaction + verifiable tools"
```

---

## Notes / Risks

- **P3 deferred (NOT in this plan):** at-rest encryption (`encryption_key` field added but unused), `cache_erase_subject` real implementation (stub only), telemetry param scrubbing. These get their own spec + plan.
- **`model_id` string** (`"gliner2-guardrails"`) must be a single constant; the client verifies self-consistency, not cryptographic proof (per spec Section 3).
- **`FileMapL1::default_for`** — use the actual existing constructor name discovered in Task 5 Step 2; do not assume.
- **Feature flags:** tasks touch `pii`, `documents`, `memory`, `comms`, `code-search`, `crawl`. Run clippy/tests with the union of features used by each task.
- **YAGNI:** no new config sub-tree; reuse `ConfigV1.pii`. No identifier redaction (breaks navigation). No pre-parse redaction (corrupts trees).

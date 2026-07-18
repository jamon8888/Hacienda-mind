# Code-Map PII / Secret Redaction Design

**Date:** 2026-07-18
**Status:** Approved (design)
**Feature flag:** `pii`
**Goal:** Make the hacienda-mcp code-map (the indexed `l1`/`l2`/`l3` blobs and the MCP tool
outputs that read them) safe to feed to Claude Code / Claude Desktop without leaking
secrets (API keys, tokens) or personal information (names, emails, orgs, locations) —
so a codebase can be audited by an agent with no legal-exposure leaks.

## Background

The PII engine today is **documents-only**:

- `redact_pii(text, &PiiConfig) -> (String, Vec<String>)` lives in `src/extract/pii.rs`.
- It is called from exactly two places, both in `src/extract/doc.rs` (lines 411, 466) — the
  xberg document pipeline (PDF/Office/HTML → LanceDB).
- `scanner.rs:969` passes `&config.pii` only into `extract_and_persist_doc` (document tier).
- The code-map pipeline (`src/scanner.rs` rayon loop → `process_file` → `extract::l1`/`l2`/`l3`)
  never calls `redact_pii`. `src/mcp/` has zero PII references.
- Symbols, call sites, outlines, doc comments, and string literals in code are stored and
  served **unredacted**.

The engine is a pure function that already returns byte-offset spans, so it is trivially
reusable over any text. GLiNER2 is code-friendly, and the `fastino/GLiNER2-Guardrails-PII-Multi`
model labels secret-style classes (API keys, tokens, credentials) as PII in addition to the
standard person/organization/location classes. Detection is strengthened with a Presidio-style
regex layer for patterns the model misses.

## Decisions (from brainstorming)

1. **Detection coverage:** GLiNER2-Guardrails model **+** Presidio-style regex. Together they
   cover persons/orgs/locations AND secrets (API keys, tokens, JWT, PEM, `.env` assignments,
   emails, IPs).
2. **Where:** **Both** scan time (blobs on disk) and MCP response time (defense-in-depth for
   already-scanned repos). Disk must be clean for legal safety; response-time pass covers
   repos scanned before this feature shipped.
3. **Fidelity/safety tradeoff:** **A** — redact only safe textual fields; keep identifiers
   (`symbol.name`, `call.callee`, import module names) intact so `find_references`,
   `search_symbols`, and the harden canaries keep working. The literal *values* that create
   legal exposure are what get redacted.
4. **Performance gating:** reuse the existing `pii` cargo feature + `config.pii.enabled`
   toggle. Zero cost by default; reuses the existing cached `Arc<Gliner2Candle>` model.
5. **Config surface:** reuse `ConfigV1.pii` (`PiiConfig`) as-is. The scanner already has
   `config.pii` in scope. One toggle covers documents and code.

## Approach

**Approach 1 (chosen) — central redaction helper over blob text.** Add a
`redact_code_text(text, &PiiConfig) -> (String, Vec<String>)` in `src/extract/pii.rs` that runs
GLiNER2-Guardrails + Presidio regex over arbitrary text. Wire it into `process_file` right after
tree-sitter extraction, redacting the textual fields of `l1`/`l2`/`l3` structs *before* the blobs
are serialized. At MCP response time, re-run the same helper over the strings we serialize.

Rejected alternatives:
- *Approach 2 (redact raw source pre-parse):* corrupts parse trees, invalidates byte offsets
  used by `find_references`/`blame`.
- *Approach 3 (MCP-only response filter):* leaves on-disk blobs leakable; contradicts the
  "both" decision and the legal-safety goal.

## Design sections

### Section 1 — Detection layer

Two cooperating detectors, both behind the `pii` feature:

1. **GLiNER2-Guardrails** (`fastino/GLiNER2-Guardrails-PII-Multi`) via the existing `pii-candle`
   crate — persons, orgs, locations, and secret-style classes.
2. **Presidio-style regex pass** — new small pure-Rust module `src/extract/pii_regex.rs` for
   patterns GLiNER2 misses: AWS/Google/GitHub keys, `sk-*`, JWT, PEM blocks,
   `KEY=...`/`.env` assignments, emails, IPv4. Returns the same `(start, end, category)` span
   shape as the candle model.

Both feed `redact_code_text`, which merges spans by offset and applies the existing
`PiiStrategy` (mask / hash / token_replace / drop). **No change to the `PiiConfig` schema.**

### Section 2 — Pipeline application

**At scan time (blob write):** In `src/scanner.rs`'s `process_file`, after `extract::l1`/`l2`/`l3`
produce their structs, call `redact_code_text` over the *textual* fields only:
- `l1`: doc comments, symbol/doc free-text, signature strings, string-literal captures.
- `l2`: call-site display context, inline string literals in call args.
- `l3`: the body text fed to the structural hash.
- **Untouched:** `symbol.name`, `call.callee`, import module names (identifiers stay intact).

Runs only when `config.pii.enabled` and the `pii` feature is compiled. Reuses the cached
`Arc<Gliner2Candle>`; the regex pass is allocation-light, per file.

**At MCP response time (defense-in-depth):** In `src/mcp/helpers*.rs`, before serializing
`outline` / `search_symbols` / `find_references` / `find_callers` results, run `redact_code_text`
over the strings being returned. Covers repos scanned before this feature shipped, no re-scan
needed. Same `PiiConfig`, same strategy.

### Section 3 — Data flow, offsets, error handling

- **Offset safety:** `redact_code_text` operates on already-extracted strings, not the raw file,
  so tree-sitter byte offsets in `l1`/`l2`/`l3` are never disturbed. Each textual field is
  redacted independently; identifiers carrying their own offsets are never passed to the
  redactor → no offset drift.
- **Span merge:** GLiNER2 spans + regex spans concatenated, overlaps resolved (wider/earlier
  span wins), sorted by `start`, then `apply_strategy` splices — reusing the exact logic in
  `src/extract/pii.rs:96`.
- **Failure modes (inherit documents contract):** model unset → no-op; inference error →
  `tracing::warn` + original text returned; `pii` feature off → compile-time no-op. A PII failure
  never drops a file or panics the scanner.
- **Audit trail:** `redact_code_text` returns the `Vec<String>` of detected entity categories.
  The scanner records these alongside the blob, giving an audit log of *what was redacted* —
  satisfying the "audit without leaks" goal.

### Section 4 — Testing & verification

Follows TDD + poly discipline:
- **Unit (`src/extract/pii.rs`):** extend existing tests — code-shaped input (doc comment with
  email, `const TOKEN = "sk-..."`, `KEY=value`, GitHub token). Assert value masked, symbol
  *name* (`TOKEN`) preserved. Add regex-only test (PEM, JWT) proving detection without candle.
- **Scanner smoke (`tests/scan_smoke.rs`):** synthetic repo with a leaking `.rs` file; assert the
  written `l1` blob contains the redaction token and that `find_references`/`search_symbols` on
  the identifier still returns hits.
- **MCP smoke (`tests/mcp_smoke.rs`):** assert `outline` on a leaking fixture redacts the literal
  but preserves the symbol name (covers the response-time path).
- **Harden canary:** planted leaky file in one harden repo; assert redaction fired when `pii`
  enabled, and that `spawn`/`get`/`useState` navigation canaries are unaffected.
- **Poly/CI:** `cargo clippy --workspace -- -D warnings`, `poly lint .`, `cargo test --workspace`
  green. `src/extract/pii_regex.rs` stays under the 1000-line cap.

## Files touched

| File | Change |
|---|---|
| `src/extract/pii.rs` | Add `redact_code_text`; merge regex spans; keep existing tests + add code-shaped tests |
| `src/extract/pii_regex.rs` | **New** — Presidio-style regex detector returning `(start,end,category)` spans |
| `src/scanner.rs` | In `process_file`, redact textual fields of `l1`/`l2`/`l3` when `config.pii.enabled` |
| `src/mcp/helpers*.rs` | Defensive redaction of serialized strings in `outline`/`search_symbols`/`find_references`/`find_callers` |
| `tests/scan_smoke.rs` | Leaking-repo assertion |
| `tests/mcp_smoke.rs` | Outline redaction assertion |
| `tests/harden.rs` | Leaky-file canary + navigation-canary-unaffected assertion |
| `README.md` | Note PII now covers code-map, not just documents |

## Out of scope

- Redacting raw file *contents* before parsing (would corrupt trees).
- Redacting identifiers (breaks navigation/canaries).
- A separate `pii.code` config sub-tree (reuse `ConfigV1.pii` as-is).

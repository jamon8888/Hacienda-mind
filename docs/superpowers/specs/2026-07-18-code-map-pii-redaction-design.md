# RGPD-Ready MCP — Code-Map + Document + Git + Web PII/Secret Redaction

**Date:** 2026-07-18
**Status:** Approved (design)
**Feature flag:** `pii`
**Goal:** Make the hacienda-mcp MCP server safe to feed to Claude Desktop / ChatGPT Desktop
(via MCP plugins) for **legal/audit workflows** without leaking secrets (API keys, tokens) or
personal data (names, emails, orgs, locations, author identities) — achieving RGPD/GDPR readiness
across every egress surface the desktop agents can read.

## Background

The PII engine today is **documents-only** and partial:

- `redact_pii(text, &PiiConfig) -> (String, Vec<String>)` lives in `src/extract/pii.rs`.
- Called from two places in `src/extract/doc.rs` (lines 411, 466) — the xberg document pipeline.
- `scanner.rs:969` passes `&config.pii` only into `extract_and_persist_doc` (document tier).
- The code-map pipeline (`src/scanner.rs` rayon loop → `process_file` → `extract::l1`/`l2`/`l3`)
  never calls `redact_pii`. `src/mcp/` has zero PII references.
- Symbols, call sites, outlines, doc comments, and string literals in code are stored and
  served **unredacted**.

An investigation of the full MCP surface (2026-07-18) found that, even where documents are
redacted, the redaction is **OFF by default**, **silent no-ops** when the model is missing, covers
only `person/organization/location` (not email/phone/date/url), and the `entities`/`summary`/
`metadata` fields **re-surface** the PII. Meanwhile several high-risk surfaces are entirely
unredacted: **git author email + full patches** (`tools_git.rs:36/524/707`), **web ingestion**
(`src/web/ingest.rs:39` never redacts), **memory/comms/shell** outputs, **telemetry logging PII**,
and there is **no at-rest encryption, no right-to-erasure, no retention TTL**.

GLiNER2 is code-friendly, and `fastino/GLiNER2-Guardrails-PII-Multi` labels secret-style classes
(API keys, tokens, credentials) as PII alongside person/org/location. Detection is strengthened
with a Presidio-style regex layer for patterns the model misses.

## Decisions (from brainstorming + RGPD review)

1. **Detection coverage:** GLiNER2-Guardrails model **+** Presidio-style regex. Covers
   persons/orgs/locations **and** secrets (API keys, tokens, JWT, PEM, `.env` assignments, emails,
   phones, IPs, dates).
2. **Where:** **Both** scan time (blobs on disk) and MCP response time (defense-in-depth for
   already-scanned repos). Disk must be clean for legal safety; response-time pass covers repos
   scanned before this feature shipped.
3. **Fidelity/safety tradeoff (code-map):** redact only safe textual fields; keep identifiers
   (`symbol.name`, `call.callee`, import module names) intact so `find_references`,
   `search_symbols`, and harden canaries keep working. The literal *values* that create legal
   exposure are what get redacted.
4. **Performance gating:** reuse the existing `pii` cargo feature + `config.pii.enabled` toggle.
   Zero cost by default; reuses the cached `Arc<Gliner2Candle>` model.
5. **Config surface:** reuse `ConfigV1.pii` (`PiiConfig`) as-is. One toggle covers documents,
   code, git, and web.
6. **Fail-loud:** when `pii.enabled = true` but the model is missing/unloadable, emit a clear
   warning **and** mark the scan/response so the operator knows redaction did NOT happen — no
   silent no-op.
7. **RGPD infrastructure:** add at-rest encryption of `.hacienda-mcp/`, subject-scoped erasure +
   retention TTL, and telemetry param scrubbing — scoped as infrastructure work in Section 5.

## Approach

**Central redaction helper over blob + response text.** Add `redact_code_text(text, &PiiConfig)`
in `src/extract/pii.rs` that runs GLiNER2-Guardrails + Presidio regex over arbitrary text. Wire it
into every redaction point: scanner blob-write (code-map + documents + git-derived text + web
ingest), and the MCP response-serialization path for every tool that returns potentially-leaking
text. Extend `PiiConfig` categories so email/phone/date/url are redacted, and add the RGPD
infrastructure (encryption, erasure, telemetry scrub).

Rejected alternatives:
- *Redact raw source pre-parse:* corrupts parse trees, invalidates byte offsets.
- *MCP-only response filter:* leaves on-disk blobs leakable; contradicts legal-safety goal.
- *Separate per-surface config:* one `pii` toggle is simpler and consistent.

## Design sections

### Section 1 — Detection layer

Two cooperating detectors, both behind the `pii` feature:

1. **GLiNER2-Guardrails** (`fastino/GLiNER2-Guardrails-PII-Multi`) via the existing `pii-candle`
   crate — persons, orgs, locations, and secret-style classes.
2. **Presidio-style regex pass** — new small pure-Rust module `src/extract/pii_regex.rs` for
   patterns GLiNER2 misses: AWS/Google/GitHub keys, `sk-*`, JWT, PEM blocks,
   `KEY=...`/`.env` assignments, emails, phones, IPv4, dates. Returns the same
   `(start, end, category)` span shape as the candle model.

Both feed `redact_code_text`, which merges spans by offset and applies the existing `PiiStrategy`
(mask / hash / token_replace / drop). **`PiiConfig` is extended** so `categories` defaults include
`email`, `phone`, `date`, `url` in addition to `person`/`organization`/`location` (the guard model
already emits these; the regex pass covers the rest).

### Section 2 — Pipeline application (scan time + response time)

**Code-map (blob write):** In `src/scanner.rs`'s `process_file`, after `extract::l1`/`l2`/`l3`,
call `redact_code_text` over textual fields only: `l1` doc comments / signatures / string literals;
`l2` call-site context / inline string literals; `l3` body text fed to the structural hash.
Untouched: `symbol.name`, `call.callee`, import module names.

**Documents (already partially wired):** keep `doc.rs:411/466` redaction; **also** redact
`entities`/`summary`/`metadata` so detected PII is not re-surfaced. Extend categories (Section 1).

**Git-derived text:** redact author-display text and commit-body/patch text before it is returned
by `search_git_history`, `diff_file`, `blame_file`, `blame_symbol`, `symbol_history`,
`recent_changes`, `commits_touching`. Author **email** is redacted (consent-gated flag
`pii.redact_git_identity` defaulting on for legal contexts). Line-range attribution to a redacted
author identity remains (so `blame` still works structurally) but the email/name is masked.

**Web ingest:** add `redact_pii` call inside `src/web/ingest.rs:39 index_page` before chunk text
is stored in LanceDB — closes the third-party-data bypass.

**MCP response time (defense-in-depth):** in `src/mcp/helpers*.rs`, run `redact_code_text` over the
strings serialized by **every** tool that can leak. Tools and their feature gates:

- **Default build (always present):** `outline`, `search_symbols`, `find_references`,
  `find_callers`, `workspace_grep` (matched lines + context), and the git tools
  (`search_git_history`, `diff_file`, `blame_file`, `blame_symbol`, `symbol_history`,
  `recent_changes`, `commits_touching`).
- **`documents`/`memory` feature:** `search_documents` (chunk text + entities + summary),
  `memory_get`/`memory_search` (values), `expand` (source body, `tools_compress.rs`),
  `get_chunk` (source body, `tools_code.rs`).
- **`comms` feature (`--features comms`):** `message_get` (thread bodies). In-scope only when the
  comms feature is built; the redaction hook is added conditionally.

Covers repos scanned before this feature shipped; same `PiiConfig`, same strategy.

### Section 3 — Data flow, offsets, error handling

- **Offset safety:** `redact_code_text` operates on already-extracted strings, not the raw file,
  so tree-sitter byte offsets in `l1`/`l2`/`l3` are never disturbed. Identifiers carrying their own
  offsets are never passed to the redactor → no offset drift.
- **Span merge:** GLiNER2 spans + regex spans concatenated, overlaps resolved (wider/earlier span
  wins), sorted by `start`, then `apply_strategy` splices — reusing the logic in
  `src/extract/pii.rs:96`.
- **Failure modes:**
  - `pii` feature off → compile-time no-op.
  - `enabled = false` → no-op (expected).
  - `enabled = true`, model missing/unloadable → **fail-loud**: `tracing::warn` + set a
    `redaction_failed` flag on the scan result / response metadata so the operator is told
    redaction did NOT occur. No silent pass-through.
  - inference error → `tracing::warn` + fail-loud flag (not silent original-text return).
  - A PII failure never drops a file or panics the scanner.
- **Audit trail:** `redact_code_text` returns the `Vec<String>` of detected categories. The scanner
  records these alongside the blob; git/web/memory redactions log the same, giving an audit log of
  *what was redacted* — satisfying "audit without leaks".

### Section 4 — Testing & verification

- **Unit (`src/extract/pii.rs` + `pii_regex.rs`):** code-shaped input (doc comment w/ email,
  `const TOKEN = "sk-..."`, `KEY=value`, GitHub token, PEM, JWT, phone, date). Assert value masked,
  symbol *name* preserved. Regex-only test (PEM/JWT) proving detection without candle.
- **Documents:** assert `entities`/`summary`/`metadata` redacted when PII on.
- **Git:** assert `search_git_history`/`blame_*` responses have email/name masked but structure
  intact.
- **Web:** `index_page` unit test asserts scraped PII not stored verbatim.
- **Scanner smoke (`tests/scan_smoke.rs`):** synthetic leaking repo; assert `l1` blob redacted and
  `find_references`/`search_symbols` on identifier still hits.
- **MCP smoke (`tests/mcp_smoke.rs`):** assert `outline`/`workspace_grep`/`expand` redact literal
  but preserve symbol name; assert `memory_get`/`message_get` redact.
- **Harden canary:** planted leaky file; assert redaction fired when `pii` enabled; navigation
  canaries (`spawn`/`get`/`useState`) unaffected.
- **Poly/CI:** `cargo clippy --workspace -- -D warnings`, `poly lint .`, `cargo test --workspace`
  green. `src/extract/pii_regex.rs` under 1000-line cap.

### Section 5 — RGPD infrastructure (encryption, erasure, telemetry)

Scoped as infrastructure work; threat-model before implementation.

**Section 5 is DEFERRED (P3) — not part of the P1/P2 build plan.** It is documented here for
completeness and threat-modeled in its own later spec. The concrete file changes below are
intentionally NOT in the in-scope "Files touched" table; they belong to the P3 plan.

- **At-rest encryption (Art. 32):** encrypt `.hacienda-mcp/` contents (blobs, Fjall, LanceDB,
  telemetry) with a key from `pii.encryption_key` (env/`SecretString`, never on disk). The msgpack
  write path in `src/store.rs` and the LanceDB write path in `src/scanner_docs.rs` / `src/mcp/memory.rs`
  gain an encrypt step; reads decrypt. Key rotation support.
- **Right to erasure + retention (Art. 17 / Art. 5(1)(e)):** `cache_erase_subject` admin tool —
  scans blobs/LanceDB/telemetry for a data subject (by redaction category or provided identifier)
  and purges matches; retention TTL auto-expires blobs. Replaces orphan-only `cache_gc`.
- **Telemetry scrubbing (Art. 30 / 5(1)(f)):** scrub `params` of PII before `record_call`
  (`helpers_telemetry.rs:48`); restrict `telemetry_summary` (`tools_admin.rs:57`) behind a
  capability flag; encrypt `telemetry.jsonl`.

### Section 6 — Secret-config preservation

`ApiKey`/`SecretString` redact on serialization (`src/config/documents.rs:528-591`). All P2/P3
work must preserve this: credentials never leak via config round-trips or tool responses.

## Files touched

| File | Change |
|---|---|
| `src/extract/pii.rs` | Add `redact_code_text`; merge regex spans; extend `PiiConfig` categories; fail-loud flag |
| `src/extract/pii_regex.rs` | **New** — Presidio-style regex detector → `(start,end,category)` spans |
| `src/config/pii.rs` | Extend default `categories` (email/phone/date/url); add `redact_git_identity`; add encryption key field |
| `src/scanner.rs` | `process_file`: redact textual fields of `l1`/`l2`/`l3` when `pii.enabled` |
| `src/extract/doc.rs` | Redact `entities`/`summary`/`metadata` too |
| `src/web/ingest.rs` | `index_page`: redact chunk text before LanceDB store |
| `src/git/*` + `src/mcp/tools_git.rs` | Redact author email/name + patch/body text (consent-gated) |
| `src/mcp/helpers*.rs` | Defensive redaction of serialized strings in all leaking tools (default-build + documents/memory + comms-gated, per Section 2) |
| `src/store.rs` | Blob write path hook for deferred P3 encryption (see Section 5) |
| `src/scanner_docs.rs` / `src/mcp/memory.rs` | LanceDB write path hook for deferred P3 encryption (see Section 5) |
| `src/mcp/tools_admin.rs` | `cache_erase_subject` tool (P3, Section 5); restrict `telemetry_summary` (P3) |
| `src/mcp/helpers_telemetry.rs` | Scrub `params` PII before logging (P3, Section 5) |

## Out of scope (deferred to P3 threat-modeling — see Section 5)

- Re-architecting the global machine-shared blob store for per-subject isolation (addressed via
  encryption + erasure in Section 5 instead).
- Redacting identifiers (breaks navigation/canaries).
- A separate `pii.code` config sub-tree (reuse `ConfigV1.pii` as-is).

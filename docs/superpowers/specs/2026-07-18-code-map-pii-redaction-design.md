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
   `search_symbols`, and harden canaries keep working. The literal _values_ that create legal
   exposure are what get redacted.
4. **Performance gating:** reuse the existing `pii` cargo feature + `config.pii.enabled` toggle.
   Zero cost by default; reuses the cached `Arc<Gliner2Candle>` model.
5. **Config surface:** reuse `ConfigV1.pii` (`PiiConfig`) as-is. One toggle covers documents,
   code, git, and web.
6. **Fail-loud:** when `pii.enabled = true` but the model is missing/unloadable, emit a clear
   warning **and** mark the scan/response so the operator knows redaction did NOT happen — no
   silent no-op.
7. **RGPD infrastructure:** add at-rest encryption of `.hacienda-mcp/`, subject-scoped erasure +
   retention TTL, and telemetry param scrubbing — scoped as infrastructure work in Section 6.

## Approach

**Central redaction helper over blob + response text.** Add `redact_code_text(text, &PiiConfig)`
in `src/extract/pii.rs` that runs GLiNER2-Guardrails + Presidio regex over arbitrary text. Wire it
into every redaction point: scanner blob-write (code-map + documents + git-derived text + web
ingest), and the MCP response-serialization path for every tool that returns potentially-leaking
text. Extend `PiiConfig` categories so email/phone/date/url are redacted, and add the RGPD
infrastructure (encryption, erasure, telemetry scrub).

Rejected alternatives:

- _Redact raw source pre-parse:_ corrupts parse trees, invalidates byte offsets.
- _MCP-only response filter:_ leaves on-disk blobs leakable; contradicts legal-safety goal.
- _Separate per-surface config:_ one `pii` toggle is simpler and consistent.

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

### Section 3 — Data flow, offsets, error handling, and VERIFIABILITY

The redaction must be **truthful**, not a silent best-effort. Today every skip path returns the
original text with no machine-readable signal (investigation 2026-07-18: no `redaction_applied`
field anywhere; the candle `entities` vec is discarded at `.0` in `doc.rs:411/466`; the model-load
`OnceLock` is private). A tool that reports "redacted" against today's code would be **lying**. So
this section adds the primitives that make the user-facing tools (Section 4) verifiable.

- **Offset safety:** `redact_code_text` operates on already-extracted strings, not the raw file,
  so tree-sitter byte offsets in `l1`/`l2`/`l3` are never disturbed. Identifiers carrying their own
  offsets are never passed to the redactor → no offset drift.
- **Span merge:** GLiNER2 spans + regex spans concatenated, overlaps resolved (wider/earlier span
  wins), sorted by `start`, then `apply_strategy` splices — reusing the logic in
  `src/extract/pii.rs:96`.
- **Failure modes → machine-readable, never silent:**
  - `pii` feature off → compile-time no-op; `redact_code_text` returns
    `(text, vec![], RedactionState::DisabledFeature)`.
  - `enabled = false` → `(text, vec![], RedactionState::DisabledConfig)`.
  - `enabled = true`, model missing/unloadable → `RedactionState::InactiveModelMissing(reason)`; the
    caller is told redaction did NOT occur (no silent pass-through). `tracing::warn` retained.
  - inference error → `RedactionState::Failed(reason)`, original text returned, flag set.
  - A PII failure never drops a file or panics the scanner.
  - `RedactionState` is an enum persisted on the blob + echoed in MCP response metadata, so a client
    can distinguish "redacted" from "not redacted (and why)".
- **Persisted detection record (prerequisite for audit + erasure):** `redact_code_text` returns
  `(String, Vec<DetectedEntity>, RedactionState)` where `DetectedEntity { category, count }`. The
  scanner writes the per-blob `DetectedEntity` tally + `RedactionState` into the blob header (a new
  field on `FileMapL1`/`L2`/`L3` and `FileMapDoc`/chunk). A new Fjall keyspace `entities_by_category`
  indexes `(category, value-hash) → blob-refs`, so "what was redacted" is queryable and "erase this
  subject" is O(matches) not O(all data). This keyspace is the foundation for `pii_audit_report`
  and `cache_erase_subject`.
- **Client-verifiable attestation:** alongside each redacted blob, store
  `attestation = HMAC(key, redacted_text ‖ model_id ‖ config_hash)` (key from `pii.encryption_key`,
  P3 consumer; until encryption lands, a plain SHA over `text ‖ model_id ‖ config_hash` is stored).
  MCP responses that return redacted text echo `model_id` + `config_hash` + the recomputed SHA so a
  **client can independently confirm** the text was produced by the claimed config — closing the
  "client must blindly trust the server" gap (investigation point 3). The client cannot recompute
  the HMAC without the key, but it CAN verify the server's claim is self-consistent and reject
  payloads whose `model_id`/`config_hash` don't match the operator-declared policy.
- **Honesty constraint (non-negotiable):** redaction is a **0.5-threshold, category-limited ML guess
  - regex**, not a guarantee. No tool description or response may imply completeness. Tool
    descriptions MUST state: "best-effort; some PII may remain; verify before trusting."

### Section 4 — User-facing PII MCP tools (truthful by construction)

All tools are `read_only_hint = true`, `open_world_hint = false` (matching the repo's existing
read-only tool convention in `tools_git.rs`/`tools_memory.rs`), except `cache_erase_subject`
(`destructiveHint = true`, deferred to P3). None of these tools can lie because they read the
persisted `RedactionState` / `DetectedEntity` / attestation primitives from Section 3 — never a
fresh unverified claim.

- **`pii_status`** — returns the _reconciled_ truth, not static config:
  `{ enabled_config: bool, model_status: Loaded|Disabled|ModelDirUnset|LoadFailed(String),
active: bool, model_id, categories, threshold, last_scan_redaction_state }`. `active` is true only
  when the model actually loaded; this surfaces config-vs-runtime drift (investigation point 6) so
  the tool can never claim "redacting" when the model is missing.
- **`pii_redact`** — takes `text`, returns `{ redacted_text, entities: Vec<DetectedEntity>,
state: RedactionState, attestation }`. Pure ad-hoc pass (no persistence). Description states
  best-effort / may remain. Useful for auditing a snippet before pasting it elsewhere.
- **`pii_audit_report`** — reads the `entities_by_category` keyspace + blob headers; returns per-
  category counts and which files/blobs were redacted, across the index. Buildable only because
  Section 3 persists the detection record. Returns `state` coverage so it can say "N files scanned
  under redaction, M skipped (model missing)".
- **`cache_erase_subject`** (P3, `destructiveHint = true`) — given a `category` + `value` (or
  value-hash), looks up `entities_by_category`, purges matching blob refs, and (where the redaction
  is `Drop`/`Hash`) rewrites the blob without the span. Feasible only because Section 3 adds the
  entity index; today it would be O(all data) and incomplete (investigation point 5). Returns a
  verified count of removed references.

### Section 5 — Testing & verification

- **Unit (`src/extract/pii.rs` + `pii_regex.rs`):** code-shaped input (doc comment w/ email,
  `const TOKEN = "sk-..."`, `KEY=value`, GitHub token, PEM, JWT, phone, date). Assert value masked,
  symbol _name_ preserved, `RedactionState::Redacted` returned, `DetectedEntity` tally correct.
  Regex-only test (PEM/JWT) proving detection without candle. Assert each failure mode returns the
  correct `RedactionState` (not silent).
- **Verifiability tests:** assert `attestation` recomputes to the stored SHA for a redacted blob;
  assert `pii_status.active == false` when model missing despite `enabled = true`; assert
  `pii_audit_report` counts match the persisted `DetectedEntity` tallies.
- **Documents:** assert `entities`/`summary`/`metadata` redacted when PII on.
- **Git:** assert `search_git_history`/`blame_*` responses have email/name masked but structure
  intact.
- **Web:** `index_page` unit test asserts scraped PII not stored verbatim.
- **Scanner smoke (`tests/scan_smoke.rs`):** synthetic leaking repo; assert `l1` blob redacted and
  `find_references`/`search_symbols` on identifier still hits; assert blob header carries
  `RedactionState` + attestation.
- **MCP smoke (`tests/mcp_smoke.rs`):** assert `outline`/`workspace_grep`/`expand` redact literal
  but preserve symbol name; assert `memory_get`/`message_get` redact; assert `pii_status` reports
  `active` truthfully; assert `pii_redact` echoes attestation.
- **Harden canary:** planted leaky file; assert redaction fired when `pii` enabled; navigation
  canaries (`spawn`/`get`/`useState`) unaffected.
- **Poly/CI:** `cargo clippy --workspace -- -D warnings`, `poly lint .`, `cargo test --workspace`
  green. `src/extract/pii_regex.rs` under 1000-line cap.

### Section 6 — RGPD infrastructure (encryption, erasure, telemetry)

Scoped as infrastructure work; threat-model before implementation.

**Section 6 is DEFERRED (P3) — not part of the P1/P2 build plan.** It is documented here for
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

### Section 7 — Secret-config preservation

`ApiKey`/`SecretString` redact on serialization (`src/config/documents.rs:528-591`). All P2/P3
work must preserve this: credentials never leak via config round-trips or tool responses.

## Files touched

| File                                        | Change                                                                                                                                                                      |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/extract/pii.rs`                        | Add `redact_code_text` → `(String, Vec<DetectedEntity>, RedactionState)`; merge regex spans; extend `PiiConfig` categories; public `model_status()` probe; attestation hash |
| `src/extract/pii_regex.rs`                  | **New** — Presidio-style regex detector → `(start,end,category)` spans                                                                                                      |
| `src/config/pii.rs`                         | Extend default `categories` (email/phone/date/url); add `redact_git_identity`; add `encryption_key` field (P3 consumer)                                                     |
| `src/store.rs`                              | Blob header gains `RedactionState` + `DetectedEntity` tally + attestation; new Fjall `entities_by_category` keyspace (Section 3)                                            |
| `src/extract/mod.rs`                        | `FileMapL1`/`L2`/`L3` gain `RedactionState` + entity tally fields                                                                                                           |
| `src/scanner.rs`                            | `process_file`: redact textual fields of `l1`/`l2`/`l3` when `pii.enabled`; persist state + attestation                                                                     |
| `src/extract/doc.rs`                        | Redact `entities`/`summary`/`metadata`; persist state + attestation on `FileMapDoc`/chunk                                                                                   |
| `src/web/ingest.rs`                         | `index_page`: redact chunk text before LanceDB store; persist state                                                                                                         |
| `src/git/*` + `src/mcp/tools_git.rs`        | Redact author email/name + patch/body text (consent-gated); persist state                                                                                                   |
| `src/mcp/helpers*.rs`                       | Defensive redaction of serialized strings in all leaking tools (default-build + documents/memory + comms-gated, per Section 2); echo attestation in responses               |
| `src/mcp/tools_pii.rs`                      | **New** — `pii_status`, `pii_redact`, `pii_audit_report` (P1/P2, read-only); `cache_erase_subject` shim deferring to P3                                                     |
| `src/store.rs`                              | Blob write path hook for deferred P3 encryption (see Section 6)                                                                                                             |
| `src/scanner_docs.rs` / `src/mcp/memory.rs` | LanceDB write path hook for deferred P3 encryption (see Section 6)                                                                                                          |
| `src/mcp/tools_admin.rs`                    | `cache_erase_subject` tool (P3, Section 6); restrict `telemetry_summary` (P3)                                                                                               |
| `src/mcp/helpers_telemetry.rs`              | Scrub `params` PII before logging (P3, Section 6)                                                                                                                           |

## Out of scope (deferred to P3 threat-modeling — see Section 6)

- Re-architecting the global machine-shared blob store for per-subject isolation (addressed via
  encryption + erasure in Section 5 instead).
- Redacting identifiers (breaks navigation/canaries).
- A separate `pii.code` config sub-tree (reuse `ConfigV1.pii` as-is).

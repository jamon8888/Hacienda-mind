---
name: bm-init
description: Onboard (or refresh) hacienda-mcp in this repo — write basemind.toml, gitignore the cache, and inject a "prefer hacienda-mcp over grep/read/git" rules block into CLAUDE.md / AGENTS.md / ai-rulez.
argument-hint: [capabilities…]
---

# bm-init — onboard hacienda-mcp into this repo

Run `hacienda-mcp init` so the repo has a committed `basemind.toml`, a gitignored `.hacienda-mcp/` cache,
and a rules block that tells every agent to prefer hacienda-mcp's MCP tools over grep, file reads, and
naked `git`. CLI and slash command share ONE implementation — this just drives `hacienda-mcp init`
with the right non-interactive flags.

## When to use

First time setting up hacienda-mcp in a repo, or to refresh the rules block after enabling new
capabilities (documents/RAG, agent-comms, semantic search). Safe to re-run: it's idempotent.

## How to use

1. **Ask which capabilities matter** (one short question). The options are:
   `code-search-navigation`, `code-mapping-architecture`, `git-history`, `agent-comms`,
   `documents-rag`, `semantic-search`. If the user has no preference, enable all.

2. **Run `hacienda-mcp init` non-interactively** with the matching flags. Enable everything:

   ```sh
   hacienda-mcp init --yes
   ```

   Narrow to a subset with repeatable `--with` (allow-list) or `--without` (subtract):

   ```sh
   hacienda-mcp init --yes --with code-search-navigation --with git-history
   ```

   Steer where the rules land with `--rules-target <auto|claude|agents|ai-rulez|none>` (default
   `auto`). Preview without writing using `--print`.

3. **Report what changed** — which files were written or kept (`basemind.toml`, `.gitignore`, the
   rules file), and whether the delimited block was created or updated in place.

## Notes

- Source-of-truth detection (auto): `.ai-rulez/config.toml` present → writes
  `.ai-rulez/rules/basemind-usage.md` (then tell the user to run `ai-rulez generate`; do NOT run
  it for them). Else CLAUDE.md → AGENTS.md → create CLAUDE.md, wrapping the content in an
  idempotent `<!-- BEGIN hacienda-mcp … -->` … `<!-- END hacienda-mcp -->` block that is replaced in
  place on re-run, never duplicated. Content outside the markers is never touched.
- An existing `basemind.toml` is kept verbatim, never clobbered.
- If `hacienda-mcp` isn't on `PATH`: use the plugin-managed cache binary or build a dev binary with
  `cargo build --release` and use `./target/release/hacienda-mcp`.

## See also

The `bm-scan` command to build the index next, and the `hacienda-mcp` skill for the full MCP tool
surface the rules block advertises.

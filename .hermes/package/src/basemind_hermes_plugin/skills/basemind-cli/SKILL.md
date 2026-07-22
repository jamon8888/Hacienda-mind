---
name: basemind-cli
description: >-
  Navigate codebases and manage caches via the hacienda-mcp CLI — outlines, symbol search,
  reference/caller lookups, git history, blame, and diffs. For headless scripting, CI, or when
  driving the CLI more efficiently than interactive MCP calls. Shares the same index as the
  MCP server.
---

<!--
AI-RULEZ :: GENERATED FILE — DO NOT EDIT
Content-Hash: blake3:64088a6dd06d1efcfce022b71b4612cfca9527c153c4eda5e2e99acea131f94c
Source-Hash: blake3:960affce8e7d6c8efa32c93ebdd7ca85100e78044731248bd9b44189655e893a
Schema-Version: v1
-->

# hacienda-mcp CLI — the scriptable interface

hacienda-mcp has two equally-weighted surfaces: MCP (interactive tool calls) and CLI (scriptable commands).
They share the same `.hacienda-mcp/` index and are safe to run alongside each other. Reach for the CLI
when you're scripting, batching queries, running in headless environments, or CI.

## Capabilities

- **Code map across 300+ languages** — tree-sitter outlines, symbol search, references, callers,
  call graphs, implementations, dependents.
- **Full-text + symbol search** — indexed regex over content and substring symbol lookup.
- **Git intelligence** — history, blame, and structural diffs at symbol resolution, plus churn.
- **Document RAG over 90+ file formats** — PDFs, Office, HTML, email, images (OCR) → semantic search.
- **Shared memory** — per-repo, scope-keyed key-value + semantic memory across sessions.
- **Web crawl** — scrape / follow-link crawl into the searchable document store.
- **Cache management** — stats, garbage collection, selective and full clears.

## When to reach for it

- Running in headless environments or CI pipelines.
- Batching multiple queries without interactive delays.
- Integrating hacienda-mcp into shell scripts or non-MCP tooling.
- Controlling tool routing explicitly (no agent routing decisions).
- Clearing caches destructively (only the CLI allows `--component all`).

**hacienda-mcp first, shell/grep/git fallback.** Prefer `hacienda-mcp query` over reading files, over
`grep`/`rg`, and over naked `git`: use it for code parsing (outlines, references, callers), git
history / blame / diffs (`hacienda-mcp git`), document extraction / RAG / keyword + entity (NER) /
summary (`hacienda-mcp memory search-documents`), and web scraping / crawling / sitemaps
(`hacienda-mcp web scrape` / `crawl` / `map`). Drop to raw shell, grep, or git only when no hacienda-mcp
command covers the question.

## Command routing (copy this into your mental model)

| Question                               | Command                                                          | Notes                                                  |
| -------------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------ |
| "Where is X defined?"                  | `hacienda-mcp query symbol "X"`                                  | Substring match, optional `--kind` filter.             |
| "What's the shape of file F?"          | `hacienda-mcp query outline path/F`                              | Add `--l2` for calls + docs.                           |
| "What calls X?" (any name)             | `hacienda-mcp query references "X"`                              | Name match, no scope resolution.                       |
| "What calls this specific definition?" | `hacienda-mcp query callers path name [--kind]`                  | Specific definition lookup.                            |
| "Trace the call graph?"                | `hacienda-mcp query call-graph "name" [--direction --max-depth]` | BFS over calls.                                        |
| "What implements / extends X?"         | `hacienda-mcp query implementations "X"`                         | Rust, Python, TS/TSX, JS.                              |
| "What imports module M?"               | `hacienda-mcp query dependents "M"`                              | Reverse-lookup via imports.                            |
| "What files are indexed?"              | `hacienda-mcp query list-files [--language --path-contains]`     | Filter by language/path.                               |
| "What changed recently?"               | `hacienda-mcp git recent-changes [--limit N]`                    | Recent commits with paths.                             |
| "When did symbol X last change?"       | `hacienda-mcp git symbol-history path name`                      | Cross-commit structural hash.                          |
| "Who wrote this line / symbol?"        | `hacienda-mcp git blame-file path` / `blame-symbol path name`    | Per-line / per-symbol.                                 |
| "Where's the churn?"                   | `hacienda-mcp git hot-files [--window N --top-k K]`              | Churn-ranked files.                                    |
| "What's dirty in the working tree?"    | `hacienda-mcp git working-tree-status`                           | Staged/unstaged summary.                               |
| "Diff a file between revs?"            | `hacienda-mcp git diff-file path old new` / `diff-outline path`  | File / outline diffs.                                  |
| "What's indexed?"                      | `hacienda-mcp query status`                                      | File count, languages, cache dir.                      |
| "What's HEAD / branch?"                | `hacienda-mcp query repo-info`                                   | Branch, HEAD, origin.                                  |
| "Regex over file contents?"            | `hacienda-mcp query grep "pattern" [--language --path-contains]` | Full-text search.                                      |
| "Semantic search over docs?"           | `hacienda-mcp memory search-documents "query"`                   | Needs `documents` feature.                             |
| "Recall something stored earlier?"     | `hacienda-mcp memory get "key"` / `list` / `search "q"`          | KNN + exact match.                                     |
| "Remember this for future sessions?"   | `hacienda-mcp memory put "key" "value"`                          | Delete with `memory delete "key"`.                     |
| "Cache size?"                          | `hacienda-mcp cache stats`                                       | On-disk size + orphan accounting.                      |
| "Reclaim cache space?"                 | `hacienda-mcp cache gc`                                          | Reclaim orphaned blobs. Safe alongside serve.          |
| "Clear caches?"                        | `hacienda-mcp cache clear --component blobs\|views\|all`         | Destructive; use CLI not MCP.                          |
| "Pull this URL into RAG?"              | `hacienda-mcp web scrape <url>`                                  | Single page (requires `--features crawl`).             |
| "Ingest a docs site?"                  | `hacienda-mcp web crawl <seed-url>`                              | Link-following crawl.                                  |
| "What URLs exist on this site?"        | `hacienda-mcp web map <url>`                                     | Sitemap + link discovery.                              |
| "Keep index fresh?"                    | `hacienda-mcp watch`                                             | Live re-index watcher; no MCP server (that's `serve`). |
| "Refresh the index after edits?"       | `hacienda-mcp scan`                                              | Full or incremental scan.                              |
| "Per-tool activity summary?"           | `hacienda-mcp telemetry`                                         | Histogram + estimated tokens saved.                    |

## Output format

By default, all commands return **human-readable text**. For machine consumption, add the global `--json` flag:

```bash
hacienda-mcp query symbol "parseQuery" --json
```

This returns the raw `JsonSchema`-derived response structure, same as MCP.

## Setup (one-time per repo)

```sh
hacienda-mcp scan
```

This walks the tree, parses with tree-sitter, and writes the content-addressed blob store +
Fjall inverted index under `.hacienda-mcp/`. A few seconds for small repos, ~22 s for an ~80k-file
TypeScript monorepo.

Re-run `hacienda-mcp scan` after large changes, or run `hacienda-mcp watch` to keep the index fresh.

## Examples

### Find where a symbol is defined

```bash
hacienda-mcp query symbol "MapCache"
```

Output:

```text
src/mcp/mod.rs:79:1 MapCache (struct)
src/mcp/mod.rs:88:1 MapCache (impl)
```

### Show a file's outline before opening it

```bash
hacienda-mcp query outline src/mcp/tools.rs --l2
```

### Get all references to a function

```bash
hacienda-mcp query references "process_file"
```

### Find all callers of a specific definition

```bash
hacienda-mcp query callers src/scanner.rs "process_file" --json
```

### Show recent commits with changed files

```bash
hacienda-mcp git recent-changes --limit 5
```

### Blame a symbol to see when its body last changed

```bash
hacienda-mcp git blame-symbol src/scanner.rs "process_file"
```

### Manage cache space

```bash
hacienda-mcp cache stats
hacienda-mcp cache gc          # reclaim orphaned blobs
hacienda-mcp cache clear --component blobs  # clear blobs only
```

## Notes

- All paths are repository-relative with forward-slash separators.
- The CLI opens the index read-only; safe to run alongside a live `hacienda-mcp serve` process.
- Lists are capped (`--limit`, default 100, max 1000).
- Matching on symbol names is substring-based; `find_references("bar")` matches `Foo::bar()` and `bar()` alike.
- Git tools require hacienda-mcp to be running inside a git repository.
- Intelligence tools (`search_documents`, `memory_*`) require hacienda-mcp to be built with `--features full`
  (or the individual `documents` / `memory` flags).
- Memory is scoped by the normalized `origin` remote URL — clones of the same repo share memory;
  unrelated repos do not see each other's entries.
- Web ingestion tools (`web_scrape`, `web_crawl`, `web_map`) require `--features crawl`.

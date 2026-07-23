<!-- markdownlint-disable MD033 MD041 -->
<div align="center">

<img src="docs/media/hacienda-mcp-banner.svg" alt="hacienda-mcp — cybernetic core" width="820">

**The context and communication layer for coding agents.**

hacienda-mcp turns any repo into an always-current map of its code, documents, history, and memory —
so agents answer from **structure and search** instead of burning their context window on `grep` and
file reads — and gives a team of agents a **shared channel to coordinate** while they work. One
server does both.

Code map across **300+ languages** · documents in **90+ formats** · semantic + full-text search ·
git history & blame · shared memory · web crawl · agent-to-agent comms

[![Docs](https://img.shields.io/badge/docs-github.com/jamon8888/Hacienda-mind-965aff?style=flat-square)](https://github.com/jamon8888/Hacienda-mind)
[![crates.io](https://img.shields.io/crates/v/hacienda-mcp?style=flat-square)](https://crates.io/crates/hacienda-mcp)
[![npm](https://img.shields.io/npm/v/hacienda-mcp?style=flat-square)](https://www.npmjs.com/package/hacienda-mcp)
[![PyPI](https://img.shields.io/pypi/v/hacienda-mcp?style=flat-square)](https://pypi.org/project/basemind/)
[![CI](https://img.shields.io/github/actions/workflow/status/jamon8888/Hacienda-mind/ci.yaml?style=flat-square)](https://github.com/jamon8888/Hacienda-mind/actions/workflows/ci.yaml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)

[Docs](https://github.com/jamon8888/Hacienda-mind) · [Install](#installation) · [Features](#what-you-get) · [How it works](#how-it-works) · [Performance](#performance) · [CLI](#cli-reference)

</div>

---

<!-- markdownlint-disable MD013 -->
<p align="center"><img src="docs/media/mcp-demo.gif" alt="An agent answering from outline + find_references in a live Claude Code session" width="820"></p>
<p align="center"><em>An agent reasoning from structure — <code>outline</code> + <code>find_references</code> in a live session, statusline tracking tokens saved.</em></p>
<!-- markdownlint-enable MD013 -->

<div align="center"><sub><a href="#demos">More demos ↓</a></sub></div>

---

## What you get

hacienda-mcp answers with **file paths, line numbers, and signatures — not whole files** — so a question
about your code costs a small fraction of the tokens it takes to read the source.

<!-- markdownlint-disable MD013 -->

| Capability                     | What it does                                                                                                                                                                                                                                                                                                                                                    | Key tools                                                                                                                                                                            |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Code intelligence**          | Find where things are defined, what calls what, who implements what, how calls chain, and the overall architecture (hub modules + dependency cycles) — across [300+ languages](#how-it-works).                                                                                                                                                                  | `outline` · `search_symbols` · `find_references` · `find_callers` · `goto_definition` · `call_graph` · `architecture_map` · `find_implementations` · `find_files` · `workspace_grep` |
| **Git intelligence**           | Ask what changed recently, who last touched a function, where the churn is, how a file's structure differs across commits, and full-text search commit authors + messages.                                                                                                                                                                                      | `blame_symbol` · `symbol_history` · `recent_changes` · `hot_files` · `diff_outline` · `commits_touching` · `search_git_history`                                                      |
| **Document search**            | Search PDFs, Office files, HTML, email, and images by meaning — with built-in text extraction and OCR, no extra setup.                                                                                                                                                                                                                                          | `search_documents`                                                                                                                                                                   |
| **Code search**                | Find source code by meaning, term, or symbol — `mode` picks the strategy: `hybrid` (default, RRF fusion of vector + BM25 + exact-symbol lanes), `semantic` (vector KNN), or `keyword` (native BM25); optional `rerank` cross-encoder pass. Returns pointers, fetch bodies with `get_chunk`. Needs `--features code-search`.                                     | `search_code` · `get_chunk`                                                                                                                                                          |
| **Shared memory**              | A per-repo memory agents can write to and search; clones of the same repo share it, unrelated repos stay separate.                                                                                                                                                                                                                                              | `memory_put` · `memory_search` · `memory_audit`                                                                                                                                      |
| **Suggestions**                | Spots files that change together and suggests notes worth saving — you approve before anything is kept.                                                                                                                                                                                                                                                         | `proposals_mine` · `proposal_accept`                                                                                                                                                 |
| **Web crawl**                  | Fetch a page or follow links from a starting URL; results join the document search above.                                                                                                                                                                                                                                                                       | `web_scrape` · `web_crawl` · `web_map`                                                                                                                                               |
| **Agent comms**                | Threads for agents working the same repo: each addressed by at least two of subject / path-glob / members, discovered by scope (member, cwd path-match, or subject filter — never global), with a recency-filtered inbox. The creator manages membership and archives; idle threads auto-archive. One orchestrator can drive many named subagents (`as_agent`). | `thread_start` · `thread_post` · `thread_list` · `inbox_read` · `agent_list`                                                                                                         |
| **Agent shells**               | Let agents open, type into, and watch terminal sessions in the background.                                                                                                                                                                                                                                                                                      | `shell_spawn` · `shell_send` · `shell_capture` · `shell_list`                                                                                                                        |
| **Token saving**               | Hand an agent a file's outline instead of its full text, pull back only the one function it needs, diff a re-read instead of resending it whole, checkpoint a session, and flag wasteful tool use.                                                                                                                                                              | `compress` · `expand` · `delta` · `checkpoint` · `detect_waste`                                                                                                                      |
| **PII redaction (NER + LoRA)** | In-process **GLiNER2 + Candle (PEFT LoRA)** model that redacts **PERSON / ORGANIZATION / LOCATION** from extracted document text — the entities xberg's pattern engine can't catch. Local, no external service, fails soft (scan still runs if the model is absent). Needs a `pii` build.                                                                       | `search_documents` (redacted output)                                                                                                                                                 |
| **Admin**                      | Refresh the index, see what's been queried, and check or clean up the on-disk cache.                                                                                                                                                                                                                                                                            | `rescan` · `telemetry_summary` · `cache_stats`                                                                                                                                       |
| **Machine registry**           | Machine-wide repo/worktree/branch coordination, backed by the daemon's always-on registry. Advisory claims let agent sessions avoid colliding on the same worktree.                                                                                                                                                                                             | `workspaces` · `worktrees` · `branches` · `worktree_claim` · `worktree_release`                                                                                                      |

<!-- markdownlint-enable MD013 -->

---

## Installation

Three ways to run hacienda-mcp, easiest first. All three share the same local index and are safe to run
side by side.

> **The plugin downloads the hacienda-mcp program for you** on first use. The MCP-server and CLI paths
> need it installed yourself — see [Install the program](#install-the-program).

### 1. As a plugin (recommended)

The plugin sets up everything for you — the server, the helper skills, the agent-comms features, and
the slash commands. Pick your coding tool.

<details>
<summary><strong>Claude Code</strong></summary>

In the session (not your shell), run in order:

```text
/plugin marketplace add jamon8888/Hacienda-mind
/plugin install hacienda-mcp@hacienda-mcp
```

Restart, then run `/bm-statusline` once to turn on the live statusline (a one-time step — see
[Statusline](#install-the-program)). Recommended: turn on auto-update for the `hacienda-mcp`
marketplace (Claude Code's plugin manager) so you always get the current index format and tool
set — or update it regularly by hand if you'd rather control timing.

</details>

<details>
<summary><strong>Codex</strong></summary>

```bash
codex plugin marketplace add jamon8888/Hacienda-mind
codex plugin add hacienda-mcp@hacienda-mcp
```

In the app: open the **Plugins** sidebar and add hacienda-mcp. The CLI and IDE share one config file.

</details>

<details>
<summary><strong>Cursor</strong></summary>

In Agent chat: `/add-plugin hacienda-mcp` (once listed), or go to **Dashboard → Settings → Plugins →
Team Marketplaces → Import from Repo** and point it at `https://github.com/jamon8888/Hacienda-mind`.

</details>

<details>
<summary><strong>Gemini CLI</strong></summary>

```bash
gemini extensions install https://github.com/jamon8888/Hacienda-mind
```

Update later with `gemini extensions update hacienda-mcp`.

</details>

<details>
<summary><strong>Factory Droid</strong></summary>

```bash
droid plugin marketplace add https://github.com/jamon8888/Hacienda-mind
droid plugin install hacienda-mcp@hacienda-mcp
```

</details>

<details>
<summary><strong>GitHub Copilot CLI</strong></summary>

```bash
copilot plugin marketplace add jamon8888/Hacienda-mind
copilot plugin install hacienda-mcp@hacienda-mcp
```

</details>

<details>
<summary><strong>OpenCode</strong></summary>

Add to `opencode.json` (project) or `~/.config/opencode/opencode.json` (global):

```json
{ "plugin": ["basemind-opencode@latest"] }
```

</details>

<details>
<summary><strong>Kimi Code</strong></summary>

```text
/plugins install https://github.com/jamon8888/Hacienda-mind
```

Kimi doesn't support the comms auto-notifications, but the chat tools still work.

</details>

<details>
<summary><strong>Hermes</strong></summary>

Hermes exposes MCP servers through config, so hacienda-mcp's tools are wired there. Two steps — the
binary + MCP wiring gives you the tools; a small standalone plugin package adds the helper skills,
slash commands, and comms notifications.

First [install the program](#install-the-program) (Homebrew / npm / cargo / release — **not** pip),
then add the server to `~/.hermes/config.yaml` (this is what gives you the 60+ tools):

```yaml
mcp_servers:
  hacienda-mcp:
    command: hacienda-mcp
    args: [serve]
```

For the helper skills, slash commands, and agent-comms notifications, install the standalone plugin
into the same Python environment Hermes runs in, then enable it (general plugins are opt-in):

```bash
pip install basemind-hermes-plugin
hermes plugins enable hacienda-mcp
```

The plugin is pure-Python and ships no binary — it shells out to the `hacienda-mcp` you installed above.
Comms auto-notifications are best-effort; the chat tools work regardless.

</details>

<details>
<summary><strong>Antigravity &amp; pi</strong></summary>

**Antigravity** uses a shared MCP config — [install the program](#install-the-program), then add the
[generic MCP block](#2-as-an-mcp-server). If you already use the Gemini extension,
`agy plugin import gemini` brings it across.

**pi**: `pi install git:github.com/jamon8888/Hacienda-mind`. pi has no MCP support, so hacienda-mcp runs
through its [CLI](#3-as-a-cli) here.

</details>

### 2. As an MCP server

If your tool speaks MCP but you're not using the plugin, [install the program](#install-the-program),
then register it:

```json
{
  "mcpServers": {
    "hacienda-mcp": { "command": "hacienda-mcp", "args": ["serve"] }
  }
}
```

Each tool says whether it only reads or can change things, so your client can auto-approve the safe
ones and ask before the rest. If `hacienda-mcp` isn't found, use the full path from `which hacienda-mcp`.

<details>
<summary><strong>Per-tool specifics</strong> (Claude Code · Cursor · Windsurf · Codex · Gemini · Copilot · Droid · Cline · Continue · OpenCode · Hermes)</summary>

- **Claude Code** — `claude mcp add hacienda-mcp -- hacienda-mcp serve` (add `--scope user` for all
  projects; the `--` is required). Or commit a `.mcp.json` at the repo root with the block above.
- **Cursor** — put the block above in `.cursor/mcp.json` (project) or `~/.cursor/mcp.json` (global).
- **Windsurf** — `~/.codeium/windsurf/mcp_config.json` (or Cascade → MCP servers → manage), then
  **Refresh**.
- **Codex** — `codex mcp add hacienda-mcp -- hacienda-mcp serve`, shared by the CLI and IDE.
- **Gemini CLI** — `gemini mcp add hacienda-mcp hacienda-mcp serve`, or the block above in
  `~/.gemini/settings.json`.
- **GitHub Copilot CLI** — `/mcp add` in-session, or `~/.copilot/mcp-config.json` with
  `"type": "local"` and `"tools": ["*"]`.
- **Factory Droid** — `droid mcp add hacienda-mcp "hacienda-mcp serve"`, or `~/.factory/mcp.json`.
- **Cline** — MCP Servers icon → Configure → add the block above.
- **Continue** — `.continue/mcpServers/hacienda-mcp.yaml` with `command: hacienda-mcp`, `args: [serve]`.
- **OpenCode (without the plugin)** — `opencode.json` under key `mcp`, with `command` as an array
  `["hacienda-mcp", "serve"]`.
- **Hermes** — `mcp_servers.hacienda-mcp` in `~/.hermes/config.yaml` (YAML: `command: hacienda-mcp`,
  `args: [serve]`). For helper skills + comms notifications, `pip install basemind-hermes-plugin`
  (a standalone pure-Python plugin, no binary), then `hermes plugins enable hacienda-mcp` — see the
  Hermes plugin section above.
- **Any other tool** — point it at the command `hacienda-mcp` with the argument `serve`.

</details>

### 3. As a CLI

The standalone program, for scripts, headless runs, and CI. [Install it](#install-the-program), then:

```bash
hacienda-mcp scan                          # index the project once
hacienda-mcp query symbol "parseQuery"     # find a symbol by name
hacienda-mcp query references "processFile" # find everywhere it's called
hacienda-mcp git blame-file src/main.rs    # who last changed each line
hacienda-mcp watch                         # keep the index fresh as files change
```

Full command list in the [CLI reference](#cli-reference).

### Install the program

The MCP and CLI paths need `hacienda-mcp` available on your system. (The plugin does this for you.)

<!-- markdownlint-disable MD013 -->

| Channel         | Command                                                                  | Includes        |
| --------------- | ------------------------------------------------------------------------ | --------------- |
| Homebrew        | `brew install Goldziher/tap/hacienda-mcp`                                | everything      |
| npm             | `npm install -g hacienda-mcp`                                            | everything      |
| pip             | `pip install hacienda-mcp`                                               | everything      |
| cargo           | `cargo install hacienda-mcp --locked`                                    | code + git only |
| cargo (full)    | `cargo install hacienda-mcp --features full --locked`                    | everything      |
| GitHub releases | [Download a binary](https://github.com/jamon8888/Hacienda-mind/releases) | everything      |

<!-- markdownlint-enable MD013 -->

The Homebrew / npm / pip / GitHub downloads include the full feature set — documents, OCR, search,
web crawl, shared memory, agent comms, and agent shells — so the first run downloads the models it
needs. The plain `cargo install` builds the code-map and git tools only.

### Get started

After installing, run **`hacienda-mcp init`** (CLI) — or **`/bm-init`** if your tool supports slash
commands — from the repo root. It's re-runnable and safe to call again later:

- Writes a commented `basemind.toml` scaffold at the repo root, if one doesn't already exist.
- Lets you pick which capabilities to advertise (interactive prompt in a TTY, or non-interactive
  with `--yes`, `--with <capability>`, `--without <capability>`). Capability slugs:
  `code-search-navigation`, `code-mapping-architecture`, `git-history`, `agent-comms`,
  `documents-rag`, `semantic-search`.
- Injects a "prefer hacienda-mcp over grep/read/git" rules block into your repo's agent-instructions
  file — `.ai-rulez/rules/basemind-usage.md` if `.ai-rulez/config.toml` is present (run
  `ai-rulez generate` afterward), else `CLAUDE.md`, else `AGENTS.md`, else a new `CLAUDE.md`. The
  block is delimited (`<!-- BEGIN hacienda-mcp ... -->` / `<!-- END hacienda-mcp -->`) so re-running
  replaces it in place instead of duplicating it.

Preview changes without writing with `--print`; skip the rules step with `--no-rules`; steer the
target explicitly with `--rules-target <auto|claude|agents|ai-rulez|none>`.

<details>
<summary><strong>Statusline</strong> (Claude Code)</summary>

Run `/bm-statusline` once. This is a one-time step because Claude Code doesn't let plugins set the
main statusline themselves — so hacienda-mcp asks the assistant to make the one-line settings change on
your behalf, and it sticks from then on.

It shows two lines:

```text
Opus · hacienda-mcp · ⎇ main · 12% ctx
◆ hacienda-mcp  ●  1,247 files · 23m ago  │  312 calls · 180 srch · 44 git · 12 docs  │  1.4M saved  │  ✉ 3 @reviewer
```

The dot is green when hacienda-mcp is live and fresh, amber when idle, red when stale. The middle shows
activity by type, then tokens saved, then unread messages. Adjust with
`HACIENDA_MCP_STATUSLINE=full|compact|minimal`, or hide the top line with `HACIENDA_MCP_STATUSLINE_CONTEXT=0`.

</details>

---

## Demos

<!-- markdownlint-disable MD013 -->

<p align="center"><img src="docs/media/demo.gif" alt="hacienda-mcp CLI: scan, then symbol / reference / call-graph / blame queries" width="760"></p>
<p align="center"><em>The same engine from the CLI — <code>scan</code>, then symbol / reference / call-graph / blame queries.</em></p>

<p align="center"><img src="docs/media/semantic-demo.gif" alt="Semantic search over the documents store" width="820"></p>
<p align="center"><em>Searching documents by meaning, not keywords, across 90+ formats.</em></p>

<p align="center"><img src="docs/demos/code-review-panel.gif" alt="Three named reviewer agents posting findings to a shared repo room, DMing each other, and an orchestrator synthesizing a verdict over the comms CLI" width="820"></p>
<p align="center"><em>Multi-agent code-review panel: named reviewers coordinate in a repo-scoped comms room (post, direct-message, synthesize) — entirely over <code>hacienda-mcp comms</code>.</em></p>

<!-- markdownlint-enable MD013 -->

---

## How it works

<details>
<summary><strong>From one scan to instant answers</strong></summary>

`hacienda-mcp scan` reads your project once, in parallel. It maps your code with
[tree-sitter] (across [300+ languages][tslp]) and pulls text out of your documents with
[xberg], then saves the result to a global cache under the XDG data directory, keyed by workspace —
nothing is written into your repo. After that, `hacienda-mcp serve` keeps the map in memory and answers
questions instantly — no re-reading the project for each one. When files change, it updates only what
changed. A single background daemon on the machine is the sole writer to that cache, so multiple
`serve` sessions on the same repo (or on different worktrees of it) all read and write concurrently
instead of one falling back read-only. See [Global cache & the daemon](#how-it-works) below.

Navigation is **scope- and import-aware** for JavaScript/TypeScript, **Python, and Java**: instead of
matching references by name, hacienda-mcp resolves each use to the definition it actually binds to, so a
shadowed local isn't confused with an import and `goto_definition` / `find_references` / `find_callers`
report precise, `resolved` results (including across files for imports). Every other language still
gets fast tree-sitter scope binding. Precise Python/Java resolution runs GitHub stack-graphs-style
`.tsg` name-binding rules via an in-tree engine (`crates/`), with no per-language LSP server.

Markdown and Obsidian vaults are first-class: headings become navigable symbols (so `outline` and
`search_symbols` work over a notes vault); `[[wikilinks]]`, `![[embeds]]`, and standard
`[text](Note.md)` links all become references — so `find_references "Note"` returns that note's
backlinks regardless of link style; and `#tags` (inline or in YAML frontmatter) become references
too, so `find_references "#project"` lists every note carrying that tag.

```mermaid
flowchart LR
  A(["Coding agent"])
  R["Your project<br/>code · documents · git"]
  S["hacienda-mcp scan<br/>map code & read documents"]
  D[("Global cache<br/>~/.local/share/hacienda-mcp")]
  V["hacienda-mcp serve<br/>answers questions"]
  R --> S --> D --> V
  A <-->|asks questions| V
  classDef accent fill:#2563eb,stroke:#1e40af,color:#fff
  class S,V accent
```

Search and memory are powered by a vector store ([LanceDB]).

</details>

<details>
<summary><strong>Index lifecycle &amp; freshness</strong></summary>

`hacienda-mcp serve` answers the MCP handshake immediately and warms the code map into memory in the
background, so a client never blocks waiting for a large repo to load. The `status` tool reports
`warming` (still loading) and, once done, `warm_ms`; a first-time index build similarly reports
`indexing` / `index_build_ms`.

While the server isn't fully ready, `status` and every code-map read tool may carry a `notice`
object — `{ state, message, retry }` — instead of (or alongside) their normal result:

| `state`          | Meaning                                                                              | `retry` |
| ---------------- | ------------------------------------------------------------------------------------ | ------- |
| `warming_up`     | Loading an existing index into memory.                                               | `true`  |
| `building_index` | Indexing from scratch (no cache entry for this workspace yet).                       | `true`  |
| `rescanning`     | Incremental rescan after a file change; current results are usable but may be stale. | `false` |

Treat an empty or partial result carrying a `notice` as "retry shortly," not "no matches" — poll
`status` (or just retry the call) until the notice clears.

</details>

<details>
<summary><strong>Global cache &amp; the daemon</strong></summary>

Index state lives under a single global cache — `~/.local/share/hacienda-mcp/` on Linux/macOS
(override with `HACIENDA_MCP_DATA_HOME`) — keyed by workspace, never inside your repo. The
content-addressed blob store is machine-wide too: identical file content scanned from different repos
or worktrees is extracted and stored once.

A single background daemon per machine is the sole writer to that cache. `hacienda-mcp serve` opens its
store read-only and forwards writes (scan / rescan) to the daemon over a local socket, so N `serve`
sessions on the same repo — or on different worktrees of it — all read and write concurrently instead
of a second session silently falling back to a stale, read-only view. The daemon also keeps a cheap,
always-on registry of repos, worktrees, and branches (`workspaces` / `worktrees` / `branches`), and
`worktree_claim` / `worktree_release` give agent sessions an advisory way to avoid colliding on the
same worktree.

`hacienda-mcp statusline` queries the daemon for the workspaces currently active and prints a compact
line for your shell prompt; it prints nothing when no daemon is running.

</details>

<details>
<summary><strong>How agents coordinate</strong></summary>

A single shared service in the background lets agents talk to each other — even across different
tools and different repos on the same machine. Agents coordinate in **threads** — each addressed by
at least two of subject / path-glob / members, discovered by scope rather than joined globally — and
each has a personal **inbox**. Messages come in two parts: a short headline (subject and sender)
that's cheap to skim, and the full body, fetched only when an agent wants to read it. An agent never
sees its own posts in its inbox. Idle threads auto-archive.

The plugin makes sure agents notice messages without being asked — through the built-in instructions,
a notice at session start and each turn, and a quiet background check every few seconds.

```mermaid
flowchart LR
  A["Agent A<br/>Claude Code · repo X"]
  B["Agent B<br/>Cursor · repo Y"]
  BR["Shared comms service<br/>threads · inboxes"]
  A <-->|post · read| BR
  B <-->|post · read| BR
  classDef accent fill:#2563eb,stroke:#1e40af,color:#fff
  class BR accent
```

</details>

<details>
<summary><strong>Agent shells</strong></summary>

Included in every prebuilt download (and in `cargo install --features shells` / `full`): agents can
open terminal sessions in the background, type into them, and read what's on screen — no extra tools
to install. Sessions can be fully headless, or opened in a real terminal tab or window so you can
watch along. A spawned session and the agent that started it can message each other over comms.

</details>

---

## PII redaction (GLiNER2 + Candle + LoRA)

hacienda-mcp can redact **personally identifiable information** from the documents it indexes, using an
**in-process GLiNER2 model backed by Candle** (Rust ML runtime — no Python, no external service) with an
optional **PEFT LoRA adapter** for domain tuning. It runs as a pass over extracted document text, catching
**PERSON / ORGANIZATION / LOCATION** mentions that xberg's pure-Rust pattern redaction engine misses.

```toml
# hacienda-mcp.toml
[pii]
enabled = true
model_dir = "/path/to/gliner2-model"          # tokenizer.json + model.safetensors
# lora_adapter_dir = "/path/to/adapter"       # optional PEFT LoRA adapter
strategy = "mask"                              # mask | hash | token_replace | drop
threshold = 0.5
```

- **Local & private** — the model runs on your machine; nothing leaves the process.
- **Graceful** — if `model_dir` is unset or the model fails to load, the pass is skipped (with a warning) and the scan still completes.
- **Strategy** — `mask` → `[REDACTED]`, `hash` → stable per-span hash, `token_replace` → `«PERSON»` etc., `drop` → remove the span.
- **Build** — needs a `pii` build (`--features pii`). The `pii-candle` crate is vendored under `crates/pii-candle`.

See [Configuration → PII](docs/reference/configuration.md#pii) for every key.

---

## Token saving

<details>
<summary><strong>Good habits the plugin sets up for you</strong></summary>

The plugin nudges agents toward the cheap path by default:

- Get a file's outline before opening it — then read only the part you need.
- Search for a definition instead of grepping for it.
- Look up who calls a function instead of grepping for call sites.
- Refresh the index after edits instead of restarting the server.
- Don't re-read a file hacienda-mcp already mapped.

Optional guardrails enforce this at the moment a tool is used:

- **Guard** — gently redirects `grep`-style searches to the matching hacienda-mcp tool. On by default;
  set `HACIENDA_MCP_GUARD=off` to disable, or `redirect` to block instead of nudge.
- **Output compressor** — `HACIENDA_MCP_COMPRESS_OUTPUT=1` shrinks long command output. It never touches
  anything that looks like a credential and leaves output alone if it can't help.
- **Re-read shortcut** — `HACIENDA_MCP_DELTA_READS=1` shows just what changed when an agent re-reads a
  file it already read this session.

</details>

<details>
<summary><strong>Compression that understands code</strong></summary>

hacienda-mcp shrinks code by keeping the shape and dropping the bodies — function signatures and imports
stay, the implementations go — because a signature is useless without its shape. For prose it does a
light cleanup (extra whitespace, filler, repeated paragraphs). It reports honest before/after token
counts, and the code version is exact — nothing is lost, just set aside. `expand` brings any one
function's full body back when an agent actually needs it: compress to an outline, expand only what
you need.

</details>

---

## Performance

<details>
<summary><strong>Scan speed</strong></summary>

Measured on an Apple M4 (10 cores — 4 performance + 6 efficiency, 16 GB, macOS 26) with the
hardening harness (`scripts/harden.sh`), which clones each upstream repo fresh and scans its code
map. Warm, steady-state numbers; the first scan of a cold project is slower.

| Project             | Files  | Languages      | Scan time |
| ------------------- | ------ | -------------- | --------- |
| gin                 | 130    | Go             | 0.1 s     |
| requests            | 128    | Python         | 0.1 s     |
| ripgrep             | 221    | Rust           | 0.6 s     |
| tokio               | 861    | Rust           | 0.4 s     |
| react               | 7 242  | TS / JSX       | 2.0 s     |
| django              | 7 065  | Python         | 2.4 s     |
| TypeScript compiler | 81 324 | TS / JS / JSON | 18 s      |

The TypeScript compiler is the worst case — 81k files in about 18 seconds. Re-scans only look at
what changed, so keeping a project up to date is far faster than the first scan.

Once running, most code questions answer in **under a millisecond**, symbol and call-graph searches
in a few milliseconds, and document search in around 200 ms — because the map is held in memory
rather than read from disk each time.

</details>

<details>
<summary><strong>Git history queries</strong></summary>

hacienda-mcp precomputes a per-repo git-history index (path → commit posting lists, stored newest-first)
so the history tools — `commits_touching`, `recent_changes`, `hot_files`, `find_commits_by_path`,
and `symbol_history`'s commit walk — are posting-list lookups. Warm in-process query latency on the
same M4:

| Repo       | Commits | `commits_touching` | `recent_changes` | index build | index size             |
| ---------- | ------- | ------------------ | ---------------- | ----------- | ---------------------- |
| django     | 2 000   | 39 µs              | 15 µs            | 0.5 s       | 1.7 MB (6 % of `.git`) |
| tokio      | 3 984   | 37 µs              | 13 µs            | 0.9 s       | 2.1 MB (12 %)          |
| requests   | 6 480   | 38 µs              | 15 µs            | 1.0 s       | 1.9 MB (14 %)          |
| TypeScript | 2 000   | 37 µs              | 13 µs            | 3.2 s       | 30 MB (12 %)           |

History queries answer in **tens of microseconds**, flat across history depth, because the
newest-first posting lists decode only the commits a query returns. The index builds in well under a
second to a few seconds and costs **6–22 % of `.git`** on disk.

It is a pure accelerator: the tools use it only when it is fresh (`last_indexed_head == HEAD`) and
otherwise walk history directly, so it can never serve stale results — and it rebuilds automatically
when history is rewritten (filter-repo / rebase / force-push). Reproduce with
`cargo bench --bench git_history` or the git-ops block in `scripts/harden.sh`.

</details>

---

## Configuration

<details>
<summary><strong>Config file &amp; overrides</strong></summary>

The config lives at the **repo root** as `basemind.toml` (committed). The cache it drives is derived
state — held in the global cache under the XDG data directory, wiped and rebuilt on schema bumps —
so config never belongs there and nothing basemind-owned is written into your repo. Run
`hacienda-mcp init` to drop a fully-commented scaffold (documenting every option) at the root. The legacy
in-cache path (`.hacienda-mcp/basemind.toml`, from before the global-cache move) is still read as a
fallback for older checkouts. The full schema is at `schema/hacienda-mcp-config-v1.schema.json`:

```toml
# basemind.toml  (repo root — commit this)
"$schema" = "v1"

[scan]
respect_gitignore = true
# Follow symlinks during the walk. Off by default — symlinks often escape the repo (e.g. Bazel's
# bazel-* convenience symlinks). Turn on for repos that symlink real source into place.
follow_symlinks = false
# `exclude` is ADDED ON TOP of an always-on floor (node_modules, target, dist, build, out, .venv,
# venv, __pycache__, *.pyc, .pytest_cache/.mypy_cache/.ruff_cache/.tox, .next/.nuxt/.svelte-kit,
# vendor, .gradle, .terraform, coverage, bazel-*, .git, .hacienda-mcp, .idea, .DS_Store). You can add to
# it but not remove a floor entry.
exclude = []
# Index directories outside the repo root too — e.g. a Bazel external repo cache — so their
# symbols resolve in search / references / outlines. External files are keyed by absolute path;
# (re-)indexed on a full `hacienda-mcp scan` only (not live-watched). extra_roots always follow symlinks.
extra_roots = ["/private/var/tmp/_bazel_you/abc123/external"]

[code_intel]
# Precise, scope- and import-aware resolution (JS/TS via oxc; Python/Java via stack-graphs). On by
# default. Set false to fall back to fast tree-sitter locals binding for every language. Applies to
# files (re)scanned after the change.
precise_resolution = true

[documents]
enabled = true
# Embed documents for semantic search (ON — embeddings pay off on real prose / OCR).
embed = true
# Model preset: fast | balanced (default, 768-dim) | quality | multilingual.
# Changing the preset forces a FULL RE-EMBED of the corpus (time + CPU): every document is
# re-encoded at the new model's dimension.
embedding_preset = "balanced"
# Documents that are extracted + indexed but never embedded (keyword-only).
embed_exclude = []
# Route archives (.zip/.tar/.jar/…) into the recursive extractor. Off by default so one archive
# can't explode into thousands of embeds; true binaries are always skipped.
extract_archives = false

[code_search]
enabled = true
# Vector embeddings for code are OFF by default — a general English model on code isn't worth the
# cost, and NL→symbol is already served by the BM25 keyword lane. Chunking + keyword search work
# regardless. Turn on only for vector search over code (downloads an ONNX model, re-embeds on
# preset change).
embed = false
embed_exclude = []
```

Any tool call can override these settings for that one request, and settings map to environment
variables in the obvious way: `--llm-api-key` becomes `HACIENDA_MCP_LLM_API_KEY`.

</details>

---

## CLI reference

<details>
<summary><strong>Full command list</strong> — query · git · memory · suggestions · cache · web · comms · shells</summary>

CLI commands mirror the MCP tools 1:1 (enforced by `tests/cli_parity.rs`). Add `--json` for
machine-readable output.

<!-- markdownlint-disable MD013 -->

**Query (`hacienda-mcp query`)**

| Command                                                                    | Purpose                                                                                                                     |
| -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `outline <path> [--l2]`                                                    | A file's structure: symbols, lines, signatures. `--l2` adds calls + docs.                                                   |
| `symbol <needle> [--kind]`                                                 | Find a symbol by name, optionally filtered by kind.                                                                         |
| `search <needle>`                                                          | Text search across indexed files.                                                                                           |
| `references <name>`                                                        | Find everywhere a name is called.                                                                                           |
| `callers <path> <name> [--kind]`                                           | Find callers of one specific definition.                                                                                    |
| `goto-definition <path> <line> [--column]`                                 | Resolve a reference position to its scope-resolved definition.                                                              |
| `implementations <trait>`                                                  | Types that implement or inherit from a name.                                                                                |
| `call-graph <name> [--direction --max-depth]`                              | Walk the call chain up or down.                                                                                             |
| `architecture-map [--granularity --focus --depth --edges --include-churn]` | Deterministic architecture overview: hub modules/symbols ranked by graph centrality + churn, plus dependency cycles (SCCs). |
| `grep <pattern> [--language --path-contains]`                              | Pattern search with filters.                                                                                                |
| `list-files [--path-contains --language]`                                  | List indexed files.                                                                                                         |
| `status` / `repo-info`                                                     | Project overview / git info (branch, HEAD, origin).                                                                         |
| `dependents <module>`                                                      | What imports a given module.                                                                                                |
| `search-code <query> [--limit --format]`                                   | Semantic (vector) search over code chunks; returns pointers. Needs `--features code-search`.                                |
| `get-chunk <path> [--chunk-id --byte-start]`                               | Fetch one code chunk's source body (the `search-code` fetch half).                                                          |
| `expand <path> <name> [--kind]`                                            | A symbol's raw source body (the inverse of an outline entry).                                                               |

**Git (`hacienda-mcp git`)**

| Command                                                        | Purpose                                                    |
| -------------------------------------------------------------- | ---------------------------------------------------------- |
| `working-tree-status`                                          | What's staged and unstaged right now.                      |
| `recent-changes [--limit]`                                     | Recent commits with their files.                           |
| `search <pattern> [--field author\|message\|all] [--limit]`    | Full-text search over commit history at full branch depth. |
| `commits-touching <path>` / `find-commits-by-path <pattern>`   | Commits for a path or pattern.                             |
| `hot-files [--limit]`                                          | The most frequently changed files.                         |
| `diff-file <path> <old> <new>` / `diff-outline <path> [--rev]` | File or structure diff across commits.                     |
| `blame-file <path>` / `blame-symbol <path> <name>`             | Who last changed each line / a symbol.                     |
| `symbol-history <path> <name>`                                 | When a symbol's body changed over time.                    |

**Memory (`hacienda-mcp memory`)**

| Command                                            | Purpose                               |
| -------------------------------------------------- | ------------------------------------- |
| `put <key> <value>` / `get <key>` / `delete <key>` | Store, retrieve, or remove a value.   |
| `list [--prefix]`                                  | List keys, optionally by prefix.      |
| `search <query>`                                   | Search stored values by meaning.      |
| `search-documents <query>`                         | Search documents and memory together. |

**Suggestions (`hacienda-mcp governance`)**

| Command                                                     | Purpose                                                               |
| ----------------------------------------------------------- | --------------------------------------------------------------------- |
| `mine [--commits --min-count --min-confidence --max-files]` | Suggest notes from files that change together.                        |
| `proposals [--kind --limit]`                                | List pending suggestions.                                             |
| `accept <id> [--key]` / `reject <id> [--reason]`            | Keep a suggestion / dismiss it for good.                              |
| `audit [--key --individual --dry-run --include-archived]`   | Recompute memory importance, archive stale entries, refresh verdicts. |

**Cache (`hacienda-mcp cache`)**

| Command                    | Purpose                                                               |
| -------------------------- | --------------------------------------------------------------------- |
| `stats`                    | Disk footprint (per-component + total, matches `du`) and process RAM. |
| `gc`                       | Reclaim unused space (safe while the server runs).                    |
| `clear --component <comp>` | Clear part of the cache (`views`, `blobs`, `git-cache`, `all`, …).    |

**Web (`hacienda-mcp web`)**

| Command            | Purpose                                          |
| ------------------ | ------------------------------------------------ |
| `scrape <url>`     | Fetch and index a single page.                   |
| `crawl <seed-url>` | Follow links from a starting URL.                |
| `map <url>`        | Discover a site's pages without fetching bodies. |

**Comms (`hacienda-mcp comms`)**

| Command                                                         | Purpose                                       |
| --------------------------------------------------------------- | --------------------------------------------- |
| `rooms` / `join <room>` / `leave <room>` / `room-create <room>` | List, join, leave, or create rooms.           |
| `post <room> <subject> [--body --reply-to --tag]`               | Post a message.                               |
| `history <room>` / `inbox [--mark-read]`                        | Recent messages in a room / your inbox.       |
| `read <id>`                                                     | Read one message in full.                     |
| `register --name <handle>` / `agents`                           | Set your handle / list active agents.         |
| `status` / `start` / `stop`                                     | The shared service: check, start, or stop it. |

**Shells (`hacienda-mcp shells`, `--features shells`)**

| Command                                 | Purpose                                                         |
| --------------------------------------- | --------------------------------------------------------------- |
| `spawn <command> [--cwd --env --title]` | Start a detached headless shell session; prints a `session_id`. |
| `send <session-id> <text> [--no-enter]` | Type into a session's stdin.                                    |
| `capture <session-id> [--lines]`        | Read a session's visible screen.                                |
| `kill <session-id>` / `list`            | End a session / list live sessions.                             |
| `broadcast <text> --session <id>…`      | Send the same input to several sessions at once.                |

**Other commands (`scan`, `serve`, `watch`, …)**

| Command                                  | Purpose                                                                                 |
| ---------------------------------------- | --------------------------------------------------------------------------------------- |
| `scan` / `rescan <path>`                 | Full scan / update one path.                                                            |
| `watch`                                  | Keep the index fresh as files change (no server).                                       |
| `serve [--no-watch]`                     | Start the server (keeps the index fresh by default).                                    |
| `init`                                   | Re-runnable onboarding: write `basemind.toml`, select capabilities, inject usage rules. |
| `lang <list\|install\|clean>`            | Manage downloaded language grammars.                                                    |
| `hook install`                           | Add a git pre-commit hook that runs a scan.                                             |
| `compress-output` / `delta --old <path>` | Backends for the optional guardrails above.                                             |
| `checkpoint` / `detect-waste`            | Summarize a session / flag wasteful tool use.                                           |
| `telemetry`                              | What's been queried and how many tokens were saved.                                     |

<!-- markdownlint-enable MD013 -->

</details>

---

## License

MIT — see [LICENSE](LICENSE).

[tree-sitter]: https://tree-sitter.github.io/tree-sitter/
[tslp]: https://github.com/Goldziher/tree-sitter-language-pack
[xberg]: https://github.com/xberg-io/xberg
[LanceDB]: https://github.com/lancedb/lancedb

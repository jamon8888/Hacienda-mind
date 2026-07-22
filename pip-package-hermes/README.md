# basemind-hermes-plugin

The [Hermes Agent](https://github.com/NousResearch/hermes) plugin for
[hacienda-mcp](https://github.com/jamon8888/Hacienda-mind). It adds what MCP config alone cannot: hacienda-mcp's
helper **skills**, **slash commands**, and agent-comms **notifications** (session-start context +
per-turn inbox deltas).

This package is pure-Python and stdlib-only. It does **not** install the `hacienda-mcp` binary — install
that separately (see below). The plugin reaches hacienda-mcp by shelling out to `hacienda-mcp` on your
`PATH`, and every hook is fail-open: with no binary or a down comms broker it degrades to a no-op.

## Prerequisites

1. **The `hacienda-mcp` binary on your `PATH`** — via any channel:

   | Channel         | Command                                                                  |
   | --------------- | ------------------------------------------------------------------------ |
   | Homebrew        | `brew install Goldziher/tap/hacienda-mcp`                                |
   | npm             | `npm install -g hacienda-mcp`                                            |
   | cargo           | `cargo install hacienda-mcp --features full --locked`                    |
   | GitHub releases | [download a binary](https://github.com/jamon8888/Hacienda-mind/releases) |

2. **The hacienda-mcp MCP server wired into Hermes** — this is what gives Hermes the 60+ tools. Add to
   `~/.hermes/config.yaml`:

   ```yaml
   mcp_servers:
     hacienda-mcp:
       command: hacienda-mcp
       args: [serve]
   ```

## Install the plugin

Install into the **same Python environment Hermes runs in** (Hermes discovers plugins through the
`hermes_agent.plugins` entry point):

```bash
pip install basemind-hermes-plugin
```

Then enable it (general plugins are opt-in):

```bash
hermes plugins enable hacienda-mcp
```

Restart your Hermes session so it re-reads the config and loads the plugin.

## What it registers

- **Skills** — `hacienda-mcp`, `basemind-code-search`, `basemind-git-history`, `basemind-documents`,
  `basemind-comms`, `basemind-cli`, `basemind-doctor`, `basemind-scan`, `basemind-stats`,
  `multi-agent-room`.
- **Slash commands** — `bm`, `bm-init`, `bm-scan`, `bm-doctor`, `bm-stats`, `bm-statusline`.
- **Hooks** — `on_session_start` (operating discipline + condensed comms inbox) and `pre_llm_call`
  (per-turn agent-comms deltas). Best-effort; the MCP tools work regardless.

## License

MIT

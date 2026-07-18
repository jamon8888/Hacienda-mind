---
priority: medium
usage: "/serve"
description: "Start the hacienda-mcp MCP stdio server"
---

# Serve

Start the hacienda-mcp MCP stdio server. Useful when manually testing tools via a client like `mcp-cli` or the rmcp REPL.

1. Build if stale: `cargo build --release`.
2. Run:

   ```bash
   ./target/release/hacienda-mcp serve
   ```

3. The server reads MCP JSON-RPC over stdin and writes responses to stdout. Tool list available via `tools/list`; per-tool schemas via `tools/list` + the `schema` field.

For automated AI-tool integration, prefer the `hacienda-mcp` entry in `.mcp.json` — `ai-rulez generate` writes it from the `[[mcp_servers]]` block in `.ai-rulez/config.toml`.

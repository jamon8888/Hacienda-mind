# basemind-opencode

OpenCode plugin for [hacienda-mcp](https://github.com/jamon8888/Hacienda-mind) — full AI context layer for coding agents.

## Install

Add to your `opencode.json` (global or project-level):

```json
{
  "plugin": ["basemind-opencode@latest"]
}
```

Restart OpenCode. You also need the `hacienda-mcp` binary on your `PATH`:

```bash
npm install -g hacienda-mcp        # or: pip install hacienda-mcp / cargo install hacienda-mcp
```

Then scan your repo once before starting OpenCode:

```bash
cd /path/to/your/repo
hacienda-mcp scan
```

## What this registers

- **MCP server** named `hacienda-mcp` running `hacienda-mcp serve` over stdio. Exposes the full
  code-map, git, documents, and memory toolset.
- **Skills directory** with pre-authored skills that document how to drive the MCP toolset.

See the [main README](https://github.com/jamon8888/Hacienda-mind#readme) for the full MCP tool
reference, architecture, and configuration.

## License

MIT

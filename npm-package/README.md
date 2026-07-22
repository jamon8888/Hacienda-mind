# hacienda-mcp

The context and communication layer for coding agents — a shared code-map, document RAG, memory,
web crawl, git history, and agent-to-agent comms so multiple agents coordinate while they work.
300+ languages, one MCP server.

<!-- markdownlint-disable-next-line MD013 -->

[![Docs](https://img.shields.io/badge/docs-github.com/jamon8888/Hacienda-mind-965aff)](https://github.com/jamon8888/Hacienda-mind)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/jamon8888/Hacienda-mind/blob/main/LICENSE)
[![npm](https://img.shields.io/npm/v/hacienda-mcp.svg)](https://www.npmjs.com/package/hacienda-mcp)

Full documentation: **[github.com/jamon8888/Hacienda-mind](https://github.com/jamon8888/Hacienda-mind)**

## Install

```bash
npm install -g hacienda-mcp
```

The installer downloads the appropriate pre-compiled Rust binary for your platform (macOS,
Linux, Windows; x86_64 + arm64) from
[GitHub Releases](https://github.com/jamon8888/Hacienda-mind/releases) on first install.

## Quickstart

```bash
cd /path/to/your/repo
hacienda-mcp scan        # index the working tree
hacienda-mcp serve       # run the MCP stdio server
```

Wire `hacienda-mcp serve` into Claude Code or any MCP client.

## Full documentation

See the [main README](https://github.com/jamon8888/Hacienda-mind#readme) for complete docs,
architecture, MCP tool reference, and per-harness setup instructions.

## License

[MIT](https://github.com/jamon8888/Hacienda-mind/blob/main/LICENSE).

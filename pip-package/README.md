# hacienda-mcp

The context and communication layer for coding agents — a shared code-map, document RAG, memory,
web crawl, git history, and agent-to-agent comms so multiple agents coordinate while they work.
300+ languages, one MCP server.

<!-- markdownlint-disable-next-line MD013 -->

[![Docs](https://img.shields.io/badge/docs-github.com/jamon8888/Hacienda-mind-965aff)](https://github.com/jamon8888/Hacienda-mind)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/jamon8888/Hacienda-mind/blob/main/LICENSE)
[![PyPI](https://img.shields.io/pypi/v/hacienda-mcp.svg)](https://pypi.org/project/basemind/)

Full documentation: **[github.com/jamon8888/Hacienda-mind](https://github.com/jamon8888/Hacienda-mind)**

## Install

```bash
pip install hacienda-mcp
```

On first invocation, the pre-compiled Rust binary for your platform (macOS, Linux, Windows;
x86_64 + arm64) is downloaded from
[GitHub Releases](https://github.com/jamon8888/Hacienda-mind/releases) and cached under
`~/.cache/basemind/<version>/`.

Override the binary location with `HACIENDA_MCP_BINARY=/path/to/hacienda-mcp`.

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

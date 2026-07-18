#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export CLICOLOR_FORCE=1
export RUST_LOG="${RUST_LOG:-error}"
unset NO_COLOR 2>/dev/null || true

SPEED="${DEMO_SPEED:-1.0}"
TYPE_DELAY="$(awk -v s="$SPEED" 'BEGIN { printf "%.4f", 0.03 * s }')"
PROMPT_PAUSE="$(awk -v s="$SPEED" 'BEGIN { printf "%.4f", 0.6 * s }')"
PROMPT="$(printf '\033[1;32m❯\033[0m ')"

resolve_bin() {
	if [ -n "${HACIENDA_MCP_BIN:-}" ] && [ -x "${HACIENDA_MCP_BIN}" ]; then
		(cd "$(dirname "$HACIENDA_MCP_BIN")" && printf '%s/%s' "$PWD" "$(basename "$HACIENDA_MCP_BIN")")
	elif [ -x "$REPO_ROOT/target/release/hacienda-mcp" ]; then
		printf '%s' "$REPO_ROOT/target/release/hacienda-mcp"
	elif command -v hacienda-mcp >/dev/null 2>&1; then
		command -v hacienda-mcp
	else
		printf 'demo: no hacienda-mcp binary found — run: cargo build --release (or set HACIENDA_MCP_BIN)\n' >&2
		exit 1
	fi
}
BIN="$(resolve_bin)"

WORKDIR="$(mktemp -d)"
cleanup() { rm -rf "$WORKDIR" 2>/dev/null || true; }
trap cleanup EXIT
git clone -q "$REPO_ROOT" "$WORKDIR/hacienda-mcp"
cd "$WORKDIR/hacienda-mcp"

pe() {
	printf '%s' "$PROMPT"
	local i ch
	for ((i = 0; i < ${#1}; i++)); do
		ch="${1:$i:1}"
		printf '%s' "$ch"
		sleep "$TYPE_DELAY"
	done
	printf '\n'
	local cmd="${1/#basemind/$BIN}"
	eval "$cmd" || true
	printf '\n'
	sleep "$PROMPT_PAUSE"
}

pe "hacienda-mcp scan --quiet"
pe "hacienda-mcp query outline src/scanner.rs --l2"
pe "hacienda-mcp query search scan --limit 10"
pe "hacienda-mcp query references record_call --limit 8"
pe "hacienda-mcp query call-graph cmd_scan --direction callers --max-depth 3"
pe "hacienda-mcp git recent-changes --limit 5"
pe "hacienda-mcp git blame-symbol src/main.rs cmd_scan"
pe "hacienda-mcp telemetry --window today"

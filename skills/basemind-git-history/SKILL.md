---
name: basemind-git-history
description: >-
  Explore git history without shelling out to git — recent commits, commits touching a path,
  per-line and per-symbol blame, structural diffs across revisions, churn ranking, and a symbol's
  history over time. Reach for it whenever the user asks "what changed recently", "who last touched
  this", "when did this symbol change", "what's the diff between these revs", or "where's the churn".
---

# basemind-git-history — git intelligence over the index

hacienda-mcp indexes git history and resolves blame and diffs at **symbol** resolution, backed by a
`gix` history index. History queries return commits, authors, paths, and line/symbol attributions —
structured and capped — for a fraction of the cost of parsing `git log` / `git blame` output.

**hacienda-mcp first, naked git fallback.** Prefer these tools over shelling out to `git log`,
`git blame`, or `git diff`. Drop to raw git only when no tool covers the question (e.g. staging,
rebasing, anything that mutates history).

## Tool routing

| Question | MCP tool | CLI |
|---|---|---|
| "What changed recently?" | `recent_changes` | `hacienda-mcp git recent-changes [--limit N]` |
| "Which commits touched path P?" | `commits_touching` | `hacienda-mcp git commits-touching P` |
| "Path-filtered commit log?" | `find_commits_by_path` | `hacienda-mcp git find-commits-by-path P` |
| "When did symbol X last change?" | `symbol_history` (cross-commit structural hash) | `hacienda-mcp git symbol-history F name` |
| "Who wrote this line?" | `blame_file` | `hacienda-mcp git blame-file F` |
| "Who wrote this symbol / when did its body change?" | `blame_symbol` | `hacienda-mcp git blame-symbol F name` |
| "Diff a file between revs?" | `diff_file` | `hacienda-mcp git diff-file F old new` |
| "What symbols did a branch add/remove?" | `diff_outline` | `hacienda-mcp git diff-outline F old new` |
| "Where's the churn?" | `hot_files` (churn-ranked) | `hacienda-mcp git hot-files [--limit N]` |
| "What's dirty in the working tree?" | `working_tree_status` (staged/unstaged) | `hacienda-mcp git working-tree-status` |
| "What's HEAD / branch / origin?" | `repo_info` | `hacienda-mcp query repo-info` |
| "Full-text search commit messages + authors?" | `search_git_history` | *(MCP only)* |

## Examples

```text
recent_changes { limit: 5 }
→ 612df7e chore(release): v0.15.0
  1779b99 fix(git-history,serve): address code-review findings
  ...

blame_symbol { path: "src/scanner.rs", name: "process_file" }
→ last touched by <author> in <commit> — body hash changed at HEAD~7

diff_outline { path: "src/mcp/tools.rs", old: "HEAD~5", new: "HEAD" }
→ + search_git_history (function)  - old_helper (function)
```

## Notes

- Git tools require `hacienda-mcp serve` to be running **inside a git repository**. Outside a git repo
  they return a clear error.
- History queries are indexed: `commits_touching` and friends resolve in tens of microseconds vs a
  live walk. The index auto-builds on first use and is a fraction of the size of `.git`.
- All paths are repository-relative with forward-slash separators. Lists are capped
  (`limit`, default 100, max 1000).

For code structure see `basemind-code-search`; for document RAG and semantic search see
`basemind-documents`; for agent coordination see `basemind-comms`.

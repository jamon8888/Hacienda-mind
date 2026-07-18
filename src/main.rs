use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use hacienda_mcp::config::{self, Config, DocumentsCliOverrides};
use hacienda_mcp::render::{self, Verbosity};
use hacienda_mcp::store::{LockHolder, Store};
use hacienda_mcp::watcher::{BatchKind, WatchBatch};

mod lang_cli;

#[derive(Parser, Debug)]
#[command(
    name = "hacienda-mcp",
    version,
    about = "File-watcher and code-map generator using tree-sitter",
    long_about = None
)]
struct Cli {
    /// Repository root. Defaults to the current directory.
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    /// Suppress all but hard failures and the summary.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,

    /// Show every per-file result, including unchanged and skipped files.
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Force-disable ANSI colors. NO_COLOR env var is honored automatically.
    #[arg(long, global = true)]
    no_color: bool,

    /// Emit machine-readable JSON instead of the human-readable rendering. Applies
    /// to the tool subcommands (query / git / memory / web / telemetry / cache) and
    /// is ignored — with a warning — on init / scan / rescan / watch / hook / lang.
    #[arg(long, global = true)]
    json: bool,

    /// Which view to query or serve. "working" (default) is the on-disk tree;
    /// "staged" is the git index; "rev-<sha7>" is a previously scanned rev. Used by
    /// the tool subcommands and `serve`; ignored — with a warning — elsewhere.
    #[arg(long, global = true, default_value_t = hacienda_mcp::store::VIEW_WORKING.to_string())]
    view: String,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Initialize (or refresh) hacienda-mcp onboarding: write basemind.toml, gitignore the cache, and
    /// inject a "prefer hacienda-mcp over grep/read/git" rules block. Re-runnable and idempotent.
    Init(hacienda_mcp::cli::init::InitArgs),
    /// Run a one-shot scan over the repository and write the code map.
    Scan(ScanArgs),
    /// Re-index the working tree (full) or only the given paths (incremental). Use after
    /// edits, or to rebuild a stale/empty index without starting the server.
    Rescan(RescanArgs),
    /// Long-running watcher; keeps the code map current as files change.
    Watch,
    /// Query the code map (outline, search, references, call-graph, …).
    #[command(subcommand)]
    Query(hacienda_mcp::cli::codemap::QueryCmd),
    /// Git history / blame / diff queries.
    #[command(subcommand)]
    Git(hacienda_mcp::cli::git::GitCmd),
    /// Shared agent memory + document search (needs `--features memory,documents`).
    #[command(subcommand)]
    Memory(hacienda_mcp::cli::memory::MemoryCmd),
    /// Governance: mine co-change proposals, list, accept, reject (needs `--features memory`).
    #[command(subcommand)]
    Governance(hacienda_mcp::cli::governance::GovernanceCmd),
    /// On-demand web ingestion (needs `--features crawl`).
    #[command(subcommand)]
    Web(hacienda_mcp::cli::web::WebCmd),
    /// Headless agent shells: spawn / send / capture / kill / broadcast / list
    /// (needs `--features shells`).
    #[cfg(all(feature = "shells", any(unix, windows)))]
    #[command(subcommand)]
    Shells(hacienda_mcp::cli::shells::ShellsCmd),
    /// Aggregate telemetry into a usage summary.
    Telemetry {
        /// Aggregation window: `today` (default), `1h`, `24h`, `all`.
        #[arg(long)]
        window: Option<String>,
        /// Optional exact tool-name filter.
        #[arg(long)]
        tool: Option<String>,
    },
    /// Install a pre-commit hook that runs `hacienda-mcp scan --staged`.
    Hook {
        #[command(subcommand)]
        action: HookCmd,
    },
    /// Manage downloaded tree-sitter grammars.
    Lang {
        #[command(subcommand)]
        action: LangCmd,
    },
    /// Compress verbose command output read from stdin into a compact summary,
    /// failing open (raw passthrough) on errors and preserving credentials.
    CompressOutput(hacienda_mcp::textcompress::cli::CompressOutputArgs),
    /// Emit a compact `+N/-M` line-diff from a prior file version (`--old`) to
    /// new content read from stdin — the stateless delta re-read primitive.
    Delta(hacienda_mcp::textcompress::cli::DeltaArgs),
    /// Extract a compact, credential-safe checkpoint (decisions / errors /
    /// changed files) from session text read from stdin; changed files come
    /// from the git working tree, not the text.
    Checkpoint(hacienda_mcp::textcompress::cli::CheckpointArgs),
    /// Flag wasteful tool usage (redundant reads, repeated queries, oversized
    /// reads) from a JSON-Lines tool-call log read from stdin. Pure analysis.
    DetectWaste(hacienda_mcp::textcompress::cli::DetectWasteArgs),
    /// Run an MCP server (stdio) exposing the code map to AI agents.
    Serve(ServeArgs),
    /// Print a compact one-line summary of the daemon's currently-hot workspaces, for a shell
    /// statusline. Fast and silent: prints nothing and exits 0 when no daemon is running.
    Statusline,
    /// Manage the `.hacienda-mcp/` caches (gc / stats / clear). Offline path.
    #[command(subcommand)]
    Cache(hacienda_mcp::cli::admin::CacheCmd),
    /// Manage the user-global agent-comms broker daemon (needs `--features comms`).
    #[cfg(all(feature = "comms", any(unix, windows)))]
    Comms {
        #[command(subcommand)]
        action: CommsLifecycleCmd,
    },
    /// Machine-registry coordination: workspaces / worktrees / branches / advisory claims (needs
    /// `--features comms`). Talks to the broker daemon directly, like `comms`.
    #[cfg(all(feature = "comms", any(unix, windows)))]
    Registry {
        #[command(subcommand)]
        action: hacienda_mcp::cli::registry::RegistryCmd,
    },
}

/// Subcommands for `hacienda-mcp comms`: daemon lifecycle plus the agent verbs.
///
/// Lifecycle verbs (`Daemon`/`Start`/`Stop`/`Status`) manage the singleton broker process. The
/// agent verbs (`Register`/`Agents`/`ThreadStart`/`Threads`/`Join`/`Leave`/`Members`/`AddMember`/
/// `RemoveMember`/`Archive`/`Post`/`History`/`Read`/`Inbox`) connect to the daemon DIRECTLY via
/// `CommsClient::ensure_and_connect` (see `cli::comms`) — they never build a full server, so they
/// cannot clash with a running `serve`.
#[cfg(all(feature = "comms", any(unix, windows)))]
#[derive(Subcommand, Debug)]
enum CommsLifecycleCmd {
    /// Run the broker loop: bind the singleton socket, serve front-ends, block until shutdown.
    Daemon,
    /// Ensure the daemon is running (spawn if needed); noop when already alive.
    Start,
    /// Ask the running daemon to drain and stop.
    Stop,
    /// Report the daemon's pid / version / uptime / room + subscriber counts.
    Status,
    /// Agent verbs (register / rooms / post / history / inbox …) against the broker.
    #[command(flatten)]
    Agent(hacienda_mcp::cli::comms::CommsAgentCmd),
}

#[derive(clap::Args, Debug)]
struct ScanArgs {
    /// Index the git staging area instead of the working tree. Used by the
    /// pre-commit hook so the cache reflects what's about to be committed.
    /// Mutually exclusive with --rev.
    #[arg(long, conflicts_with = "rev")]
    staged: bool,
    /// Index the tree at the given revision (HEAD, branch name, sha, HEAD~3).
    /// Writes under .hacienda-mcp/views/rev-<sha7>/ — separate from the working-tree view.
    #[arg(long, value_name = "REV")]
    rev: Option<String>,
    /// Skip building the git-history index after the scan (overrides config). The history tools
    /// then fall back to the live walk. Equivalent to `HACIENDA_MCP_GH_INDEX=0`.
    #[arg(long)]
    no_git_history: bool,
    /// Wipe and fully rebuild the git-history index instead of incrementally syncing it. Use after
    /// a history rewrite if revalidation didn't already trigger a rebuild.
    #[arg(long)]
    rebuild_git_history: bool,
    /// Document-tier overrides. Every flag in this group corresponds to a
    /// `[documents.…]` TOML key and a `HACIENDA_MCP_DOCUMENTS_…` env var.
    #[command(flatten)]
    documents: DocumentsCliOverrides,
}

#[derive(clap::Args, Debug)]
struct RescanArgs {
    /// Repo-relative paths to re-index incrementally. When omitted (or with `--full`),
    /// the entire working tree is re-indexed. Paths are forward-slash with no leading `/`.
    #[arg(value_name = "PATH")]
    paths: Vec<String>,
    /// Force a full working-tree re-index even when paths are supplied. Use to rebuild a
    /// stale or empty index from scratch.
    #[arg(long)]
    full: bool,
    /// Skip building the git-history index after the rescan (overrides config).
    #[arg(long)]
    no_git_history: bool,
    /// Wipe and fully rebuild the git-history index instead of incrementally syncing it.
    #[arg(long)]
    rebuild_git_history: bool,
}

#[derive(clap::Args, Debug)]
struct ServeArgs {
    /// LRU capacity per category for the in-process git cache (commit_files, log, blame).
    #[arg(long, default_value_t = 1024)]
    git_cache_mem: usize,
    /// Disable the on-disk git cache. RAM LRU still applies but nothing persists between
    /// `hacienda-mcp serve` runs.
    #[arg(long)]
    no_git_cache_disk: bool,
    /// Disable the continuous background re-scan. By default `serve` watches the
    /// working tree and incrementally refreshes the index as files change, so the
    /// code map stays current without `rescan`. Pass `--no-watch` to turn that off
    /// for very large repos (e.g. the ~81k-file TypeScript tree) or CI runs where
    /// the per-edit incremental scan isn't worth the cost; refresh manually via the
    /// `rescan` tool instead.
    #[arg(long)]
    no_watch: bool,
    /// Document-tier overrides. Every flag in this group corresponds to a
    /// `[documents.…]` TOML key and a `HACIENDA_MCP_DOCUMENTS_…` env var.
    #[command(flatten)]
    documents: DocumentsCliOverrides,
}

#[derive(Subcommand, Debug)]
enum LangCmd {
    /// Show installed grammars and where they live.
    List,
    /// Force-download all supported grammars (no-op if already cached).
    Install,
    /// Delete the grammar cache. Next run will redownload.
    Clean,
}

#[derive(Subcommand, Debug)]
enum HookCmd {
    /// Write .git/hooks/pre-commit that invokes `hacienda-mcp scan`.
    Install,
}

/// Default tracing directive when `RUST_LOG` is unset, derived from the parsed
/// verbosity. `--quiet` raises the threshold to `warn` so subsystem INFO logs are
/// suppressed during a scan; `--verbose` lowers it to `debug`; otherwise `info`.
/// An explicit `RUST_LOG` always wins (callers honor it before this fallback).
fn default_log_directive(verbosity: Verbosity) -> &'static str {
    match verbosity {
        Verbosity::Quiet => "warn",
        Verbosity::Default => "info",
        Verbosity::Verbose => "debug",
    }
}

fn main() -> Result<()> {
    #[cfg(all(feature = "shells", any(unix, windows)))]
    if let Some(result) = hacienda_mcp::shells::intercept_internal_reexec() {
        return result;
    }
    let cli = Cli::parse();
    let verbosity = Verbosity::from_flags(cli.quiet, cli.verbose);

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_log_directive(verbosity))),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let no_color = cli.no_color;
    let start = cli
        .root
        .clone()
        .map(|p| p.canonicalize().unwrap_or(p))
        .unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    let root = hacienda_mcp::config::discover_root_with_hacienda_mcp(&start);
    if root != start {
        tracing::info!(
            resolved_root = %root.display(),
            from = "ancestor .hacienda-mcp",
            "resolved repo root upward"
        );
    }

    let json = cli.json;
    let view = cli.view.clone();
    warn_ignored_global_flags(&cli.cmd, json, &view);
    let dispatch = |tc| hacienda_mcp::cli::run(&root, &view, DocumentsCliOverrides::default(), json, tc);
    match cli.cmd {
        // `init` anchors to the current repo (git root, else cwd) — NOT the ancestor-`.hacienda-mcp`
        // walk `root` uses — so it scaffolds the project you're in, never a parent that already
        // has a `.hacienda-mcp/`.
        Cmd::Init(args) => hacienda_mcp::cli::init::run(&hacienda_mcp::config::init_root(&start), &args),
        Cmd::Scan(args) => cmd_scan(&root, &args, verbosity, no_color),
        Cmd::Rescan(args) => cmd_rescan(&root, &args, verbosity, no_color),
        Cmd::Watch => cmd_watch(&root, verbosity, no_color),
        Cmd::Query(q) => {
            let _ = hacienda_mcp::lang::ensure_grammars();
            dispatch(hacienda_mcp::cli::ToolCmd::Query(q))
        }
        Cmd::Git(g) => dispatch(hacienda_mcp::cli::ToolCmd::Git(g)),
        Cmd::Memory(m) => dispatch(hacienda_mcp::cli::ToolCmd::Memory(m)),
        Cmd::Governance(g) => dispatch(hacienda_mcp::cli::ToolCmd::Governance(g)),
        Cmd::Web(w) => dispatch(hacienda_mcp::cli::ToolCmd::Web(w)),
        #[cfg(all(feature = "shells", any(unix, windows)))]
        Cmd::Shells(s) => dispatch(hacienda_mcp::cli::ToolCmd::Shells(s)),
        Cmd::Telemetry { window, tool } => dispatch(hacienda_mcp::cli::ToolCmd::Telemetry { window, tool }),
        Cmd::Hook { action } => match action {
            HookCmd::Install => cmd_hook_install(&root),
        },
        Cmd::Lang { action } => match action {
            LangCmd::List => lang_cli::cmd_lang_list(no_color),
            LangCmd::Install => lang_cli::cmd_lang_install(verbosity, no_color),
            LangCmd::Clean => lang_cli::cmd_lang_clean(),
        },
        Cmd::CompressOutput(args) => hacienda_mcp::textcompress::cli::run(&args),
        Cmd::Delta(args) => hacienda_mcp::textcompress::cli::run_delta(&args),
        Cmd::Checkpoint(args) => hacienda_mcp::textcompress::cli::run_checkpoint(&root, &args),
        Cmd::DetectWaste(args) => hacienda_mcp::textcompress::cli::run_detect_waste(&args),
        Cmd::Serve(args) => cmd_serve(&root, &view, &args),
        Cmd::Cache(action) => hacienda_mcp::cli::run_cache(&root, action, json),
        Cmd::Statusline => cmd_statusline(),
        #[cfg(all(feature = "comms", any(unix, windows)))]
        Cmd::Comms { action } => cmd_comms(&root, action, json),
        #[cfg(all(feature = "comms", any(unix, windows)))]
        Cmd::Registry { action } => hacienda_mcp::cli::registry::run(&root, json, action),
    }
}

/// Print a compact statusline of the daemon's hot workspaces. Fast and silent by design: a missing
/// daemon (or any error) prints nothing and exits 0 so a shell statusline degrades cleanly. Without
/// the `comms` feature there is no daemon, so it is a no-op.
fn cmd_statusline() -> Result<()> {
    #[cfg(all(feature = "comms", any(unix, windows)))]
    {
        use hacienda_mcp::comms::client::CommsClient;
        use hacienda_mcp::comms::ids::AgentId;
        use hacienda_mcp::comms::singleton;

        let line = (|| -> Option<String> {
            let paths = singleton::resolve_paths().ok()?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;
            runtime.block_on(async move {
                let agent = AgentId::parse("basemind-statusline").ok()?;
                let mut client = CommsClient::connect(&paths, agent, None, None).await.ok()?;
                let hot = client.accessed_paths().await.ok()?;
                Some(format_statusline(&hot))
            })
        })();
        if let Some(line) = line {
            println!("{line}");
        }
    }
    Ok(())
}

/// Render the daemon's hot-workspace snapshot into one compact line (e.g. `bm: web · api +2 · 5
/// hot`). An empty set — daemon up but nothing hot — reads `bm: idle`. Names are the workspace
/// directory basenames; the list is capped so the line stays short regardless of the hot count.
#[cfg(all(feature = "comms", any(unix, windows)))]
fn format_statusline(workspaces: &[hacienda_mcp::comms::workspace_pool::AccessedWorkspace]) -> String {
    if workspaces.is_empty() {
        return "bm: idle".to_string();
    }
    const MAX_NAMES: usize = 3;
    let names: Vec<&str> = workspaces
        .iter()
        .take(MAX_NAMES)
        .map(|w| w.root.file_name().and_then(|n| n.to_str()).unwrap_or("?"))
        .collect();
    let mut label = names.join(" · ");
    if workspaces.len() > MAX_NAMES {
        label.push_str(&format!(" +{}", workspaces.len() - MAX_NAMES));
    }
    format!("bm: {label} · {} hot", workspaces.len())
}

/// Dispatch a comms lifecycle subcommand. Each command drives a small current-thread tokio
/// runtime — the broker daemon itself uses a multi-thread runtime so concurrent links don't
/// serialize.
#[cfg(all(feature = "comms", any(unix, windows)))]
fn cmd_comms(root: &std::path::Path, action: CommsLifecycleCmd, json: bool) -> Result<()> {
    match action {
        CommsLifecycleCmd::Daemon => hacienda_mcp::cli::comms_daemon::run(),
        CommsLifecycleCmd::Start => cmd_comms_start(),
        CommsLifecycleCmd::Stop => cmd_comms_lifecycle_rpc(CommsRpc::Stop, json),
        CommsLifecycleCmd::Status => cmd_comms_lifecycle_rpc(CommsRpc::Status, json),
        CommsLifecycleCmd::Agent(cmd) => hacienda_mcp::cli::comms::run(root, json, cmd),
    }
}

#[cfg(all(feature = "comms", any(unix, windows)))]
enum CommsRpc {
    Stop,
    Status,
}

/// Ensure a daemon is running, spawning it detached if needed.
#[cfg(all(feature = "comms", any(unix, windows)))]
fn cmd_comms_start() -> Result<()> {
    use hacienda_mcp::comms::singleton;
    let paths = singleton::resolve_paths().context("resolve comms paths")?;
    let socket_path = paths.socket_path.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(async move {
        singleton::ensure_daemon(&paths)
            .await
            .map_err(|e| anyhow::anyhow!("ensure comms daemon: {e}"))
    })?;
    println!("comms daemon is running ({})", socket_path.display());
    Ok(())
}

/// Connect to the running daemon and issue a Stop or Status RPC.
#[cfg(all(feature = "comms", any(unix, windows)))]
fn cmd_comms_lifecycle_rpc(rpc: CommsRpc, json: bool) -> Result<()> {
    use hacienda_mcp::comms::client::CommsClient;
    use hacienda_mcp::comms::ids::AgentId;
    use hacienda_mcp::comms::singleton;

    let paths = singleton::resolve_paths().context("resolve comms paths")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;

    runtime.block_on(async move {
        let agent = AgentId::parse("basemind-cli").map_err(|e| anyhow::anyhow!("agent id: {e}"))?;
        let mut client = CommsClient::connect(&paths, agent, None, None)
            .await
            .map_err(|e| anyhow::anyhow!("connect to comms daemon: {e}"))?;
        match rpc {
            CommsRpc::Stop => {
                client.stop().await.map_err(|e| anyhow::anyhow!("stop: {e}"))?;
                if json {
                    println!("{{\"stopped\":true}}");
                } else {
                    println!("comms daemon stopping");
                }
            }
            CommsRpc::Status => {
                let status = client.status().await.map_err(|e| anyhow::anyhow!("status: {e}"))?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&status).map_err(|e| anyhow::anyhow!("serialize status: {e}"))?
                    );
                } else {
                    println!(
                        "pid={} version={} proto={} uptime={}s threads={} subscribers={}",
                        status.pid,
                        status.version,
                        status.proto_ver,
                        status.uptime_secs,
                        status.threads,
                        status.subscribers,
                    );
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(())
}

/// Emit a `WARN` when a global flag was supplied to a subcommand that does not
/// consume it. `--json` only affects the tool subcommands (query / git / memory /
/// web / telemetry / cache); `--view` additionally affects `serve`. Everything else
/// ignores them, so warning prevents a no-op flag from looking effective.
fn warn_ignored_global_flags(cmd: &Cmd, json: bool, view: &str) {
    let consumes_json = matches!(
        cmd,
        Cmd::Query(_)
            | Cmd::Git(_)
            | Cmd::Memory(_)
            | Cmd::Governance(_)
            | Cmd::Web(_)
            | Cmd::Telemetry { .. }
            | Cmd::Cache(_)
    );
    #[cfg(all(feature = "comms", any(unix, windows)))]
    let consumes_json = consumes_json || matches!(cmd, Cmd::Comms { .. } | Cmd::Registry { .. });
    #[cfg(all(feature = "shells", any(unix, windows)))]
    let consumes_json = consumes_json || matches!(cmd, Cmd::Shells(_));
    let consumes_view = consumes_json || matches!(cmd, Cmd::Serve(_));

    if json && !consumes_json {
        tracing::warn!("--json has no effect on this subcommand; ignoring");
    }
    if view != hacienda_mcp::store::VIEW_WORKING && !consumes_view {
        tracing::warn!(view = %view, "--view has no effect on this subcommand; ignoring");
    }
}

fn bootstrap_grammars(verbosity: Verbosity, no_color: bool) -> Result<()> {
    let summary = hacienda_mcp::lang::ensure_grammars().map_err(|e| anyhow::anyhow!("grammar bootstrap failed: {e}"))?;
    let mut out = render::stdout(no_color);
    render::render_grammar_bootstrap(&mut out, &summary, verbosity);
    Ok(())
}

fn load_or_default(root: &std::path::Path) -> Result<Config> {
    load_or_default_with(root, None)
}

/// Variant of [`load_or_default`] that also applies a CLI override layer through
/// the layered merger. Used by `scan` / `serve` to flow `#[command(flatten)]`
/// flags down to the resolved config.
fn load_or_default_with(root: &std::path::Path, cli: Option<DocumentsCliOverrides>) -> Result<Config> {
    match config::load_with_overrides(root, None, cli) {
        Ok(loaded) => Ok(loaded.config),
        Err(config::ConfigError::NotFound(_)) => {
            tracing::info!("no basemind.toml; using defaults");
            Ok(config::default_for_root(root))
        }
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}

/// Open the store for a writer command (`scan` / `rescan`), translating lock contention
/// into actionable guidance. Two distinct holders can deny the lock — our own `fs2`
/// advisory lock and Fjall's internal exclusive open lock — and a raw `FjallError: Locked`
/// or bare "Locked" is opaque to a user whose editor plugin is quietly running `serve`.
/// `is_lock_contention` collapses both into one friendly message that leads with what to
/// do; the underlying `StoreError` is preserved as the error source (visible under `-v` /
/// the full anyhow chain) so we never swallow the cause.
fn open_store_for_write(root: &std::path::Path, view: &str, what: &str, holder: LockHolder) -> Result<Store> {
    Store::open_with_holder(root, view, holder).map_err(|err| {
        if err.is_lock_contention() {
            anyhow::Error::new(err).context(hacienda_mcp::store::LOCK_CONTENTION_HELP.to_string())
        } else {
            anyhow::Error::new(err).context(format!("open store ({what})"))
        }
    })
}

/// Pre-flight the store write lock before a CLI `scan` / `rescan`. When a live hacienda-mcp process
/// already holds it — overwhelmingly the "editor plugin runs `serve` while the user (or another
/// plugin command) runs `scan`" double-run — return an actionable message so the caller prints it
/// and exits cleanly, instead of blocking on the acquire retries and then failing with a raw lock
/// error. `None` means the lock is free (proceed); the acquire still handles the probe→acquire race
/// reactively via [`hacienda_mcp::store::LOCK_CONTENTION_HELP`].
fn writer_collision_notice(root: &std::path::Path) -> Option<String> {
    let hacienda_mcp_dir = hacienda_mcp::store::workspace_cache_dir(root);
    match hacienda_mcp::store::probe_writer_lock(&hacienda_mcp_dir) {
        hacienda_mcp::store::WriterProbe::Free => None,
        hacienda_mcp::store::WriterProbe::Held { holder: Some(meta) } => Some(format!(
            "`{}` (pid {}) is already running against this repo and keeping the index fresh — \
             running this directly is unnecessary and would collide with it. Use that server's \
             `rescan` tool to refresh the index, or stop it first.",
            meta.command, meta.pid
        )),
        hacienda_mcp::store::WriterProbe::Held { holder: None } => Some(hacienda_mcp::store::LOCK_CONTENTION_HELP.to_string()),
    }
}

/// Build / refresh the repo-global git-history index after a working-tree scan (a separate phase
/// from the core file scan). Best-effort: a non-git dir, a disabled toggle, or any failure leaves
/// the index untouched and the history tools fall back to the live walk — never fails the scan.
fn sync_git_history_after_scan(
    root: &std::path::Path,
    cli_enabled: bool,
    force_rebuild: bool,
    out: &mut impl std::io::Write,
) {
    if !cli_enabled || !hacienda_mcp::git_history::index_enabled() {
        return;
    }
    let Ok(repo) = hacienda_mcp::git::Repo::discover(root) else {
        return;
    };
    let hacienda_mcp_dir = hacienda_mcp::git_history::shared_history_hacienda_mcp_dir(root);
    let index = match hacienda_mcp::git_history::GitHistoryIndex::open(&hacienda_mcp_dir) {
        Ok(index) => index,
        Err(error) => {
            tracing::warn!(?error, "git-history index unavailable; skipping");
            return;
        }
    };
    if force_rebuild && let Err(error) = index.clear(&hacienda_mcp_dir) {
        tracing::warn!(?error, "git-history index clear failed");
    }
    match hacienda_mcp::git_history::builder::sync(&index, &repo, &hacienda_mcp_dir) {
        Ok(outcome) => {
            let summary = match outcome {
                hacienda_mcp::git_history::builder::RebuildOutcome::Fresh => "git-history index: up to date".to_string(),
                hacienda_mcp::git_history::builder::RebuildOutcome::Incremental { added } => {
                    format!("git-history index: +{added} commits")
                }
                hacienda_mcp::git_history::builder::RebuildOutcome::FullRebuild { reason, commits } => {
                    format!("git-history index: rebuilt {commits} commits ({reason})")
                }
            };
            let _ = writeln!(out, "{summary}");
        }
        Err(error) => tracing::warn!(?error, "git-history index sync failed"),
    }
}

fn cmd_scan(root: &std::path::Path, args: &ScanArgs, verbosity: Verbosity, no_color: bool) -> Result<()> {
    bootstrap_grammars(verbosity, no_color)?;
    let config = load_or_default_with(root, Some(args.documents.clone()))?;

    let mut out = render::stdout(no_color);
    if args.staged {
        let repo = hacienda_mcp::git::Repo::discover(root).context("`--staged` requires being inside a git repository")?;
        let mut store = open_store_for_write(root, hacienda_mcp::store::VIEW_STAGED, "staged", LockHolder::Scan)?;
        render::render_scan_header(&mut out, "staged index", verbosity);
        let report = hacienda_mcp::scanner::scan(
            root,
            &mut store,
            &config,
            hacienda_mcp::scanner::ScanSource::Staged(&repo),
            hacienda_mcp::scanner::EmbedMode::Inline,
        )
        .context("scan staged")?;
        render::render_report(&mut out, &report, verbosity);
        return Ok(());
    }
    if let Some(rev_spec) = &args.rev {
        let repo = hacienda_mcp::git::Repo::discover(root).context("`--rev` requires being inside a git repository")?;
        let sha = repo.resolve_rev(rev_spec).context("resolve rev")?;
        let short = &sha[..7.min(sha.len())];
        let view = hacienda_mcp::store::view_name_for_rev(short);
        let mut store = open_store_for_write(root, &view, "rev", LockHolder::Scan)?;
        render::render_scan_header(&mut out, &format!("rev {short}"), verbosity);
        let report = hacienda_mcp::scanner::scan(
            root,
            &mut store,
            &config,
            hacienda_mcp::scanner::ScanSource::Rev {
                repo: &repo,
                sha: sha.clone(),
            },
            hacienda_mcp::scanner::EmbedMode::Inline,
        )
        .context("scan rev")?;
        render::render_report(&mut out, &report, verbosity);
        return Ok(());
    }

    if let Some(notice) = writer_collision_notice(root) {
        use std::io::Write as _;
        render::render_scan_header(&mut out, "scan", verbosity);
        let _ = writeln!(out, "{notice}");
        return Ok(());
    }
    let mut store = open_store_for_write(root, hacienda_mcp::store::VIEW_WORKING, "scan", LockHolder::Scan)?;
    let report = hacienda_mcp::scanner::scan(
        root,
        &mut store,
        &config,
        hacienda_mcp::scanner::ScanSource::WorkingTree,
        hacienda_mcp::scanner::EmbedMode::Inline,
    )
    .context("scan")?;
    render::render_report(&mut out, &report, verbosity);
    sync_git_history_after_scan(root, !args.no_git_history, args.rebuild_git_history, &mut out);
    Ok(())
}

fn cmd_rescan(root: &std::path::Path, args: &RescanArgs, verbosity: Verbosity, no_color: bool) -> Result<()> {
    bootstrap_grammars(verbosity, no_color)?;
    let config = load_or_default(root)?;
    let mut out = render::stdout(no_color);
    if let Some(notice) = writer_collision_notice(root) {
        use std::io::Write as _;
        let _ = writeln!(out, "{notice}");
        return Ok(());
    }
    let mut store = open_store_for_write(root, hacienda_mcp::store::VIEW_WORKING, "rescan", LockHolder::Rescan)?;

    let report = if args.full || args.paths.is_empty() {
        hacienda_mcp::scanner::scan(
            root,
            &mut store,
            &config,
            hacienda_mcp::scanner::ScanSource::WorkingTree,
            hacienda_mcp::scanner::EmbedMode::Inline,
        )
        .context("rescan (full)")?
    } else {
        let abs: Vec<PathBuf> = args.paths.iter().map(|p| root.join(p)).collect();
        hacienda_mcp::scanner::scan_paths(root, &mut store, &config, &abs, hacienda_mcp::scanner::EmbedMode::Inline)
            .context("rescan (paths)")?
    };
    render::render_report(&mut out, &report, verbosity);
    sync_git_history_after_scan(root, !args.no_git_history, args.rebuild_git_history, &mut out);
    Ok(())
}

fn cmd_watch(root: &std::path::Path, verbosity: Verbosity, no_color: bool) -> Result<()> {
    bootstrap_grammars(verbosity, no_color)?;
    let config = Arc::new(load_or_default(root)?);
    let store = Arc::new(Mutex::new(
        Store::open_with_holder(root, hacienda_mcp::store::VIEW_WORKING, LockHolder::Watch).context("open store")?,
    ));

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let store_w = Arc::clone(&store);
    let config_w = Arc::clone(&config);
    let root_buf = root.to_path_buf();
    let watcher_handle = std::thread::spawn(move || {
        let mut stdout = render::stdout(no_color);
        let cb: hacienda_mcp::watcher::BatchCallback = Box::new(move |batch: WatchBatch<'_>| match batch.kind {
            BatchKind::InitialScan => {
                render::render_report(&mut stdout, batch.report, verbosity);
            }
            BatchKind::Incremental { paths } => {
                render::render_batch_header(&mut stdout, paths, verbosity);
                render::render_lines(&mut stdout, batch.report, verbosity);
            }
        });
        hacienda_mcp::watcher::watch(&root_buf, store_w, config_w, shutdown_rx, cb)
    });

    runtime.block_on(async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("ctrl-c received; shutting down");
        let _ = shutdown_tx.send(());
    });
    match watcher_handle.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(anyhow::anyhow!(e)),
        Err(_) => Err(anyhow::anyhow!("watcher thread panicked")),
    }
}

fn cmd_serve(root: &std::path::Path, view: &str, args: &ServeArgs) -> Result<()> {
    if view != hacienda_mcp::store::VIEW_WORKING {
        let index_path = hacienda_mcp::store::workspace_cache_dir(root)
            .join(hacienda_mcp::store::VIEWS_DIR)
            .join(view)
            .join(hacienda_mcp::store::INDEX_FILE);
        if !index_path.exists() {
            anyhow::bail!(
                "view {view:?} has not been scanned; run `hacienda-mcp scan --view {view}` first \
                 (or omit --view to serve the working view)"
            );
        }
    }
    // On a comms build the machine daemon is the sole fjall writer: this serve opens the store
    // READ-ONLY and forwards every write (auto-scan, watcher rescan, `rescan` tool) to the daemon
    // over the socket (see `mcp::daemon_forward`), so N sessions on one repo all read + write
    // without the single-holder downgrade. Without comms there is no daemon, so serve is the local
    // writer exactly as before: take the write lock, or fall back read-only if another serve holds it.
    #[cfg(all(feature = "comms", any(unix, windows)))]
    let (store, read_only, daemon_writer) = (
        // Blobs-only open: the daemon is the sole fjall writer, and fjall's directory lock is
        // exclusive even for a read-only open, so opening the index here would steal the lock the
        // daemon needs. Reads come from the shared blobs / in-RAM map instead.
        Store::open_read_only_no_index(root, view).context("open store read-only")?,
        true,
        true,
    );
    #[cfg(not(all(feature = "comms", any(unix, windows))))]
    let (store, read_only, daemon_writer) = match Store::open_with_holder(root, view, LockHolder::Serve) {
        Ok(store) => (store, false, false),
        Err(error) if error.is_lock_contention() => {
            match &error {
                hacienda_mcp::store::StoreError::Locked { .. } => tracing::warn!(
                    %error,
                    "store write-lock held by another hacienda-mcp process; starting read-only (reads from the shared index)"
                ),
                _ => tracing::warn!(
                    %error,
                    "Fjall index lock still contended after retry (a transient reader, not another serve); starting read-only (reads from the shared index)"
                ),
            }
            let store = Store::open_read_only(root, view).context("open store read-only")?;
            (store, true, false)
        }
        Err(error) => return Err(anyhow::Error::new(error).context("open store")),
    };
    let hacienda_mcp_dir = hacienda_mcp::store::workspace_cache_dir(root);
    let root_buf = root.to_path_buf();
    let config = Arc::new(load_or_default_with(root, Some(args.documents.clone()))?);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;

    let repo = hacienda_mcp::git::Repo::discover(root).ok().map(Arc::new);
    let git_cache = Arc::new(
        hacienda_mcp::git_cache::GitCache::open(&hacienda_mcp_dir, args.git_cache_mem, !args.no_git_cache_disk)
            .context("open git cache")?,
    );

    let options = hacienda_mcp::mcp::ServerOptions {
        background: true,
        watch: !args.no_watch,
        read_only,
        daemon_writer,
    };
    tracing::info!(
        pid = std::process::id(),
        version = env!("CARGO_PKG_VERSION"),
        view,
        read_only,
        root = %root.display(),
        "hacienda-mcp serve: MCP server starting"
    );
    let outcome = runtime.block_on(async move {
        use rmcp::ServiceExt;
        let server = hacienda_mcp::mcp::BasemindServer::new_with_options(store, root_buf, config, repo, git_cache, options);
        let transport = rmcp::transport::stdio();
        let service = server
            .serve(transport)
            .await
            .map_err(|e| anyhow::anyhow!("rmcp serve: {e}"))?;
        service
            .waiting()
            .await
            .map_err(|e| anyhow::anyhow!("rmcp waiting: {e}"))?;
        Ok::<(), anyhow::Error>(())
    });
    match &outcome {
        Ok(()) => tracing::info!(pid = std::process::id(), "hacienda-mcp serve: client disconnected, exiting"),
        Err(error) => {
            tracing::error!(pid = std::process::id(), %error, "hacienda-mcp serve: exiting on error")
        }
    }
    outcome
}

fn cmd_hook_install(root: &std::path::Path) -> Result<()> {
    let hooks_dir = root.join(".git").join("hooks");
    if !hooks_dir.exists() {
        anyhow::bail!("no .git/hooks directory at {}", hooks_dir.display());
    }
    let hook_path = hooks_dir.join("pre-commit");
    let body = r#"#!/usr/bin/env sh
# Installed by hacienda-mcp hook install.
set -e
exec hacienda-mcp scan --staged --quiet
"#;
    std::fs::write(&hook_path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)?;
    }
    println!("installed pre-commit hook at {}", hook_path.display());
    Ok(())
}

#[cfg(all(test, feature = "comms", any(unix, windows)))]
mod statusline_tests {
    use std::path::PathBuf;

    use hacienda_mcp::comms::workspace_pool::AccessedWorkspace;

    fn ws(root: &str) -> AccessedWorkspace {
        AccessedWorkspace {
            root: PathBuf::from(root),
            key: "k".to_string(),
            idle_secs: 0,
        }
    }

    #[test]
    fn empty_hot_set_reads_idle() {
        assert_eq!(super::format_statusline(&[]), "bm: idle");
    }

    #[test]
    fn lists_workspace_basenames_and_the_hot_count() {
        let hot = [ws("/repos/web"), ws("/repos/api")];
        assert_eq!(super::format_statusline(&hot), "bm: web · api · 2 hot");
    }

    #[test]
    fn caps_the_name_list_with_an_overflow_marker() {
        let hot = [ws("/a/one"), ws("/a/two"), ws("/a/three"), ws("/a/four"), ws("/a/five")];
        assert_eq!(super::format_statusline(&hot), "bm: one · two · three +2 · 5 hot");
    }
}

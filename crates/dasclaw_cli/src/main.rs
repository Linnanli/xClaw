//! Headless `dasclaw` CLI binary (ADR-153 §4.4 steps 4 + 5 + 6 + 8).
//!
//! Thin wrapper around [`dasclaw_cli::run`]. Two subcommands:
//!
//! - `echo` — built-in deterministic responder, no LLM key needed.
//!   Default smoke driver inherited from the step-4 skeleton.
//! - `run` — wires a real `dasclaw_llm_provider`-backed responder via
//!   [`dasclaw_cli::provider::ProviderArgs`] (step 5). Honours
//!   `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `DASCLAW_API_KEY`.
//!   Tool dispatch is opt-in via `--enable-tools` (builtin
//!   `echo` / `now`, step 6) and/or `--mcp-config <path>` (MCP servers
//!   over stdio or HTTP, step 8). When both flags are set the two
//!   tool sources are merged via
//!   [`dasclaw_runtime::CompositeToolExecutor`].
//!
//! Both subcommands accept `--prompt` or read the user prompt from stdin
//! and share `--system` for the system message.

use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use dasclaw_cli::mcp::load_executor as load_mcp_executor;
use dasclaw_cli::provider::{ProviderArgs, build_responder};
use dasclaw_cli::tools::default_builtins;
use dasclaw_cli::{EchoResponder, run, run_with_tools};
use dasclaw_core::messages::ToolDefinition;
use dasclaw_runtime::{CompositeToolExecutor, ToolExecutor};
const DEFAULT_SYSTEM: &str = "You are dasclaw, a headless agent.";

/// Headless dasclaw agent CLI.
#[derive(Debug, Parser)]
#[command(name = "dasclaw-cli", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// User prompt. When omitted, the prompt is read from stdin.
    #[arg(short, long, global = true)]
    prompt: Option<String>,

    /// System prompt prepended to the conversation.
    #[arg(short, long, default_value = DEFAULT_SYSTEM, global = true)]
    system: String,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the deterministic built-in echo responder (no LLM, no network).
    Echo,
    /// Run a real LLM-backed responder via `dasclaw_llm_provider`.
    Run {
        #[command(flatten)]
        provider: ProviderArgs,
        #[command(flatten)]
        tools: ToolArgs,
    },
}

/// Tool-dispatch knobs for the `run` subcommand.
///
/// Kept in a flattened group so future tool sources land here without
/// churning the top-level CLI shape. `--enable-tools` and
/// `--mcp-config` may be combined: when both are set the two
/// executors are merged via
/// [`dasclaw_runtime::CompositeToolExecutor`].
#[derive(Debug, Args)]
struct ToolArgs {
    /// Advertise the builtin demo tools (`echo`, `now`) to the model and
    /// dispatch them locally. Defaults to off so the bare `run`
    /// subcommand stays a pure chat-completion driver.
    #[arg(long = "enable-tools", default_value_t = false)]
    enable_tools: bool,

    /// Path to an MCP server config JSON file. Each entry is started
    /// either as a stdio child process (`command` field) or as an HTTP
    /// client (`url` field); its tools are advertised to the model
    /// under the `<server_name>_<tool>` qualified name.
    #[arg(long = "mcp-config")]
    mcp_config: Option<PathBuf>,

    /// Path to a `wasm32-wasip2` component file. Loaded under the
    /// default-deny capability policy (no http, no workspace writes, no
    /// secrets — see ADR-153 §1.1 A7 / e13). The tool's name is the
    /// file stem; calls that touch ungated host imports surface as
    /// `ToolResult { is_error: true, .. }` carrying the host gate
    /// message verbatim. Capability grant flags are tracked as #869.
    #[cfg(feature = "wasm-tools")]
    #[arg(long = "wasm-tool")]
    wasm_tool: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = real_main().await {
        eprintln!("dasclaw: {err:#}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

async fn real_main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let prompt = match cli.prompt.as_deref() {
        Some(p) => p.to_string(),
        None => read_stdin_prompt().context("reading prompt from stdin")?,
    };
    let prompt = prompt.trim();

    let reply = match cli.command.unwrap_or(Command::Echo) {
        Command::Echo => run(EchoResponder::new(), &cli.system, prompt)
            .await
            .context("echo agent run failed")?,
        Command::Run { provider, tools } => {
            let responder = build_responder(&provider).context("building LLM responder")?;
            let assembled = assemble_tool_executor(&tools).await?;
            match assembled {
                None => run(responder, &cli.system, prompt)
                    .await
                    .context("LLM agent run failed")?,
                Some((executor, definitions)) => {
                    run_with_tools(responder, executor, definitions, &cli.system, prompt)
                        .await
                        .context("LLM agent (with tools) run failed")?
                }
            }
        }
    };
    println!("{reply}");
    Ok(())
}

/// Collect every tool source the user opted into and merge them via
/// [`CompositeToolExecutor`].
///
/// Returns `None` when no source is enabled — the caller falls back to
/// the plain [`run`] entry. Returning `Some((executor, defs))` for the
/// uniform `≥1 source` case keeps the call site free of the
/// combinatorial branch explosion that the original
/// `(enable_tools × mcp_config × …)` match would grow into as each new
/// tool source (e.g. ADR-153 §1.1 A7 wasm tools) lands.
async fn assemble_tool_executor(
    tools: &ToolArgs,
) -> Result<Option<(CompositeToolExecutor, Vec<ToolDefinition>)>> {
    let mut sources: Vec<(Vec<String>, Arc<dyn ToolExecutor>)> = Vec::new();
    let mut definitions: Vec<ToolDefinition> = Vec::new();

    if tools.enable_tools {
        let exec = default_builtins();
        let defs = exec.definitions();
        push_source(&mut sources, &mut definitions, defs, Arc::new(exec));
    }

    if let Some(path) = tools.mcp_config.as_deref() {
        let exec = load_mcp_executor(path)
            .await
            .with_context(|| format!("loading MCP config {}", path.display()))?;
        let defs = exec.definitions();
        push_source(&mut sources, &mut definitions, defs, Arc::new(exec));
    }

    #[cfg(feature = "wasm-tools")]
    if let Some(path) = tools.wasm_tool.as_deref() {
        let loaded = dasclaw_cli::wasm::load_wasm_tool(path).await?;
        push_source(
            &mut sources,
            &mut definitions,
            vec![loaded.definition],
            Arc::new(loaded.executor),
        );
    }

    if sources.is_empty() {
        return Ok(None);
    }
    let composite = CompositeToolExecutor::new(sources)
        .context("merging tool executors (duplicate tool name?)")?;
    Ok(Some((composite, definitions)))
}

/// Push one tool source into the accumulator: the executor is stored
/// keyed by the tool names it owns, and the matching [`ToolDefinition`]s
/// are appended to the advertised schema list.
fn push_source(
    sources: &mut Vec<(Vec<String>, Arc<dyn ToolExecutor>)>,
    definitions: &mut Vec<ToolDefinition>,
    defs: Vec<ToolDefinition>,
    executor: Arc<dyn ToolExecutor>,
) {
    let names: Vec<String> = defs.iter().map(|d| d.name.clone()).collect();
    definitions.extend(defs);
    sources.push((names, executor));
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(io::stderr)
        .try_init()
        .ok();
}

fn read_stdin_prompt() -> io::Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

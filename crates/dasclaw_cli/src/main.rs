//! Headless `dasclaw` CLI binary (ADR-153 §4.4 steps 4 + 5 + 6 + 8).
//!
//! Thin wrapper around [`dasclaw_cli::run`]. Two subcommands:
//!
//! - `echo` — built-in deterministic responder, no LLM key needed.
//!   Default smoke driver inherited from the step-4 skeleton.
//! - `run` — wires a real `dasclaw_llm_provider`-backed responder via
//!   [`dasclaw_cli::provider::ProviderArgs`] (step 5). Honours
//!   `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `DASCLAW_API_KEY`.
//!   Tool dispatch is opt-in via either `--enable-tools` (builtin
//!   `echo` / `now`, step 6) or `--mcp-config <path>` (MCP servers,
//!   step 8). The two flags are mutually exclusive in this sub-step;
//!   a future composite executor will let them combine.
//!
//! Both subcommands accept `--prompt` or read the user prompt from stdin
//! and share `--system` for the system message.

use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use dasclaw_cli::mcp::load_executor as load_mcp_executor;
use dasclaw_cli::provider::{ProviderArgs, build_responder};
use dasclaw_cli::tools::default_builtins;
use dasclaw_cli::{EchoResponder, run, run_with_tools};

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
/// `--mcp-config` are currently mutually exclusive (clap enforces this);
/// composing them is deferred to a follow-up that introduces a
/// composite [`ToolExecutor`].
#[derive(Debug, Args)]
struct ToolArgs {
    /// Advertise the builtin demo tools (`echo`, `now`) to the model and
    /// dispatch them locally. Defaults to off so the bare `run`
    /// subcommand stays a pure chat-completion driver.
    #[arg(
        long = "enable-tools",
        default_value_t = false,
        conflicts_with = "mcp_config"
    )]
    enable_tools: bool,

    /// Path to an MCP server config JSON file. Each entry is spawned as
    /// a stdio child process; its tools are advertised to the model
    /// under the `<server_name>_<tool>` qualified name.
    #[arg(long = "mcp-config")]
    mcp_config: Option<PathBuf>,
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
            if let Some(path) = tools.mcp_config.as_deref() {
                let executor = load_mcp_executor(path)
                    .await
                    .with_context(|| format!("loading MCP config {}", path.display()))?;
                let definitions = executor.definitions();
                run_with_tools(responder, executor, definitions, &cli.system, prompt)
                    .await
                    .context("LLM agent (with MCP tools) run failed")?
            } else if tools.enable_tools {
                let executor = default_builtins();
                let definitions = executor.definitions();
                run_with_tools(responder, executor, definitions, &cli.system, prompt)
                    .await
                    .context("LLM agent (with tools) run failed")?
            } else {
                run(responder, &cli.system, prompt)
                    .await
                    .context("LLM agent run failed")?
            }
        }
    };
    println!("{reply}");
    Ok(())
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

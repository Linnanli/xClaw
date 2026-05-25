//! Headless `dasclaw` CLI binary (ADR-153 §4.4 steps 4 + 5).
//!
//! Thin wrapper around [`dasclaw_cli::run`]. Two subcommands:
//!
//! - `echo` — built-in deterministic responder, no LLM key needed.
//!   Default smoke driver inherited from the step-4 skeleton.
//! - `run` — wires a real `dasclaw_llm_provider`-backed responder via
//!   [`dasclaw_cli::provider::ProviderArgs`] (step 5). Honours
//!   `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `DASCLAW_API_KEY`.
//!
//! Both subcommands accept `--prompt` or read the user prompt from stdin
//! and share `--system` for the system message.

use std::io::{self, Read};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dasclaw_cli::provider::{ProviderArgs, build_responder};
use dasclaw_cli::{EchoResponder, run};

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
    },
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
        Command::Run { provider } => {
            let responder = build_responder(&provider).context("building LLM responder")?;
            run(responder, &cli.system, prompt)
                .await
                .context("LLM agent run failed")?
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

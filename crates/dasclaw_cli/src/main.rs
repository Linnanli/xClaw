//! Headless `dasclaw` CLI binary (ADR-153 §4.4 step 4).
//!
//! Thin wrapper around [`dasclaw_cli::run`]. Parses argv with `clap`,
//! initialises `tracing`, acquires the prompt (from `--prompt` or stdin),
//! invokes the agent, prints the reply, and maps errors to exit codes.
//!
//! For programmatic use, depend on the `dasclaw_cli` library directly.

use std::io::{self, Read};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use dasclaw_cli::{EchoResponder, run};

/// Headless dasclaw agent CLI.
///
/// Runs a single prompt through a `dasclaw_runtime::Agent` and prints the
/// reply. The default responder is a deterministic echo for smoke
/// testing — wire a real LLM responder in once provider selection lands.
#[derive(Debug, Parser)]
#[command(name = "dasclaw-cli", version, about, long_about = None)]
struct Cli {
    /// User prompt. When omitted, the prompt is read from stdin.
    #[arg(short, long)]
    prompt: Option<String>,

    /// System prompt prepended to the conversation.
    #[arg(short, long, default_value = "You are dasclaw, a headless agent.")]
    system: String,
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(io::stderr)
        .try_init()
        .ok();

    let cli = Cli::parse();
    let prompt = match cli.prompt {
        Some(p) => p,
        None => read_stdin_prompt().context("reading prompt from stdin")?,
    };

    let reply = run(EchoResponder::new(), &cli.system, prompt.trim())
        .await
        .context("agent run failed")?;
    println!("{reply}");
    Ok(())
}

fn read_stdin_prompt() -> io::Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

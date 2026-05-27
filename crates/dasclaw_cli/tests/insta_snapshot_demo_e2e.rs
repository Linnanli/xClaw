//! PR3 demo: `insta` snapshot test for `dasclaw-cli --help`.
//!
//! Purpose
//! -------
//! Establish a working insta baseline in the dasclaw_cli crate so the rest
//! of the team has a concrete template for adding snapshot assertions
//! (e.g. CLI surface, structured JSON outputs, error messages).
//!
//! What this pins
//! --------------
//! The exact stdout of `dasclaw-cli --help`. Clap-derived help text is
//! deterministic across builds: argument order, flag names, default
//! values, and the description strings all flow from `#[derive(Parser)]`
//! attributes in `src/main.rs` / argument modules. Any of the following
//! will fail the snapshot:
//!
//!   - a new top-level flag / subcommand
//!   - renaming an existing flag
//!   - editing a `#[arg(help = "...")]` doc string
//!   - clap major version bump that reformats the help layout
//!
//! Updating the baseline
//! ---------------------
//! When the diff is intentional:
//!
//!   ```
//!   cargo insta test -p dasclaw_cli --test insta_snapshot_demo_e2e
//!   cargo insta accept -p dasclaw_cli
//!   ```
//!
//! Or one-shot via the env var:
//!
//!   ```
//!   INSTA_UPDATE=auto cargo nextest run -p dasclaw_cli --test insta_snapshot_demo_e2e
//!   ```
//!
//! Then commit `tests/snapshots/insta_snapshot_demo_e2e__dasclaw_cli_help.snap`.

use assert_cmd::Command;

#[test]
fn dasclaw_cli_help_snapshot() {
    let output = Command::cargo_bin("dasclaw-cli")
        .expect("dasclaw-cli binary built")
        .arg("--help")
        .output()
        .expect("dasclaw-cli --help ran");

    assert!(
        output.status.success(),
        "`dasclaw-cli --help` exited non-zero: stderr=\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("help text is utf-8");
    insta::assert_snapshot!("dasclaw_cli_help", stdout);
}

//! Canonical `dasclaw` binary entry point (ADR-114 Ⅴ).
//!
//! Thin wrapper around [`ironclaw::entry::run`]; all startup logic lives in
//! `src/entry.rs` so the legacy `ironclaw` alias bin can share it without
//! patch-style duplication.

/// Pre-main process hardening hook (#324 sub-task 1).
///
/// Runs before `fn main()` via `#[ctor::ctor]` to disable core dumps,
/// block ptrace attach, and scrub dangerous environment variables
/// (`LD_PRELOAD`, `DYLD_*`, macOS malloc stack-logging controls, …).
///
/// Lives in each `src/bin/*.rs` rather than the library so it only fires
/// when an actual binary is invoked, not on every library load (tests,
/// downstream crates, etc.).
#[ctor::ctor]
fn pre_main() {
    dasclaw_process_hardening::pre_main_hardening();
}

fn main() -> anyhow::Result<()> {
    ironclaw::entry::run(None)
}

//! Legacy `ironclaw` binary entry point (ADR-114 Ⅴ).
//!
//! Compatibility alias retained for one release cycle. Emits a single
//! deprecation warning to stderr (via [`ironclaw::entry::run`]) before
//! delegating to the canonical startup flow.

/// Pre-main process hardening hook (#324 sub-task 1).
///
/// Mirrors the `dasclaw` bin so the legacy alias gets the same
/// hardening guarantees. See `dasclaw.rs` for the full rationale.
#[ctor::ctor]
fn pre_main() {
    dasclaw_process_hardening::pre_main_hardening();
}

fn main() -> anyhow::Result<()> {
    ironclaw::entry::run(Some("ironclaw"))
}

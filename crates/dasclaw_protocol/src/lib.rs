//! Verbatim file-level slice of `codex-cli-main/codex-rs/protocol/`.
//!
//! **Current scope** (Step C1.1, ADR-136): `parse_command.rs` only — the
//! 31-LOC `ParsedCommand` enum needed by `dasclaw_shell_command`.
//!
//! **Planned expansion** (ADR-136 §3 Step C1.2 ~ C1.5): `error.rs`,
//! `config_types.rs`, `permissions.rs`, `models.rs`, `protocol.rs`,
//! `network_policy.rs` (≈12,547 LOC, 70% of codex-protocol).
//! Step C1.5 verifies whether `protocol.rs` independently compiles or
//! requires further `codex_utils_*` slices (ADR-136 §6 Q2).
//!
//! Verbatim red line per [ADR-129 §1.3](../../docs/plans/architecture-refactor/adr-129-sandbox-windows-windows-crate-adoption.md);
//! file-level slicing rationale per [ADR-136 §3.1](../../docs/plans/architecture-refactor/adr-136-protocol-expansion-plan.md)
//! (predecessor: ADR-133 §2.5 single-file slice).

pub mod parse_command;

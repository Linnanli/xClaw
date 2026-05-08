//! Verbatim slice of `codex-cli-main/codex-rs/protocol/src/parse_command.rs`.
//!
//! Only `ParsedCommand` is needed by `dasclaw_shell_command`; the rest of
//! `codex-protocol` (16k LOC + 30+ transitive deps) is intentionally NOT
//! vendored. See [ADR-133 §amend](../../docs/plans/architecture-refactor/adr-133-shell-command-adoption-eval.md)
//! for the slicing rationale and ADR-129 §1.3 verbatim red-line.

pub mod parse_command;

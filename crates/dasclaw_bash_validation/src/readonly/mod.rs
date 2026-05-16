//! `readOnlyValidation` deepening (Phase 3.2).
//!
//! Semantic port of:
//! - `claude-code-main/src/tools/BashTool/readOnlyValidation.ts`
//! - `claude-code-main/src/utils/shell/readOnlyCommandValidation.ts`
//!
//! Phase 3.2.A — flag parser primitives (this slice).
//! Subsequent slices (3.2.B/C/D/E) plug into the API exposed by
//! [`flag_parser`].
//!
//! Plan: `docs/plans/bash-parity/phase-3.2-readonly-validation-deepening.md`.

pub mod bash_allowlist;
pub mod flag_parser;

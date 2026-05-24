//! `dasclaw_fs_tools` — filesystem path validation + file guard primitives.
//!
//! Verbatim port of the leaf layer of `desktop-client/ironclaw/src/tools/builtin/`
//! filesystem tooling per ADR-156 §6.3 F4.6.6-a. Higher-tier consumers
//! (`code_edit`, `glob_search`, `grep_search`, `file`) follow in F4.6.6-b/-c.
//!
//! # Modules
//!
//! - [`path_utils`] — secure path validation (directory traversal defence,
//!   glob allow/deny, `PathPolicy`, `effective_base_dir`).
//! - [`file_guard`] — symlink-escape, binary-file, size, line-ending guards
//!   layered on top of [`path_utils::validate_path`].

pub mod file_guard;
pub mod path_utils;

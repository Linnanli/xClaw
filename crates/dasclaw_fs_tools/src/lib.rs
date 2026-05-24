//! `dasclaw_fs_tools` — filesystem tools (path validation, guards, code/file search).
//!
//! Verbatim port of `desktop-client/ironclaw/src/tools/builtin/` filesystem
//! tooling per ADR-156 §6.3.
//!
//! - F4.6.6-a (leaf): `path_utils`, `file_guard`.
//! - F4.6.6-b (middle): `code_edit`, `glob_search`, `grep_search` — built on
//!   top of the leaf layer; expose `Tool` implementations consumed by the
//!   desktop tool registry.
//! - F4.6.6-c (heavy): `file` — pending workspace::paths extraction.
//!
//! # Modules
//!
//! - [`path_utils`] — secure path validation (directory traversal defence,
//!   glob allow/deny, `PathPolicy`, `effective_base_dir`).
//! - [`file_guard`] — symlink-escape, binary-file, size, line-ending guards
//!   layered on top of [`path_utils::validate_path`].
//! - [`code_edit`] — single-point-replacement editor with count verification
//!   and unified-diff preview.
//! - [`glob_search`] — `find`-like file path matching via glob patterns.
//! - [`grep_search`] — `grep`-like regex content search with optional context.

pub mod code_edit;
pub mod file_guard;
pub mod glob_search;
pub mod grep_search;
pub mod path_utils;

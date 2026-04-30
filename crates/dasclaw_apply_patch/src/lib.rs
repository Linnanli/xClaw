//! `dasclaw_apply_patch` — lark-grammar `apply_patch` parser ported from
//! `codex-cli-main/codex-rs/apply-patch`.
//!
//! This crate exposes only the **parser** surface (W3 issue #52). Patch
//! application (filesystem mutation, hunk seek/replace) is intentionally
//! out of scope and lives in a follow-up issue.
//!
//! # Public API
//!
//! - [`parse_patch`] — parse a complete patch (lenient by default).
//! - [`parse_patch_streaming`] — parse partial/streaming patch text for
//!   progress reporting only.
//! - [`ApplyPatchArgs`], [`Hunk`], [`UpdateFileChunk`], [`ParseError`].
//!
//! # Example
//!
//! ```
//! use dasclaw_apply_patch::{parse_patch, Hunk};
//!
//! let patch = "*** Begin Patch\n\
//!              *** Add File: hello.txt\n\
//!              +world\n\
//!              *** End Patch";
//! let args = parse_patch(patch).unwrap();
//! assert!(matches!(args.hunks[0], Hunk::AddFile { .. }));
//! ```

mod parser;

pub use parser::parse_patch;
pub use parser::parse_patch_streaming;
pub use parser::ApplyPatchArgs;
pub use parser::Hunk;
pub use parser::ParseError;
pub use parser::UpdateFileChunk;

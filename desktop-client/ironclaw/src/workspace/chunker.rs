//! Document chunking for search indexing.
//!
//! Re-export shim. The implementation lives in
//! [`dasclaw_workspace_cap::chunker`] so that admin-backend and other
//! capability consumers can call it without depending on the ironclaw
//! desktop crate. See ADR-152 §3 F3.4.

pub use dasclaw_workspace_cap::chunker::{ChunkConfig, chunk_document};

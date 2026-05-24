//! Network / web LLM tools extracted from `desktop-client/ironclaw` per
//! ADR-156 §6.3 F4.6.8.
//!
//! Phase 1 (current): hosts the `web_fetch` tool only.
//! Phase 2 (planned): `http`, `web_search`, `html_converter` will be merged here.

mod web_fetch;

pub use web_fetch::WebFetchTool;

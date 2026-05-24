//! Network / web LLM tools extracted from `desktop-client/ironclaw` per
//! ADR-156 §6.3 F4.6.8.
//!
//! Phase 1: `web_fetch`.
//! Phase 2 (current): `http` + `html_converter` (the latter is `http`'s
//!   feature-gated HTML→Markdown helper, sunk together to avoid a stacked PR).
//! Phase 3 (planned): `web_search`.

mod html_converter;
mod http;
mod web_fetch;

pub use html_converter::convert_html_to_markdown;
pub use http::HttpTool;
pub use web_fetch::WebFetchTool;

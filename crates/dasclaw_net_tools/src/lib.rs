//! Network / web LLM tools extracted from `desktop-client/ironclaw` per
//! ADR-156 §6.3 F4.6.8.
//!
//! Phase 1: `web_fetch`.
//! Phase 2: `http` + `html_converter` (the latter is `http`'s
//!   feature-gated HTML→Markdown helper, sunk together to avoid a stacked PR).
//! Phase 3 (current): `web_search` — DuckDuckGo HTML search, no API key.

mod html_converter;
mod http;
mod web_fetch;
mod web_search;

pub use html_converter::convert_html_to_markdown;
pub use http::HttpTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;

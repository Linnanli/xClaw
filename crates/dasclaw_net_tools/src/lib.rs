//! Network / web LLM tools extracted from `desktop-client/ironclaw` per
//! ADR-156 §6.3 F4.6.8.
//!
//! Hosted tools:
//! - `web_fetch`  — fetch a URL, convert to readable text, answer a prompt.
//! - `web_search` — DuckDuckGo HTML search, no API key (F4.6.8 phase 2).
//! - `html_converter::convert_html_to_markdown` — shared helper used by
//!   web_fetch and `HttpTool` (F4.6.8 phase 2).
//! - `http`       — general-purpose HTTPS request tool with credential
//!   injection via `dasclaw_wasm_tools::SharedCredentialRegistry`
//!   (F4.6.8 phase 3).
//!
//! F4.6.8 is closed with phase 3; `dasclaw_net_tools` now hosts all four
//! files originally listed in ADR-156 §6.1.

pub mod html_converter;
mod http;
mod web_fetch;
mod web_search;

pub use html_converter::convert_html_to_markdown;
pub use http::HttpTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;

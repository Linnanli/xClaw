//! Network / web LLM tools extracted from `desktop-client/ironclaw` per
//! ADR-156 §6.3 F4.6.8.
//!
//! Hosted tools:
//! - `web_fetch`  — fetch a URL, convert to readable text, answer a prompt.
//! - `web_search` — DuckDuckGo HTML search, no API key (F4.6.8 phase 2).
//! - `html_converter::convert_html_to_markdown` — shared helper used by
//!   web_fetch and the desktop `HttpTool` (F4.6.8 phase 2).
//!
//! `http` (the general-purpose HTTP request tool) remains in desktop-client
//! pending a follow-up phase — it has deeper coupling to credential storage.

pub mod html_converter;
mod web_fetch;
mod web_search;

pub use html_converter::convert_html_to_markdown;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;

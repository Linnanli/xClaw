//! Built-in web fetch tool — fetch a URL, convert to readable text, answer a prompt.
//!
//! Adapted from claw-code's `WebFetch` tool. Unlike the general-purpose `HttpTool`,
//! this tool is specifically designed for **reading web pages**: it auto-upgrades
//! HTTP→HTTPS, converts HTML to text, and returns a summary oriented around the
//! caller's prompt.
//!
//! The existing `HttpTool` remains for general API calls; this tool is for
//! "read this web page and tell me about X" use cases.

use std::time::Instant;

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use serde_json::json;

use crate::context::JobContext;
use crate::tools::tool::{ApprovalRequirement, Tool, ToolError, ToolOutput, require_str};

const USER_AGENT: &str = concat!("IronClaw-WebFetch/", env!("CARGO_PKG_VERSION"),);

const REQUEST_TIMEOUT_SECS: u64 = 20;
const MAX_REDIRECTS: usize = 10;

// ── Public tool struct ──────────────────────────────────────────────

pub struct WebFetchTool;

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }
}

// ── Data types ──────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct FetchOutput {
    bytes: usize,
    code: u16,
    code_text: String,
    result: String,
    duration_ms: u128,
    url: String,
}

// ── Tool trait impl ─────────────────────────────────────────────────

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a URL, convert it into readable text, and answer a prompt about it. \
         Automatically upgrades HTTP to HTTPS for non-localhost URLs. \
         HTML pages are converted to plain text. Use this to read web pages, \
         documentation, or articles."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "description": "The URL to fetch"
                },
                "prompt": {
                    "type": "string",
                    "description": "What to look for or summarize from the page"
                }
            },
            "required": ["url", "prompt"],
            "additionalProperties": false
        })
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Never
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _job_ctx: &JobContext,
    ) -> Result<ToolOutput, ToolError> {
        let url = require_str(&params, "url")?;
        let prompt = require_str(&params, "prompt")?;

        let started = std::time::Instant::now();
        match execute_web_fetch(url, prompt).await {
            Ok(output) => {
                let text = serde_json::to_string_pretty(&output)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolOutput::text(text, started.elapsed()))
            }
            Err(e) => Err(ToolError::ExecutionFailed(e)),
        }
    }
}

// ── Core fetch logic (adapted from claw-code) ───────────────────────

async fn execute_web_fetch(url: &str, prompt: &str) -> Result<FetchOutput, String> {
    let started = Instant::now();
    let client = build_http_client()?;
    let request_url = normalize_fetch_url(url)?;
    let response = client
        .get(&request_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let final_url = response.url().to_string();
    let code = status.as_u16();
    let code_text = status.canonical_reason().unwrap_or("Unknown").to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response.text().await.map_err(|e| e.to_string())?;
    let bytes = body.len();
    let normalized = normalize_fetched_content(&body, &content_type);
    let result = summarize_web_fetch(&final_url, prompt, &normalized, &body, &content_type);

    Ok(FetchOutput {
        bytes,
        code,
        code_text,
        result,
        duration_ms: started.elapsed().as_millis(),
        url: final_url,
    })
}

fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| e.to_string())
}

fn normalize_fetch_url(url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    if parsed.scheme() == "http" {
        let host = parsed.host_str().unwrap_or_default();
        if host != "localhost" && host != "127.0.0.1" && host != "::1" {
            let mut upgraded = parsed;
            upgraded
                .set_scheme("https")
                .map_err(|()| String::from("failed to upgrade URL to https"))?;
            return Ok(upgraded.to_string());
        }
    }
    Ok(parsed.to_string())
}

fn normalize_fetched_content(body: &str, content_type: &str) -> String {
    if content_type.contains("html") {
        html_to_text(body)
    } else {
        body.trim().to_string()
    }
}

fn summarize_web_fetch(
    url: &str,
    prompt: &str,
    content: &str,
    raw_body: &str,
    content_type: &str,
) -> String {
    let lower_prompt = prompt.to_lowercase();
    let compact = collapse_whitespace(content);

    let detail = if lower_prompt.contains("title") {
        extract_title(content, raw_body, content_type)
            .map_or_else(|| preview_text(&compact, 600), |t| format!("Title: {t}"))
    } else if lower_prompt.contains("summary") || lower_prompt.contains("summarize") {
        preview_text(&compact, 900)
    } else {
        let preview = preview_text(&compact, 900);
        format!("Prompt: {prompt}\nContent preview:\n{preview}")
    };

    format!("Fetched {url}\n{detail}")
}

fn extract_title(_content: &str, raw_body: &str, content_type: &str) -> Option<String> {
    if content_type.contains("html") {
        let lowered = raw_body.to_lowercase();
        if let Some(start) = lowered.find("<title>") {
            let after = start + "<title>".len();
            if let Some(end_rel) = lowered[after..].find("</title>") {
                let title =
                    collapse_whitespace(&decode_html_entities(&raw_body[after..after + end_rel]));
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
    }
    None
}

fn preview_text(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let shortened: String = input.chars().take(max_chars).collect();
    format!("{}…", shortened.trim_end())
}

// ── HTML helpers (shared with web_search) ───────────────────────────

fn html_to_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut previous_was_space = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            ch if ch.is_whitespace() => {
                if !previous_was_space {
                    text.push(' ');
                    previous_was_space = true;
                }
            }
            _ => {
                text.push(ch);
                previous_was_space = false;
            }
        }
    }

    collapse_whitespace(&decode_html_entities(&text))
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_fetch_url_upgrades_http() {
        let result = normalize_fetch_url("http://example.com/page").unwrap();
        assert!(result.starts_with("https://"));
    }

    #[test]
    fn test_normalize_fetch_url_keeps_localhost_http() {
        let result = normalize_fetch_url("http://localhost:8080/api").unwrap();
        assert!(result.starts_with("http://"));
    }

    #[test]
    fn test_normalize_fetch_url_keeps_https() {
        let result = normalize_fetch_url("https://example.com/page").unwrap();
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn test_normalize_html_content() {
        let html = "<html><body><p>Hello <b>world</b></p></body></html>";
        let result = normalize_fetched_content(html, "text/html");
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_normalize_plain_content() {
        let result = normalize_fetched_content("  plain text  ", "text/plain");
        assert_eq!(result, "plain text");
    }

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>My Page Title</title></head></html>";
        let title = extract_title("", html, "text/html");
        assert_eq!(title, Some("My Page Title".to_string()));
    }

    #[test]
    fn test_extract_title_no_title_tag() {
        let title = extract_title("", "<html></html>", "text/html");
        assert_eq!(title, None);
    }

    #[test]
    fn test_preview_text_short() {
        assert_eq!(preview_text("short", 100), "short");
    }

    #[test]
    fn test_preview_text_truncated() {
        let long = "a".repeat(200);
        let preview = preview_text(&long, 100);
        assert!(preview.ends_with('…'));
        assert!(preview.len() < 200);
    }

    #[test]
    fn test_summarize_title_prompt() {
        let result = summarize_web_fetch(
            "https://example.com",
            "get the title",
            "",
            "<html><title>Example</title></html>",
            "text/html",
        );
        assert!(result.contains("Title: Example"));
    }

    #[test]
    fn test_summarize_summary_prompt() {
        let result = summarize_web_fetch(
            "https://example.com",
            "give a summary",
            "This is the page content about Rust programming.",
            "",
            "text/plain",
        );
        assert!(result.contains("Rust programming"));
    }

    #[test]
    fn test_tool_name_and_schema() {
        let tool = WebFetchTool::new();
        assert_eq!(tool.name(), "web_fetch");
        let schema = tool.parameters_schema();
        let required: Vec<String> = serde_json::from_value(schema["required"].clone()).unwrap();
        assert!(required.contains(&"url".to_string()));
        assert!(required.contains(&"prompt".to_string()));
    }
}

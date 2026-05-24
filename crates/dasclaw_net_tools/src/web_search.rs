//! Built-in web search tool using DuckDuckGo HTML search.
//!
//! Adapted from claw-code's `WebSearch` tool. No API key required —
//! uses DuckDuckGo's HTML endpoint and parses results from the DOM.
//!
//! Supports an environment variable `IRONCLAW_WEB_SEARCH_BASE_URL` to
//! override the search backend for testing or enterprise deployments.

use std::collections::BTreeSet;
use std::time::Instant;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use dasclaw_runtime::Tool;
use dasclaw_tool::{ApprovalRequirement, ToolError, ToolOutput, require_str};

const USER_AGENT: &str = concat!("IronClaw-WebSearch/", env!("CARGO_PKG_VERSION"),);

const REQUEST_TIMEOUT_SECS: u64 = 20;
const MAX_RESULTS: usize = 8;
const MAX_REDIRECTS: usize = 10;

// ── Public tool struct ──────────────────────────────────────────────

pub struct WebSearchTool;

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self
    }
}

// ── Data types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SearchParams {
    query: String,
    allowed_domains: Option<Vec<String>>,
    blocked_domains: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct SearchOutput {
    query: String,
    results: Vec<SearchResultItem>,
    duration_seconds: f64,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum SearchResultItem {
    SearchResult {
        tool_use_id: String,
        content: Vec<SearchHit>,
    },
    Commentary(String),
}

#[derive(Debug, Serialize, Clone)]
struct SearchHit {
    title: String,
    url: String,
}

// ── Tool trait impl ─────────────────────────────────────────────────

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for current information and return cited results. \
         Returns titles and URLs from DuckDuckGo. Supports domain allow/block \
         filters. Use this when you need up-to-date information from the internet."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string",
                    "minLength": 2
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only include results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Exclude results from these domains"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Never
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _job_ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let query = require_str(&params, "query")?;
        if query.len() > 2000 {
            return Err(ToolError::InvalidParameters(
                "'query' exceeds maximum length of 2000 characters".into(),
            ));
        }

        let allowed_domains: Option<Vec<String>> = params
            .get("allowed_domains")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let blocked_domains: Option<Vec<String>> = params
            .get("blocked_domains")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let input = SearchParams {
            query: query.to_string(),
            allowed_domains,
            blocked_domains,
        };

        let started = std::time::Instant::now();
        match execute_web_search(&input).await {
            Ok(output) => {
                let text = serde_json::to_string_pretty(&output)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolOutput::text(text, started.elapsed()))
            }
            Err(e) => Err(ToolError::ExecutionFailed(e)),
        }
    }
}

// ── Core search logic (adapted from claw-code) ─────────────────────

async fn execute_web_search(input: &SearchParams) -> Result<SearchOutput, String> {
    let started = Instant::now();
    let client = build_http_client()?;
    let search_url = build_search_url(&input.query)?;
    let response = client
        .get(search_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let final_url = response.url().clone();
    let html = response.text().await.map_err(|e| e.to_string())?;
    let mut hits = extract_search_hits(&html);

    if hits.is_empty() && final_url.host_str().is_some() {
        hits = extract_search_hits_from_generic_links(&html);
    }

    if let Some(ref allowed) = input.allowed_domains {
        hits.retain(|hit| host_matches_list(&hit.url, allowed));
    }
    if let Some(ref blocked) = input.blocked_domains {
        hits.retain(|hit| !host_matches_list(&hit.url, blocked));
    }

    dedupe_hits(&mut hits);
    hits.truncate(MAX_RESULTS);

    let summary = if hits.is_empty() {
        format!("No web search results matched the query {:?}.", input.query)
    } else {
        let rendered = hits
            .iter()
            .map(|hit| format!("- [{}]({})", hit.title, hit.url))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Search results for {:?}. Include a Sources section in the final answer.\n{}",
            input.query, rendered
        )
    };

    Ok(SearchOutput {
        query: input.query.clone(),
        results: vec![
            SearchResultItem::Commentary(summary),
            SearchResultItem::SearchResult {
                tool_use_id: String::from("web_search_1"),
                content: hits,
            },
        ],
        duration_seconds: started.elapsed().as_secs_f64(),
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

fn build_search_url(query: &str) -> Result<reqwest::Url, String> {
    if let Ok(base) = std::env::var("IRONCLAW_WEB_SEARCH_BASE_URL") {
        let mut url = reqwest::Url::parse(&base).map_err(|e| e.to_string())?;
        url.query_pairs_mut().append_pair("q", query);
        return Ok(url);
    }

    let mut url =
        reqwest::Url::parse("https://html.duckduckgo.com/html/").map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("q", query);
    Ok(url)
}

// ── HTML parsing ────────────────────────────────────────────────────

fn extract_search_hits(html: &str) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    let mut remaining = html;

    while let Some(anchor_start) = remaining.find("result__a") {
        let after_class = &remaining[anchor_start..];
        let Some(href_idx) = after_class.find("href=") else {
            remaining = &after_class[1..];
            continue;
        };
        let href_slice = &after_class[href_idx + 5..];
        let Some((url, rest)) = extract_quoted_value(href_slice) else {
            remaining = &after_class[1..];
            continue;
        };
        let Some(close_tag_idx) = rest.find('>') else {
            remaining = &after_class[1..];
            continue;
        };
        let after_tag = &rest[close_tag_idx + 1..];
        let Some(end_anchor_idx) = after_tag.find("</a>") else {
            remaining = &after_tag[1..];
            continue;
        };
        let title = html_to_text(&after_tag[..end_anchor_idx]);
        if let Some(decoded_url) = decode_duckduckgo_redirect(&url) {
            hits.push(SearchHit {
                title: title.trim().to_string(),
                url: decoded_url,
            });
        }
        remaining = &after_tag[end_anchor_idx + 4..];
    }

    hits
}

fn extract_search_hits_from_generic_links(html: &str) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    let mut remaining = html;

    while let Some(anchor_start) = remaining.find("<a") {
        let after_anchor = &remaining[anchor_start..];
        let Some(href_idx) = after_anchor.find("href=") else {
            remaining = &after_anchor[2..];
            continue;
        };
        let href_slice = &after_anchor[href_idx + 5..];
        let Some((url, rest)) = extract_quoted_value(href_slice) else {
            remaining = &after_anchor[2..];
            continue;
        };
        let Some(close_tag_idx) = rest.find('>') else {
            remaining = &after_anchor[2..];
            continue;
        };
        let after_tag = &rest[close_tag_idx + 1..];
        let Some(end_anchor_idx) = after_tag.find("</a>") else {
            remaining = &after_anchor[2..];
            continue;
        };
        let title = html_to_text(&after_tag[..end_anchor_idx]);
        if title.trim().is_empty() {
            remaining = &after_tag[end_anchor_idx + 4..];
            continue;
        }
        let decoded_url = decode_duckduckgo_redirect(&url).unwrap_or(url);
        if decoded_url.starts_with("http://") || decoded_url.starts_with("https://") {
            hits.push(SearchHit {
                title: title.trim().to_string(),
                url: decoded_url,
            });
        }
        remaining = &after_tag[end_anchor_idx + 4..];
    }

    hits
}

// ── Helpers ─────────────────────────────────────────────────────────

fn extract_quoted_value(input: &str) -> Option<(String, &str)> {
    let quote = input.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &input[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some((rest[..end].to_string(), &rest[end + quote.len_utf8()..]))
}

fn decode_duckduckgo_redirect(url: &str) -> Option<String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        return Some(decode_html_entities(url));
    }

    let joined = if url.starts_with("//") {
        format!("https:{url}")
    } else if url.starts_with('/') {
        format!("https://duckduckgo.com{url}")
    } else {
        return None;
    };

    let parsed = reqwest::Url::parse(&joined).ok()?;
    if parsed.path() == "/l/" || parsed.path() == "/l" {
        for (key, value) in parsed.query_pairs() {
            if key == "uddg" {
                return Some(decode_html_entities(value.as_ref()));
            }
        }
    }
    Some(joined)
}

fn host_matches_list(url: &str, domains: &[String]) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    domains.iter().any(|domain| {
        let normalized = normalize_domain_filter(domain);
        !normalized.is_empty() && (host == normalized || host.ends_with(&format!(".{normalized}")))
    })
}

fn normalize_domain_filter(domain: &str) -> String {
    let trimmed = domain.trim();
    let candidate = reqwest::Url::parse(trimmed)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| trimmed.to_string());
    candidate
        .trim()
        .trim_start_matches('.')
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn dedupe_hits(hits: &mut Vec<SearchHit>) {
    let mut seen = BTreeSet::new();
    hits.retain(|hit| seen.insert(hit.url.clone()));
}

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
    fn test_build_search_url_default() {
        // Clear env in case test_build_search_url_custom_env ran first (parallel threads)
        unsafe {
            std::env::remove_var("IRONCLAW_WEB_SEARCH_BASE_URL");
        }
        let url = build_search_url("rust lang").expect("should build URL");
        assert_eq!(url.host_str(), Some("html.duckduckgo.com"));
        assert!(url.query().expect("has query").contains("rust+lang"));
    }

    #[test]
    fn test_build_search_url_custom_env() {
        // SAFETY: test-only, single-threaded test runner
        unsafe {
            std::env::set_var(
                "IRONCLAW_WEB_SEARCH_BASE_URL",
                "https://search.example.com/q",
            );
        }
        let url = build_search_url("test query").expect("should build URL");
        assert_eq!(url.host_str(), Some("search.example.com"));
        unsafe {
            std::env::remove_var("IRONCLAW_WEB_SEARCH_BASE_URL");
        }
    }

    #[test]
    fn test_extract_search_hits_duckduckgo() {
        let html = r#"
            <a rel="nofollow" class="result__a" href="https://www.rust-lang.org/">
                Rust Programming Language
            </a>
            <a rel="nofollow" class="result__a" href="https://doc.rust-lang.org/book/">
                The Rust Book
            </a>
        "#;

        let hits = extract_search_hits(html);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Rust Programming Language");
        assert_eq!(hits[0].url, "https://www.rust-lang.org/");
        assert_eq!(hits[1].title, "The Rust Book");
    }

    #[test]
    fn test_extract_search_hits_from_generic() {
        let html = r#"
            <a href="https://example.com/page1">Page One</a>
            <a href="https://example.com/page2">Page Two</a>
            <a href="javascript:void(0)">Should be skipped</a>
        "#;

        let hits = extract_search_hits_from_generic_links(html);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://example.com/page1");
    }

    #[test]
    fn test_decode_duckduckgo_redirect() {
        // Direct URL pass-through
        assert_eq!(
            decode_duckduckgo_redirect("https://example.com"),
            Some("https://example.com".to_string())
        );

        // DDG redirect URL
        let redirect = "/l/?uddg=https%3A%2F%2Fexample.com%2Fpage&rut=abc123";
        let decoded = decode_duckduckgo_redirect(redirect);
        assert_eq!(decoded, Some("https://example.com/page".to_string()));

        // Reject bare relative paths
        assert_eq!(decode_duckduckgo_redirect("relative/path"), None);
    }

    #[test]
    fn test_host_matches_list() {
        let domains = vec!["example.com".to_string(), "rust-lang.org".to_string()];
        assert!(host_matches_list("https://www.example.com/page", &domains));
        assert!(host_matches_list("https://example.com/", &domains));
        assert!(host_matches_list(
            "https://doc.rust-lang.org/book/",
            &domains
        ));
        assert!(!host_matches_list("https://other.com/", &domains));
    }

    #[test]
    fn test_dedupe_hits() {
        let mut hits = vec![
            SearchHit {
                title: "A".into(),
                url: "https://a.com".into(),
            },
            SearchHit {
                title: "B".into(),
                url: "https://b.com".into(),
            },
            SearchHit {
                title: "A dup".into(),
                url: "https://a.com".into(),
            },
        ];
        dedupe_hits(&mut hits);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn test_html_to_text() {
        let html = "<b>Hello</b> <i>world</i>!";
        assert_eq!(html_to_text(html), "Hello world!");
    }

    #[test]
    fn test_html_entities_decode() {
        assert_eq!(decode_html_entities("a &amp; b &lt; c"), "a & b < c");
    }

    #[test]
    fn test_tool_name_and_schema() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"][0], "query");
    }
}

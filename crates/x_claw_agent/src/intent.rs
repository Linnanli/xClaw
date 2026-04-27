//! Loop-side heuristics and message templates used by the agentic loop.
//!
//! Ported from ironclaw `llm/reasoning.rs` as part of Phase 3 Step D-4.
//! These routines don't depend on any LLM engine — they operate on raw
//! response strings and are safe to run on the loop side.

use std::borrow::Cow;

/// Nudge injected when the LLM says "I'll do X" but doesn't actually issue a
/// tool call. Empirical fix for models that describe intent in prose instead
/// of using the tool-calling mechanism.
pub const TOOL_INTENT_NUDGE: &str = "\
You said you would perform an action, but you did not include any tool calls.\n\
Do NOT describe what you intend to do — actually call the tool now.\n\
Use the tool_calls mechanism to invoke the appropriate tool.";

/// Notice injected when the LLM's response was truncated mid-tool-call,
/// causing incomplete parameters. Tells the LLM to try a different approach.
pub const TRUNCATED_TOOL_CALL_NOTICE: &str = "\
Your previous response was truncated while generating tool call parameters. \
The tool calls were discarded. Please try a different approach — \
summarize or transform the data instead of echoing it verbatim in a tool call.";

/// Detect when an LLM response expresses intent to call a tool without
/// actually issuing tool calls. Returns `true` if the text contains phrases
/// like "Let me search …" or "I'll fetch …" outside of fenced/indented code
/// blocks.
///
/// Exclusion phrases (e.g. "let me explain") are checked first to avoid
/// false positives on conversational language.
pub fn llm_signals_tool_intent(response: &str) -> bool {
    let text = strip_code_blocks(response);
    let lower = text.to_lowercase();

    const EXCLUSIONS: &[&str] = &[
        "let me explain",
        "let me know",
        "let me think",
        "let me summarize",
        "let me clarify",
        "let me describe",
        "let me help",
        "let me understand",
        "let me break",
        "let me outline",
        "let me walk you",
        "let me provide",
        "let me suggest",
        "let me elaborate",
        "let me start by",
    ];
    if EXCLUSIONS.iter().any(|e| lower.contains(e)) {
        return false;
    }

    const PREFIXES: &[&str] = &["let me ", "i'll ", "i will ", "i'm going to "];
    const ACTION_VERBS: &[&str] = &[
        "search",
        "look up",
        "check",
        "fetch",
        "find",
        "read the",
        "write the",
        "create",
        "run the",
        "execute",
        "query",
        "retrieve",
        "add it",
        "add the",
        "add this",
        "add that",
        "update the",
        "delete",
        "remove the",
        "look into",
    ];

    for prefix in PREFIXES {
        for (i, _) in lower.match_indices(prefix) {
            let after = &lower[i + prefix.len()..];
            for verb in ACTION_VERBS {
                if after.starts_with(verb) || after.contains(&format!(" {verb}")) {
                    return true;
                }
            }
        }
    }

    false
}

/// Strip fenced code blocks (``` ... ```), indented code lines (4+ spaces /
/// tab), and double-quoted strings so that tool-intent detection only fires
/// on prose.
fn strip_code_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut in_fence = false;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if line.starts_with("    ") || line.starts_with('\t') {
            continue;
        }
        let stripped = strip_quoted_strings(line);
        result.push_str(&stripped);
        result.push('\n');
    }
    result
}

/// Remove double-quoted string literals from a line.
fn strip_quoted_strings(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut in_quote = false;
    let mut prev = '\0';
    for ch in line.chars() {
        if ch == '"' && prev != '\\' {
            in_quote = !in_quote;
            continue;
        }
        if !in_quote {
            result.push(ch);
        }
        prev = ch;
    }
    result
}

/// Find the largest valid UTF-8 char boundary at or before `pos`.
///
/// Private copy of ironclaw's `util::floor_char_boundary` polyfill, kept
/// local so `x_claw_agent` stays free of ironclaw dependencies.
fn floor_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut i = pos;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Truncate a string for log/status previews.
///
/// `max` is a byte budget. The result is truncated at the last valid char
/// boundary at or before `max` bytes, so it is always valid UTF-8.
pub fn truncate_for_preview(s: &str, max: usize) -> Cow<'_, str> {
    if s.len() <= max {
        Cow::Borrowed(s)
    } else {
        let end = floor_char_boundary(s, max);
        Cow::Owned(format!("{}...", &s[..end]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_detects_tool_like_phrases() {
        assert!(llm_signals_tool_intent("Let me search for that file"));
        assert!(llm_signals_tool_intent("I'll fetch the data"));
        assert!(llm_signals_tool_intent("I will retrieve it now"));
    }

    #[test]
    fn intent_ignores_conversational_phrases() {
        assert!(!llm_signals_tool_intent("Let me explain this"));
        assert!(!llm_signals_tool_intent("Let me know what you think"));
    }

    #[test]
    fn intent_ignores_fenced_code_blocks() {
        let text = "Here is code:\n```\nlet me search the index\n```\nDone.";
        assert!(!llm_signals_tool_intent(text));
    }

    #[test]
    fn intent_ignores_indented_code() {
        let text = "Example:\n    let me search\nDone.";
        assert!(!llm_signals_tool_intent(text));
    }

    #[test]
    fn intent_ignores_quoted_strings() {
        let text = r#"The user said "let me search" earlier."#;
        assert!(!llm_signals_tool_intent(text));
    }

    #[test]
    fn truncate_short_string_borrows() {
        let result = truncate_for_preview("hello", 10);
        assert!(matches!(result, Cow::Borrowed("hello")));
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        let result = truncate_for_preview("hello world", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn truncate_multibyte_safe() {
        let result = truncate_for_preview("café", 4);
        assert_eq!(result, "caf...");
    }

    #[test]
    fn floor_char_boundary_mid_multibyte() {
        // "déf": d=byte 0, é=bytes 1..3, f=byte 3. Byte 2 is mid-é → back to 1.
        let s = "déf";
        assert_eq!(floor_char_boundary(s, 2), 1);
    }

    #[test]
    fn floor_char_boundary_past_end() {
        assert_eq!(floor_char_boundary("hi", 100), 2);
    }

    #[test]
    fn floor_char_boundary_zero() {
        assert_eq!(floor_char_boundary("hello", 0), 0);
    }

    #[test]
    fn floor_char_boundary_empty() {
        assert_eq!(floor_char_boundary("", 5), 0);
    }
}

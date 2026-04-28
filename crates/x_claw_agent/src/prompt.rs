//! Prompt assembly contracts shared across `claw-code` and `ironclaw` runtimes.
//!
//! Currently exposes a single contract — the boundary marker that splits the
//! cacheable static prompt prefix from the volatile dynamic suffix.
//!
//! # Background
//!
//! `claw-code/rust/crates/runtime/src/prompt.rs` (upstream baseline) defines a
//! [`SystemPromptBuilder`] that emits a `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__`
//! marker between the static scaffolding and the project-context tail. The
//! ironclaw fork's `LayeredPromptBuilder` independently evolved a similar
//! scheme but historically used a different literal (`__PROMPT_CACHE_BOUNDARY__`).
//!
//! The two literals served the same purpose — let Anthropic's automatic
//! caching place its breakpoint between the stable prefix and the volatile
//! tail — but having two literals meant tools observing the marker (e.g. cache
//! hit counters, log scrubbers) had to know about both.
//!
//! As of W3-A Phase 0 P0-1 (ADR-112 §5) the two literals are unified to
//! [`PROMPT_CACHE_BOUNDARY`], using the claw-code upstream literal so that
//! claw-code's existing public `pub const` export and downstream consumers do
//! not break.
//!
//! # Future work
//!
//! Issue [#38](https://github.com/Linnanli/xClaw/issues/38) tracks the
//! follow-up unification of the two builder implementations themselves
//! (`SystemPromptBuilder` vs `LayeredPromptBuilder`). This crate intentionally
//! does not host an assembler trait yet — that lands once the unified
//! implementation is designed.
//!
//! [`SystemPromptBuilder`]: https://github.com/Linnanli/xClaw/blob/xClaw/claw-code/rust/crates/runtime/src/prompt.rs

/// Marker that separates the cacheable static prompt prefix from the volatile
/// dynamic suffix.
///
/// Anthropic's automatic prompt caching uses the longest stable prefix as its
/// cache key. Embedding this marker (typically inside an HTML comment so it is
/// inert for the model) lets the prefix terminate at a deterministic byte
/// offset, dramatically improving cache hit rate as dynamic context churns.
///
/// Non-caching providers (OpenAI, etc.) ignore the marker — assemblers should
/// only emit it when the target model supports cache control.
///
/// The literal is fixed; do **not** rename or reformat it without coordinating
/// with both `claw-code/runtime` and `ironclaw/llm/prompt/`.
pub const PROMPT_CACHE_BOUNDARY: &str = "__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_literal_is_stable() {
        // Pin the literal — any change here ripples to claw-code prompt.rs and
        // ironclaw prompt/mod.rs and must be made deliberately.
        assert_eq!(PROMPT_CACHE_BOUNDARY, "__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__");
    }
}

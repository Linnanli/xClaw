//! Layered prompt architecture — static/dynamic separation for cache optimization.
//!
//! The prompt is split into two layers:
//!
//! | Layer   | Content                                          | Changes       | Cache       |
//! |---------|--------------------------------------------------|---------------|-------------|
//! | Static  | Identity + rules + tool specs + safety policy     | < 1×/week     | Long (1h)   |
//! | Dynamic | Environment + memory + skills + MCP + admin policy| Every call    | None        |
//!
//! A boundary marker (see [`x_claw_agent::PROMPT_CACHE_BOUNDARY`]) separates
//! the two layers so Anthropic's automatic caching places the cache breakpoint
//! correctly. Non-Anthropic models receive the combined prompt with no marker.

mod dynamic_layer;
mod static_layer;

pub use dynamic_layer::DynamicLayerInput;
pub use static_layer::StaticLayer;

use std::sync::Arc;

use x_claw_agent::PROMPT_CACHE_BOUNDARY;

use crate::llm::ToolDefinition;

/// HTML-comment-wrapped boundary marker injected between the static and dynamic
/// layers. The wrapping is inert for the model but anchors Anthropic's cache
/// breakpoint at a deterministic byte offset.
fn cache_boundary_section() -> String {
    format!("\n\n<!-- {PROMPT_CACHE_BOUNDARY} -->\n\n")
}

/// A pre-built layered prompt ready for use.
#[derive(Debug, Clone)]
pub struct LayeredPrompt {
    /// The fully assembled system prompt string.
    pub text: String,
}

/// Builds system prompts with static/dynamic separation.
///
/// The static layer is computed once at construction and cached in an
/// `Arc<String>`. The dynamic layer is rebuilt every call from the provided
/// [`DynamicLayerInput`]. Provider-side prefix caching relies on byte-level
/// stability of the static prefix, not on a builder-side hash; see ADR-117
/// §2.2 D6 for why the previous `static_hash` / `refresh_static` machinery
/// was removed.
pub struct LayeredPromptBuilder {
    /// Cached static layer text.
    static_layer: Arc<String>,
    /// Whether the model supports Anthropic-style cache control.
    supports_cache_boundary: bool,
}

impl LayeredPromptBuilder {
    /// Create a new builder with the given tool definitions and static config.
    pub fn new(tools: &[ToolDefinition], static_config: &StaticLayerConfig) -> Self {
        let static_text = StaticLayer::build(tools, static_config);
        Self {
            static_layer: Arc::new(static_text),
            supports_cache_boundary: false,
        }
    }

    /// Enable cache boundary marker (for Anthropic models).
    pub fn with_cache_boundary(mut self, enabled: bool) -> Self {
        self.supports_cache_boundary = enabled;
        self
    }

    /// Build the full system prompt by combining static + dynamic layers.
    pub fn build(&self, dynamic: &DynamicLayerInput) -> LayeredPrompt {
        let dynamic_text = dynamic_layer::build(dynamic);

        let text = if self.supports_cache_boundary {
            format!(
                "{}{}{}",
                self.static_layer,
                cache_boundary_section(),
                dynamic_text
            )
        } else {
            format!("{}\n\n{}", self.static_layer, dynamic_text)
        };

        LayeredPrompt { text }
    }
}

/// Configuration inputs for the static layer that rarely change.
#[derive(Debug, Clone, Default)]
pub struct StaticLayerConfig {
    /// Workspace identity prompt (AGENTS.md + SOUL.md + IDENTITY.md etc.).
    pub identity: String,
    /// Model name for runtime section.
    pub model_name: String,
    /// Whether the model has native thinking (Qwen3, DeepSeek-R1).
    pub has_native_thinking: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tools() -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "shell".into(),
                description: "Run a shell command".into(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "read_file".into(),
                description: "Read file contents".into(),
                parameters: serde_json::json!({}),
            },
        ]
    }

    fn sample_config() -> StaticLayerConfig {
        StaticLayerConfig {
            identity: "You are a helpful assistant.".into(),
            model_name: "claude-sonnet-4-20250514".into(),
            has_native_thinking: false,
        }
    }

    #[test]
    fn test_build_includes_cache_boundary_when_enabled() {
        // 字面量沿用 claw-code 上游基线（PROMPT_CACHE_BOUNDARY = __SYSTEM_PROMPT_DYNAMIC_BOUNDARY__）
        let tools = sample_tools();
        let config = sample_config();
        let builder = LayeredPromptBuilder::new(&tools, &config).with_cache_boundary(true);
        let prompt = builder.build(&DynamicLayerInput::default());
        assert!(prompt.text.contains(PROMPT_CACHE_BOUNDARY));
    }

    #[test]
    fn test_build_omits_cache_boundary_when_disabled() {
        let tools = sample_tools();
        let config = sample_config();
        let builder = LayeredPromptBuilder::new(&tools, &config).with_cache_boundary(false);
        let prompt = builder.build(&DynamicLayerInput::default());
        assert!(!prompt.text.contains(PROMPT_CACHE_BOUNDARY));
    }

    #[test]
    fn test_build_contains_static_and_dynamic_content() {
        let tools = sample_tools();
        let config = StaticLayerConfig {
            identity: "Custom identity prompt.".into(),
            model_name: "gpt-4".into(),
            has_native_thinking: false,
        };
        let dynamic = DynamicLayerInput {
            skill_context: Some("Use pytest for testing.".into()),
            ..Default::default()
        };
        let builder = LayeredPromptBuilder::new(&tools, &config);
        let prompt = builder.build(&dynamic);
        // Static layer content
        assert!(prompt.text.contains("shell"));
        assert!(prompt.text.contains("Custom identity prompt"));
        // Dynamic layer content
        assert!(prompt.text.contains("pytest"));
    }

    /// ADR-117 D9.3: provider universality — when serialized as the body of an
    /// OpenAI ChatCompletion `system` message (a JSON string), the assembled
    /// prompt round-trips untouched. The boundary marker, when present, is an
    /// inert HTML comment that does not break OpenAI's request schema.
    #[test]
    fn test_openai_compat_prompt_serializes() {
        let tools = sample_tools();
        let config = sample_config();
        let builder = LayeredPromptBuilder::new(&tools, &config).with_cache_boundary(false);
        let dynamic = DynamicLayerInput {
            environment: Some("cwd: /x\ndate: 2026-05-01\nplatform: linux".into()),
            ..Default::default()
        };
        let text = builder.build(&dynamic).text;
        // OpenAI does not consume the boundary marker — it must be absent on
        // non-Anthropic models.
        assert!(!text.contains(PROMPT_CACHE_BOUNDARY));
        // Round-trip through serde_json (OpenAI request body shape).
        let payload = serde_json::json!({
            "role": "system",
            "content": text.clone(),
        });
        let encoded = serde_json::to_string(&payload).expect("openai system message serializes");
        let decoded: serde_json::Value =
            serde_json::from_str(&encoded).expect("openai system message round-trips");
        assert_eq!(decoded["content"].as_str().unwrap(), text);
        assert!(
            decoded["content"]
                .as_str()
                .unwrap()
                .contains("## Environment")
        );
    }

    /// ADR-117 D9.3: provider universality — for Anthropic the cache boundary
    /// marker is emitted as an inert HTML comment. It must survive JSON
    /// serialization unchanged so the on-wire bytes (and therefore prefix-cache
    /// hashing) stay deterministic.
    #[test]
    fn test_anthropic_compat_prompt_serializes() {
        let tools = sample_tools();
        let config = sample_config();
        let builder = LayeredPromptBuilder::new(&tools, &config).with_cache_boundary(true);
        let dynamic = DynamicLayerInput {
            environment: Some("cwd: /x\ndate: 2026-05-01\nplatform: macos".into()),
            ..Default::default()
        };
        let text = builder.build(&dynamic).text;
        assert!(text.contains(PROMPT_CACHE_BOUNDARY));
        assert!(
            text.contains("<!--"),
            "marker is wrapped as an inert HTML comment"
        );
        // Anthropic system block — array form. The boundary must round-trip.
        let payload = serde_json::json!([{ "type": "text", "text": text.clone() }]);
        let encoded = serde_json::to_string(&payload).expect("anthropic system block serializes");
        let decoded: serde_json::Value =
            serde_json::from_str(&encoded).expect("anthropic system block round-trips");
        let decoded_text = decoded[0]["text"].as_str().unwrap();
        assert_eq!(decoded_text, text);
        assert!(decoded_text.contains(PROMPT_CACHE_BOUNDARY));
    }
}

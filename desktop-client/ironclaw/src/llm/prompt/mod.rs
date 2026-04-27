//! Layered prompt architecture — static/dynamic separation for cache optimization.
//!
//! The prompt is split into two layers:
//!
//! | Layer   | Content                                          | Changes       | Cache       |
//! |---------|--------------------------------------------------|---------------|-------------|
//! | Static  | Identity + rules + tool specs + safety policy     | < 1×/week     | Long (1h)   |
//! | Dynamic | Environment + memory + skills + MCP + admin policy| Every call    | None        |
//!
//! A boundary marker (`__PROMPT_CACHE_BOUNDARY__`) separates the two layers
//! so Anthropic's automatic caching places the cache breakpoint correctly.
//! Non-Anthropic models receive the combined prompt with no marker.

mod dynamic_layer;
mod static_layer;

pub use dynamic_layer::DynamicLayerInput;
pub use static_layer::StaticLayer;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::llm::ToolDefinition;

/// Marker that separates the cached (static) portion from the uncached
/// (dynamic) portion.  Anthropic's automatic caching uses the prefix of the
/// system prompt for cache key computation — keeping the static portion
/// stable dramatically improves hit rate.
const CACHE_BOUNDARY: &str = "\n\n<!-- __PROMPT_CACHE_BOUNDARY__ -->\n\n";

/// A pre-built layered prompt ready for use.
#[derive(Debug, Clone)]
pub struct LayeredPrompt {
    /// The fully assembled system prompt string.
    pub text: String,
    /// Whether the static layer was rebuilt (hash changed).
    pub static_changed: bool,
}

/// Builds system prompts with static/dynamic separation.
///
/// The static layer is computed once and cached in an `Arc<String>`.
/// It is only rebuilt when the `static_hash` changes (tool set change,
/// config change, etc.).
///
/// The dynamic layer is rebuilt every call from the provided `DynamicLayerInput`.
pub struct LayeredPromptBuilder {
    /// Cached static layer text.
    static_layer: Arc<String>,
    /// Hash of the inputs that produced `static_layer`.
    static_hash: u64,
    /// Whether the model supports Anthropic-style cache control.
    supports_cache_boundary: bool,
}

impl LayeredPromptBuilder {
    /// Create a new builder with the given tool definitions and static config.
    pub fn new(tools: &[ToolDefinition], static_config: &StaticLayerConfig) -> Self {
        let static_text = StaticLayer::build(tools, static_config);
        let hash = Self::compute_hash(tools, static_config);
        Self {
            static_layer: Arc::new(static_text),
            static_hash: hash,
            supports_cache_boundary: false,
        }
    }

    /// Enable cache boundary marker (for Anthropic models).
    pub fn with_cache_boundary(mut self, enabled: bool) -> Self {
        self.supports_cache_boundary = enabled;
        self
    }

    /// Rebuild the static layer only if inputs have changed.
    /// Returns `true` if the layer was actually rebuilt.
    pub fn refresh_static(
        &mut self,
        tools: &[ToolDefinition],
        static_config: &StaticLayerConfig,
    ) -> bool {
        let new_hash = Self::compute_hash(tools, static_config);
        if new_hash == self.static_hash {
            return false;
        }
        self.static_layer = Arc::new(StaticLayer::build(tools, static_config));
        self.static_hash = new_hash;
        true
    }

    /// Build the full system prompt by combining static + dynamic layers.
    pub fn build(&self, dynamic: &DynamicLayerInput) -> LayeredPrompt {
        let dynamic_text = dynamic_layer::build(dynamic);

        let text = if self.supports_cache_boundary {
            format!("{}{}{}", self.static_layer, CACHE_BOUNDARY, dynamic_text)
        } else {
            format!("{}\n\n{}", self.static_layer, dynamic_text)
        };

        LayeredPrompt {
            text,
            static_changed: false,
        }
    }

    /// Get the current static layer hash.
    pub fn static_hash(&self) -> u64 {
        self.static_hash
    }

    fn compute_hash(tools: &[ToolDefinition], config: &StaticLayerConfig) -> u64 {
        let mut hasher = DefaultHasher::new();
        for t in tools {
            t.name.hash(&mut hasher);
            t.description.hash(&mut hasher);
        }
        config.identity.hash(&mut hasher);
        config.has_native_thinking.hash(&mut hasher);
        config.model_name.hash(&mut hasher);
        hasher.finish()
    }
}

/// Configuration inputs for the static layer that rarely change.
#[derive(Debug, Clone, Hash)]
pub struct StaticLayerConfig {
    /// Workspace identity prompt (AGENTS.md + SOUL.md + IDENTITY.md etc.).
    pub identity: String,
    /// Model name for runtime section.
    pub model_name: String,
    /// Whether the model has native thinking (Qwen3, DeepSeek-R1).
    pub has_native_thinking: bool,
}

impl Default for StaticLayerConfig {
    fn default() -> Self {
        Self {
            identity: String::new(),
            model_name: String::new(),
            has_native_thinking: false,
        }
    }
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
    fn test_static_hash_stable_for_same_inputs() {
        let tools = sample_tools();
        let config = sample_config();
        let b1 = LayeredPromptBuilder::new(&tools, &config);
        let b2 = LayeredPromptBuilder::new(&tools, &config);
        assert_eq!(b1.static_hash(), b2.static_hash());
    }

    #[test]
    fn test_static_hash_changes_on_tool_change() {
        let config = sample_config();
        let tools1 = sample_tools();
        let tools2 = vec![ToolDefinition {
            name: "grep_search".into(),
            description: "Search files".into(),
            parameters: serde_json::json!({}),
        }];
        let b1 = LayeredPromptBuilder::new(&tools1, &config);
        let b2 = LayeredPromptBuilder::new(&tools2, &config);
        assert_ne!(b1.static_hash(), b2.static_hash());
    }

    #[test]
    fn test_refresh_static_returns_false_when_unchanged() {
        let tools = sample_tools();
        let config = sample_config();
        let mut builder = LayeredPromptBuilder::new(&tools, &config);
        assert!(!builder.refresh_static(&tools, &config));
    }

    #[test]
    fn test_refresh_static_returns_true_when_tools_change() {
        let tools = sample_tools();
        let config = sample_config();
        let mut builder = LayeredPromptBuilder::new(&tools, &config);
        let new_tools = vec![ToolDefinition {
            name: "git_status".into(),
            description: "Git status".into(),
            parameters: serde_json::json!({}),
        }];
        assert!(builder.refresh_static(&new_tools, &config));
    }

    #[test]
    fn test_build_includes_cache_boundary_when_enabled() {
        let tools = sample_tools();
        let config = sample_config();
        let builder = LayeredPromptBuilder::new(&tools, &config).with_cache_boundary(true);
        let prompt = builder.build(&DynamicLayerInput::default());
        assert!(prompt.text.contains("__PROMPT_CACHE_BOUNDARY__"));
    }

    #[test]
    fn test_build_omits_cache_boundary_when_disabled() {
        let tools = sample_tools();
        let config = sample_config();
        let builder = LayeredPromptBuilder::new(&tools, &config).with_cache_boundary(false);
        let prompt = builder.build(&DynamicLayerInput::default());
        assert!(!prompt.text.contains("__PROMPT_CACHE_BOUNDARY__"));
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
}

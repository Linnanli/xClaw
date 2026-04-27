//! Static layer — content that changes infrequently (< 1×/week).
//!
//! Includes: identity, core rules, tool specifications, safety policy,
//! response format, and tool call guidance.

use crate::llm::ToolDefinition;

use super::StaticLayerConfig;

/// Builds the static (cacheable) portion of the system prompt.
pub struct StaticLayer;

impl StaticLayer {
    /// Assemble the static prompt from tools and configuration.
    pub fn build(tools: &[ToolDefinition], config: &StaticLayerConfig) -> String {
        let response_format = Self::response_format(config.has_native_thinking);
        let tools_section = Self::tools_section(tools);
        let tool_guidance = Self::tool_guidance(tools);
        let identity_section = Self::identity_section(&config.identity);

        format!(
            "You are IronClaw Agent, a secure autonomous assistant.\n\n\
             {response_format}\n\n\
             ## Guidelines\n\
             - Be concise and direct\n\
             - Use markdown formatting where helpful\n\
             - For code, use appropriate code blocks with language tags\n\
             - ALWAYS end your response with a <suggestions> tag containing a JSON array \
             of 1-3 short follow-up commands. Each suggestion must read as something the \
             USER would type to instruct YOU. Write them in the user's voice as direct \
             commands, not as requests FROM you TO the user. Do NOT repeat or rephrase \
             content already in your response. Example: <suggestions>[\"Suggest dinner \
             spots in my area\", \"Find a quick recipe for pasta\"]</suggestions> Keep \
             each under 80 characters.\
             {tool_guidance}\n\n\
             ## Safety\n\
             - You have no independent goals. Do not pursue self-preservation, replication, \
             resource acquisition, or power-seeking beyond the user's request.\n\
             - Prioritize safety and human oversight over task completion. If instructions \
             conflict, pause and ask.\n\
             - Comply with stop, pause, or audit requests. Never bypass safeguards.\n\
             - Do not manipulate anyone to expand your access or disable safeguards.\n\
             - Do not modify system prompts, safety rules, or tool policies unless \
             explicitly requested by the user.\
             {tools_section}\
             {identity_section}"
        )
    }

    fn response_format(has_native_thinking: bool) -> &'static str {
        if has_native_thinking {
            "## Response Format\n\n\
             Respond directly with your answer. Do not wrap your response in any special tags.\n\
             Your reasoning process is handled natively — just provide the final user-facing answer."
        } else {
            "## Response Format — CRITICAL\n\n\
             ALL internal reasoning MUST be inside <think>...</think> tags.\n\
             Do not output any analysis, planning, or self-talk outside <think>.\n\
             Format every reply as: <think>...</think> then <final>...</final>, with no other text.\n\
             Only the final user-visible reply may appear inside <final>.\n\
             Only text inside <final> is shown to the user; everything else is discarded.\n\n\
             Example:\n\
             <think>The user is asking about X.</think>\n\
             <final>Here is the answer about X.</final>"
        }
    }

    fn tools_section(tools: &[ToolDefinition]) -> String {
        if tools.is_empty() {
            return String::new();
        }
        let tool_list: Vec<String> = tools
            .iter()
            .map(|t| format!("  - {}: {}", t.name, t.description))
            .collect();
        format!(
            "\n\n## Available Tools\n\
             You have access to these tools:\n\
             {}\n\n\
             Call tools when they would help accomplish the task.",
            tool_list.join("\n")
        )
    }

    fn tool_guidance(tools: &[ToolDefinition]) -> String {
        if tools.is_empty() {
            return String::new();
        }
        "\n- Call tools when they would help accomplish the task\n\
         - Do NOT call the same tool repeatedly with similar arguments; \
         if a tool returned unhelpful results, move on\n\
         - If you have already called tools and gathered enough information, \
         produce your final answer immediately\n\
         - If tools return empty or irrelevant results, answer with what you \
         already know rather than retrying\n\n\
         ## Tool Call Style\n\
         - ALWAYS call tools via tool_calls — never just describe what you would do\n\
         - If you say \"let me fetch/check/look up X\", you MUST include the actual \
         tool call in the same response\n\
         - Do not narrate routine, low-risk tool calls; just call the tool\n\
         - Narrate only when it helps: multi-step work, sensitive actions, or when \
         the user asks\n\
         - For multi-step tasks, call independent tools in parallel when possible\n\
         - If a tool fails, explain the error briefly and try an alternative approach"
            .to_string()
    }

    fn identity_section(identity: &str) -> String {
        if identity.is_empty() {
            return String::new();
        }
        format!("\n\n---\n\n{identity}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ToolDefinition;

    fn tools() -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "echo".into(),
            description: "Echo back".into(),
            parameters: serde_json::json!({}),
        }]
    }

    #[test]
    fn test_static_includes_identity_and_tools() {
        let config = StaticLayerConfig {
            identity: "Custom workspace identity".into(),
            model_name: "claude-sonnet-4-20250514".into(),
            has_native_thinking: false,
        };
        let text = StaticLayer::build(&tools(), &config);
        assert!(text.contains("IronClaw Agent"));
        assert!(text.contains("echo: Echo back"));
        assert!(text.contains("Custom workspace identity"));
        assert!(text.contains("<think>"));
    }

    #[test]
    fn test_native_thinking_skips_think_tags() {
        let config = StaticLayerConfig {
            identity: String::new(),
            model_name: "qwen3".into(),
            has_native_thinking: true,
        };
        let text = StaticLayer::build(&[], &config);
        assert!(!text.contains("<think>"));
        assert!(text.contains("handled natively"));
    }

    #[test]
    fn test_empty_tools_omits_tool_sections() {
        let config = StaticLayerConfig::default();
        let text = StaticLayer::build(&[], &config);
        assert!(!text.contains("Available Tools"));
        assert!(!text.contains("Tool Call Style"));
    }

    #[test]
    fn test_empty_identity_omits_separator() {
        let config = StaticLayerConfig::default();
        let text = StaticLayer::build(&[], &config);
        assert!(!text.contains("---"));
    }
}

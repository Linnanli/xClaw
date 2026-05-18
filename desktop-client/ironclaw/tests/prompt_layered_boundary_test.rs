//! P0-1 红测：Prompt 装配三连收口（W3-A Phase 0 ADR-112 §5）
//!
//! 三层断言：
//! - **(c) 常量统一**：`dasclaw_core::PROMPT_CACHE_BOUNDARY` 是单一权威字面量，
//!   等于 claw-code 上游基线 `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__`。
//! - **(a) Builder 单一路径**：ironclaw `build_system_prompt_with_tools` 对 Claude
//!   模型永远输出含 boundary 的 prompt，不依赖 `IRONCLAW_PROMPT_LAYERING` 环境变量。
//! - **(b) Env var 移除**：源码中不再出现 `IRONCLAW_PROMPT_LAYERING` 字符串
//!   （元测试，扫描 reasoning.rs / prompt/ 目录）。
//!
//! 修订自 ADR-112 §5：原文"3 builder → 1 LayeredPromptBuilder"实际是
//! "1 builder 内部双分支收敛为单 layered 路径"，详见 issue #38。

use std::path::Path;

use ironclaw::llm::{Reasoning, ToolDefinition};
use ironclaw::testing::StubLlm;
use std::sync::Arc;

/// (c) 常量统一断言：单一权威字面量。
#[test]
fn req_p01_c_prompt_cache_boundary_constant_matches_claw_code_baseline() {
    // 字面量沿用 claw-code 上游 SYSTEM_PROMPT_DYNAMIC_BOUNDARY（Q1-A 决策）
    assert_eq!(
        dasclaw_core::PROMPT_CACHE_BOUNDARY,
        "__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__",
        "PROMPT_CACHE_BOUNDARY must match claw-code upstream baseline literal"
    );
}

/// (a) Builder 单一路径断言：Claude 模型永远输出含 boundary。
///
/// P0-1 之前需要 `IRONCLAW_PROMPT_LAYERING=1` env var 才走 layered 路径；
/// P0-1 之后 layered 是唯一路径，不依赖 env，所以这个测试不主动 set/unset env。
#[test]
fn req_p01_a_build_system_prompt_with_tools_always_layered_for_claude() {
    let llm = Arc::new(StubLlm::new("test"));
    let reasoning = Reasoning::new(llm).with_model_name("claude-sonnet-4-20250514");

    let prompt = reasoning.build_system_prompt_with_tools(&[]);

    assert!(
        prompt.contains(dasclaw_core::PROMPT_CACHE_BOUNDARY),
        "Claude model must always get cache boundary marker (env-independent), \
         got prompt:\n{prompt}"
    );
}

/// (a) Builder 单一路径断言：非 Claude 模型不输出 boundary（语义保留）。
#[test]
fn req_p01_a_build_system_prompt_with_tools_no_boundary_for_non_claude() {
    let llm = Arc::new(StubLlm::new("test"));
    let reasoning = Reasoning::new(llm).with_model_name("gpt-4o");

    let prompt = reasoning.build_system_prompt_with_tools(&[]);

    assert!(
        !prompt.contains(dasclaw_core::PROMPT_CACHE_BOUNDARY),
        "Non-Claude model must NOT get cache boundary marker"
    );
}

/// (a) 集成断言：Layered 输出包含 static + dynamic 两层内容。
#[test]
fn req_p01_a_build_system_prompt_includes_static_and_dynamic_layers() {
    let llm = Arc::new(StubLlm::new("test"));
    let reasoning = Reasoning::new(llm)
        .with_system_prompt("My workspace rules".to_string())
        .with_skill_context("Always use TDD".to_string());
    let tool_defs = vec![ToolDefinition {
        name: "echo".to_string(),
        description: "Echoes input".to_string(),
        parameters: serde_json::json!({}),
    }];

    let prompt = reasoning.build_system_prompt_with_tools(&tool_defs);

    assert!(
        prompt.contains("My workspace rules"),
        "static layer must contain identity"
    );
    assert!(
        prompt.contains("echo"),
        "static layer must contain tool name"
    );
    assert!(
        prompt.contains("Always use TDD"),
        "dynamic layer must contain skill context"
    );
}

/// (b) Env var 移除断言（元测试）：源码不再引用 IRONCLAW_PROMPT_LAYERING。
#[test]
fn req_p01_b_env_var_removed_from_source() {
    // 扫描 ironclaw/src/llm 目录下与 prompt 装配相关的文件
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let candidates = [
        "src/llm/reasoning.rs",
        "src/llm/prompt/mod.rs",
        "src/llm/prompt/static_layer.rs",
        "src/llm/prompt/dynamic_layer.rs",
    ];

    for rel in candidates {
        let path = Path::new(manifest_dir).join(rel);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        assert!(
            !content.contains("IRONCLAW_PROMPT_LAYERING"),
            "{} still references IRONCLAW_PROMPT_LAYERING env var (P0-1 b 未完成)",
            path.display()
        );
    }
}

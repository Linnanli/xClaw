//! Full-chain tool reachability test.
//!
//! Ensures every tool registered via `register_builtin_tools()` and
//! `register_dev_tools()` **actually** appears in `tool_definitions()`
//! and can be looked up by name via `registry.get()`.
//!
//! This catches:
//!  - Tools added to `mod.rs` but not to `registry.rs` (invisible to LLM)
//!  - Tools registered but with name mismatches between `name()` and schema
//!  - Tools whose `parameters_schema()` panics
//!
//! Run: `cargo test -p ironclaw --test tool_reachability`

use std::collections::BTreeSet;
use std::sync::Arc;

use ironclaw::tools::ToolRegistry;

/// All tool names that `register_builtin_tools()` should register.
/// Note: Extension tools, image tools, restart, etc. are registered
/// separately via dedicated methods that require extra state.
const BUILTIN_TOOLS: &[&str] = &["echo", "time", "json", "http"];

/// All tool names that `register_dev_tools()` should register.
const DEV_TOOLS: &[&str] = &[
    "shell",
    "read_file",
    "write_file",
    "list_dir",
    "apply_patch",
    "code_edit",
    "grep_search",
    "glob_search",
    "git_status",
    "git_diff",
    "git_log",
    "git_commit",
    "git_branch",
    "git_push",
    "git_stale_check",
    "lsp_query",
    "plan_mode",
    "session_fork",
    "sub_agent",
    "web_search",
    "web_fetch",
];

fn create_full_registry() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    registry.register_builtin_tools();
    registry.register_dev_tools();
    registry
}

/// Every tool registered at startup must be retrievable by name.
#[tokio::test]
async fn test_all_registered_tools_reachable_by_name() {
    let registry = create_full_registry();
    let all_names = registry.list().await;
    let all_set: BTreeSet<String> = all_names.into_iter().collect();

    let mut missing = Vec::new();
    for &name in BUILTIN_TOOLS.iter().chain(DEV_TOOLS.iter()) {
        if !all_set.contains(name) {
            missing.push(name);
        }
    }

    assert!(
        missing.is_empty(),
        "Tools missing from registry: {missing:?}\nRegistered tools: {all_set:?}",
    );
}

/// Every tool in the registry must appear in `tool_definitions()`.
#[tokio::test]
async fn test_all_tools_in_definitions() {
    let registry = create_full_registry();
    let defs = registry.tool_definitions().await;
    let def_names: BTreeSet<String> = defs.iter().map(|d| d.name.clone()).collect();

    let all_names = registry.list().await;
    let mut missing_from_defs = Vec::new();
    for name in &all_names {
        if !def_names.contains(name) {
            missing_from_defs.push(name.clone());
        }
    }

    assert!(
        missing_from_defs.is_empty(),
        "Tools registered but missing from tool_definitions(): {missing_from_defs:?}",
    );
}

/// `get()` for every advertised tool name must return Some.
#[tokio::test]
async fn test_get_matches_list() {
    let registry = create_full_registry();
    let all_names = registry.list().await;

    let mut unreachable = Vec::new();
    for name in &all_names {
        if registry.get(name).await.is_none() {
            unreachable.push(name.clone());
        }
    }

    assert!(
        unreachable.is_empty(),
        "Tools listed but not gettable: {unreachable:?}",
    );
}

/// `parameters_schema()` for every tool must return valid JSON with a "type" field.
#[tokio::test]
async fn test_all_tools_have_valid_schema() {
    let registry = create_full_registry();
    let all_names = registry.list().await;

    let mut bad_schemas = Vec::new();
    for name in &all_names {
        let tool = registry.get(name).await.expect("tool should exist");
        let schema = tool.parameters_schema();
        if schema.get("type").is_none() {
            bad_schemas.push(name.clone());
        }
    }

    assert!(
        bad_schemas.is_empty(),
        "Tools with missing 'type' in schema: {bad_schemas:?}",
    );
}

/// `name()` for every tool must match its registry key.
#[tokio::test]
async fn test_tool_name_matches_registry_key() {
    let registry = create_full_registry();
    let all_names = registry.list().await;

    let mut mismatched = Vec::new();
    for name in &all_names {
        let tool = registry.get(name).await.expect("tool should exist");
        if tool.name() != name {
            mismatched.push(format!("registry key={name}, tool.name()={}", tool.name()));
        }
    }

    assert!(
        mismatched.is_empty(),
        "Tool name mismatches: {mismatched:?}",
    );
}

/// web_search and web_fetch specifically exist and have correct schemas.
#[tokio::test]
async fn test_web_tools_registered() {
    let registry = create_full_registry();

    // web_search
    let ws = registry
        .get("web_search")
        .await
        .expect("web_search should be registered");
    assert_eq!(ws.name(), "web_search");
    let ws_schema = ws.parameters_schema();
    let ws_required: Vec<String> =
        serde_json::from_value(ws_schema["required"].clone()).expect("required field");
    assert!(ws_required.contains(&"query".to_string()));

    // web_fetch
    let wf = registry
        .get("web_fetch")
        .await
        .expect("web_fetch should be registered");
    assert_eq!(wf.name(), "web_fetch");
    let wf_schema = wf.parameters_schema();
    let wf_required: Vec<String> =
        serde_json::from_value(wf_schema["required"].clone()).expect("required field");
    assert!(wf_required.contains(&"url".to_string()));
    assert!(wf_required.contains(&"prompt".to_string()));
}

/// No two tools share the same name.
#[tokio::test]
async fn test_no_duplicate_tool_names() {
    let registry = create_full_registry();
    let defs = registry.tool_definitions().await;
    let mut seen = BTreeSet::new();
    let mut dupes = Vec::new();
    for def in &defs {
        if !seen.insert(def.name.clone()) {
            dupes.push(def.name.clone());
        }
    }

    assert!(
        dupes.is_empty(),
        "Duplicate tool names in definitions: {dupes:?}",
    );
}

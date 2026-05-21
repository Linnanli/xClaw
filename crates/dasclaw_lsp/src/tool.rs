//! LspQueryTool — single tool that dispatches to all LSP actions.
//!
//! The LLM calls this with an `action`, `file_path`, and optional `position`
//! to perform code intelligence queries.
//!
//! Verbatim-moved from `desktop-client/ironclaw/src/tools/builtin/lsp/tool.rs`
//! per ADR-152 §3 F3.3 (tracking issue #688). ironclaw retains a
//! `pub use dasclaw_lsp::LspQueryTool;` shim so existing registration paths
//! continue to resolve.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;

use dasclaw_runtime::{JobContextCore, Tool};
use dasclaw_tool::{ApprovalRequirement, RiskLevel, ToolDomain, ToolError, ToolOutput};

use crate::{LspAction, LspClient, LspRegistry, path_to_uri};

/// LSP code intelligence tool.
///
/// Supports: goto_definition, find_references, diagnostics, hover,
/// document_symbols, rename, completions.
pub struct LspQueryTool {
    registry: Arc<LspRegistry>,
}

impl LspQueryTool {
    pub fn new(registry: Arc<LspRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LspQueryTool {
    fn name(&self) -> &str {
        "lsp_query"
    }

    fn description(&self) -> &str {
        "Query a language server for code intelligence: go to definition, \
         find references, diagnostics, hover info, document symbols, rename, \
         or completions. The language server is chosen automatically based on \
         the file extension."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "goto_definition",
                        "find_references",
                        "diagnostics",
                        "hover",
                        "document_symbols",
                        "rename",
                        "completions"
                    ],
                    "description": "The LSP action to perform."
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to the file (relative to workspace root)."
                },
                "line": {
                    "type": "integer",
                    "description": "0-based line number (required for actions that need a position)."
                },
                "column": {
                    "type": "integer",
                    "description": "0-based column number (required for actions that need a position)."
                },
                "new_name": {
                    "type": "string",
                    "description": "New name for the symbol (required for rename action)."
                }
            },
            "required": ["action", "file_path"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let start = Instant::now();

        let action: LspAction = serde_json::from_value(
            params
                .get("action")
                .cloned()
                .ok_or_else(|| ToolError::InvalidParameters("missing 'action'".into()))?,
        )
        .map_err(|e| ToolError::InvalidParameters(format!("invalid action: {e}")))?;

        let file_path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("missing 'file_path'".into()))?;

        let file_path = PathBuf::from(file_path_str);

        // Validate position for actions that require it.
        let line = params
            .get("line")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let col = params
            .get("column")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        if action.requires_position() && (line.is_none() || col.is_none()) {
            return Err(ToolError::InvalidParameters(format!(
                "action '{:?}' requires 'line' and 'column' parameters",
                action
            )));
        }

        // Resolve workspace root.
        let workspace_root = resolve_workspace_root(ctx);

        let abs_path = if file_path.is_absolute() {
            file_path.clone()
        } else {
            workspace_root.join(&file_path)
        };

        // Ensure the file exists before starting an LSP server.
        if !abs_path.exists() {
            return Err(ToolError::InvalidParameters(format!(
                "File not found: {}",
                abs_path.display()
            )));
        }

        // Get or start the language server.
        let lsp = self
            .registry
            .client_for_file(&abs_path, &workspace_root)
            .await?;

        // Read and open the file in the server.
        let file_uri = path_to_uri(&abs_path);
        let language_id = self
            .registry
            .language_id_for(&abs_path)
            .await
            .unwrap_or_else(|| "plaintext".into());

        let content = tokio::fs::read_to_string(&abs_path).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read {}: {}", abs_path.display(), e))
        })?;

        lsp.did_open(&file_uri, &language_id, &content).await?;

        let result = dispatch_action(&lsp, action, &file_uri, line, col, &params).await;

        // Close the file after the request (best effort).
        let _ = lsp.did_close(&file_uri).await;

        let text = result?;
        Ok(ToolOutput::text(text, start.elapsed()))
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn risk_level_for(&self, params: &serde_json::Value) -> RiskLevel {
        // Rename has side effects; everything else is read-only.
        let is_rename = params
            .get("action")
            .and_then(|v| v.as_str())
            .is_some_and(|a| a == "rename");
        if is_rename {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }

    fn requires_approval(&self, params: &serde_json::Value) -> ApprovalRequirement {
        let is_rename = params
            .get("action")
            .and_then(|v| v.as_str())
            .is_some_and(|a| a == "rename");
        if is_rename {
            ApprovalRequirement::UnlessAutoApproved
        } else {
            ApprovalRequirement::Never
        }
    }

    fn requires_sanitization(&self) -> bool {
        true
    }
}

/// Dispatch to the appropriate LSP method.
async fn dispatch_action(
    lsp: &LspClient,
    action: LspAction,
    file_uri: &str,
    line: Option<u32>,
    col: Option<u32>,
    params: &serde_json::Value,
) -> Result<String, ToolError> {
    match action {
        LspAction::GotoDefinition => {
            let result = lsp
                .request(
                    "textDocument/definition",
                    serde_json::Value::Object(position_params(file_uri, line, col)),
                )
                .await?;
            Ok(format_locations(&result))
        }
        LspAction::FindReferences => {
            let mut p = position_params(file_uri, line, col);
            p.insert(
                "context".into(),
                serde_json::json!({"includeDeclaration": true}),
            );
            let result = lsp
                .request("textDocument/references", serde_json::Value::Object(p))
                .await?;
            Ok(format_locations(&result))
        }
        LspAction::Diagnostics => {
            // LSP diagnostics are push-based. We request document diagnostics
            // (3.17+) if supported, or fall back to a hint.
            let result = lsp
                .request(
                    "textDocument/diagnostic",
                    serde_json::json!({
                        "textDocument": { "uri": file_uri }
                    }),
                )
                .await;
            match result {
                Ok(val) => Ok(format_diagnostics(&val)),
                Err(_) => Ok("Diagnostics are published asynchronously by this server. \
                     Try building the project to see errors."
                    .to_string()),
            }
        }
        LspAction::Hover => {
            let result = lsp
                .request(
                    "textDocument/hover",
                    serde_json::Value::Object(position_params(file_uri, line, col)),
                )
                .await?;
            Ok(format_hover(&result))
        }
        LspAction::DocumentSymbols => {
            let result = lsp
                .request(
                    "textDocument/documentSymbol",
                    serde_json::json!({
                        "textDocument": { "uri": file_uri }
                    }),
                )
                .await?;
            Ok(format_symbols(&result))
        }
        LspAction::Rename => {
            let new_name = params
                .get("new_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::InvalidParameters("'new_name' is required for rename".into())
                })?;
            let mut p = position_params(file_uri, line, col);
            p.insert("newName".into(), serde_json::Value::String(new_name.into()));
            let result = lsp
                .request("textDocument/rename", serde_json::Value::Object(p))
                .await?;
            Ok(format_workspace_edit(&result))
        }
        LspAction::Completions => {
            let result = lsp
                .request(
                    "textDocument/completion",
                    serde_json::Value::Object(position_params(file_uri, line, col)),
                )
                .await?;
            Ok(format_completions(&result))
        }
    }
}

/// Build a `TextDocumentPositionParams` JSON value.
fn position_params(
    uri: &str,
    line: Option<u32>,
    col: Option<u32>,
) -> serde_json::Map<String, serde_json::Value> {
    let value = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": {
            "line": line.unwrap_or(0),
            "character": col.unwrap_or(0)
        }
    });
    match value {
        serde_json::Value::Object(map) => map,
        // Unreachable: the literal above is always an object. Construct an
        // empty map defensively rather than panicking.
        _ => serde_json::Map::new(),
    }
}

// ---- Formatting helpers ----
// Each formatter converts raw LSP JSON into a concise, LLM-friendly string.

fn format_locations(value: &serde_json::Value) -> String {
    if value.is_null() {
        return "No results found.".into();
    }
    let locations = if value.is_array() {
        value.as_array().cloned().unwrap_or_default()
    } else {
        vec![value.clone()]
    };

    if locations.is_empty() {
        return "No results found.".into();
    }

    let mut lines = Vec::new();
    for loc in &locations {
        let uri = loc
            .get("uri")
            .or_else(|| loc.get("targetUri"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let range = loc.get("range").or_else(|| loc.get("targetRange"));
        let line_num = range
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .map(|l| l + 1) // Convert to 1-based
            .unwrap_or(0);

        let path = uri.strip_prefix("file://").unwrap_or(uri);
        lines.push(format!("  {}:{}", path, line_num));
    }

    format!(
        "Found {} location(s):\n{}",
        locations.len(),
        lines.join("\n")
    )
}

fn format_hover(value: &serde_json::Value) -> String {
    if value.is_null() {
        return "No hover information available.".into();
    }
    // Hover result has `contents` which can be a string, MarkupContent, or array.
    let contents = value.get("contents");
    match contents {
        Some(c) if c.is_string() => c.as_str().unwrap_or("").to_string(),
        Some(c) if c.is_object() => c
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        Some(c) if c.is_array() => c
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        if item.is_string() {
                            item.as_str().map(String::from)
                        } else {
                            item.get("value").and_then(|v| v.as_str()).map(String::from)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            })
            .unwrap_or_default(),
        _ => "No hover information available.".into(),
    }
}

fn format_diagnostics(value: &serde_json::Value) -> String {
    // textDocument/diagnostic returns { items: [...] }
    let items = value
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if items.is_empty() {
        return "No diagnostics (no errors or warnings).".into();
    }

    let mut lines = Vec::new();
    for diag in &items {
        let severity = match diag.get("severity").and_then(|s| s.as_u64()) {
            Some(1) => "ERROR",
            Some(2) => "WARN",
            Some(3) => "INFO",
            Some(4) => "HINT",
            _ => "DIAG",
        };
        let msg = diag.get("message").and_then(|m| m.as_str()).unwrap_or("?");
        let line_num = diag
            .get("range")
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .map(|l| l + 1)
            .unwrap_or(0);
        lines.push(format!("  L{}: [{}] {}", line_num, severity, msg));
    }

    format!("{} diagnostic(s):\n{}", items.len(), lines.join("\n"))
}

fn format_symbols(value: &serde_json::Value) -> String {
    let symbols = value.as_array().cloned().unwrap_or_default();
    if symbols.is_empty() {
        return "No symbols found.".into();
    }

    let mut lines = Vec::new();
    for sym in &symbols {
        let name = sym.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let kind_num = sym.get("kind").and_then(|k| k.as_u64()).unwrap_or(0);
        let kind = symbol_kind_name(kind_num);
        let line_num = sym
            .get("range")
            .or_else(|| sym.get("location").and_then(|l| l.get("range")))
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .map(|l| l + 1)
            .unwrap_or(0);
        lines.push(format!("  L{}: {} {}", line_num, kind, name));
    }

    format!("{} symbol(s):\n{}", symbols.len(), lines.join("\n"))
}

fn format_workspace_edit(value: &serde_json::Value) -> String {
    if value.is_null() {
        return "Rename not supported or no changes needed.".into();
    }
    let changes = value.get("changes").and_then(|c| c.as_object());
    match changes {
        Some(map) => {
            let total_edits: usize = map
                .values()
                .filter_map(|v| v.as_array())
                .map(|arr| arr.len())
                .sum();
            format!(
                "Rename would affect {} file(s) with {} edit(s).",
                map.len(),
                total_edits
            )
        }
        None => "Rename produced no changes.".into(),
    }
}

fn format_completions(value: &serde_json::Value) -> String {
    // Can be CompletionList { items: [...] } or just an array.
    let items = if let Some(arr) = value.as_array() {
        arr.clone()
    } else if let Some(arr) = value.get("items").and_then(|v| v.as_array()) {
        arr.clone()
    } else {
        return "No completions available.".into();
    };

    if items.is_empty() {
        return "No completions available.".into();
    }

    let max_show = 20;
    let mut lines = Vec::new();
    for item in items.iter().take(max_show) {
        let label = item.get("label").and_then(|l| l.as_str()).unwrap_or("?");
        let detail = item.get("detail").and_then(|d| d.as_str()).unwrap_or("");
        if detail.is_empty() {
            lines.push(format!("  {}", label));
        } else {
            lines.push(format!("  {} — {}", label, detail));
        }
    }

    let suffix = if items.len() > max_show {
        format!("\n  ... and {} more", items.len() - max_show)
    } else {
        String::new()
    };

    format!(
        "{} completion(s):\n{}{}",
        items.len(),
        lines.join("\n"),
        suffix
    )
}

/// Map LSP SymbolKind number to a human-readable name.
fn symbol_kind_name(kind: u64) -> &'static str {
    match kind {
        1 => "File",
        2 => "Module",
        3 => "Namespace",
        4 => "Package",
        5 => "Class",
        6 => "Method",
        7 => "Property",
        8 => "Field",
        9 => "Constructor",
        10 => "Enum",
        11 => "Interface",
        12 => "Function",
        13 => "Variable",
        14 => "Constant",
        15 => "String",
        16 => "Number",
        17 => "Boolean",
        18 => "Array",
        19 => "Object",
        20 => "Key",
        21 => "Null",
        22 => "EnumMember",
        23 => "Struct",
        24 => "Event",
        25 => "Operator",
        26 => "TypeParameter",
        _ => "Symbol",
    }
}

/// Resolve the workspace root from the job context or fall back to CWD.
fn resolve_workspace_root(ctx: &dyn JobContextCore) -> PathBuf {
    ctx.metadata()
        .get("workspace_root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_locations_null() {
        assert_eq!(
            format_locations(&serde_json::Value::Null),
            "No results found."
        );
    }

    #[test]
    fn test_format_locations_single() {
        let val = serde_json::json!({
            "uri": "file:///src/main.rs",
            "range": { "start": { "line": 9, "character": 0 }, "end": { "line": 9, "character": 5 } }
        });
        let text = format_locations(&val);
        assert!(text.contains("1 location"));
        assert!(text.contains("/src/main.rs:10"));
    }

    #[test]
    fn test_format_locations_array() {
        let val = serde_json::json!([
            { "uri": "file:///a.rs", "range": { "start": { "line": 0 }, "end": { "line": 0 } } },
            { "uri": "file:///b.rs", "range": { "start": { "line": 4 }, "end": { "line": 4 } } }
        ]);
        let text = format_locations(&val);
        assert!(text.contains("2 location"));
    }

    #[test]
    fn test_format_hover_string() {
        let val = serde_json::json!({ "contents": "fn main()" });
        assert_eq!(format_hover(&val), "fn main()");
    }

    #[test]
    fn test_format_hover_markup() {
        let val = serde_json::json!({
            "contents": { "kind": "plaintext", "value": "pub struct Foo" }
        });
        assert_eq!(format_hover(&val), "pub struct Foo");
    }

    #[test]
    fn test_format_diagnostics_empty() {
        let val = serde_json::json!({ "items": [] });
        assert_eq!(
            format_diagnostics(&val),
            "No diagnostics (no errors or warnings)."
        );
    }

    #[test]
    fn test_format_diagnostics_with_errors() {
        let val = serde_json::json!({
            "items": [
                { "severity": 1, "message": "unused variable", "range": { "start": { "line": 5 } } }
            ]
        });
        let text = format_diagnostics(&val);
        assert!(text.contains("1 diagnostic"));
        assert!(text.contains("[ERROR]"));
        assert!(text.contains("L6"));
    }

    #[test]
    fn test_format_symbols() {
        let val = serde_json::json!([
            { "name": "main", "kind": 12, "range": { "start": { "line": 0 } } },
            { "name": "Foo", "kind": 23, "range": { "start": { "line": 10 } } }
        ]);
        let text = format_symbols(&val);
        assert!(text.contains("2 symbol"));
        assert!(text.contains("Function main"));
        assert!(text.contains("Struct Foo"));
    }

    #[test]
    fn test_format_completions_list() {
        let val = serde_json::json!({
            "items": [
                { "label": "println!", "detail": "macro" },
                { "label": "print!" }
            ]
        });
        let text = format_completions(&val);
        assert!(text.contains("2 completion"));
        assert!(text.contains("println!"));
    }

    #[test]
    fn test_format_workspace_edit_changes() {
        let val = serde_json::json!({
            "changes": {
                "file:///a.rs": [{ "range": {}, "newText": "new_name" }],
                "file:///b.rs": [{ "range": {}, "newText": "new_name" }, { "range": {}, "newText": "new_name" }]
            }
        });
        let text = format_workspace_edit(&val);
        assert!(text.contains("2 file(s)"));
        assert!(text.contains("3 edit(s)"));
    }

    #[test]
    fn test_symbol_kind_name_known() {
        assert_eq!(symbol_kind_name(12), "Function");
        assert_eq!(symbol_kind_name(23), "Struct");
        assert_eq!(symbol_kind_name(5), "Class");
    }

    #[test]
    fn test_symbol_kind_name_unknown() {
        assert_eq!(symbol_kind_name(999), "Symbol");
    }

    #[test]
    fn test_tool_metadata() {
        let registry = Arc::new(LspRegistry::new());
        let tool = LspQueryTool::new(registry);
        assert_eq!(tool.name(), "lsp_query");
        assert_eq!(tool.domain(), ToolDomain::Container);
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_risk_level_read_only() {
        let registry = Arc::new(LspRegistry::new());
        let tool = LspQueryTool::new(registry);
        let params = serde_json::json!({"action": "hover"});
        assert_eq!(tool.risk_level_for(&params), RiskLevel::Low);
    }

    #[test]
    fn test_risk_level_rename() {
        let registry = Arc::new(LspRegistry::new());
        let tool = LspQueryTool::new(registry);
        let params = serde_json::json!({"action": "rename"});
        assert_eq!(tool.risk_level_for(&params), RiskLevel::Medium);
    }

    #[test]
    fn test_approval_never_for_read() {
        let registry = Arc::new(LspRegistry::new());
        let tool = LspQueryTool::new(registry);
        let params = serde_json::json!({"action": "goto_definition"});
        assert_eq!(tool.requires_approval(&params), ApprovalRequirement::Never);
    }

    #[test]
    fn test_approval_required_for_rename() {
        let registry = Arc::new(LspRegistry::new());
        let tool = LspQueryTool::new(registry);
        let params = serde_json::json!({"action": "rename"});
        assert_eq!(
            tool.requires_approval(&params),
            ApprovalRequirement::UnlessAutoApproved
        );
    }
}

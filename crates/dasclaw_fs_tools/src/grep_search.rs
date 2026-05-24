//! Grep search tool — regex-based content search across files.
//!
//! Provides the LLM with `grep`-like capability: search for a pattern across
//! files under a given directory, returning matching lines with optional
//! surrounding context. Uses the `regex` crate (already a project dependency).

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::file_guard;
use crate::path_utils::validate_path;
use dasclaw_runtime::Tool;
use dasclaw_tool::{ApprovalRequirement, ToolDomain, ToolError, ToolOutput, require_str};

/// Maximum number of results returned by default.
const DEFAULT_MAX_RESULTS: usize = 50;

/// Maximum number of results the caller can request.
const HARD_MAX_RESULTS: usize = 500;

/// Maximum directory depth to recurse.
const MAX_DEPTH: usize = 20;

/// Grep search tool.
#[derive(Debug, Default)]
pub struct GrepSearchTool {
    base_dir: Option<PathBuf>,
}

impl GrepSearchTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_dir(mut self, dir: PathBuf) -> Self {
        self.base_dir = Some(dir);
        self
    }
}

#[async_trait]
impl Tool for GrepSearchTool {
    fn name(&self) -> &str {
        "grep_search"
    }

    fn description(&self) -> &str {
        "Search for a regex pattern in files under a directory. Returns matching lines \
         with optional context lines. Binary files are automatically skipped."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: current directory)"
                },
                "context_before": {
                    "type": "integer",
                    "description": "Number of lines of context before each match (default: 0)"
                },
                "context_after": {
                    "type": "integer",
                    "description": "Number of lines of context after each match (default: 0)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return (default: 50, max: 500)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let start = std::time::Instant::now();

        let pattern_str = require_str(&params, "pattern")?;
        let path_str = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let ctx_before = extract_usize(&params, "context_before", 0);
        let ctx_after = extract_usize(&params, "context_after", 0);
        let max_results =
            extract_usize(&params, "max_results", DEFAULT_MAX_RESULTS).min(HARD_MAX_RESULTS);

        let re = regex::Regex::new(pattern_str)
            .map_err(|e| ToolError::InvalidParameters(format!("Invalid regex: {}", e)))?;

        let effective = super::path_utils::effective_base_dir(self.base_dir.as_deref(), ctx);
        let search_path = validate_path(path_str, effective.as_deref())?;

        let matches = if search_path.is_file() {
            search_file(&re, &search_path, ctx_before, ctx_after, max_results)?
        } else {
            search_dir(&re, &search_path, ctx_before, ctx_after, max_results, 0)?
        };

        let output = format_matches(&matches, search_path.as_path());
        Ok(ToolOutput::text(output, start.elapsed()))
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Never
    }
}

// ── Internal types ──────────────────────────────────────────────────────

#[derive(Debug)]
struct GrepMatch {
    file: PathBuf,
    line_number: usize,
    content: String,
    context_before: Vec<String>,
    context_after: Vec<String>,
}

// ── Search logic ────────────────────────────────────────────────────────

fn search_file(
    re: &regex::Regex,
    path: &Path,
    ctx_before: usize,
    ctx_after: usize,
    max_results: usize,
) -> Result<Vec<GrepMatch>, ToolError> {
    // Skip binary files
    let head = read_head(path, file_guard::BINARY_SNIFF_SIZE)?;
    if file_guard::is_binary(&head) {
        return Ok(Vec::new());
    }

    let file = std::fs::File::open(path).map_err(|e| {
        ToolError::ExecutionFailed(format!("Cannot open {}: {}", path.display(), e))
    })?;

    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

    let mut matches = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if matches.len() >= max_results {
            break;
        }
        if re.is_match(line) {
            let before = collect_context(&lines, i, ctx_before, true);
            let after = collect_context(&lines, i, ctx_after, false);
            matches.push(GrepMatch {
                file: path.to_path_buf(),
                line_number: i + 1,
                content: line.clone(),
                context_before: before,
                context_after: after,
            });
        }
    }
    Ok(matches)
}

fn search_dir(
    re: &regex::Regex,
    dir: &Path,
    ctx_before: usize,
    ctx_after: usize,
    max_results: usize,
    depth: usize,
) -> Result<Vec<GrepMatch>, ToolError> {
    if depth > MAX_DEPTH {
        return Ok(Vec::new());
    }

    let mut all_matches = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        ToolError::ExecutionFailed(format!("Cannot read dir {}: {}", dir.display(), e))
    })?;

    let mut sorted: Vec<_> = entries.filter_map(Result::ok).collect();
    sorted.sort_by_key(|e| e.file_name());

    for entry in sorted {
        if all_matches.len() >= max_results {
            break;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden dirs/files and common large dirs
        if name_str.starts_with('.') || is_ignored_dir(&name_str) {
            continue;
        }

        let remaining = max_results - all_matches.len();
        if path.is_dir() {
            let sub = search_dir(re, &path, ctx_before, ctx_after, remaining, depth + 1)?;
            all_matches.extend(sub);
        } else if path.is_file() {
            let file_matches = search_file(re, &path, ctx_before, ctx_after, remaining)?;
            all_matches.extend(file_matches);
        }
    }
    Ok(all_matches)
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn collect_context(lines: &[String], index: usize, count: usize, before: bool) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    if before {
        let start = index.saturating_sub(count);
        lines[start..index].to_vec()
    } else {
        let end = (index + 1 + count).min(lines.len());
        lines[index + 1..end].to_vec()
    }
}

fn read_head(path: &Path, n: usize) -> Result<Vec<u8>, ToolError> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| {
        ToolError::ExecutionFailed(format!("Cannot open {}: {}", path.display(), e))
    })?;
    let mut buf = vec![0u8; n];
    let read = file.read(&mut buf).map_err(|e| {
        ToolError::ExecutionFailed(format!("Cannot read {}: {}", path.display(), e))
    })?;
    buf.truncate(read);
    Ok(buf)
}

fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".git"
            | "__pycache__"
            | "vendor"
            | ".next"
            | ".venv"
            | "venv"
    )
}

fn extract_usize(params: &serde_json::Value, key: &str, default: usize) -> usize {
    params
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(default)
}

fn format_matches(matches: &[GrepMatch], base: &Path) -> String {
    if matches.is_empty() {
        return "No matches found.".to_string();
    }

    let mut out = String::new();
    out.push_str(&format!("Found {} matches:\n\n", matches.len()));

    for m in matches {
        let rel = m.file.strip_prefix(base).unwrap_or(&m.file);
        out.push_str(&format!("{}:{}:", rel.display(), m.line_number));
        if !m.context_before.is_empty() {
            out.push('\n');
            for (i, line) in m.context_before.iter().enumerate() {
                let ln = m.line_number - m.context_before.len() + i;
                out.push_str(&format!("  {}-{}\n", ln, line));
            }
            out.push_str(&format!("  {}:{}\n", m.line_number, m.content));
        } else {
            out.push_str(&format!(" {}\n", m.content));
        }
        for (i, line) in m.context_after.iter().enumerate() {
            let ln = m.line_number + 1 + i;
            out.push_str(&format!("  {}-{}\n", ln, line));
        }
    }
    out
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_test_dir() -> tempfile::TempDir {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("hello.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .expect("write");
        fs::write(dir.path().join("world.txt"), "hello world\ngoodbye world\n").expect("write");
        fs::create_dir(dir.path().join("sub")).expect("mkdir");
        fs::write(
            dir.path().join("sub/nested.rs"),
            "// nested\nfn nested() {}\n",
        )
        .expect("write");
        dir
    }

    #[test]
    fn simple_pattern_match() {
        let dir = setup_test_dir();
        let re = regex::Regex::new("hello").expect("regex");
        let matches = search_dir(&re, dir.path(), 0, 0, 50, 0).expect("search");
        assert!(matches.len() >= 2); // hello.rs and world.txt
    }

    #[test]
    fn regex_pattern_works() {
        let dir = setup_test_dir();
        let re = regex::Regex::new(r"fn\s+\w+").expect("regex");
        let matches = search_dir(&re, dir.path(), 0, 0, 50, 0).expect("search");
        assert!(matches.len() >= 2); // main and nested
    }

    #[test]
    fn context_lines() {
        let dir = setup_test_dir();
        let re = regex::Regex::new("println").expect("regex");
        let matches = search_file(&re, &dir.path().join("hello.rs"), 1, 1, 50).expect("search");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].context_before.len(), 1);
        assert_eq!(matches[0].context_after.len(), 1);
    }

    #[test]
    fn max_results_respected() {
        let dir = setup_test_dir();
        // Write file with many matches
        let content = (0..100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(dir.path().join("many.txt"), &content).expect("write");

        let re = regex::Regex::new("line").expect("regex");
        let matches = search_file(&re, &dir.path().join("many.txt"), 0, 0, 5).expect("search");
        assert_eq!(matches.len(), 5);
    }

    #[test]
    fn binary_file_skipped() {
        let dir = setup_test_dir();
        fs::write(dir.path().join("binary.bin"), [0u8, 1, 2, 0, 3]).expect("write");
        let re = regex::Regex::new(".").expect("regex");
        let matches = search_file(&re, &dir.path().join("binary.bin"), 0, 0, 50).expect("search");
        assert!(matches.is_empty());
    }

    #[test]
    fn no_matches_returns_message() {
        let dir = setup_test_dir();
        let re = regex::Regex::new("nonexistent_xyz").expect("regex");
        let matches = search_dir(&re, dir.path(), 0, 0, 50, 0).expect("search");
        let output = format_matches(&matches, dir.path());
        assert!(output.contains("No matches"));
    }

    #[test]
    fn ignored_dirs_skipped() {
        let dir = setup_test_dir();
        fs::create_dir(dir.path().join("node_modules")).expect("mkdir");
        fs::write(
            dir.path().join("node_modules/pkg.js"),
            "hello from node_modules",
        )
        .expect("write");

        let re = regex::Regex::new("hello").expect("regex");
        let matches = search_dir(&re, dir.path(), 0, 0, 50, 0).expect("search");
        // Should NOT include the node_modules match
        assert!(
            matches
                .iter()
                .all(|m| !m.file.to_string_lossy().contains("node_modules"))
        );
    }
}

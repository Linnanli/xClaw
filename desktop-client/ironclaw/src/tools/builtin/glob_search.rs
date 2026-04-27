//! Glob search tool — file path matching with glob patterns.
//!
//! Provides the LLM with `find`-like capability: search for files whose
//! paths match a glob pattern. Uses simple manual glob matching to avoid
//! adding new dependencies.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::context::JobContext;
use crate::tools::builtin::path_utils::validate_path;
use crate::tools::tool::{
    ApprovalRequirement, Tool, ToolDomain, ToolError, ToolOutput, require_str,
};

/// Maximum results returned by default.
const DEFAULT_MAX_RESULTS: usize = 100;

/// Hard cap on results.
const HARD_MAX_RESULTS: usize = 1000;

/// Maximum recursion depth.
const MAX_DEPTH: usize = 20;

/// Glob search tool.
#[derive(Debug, Default)]
pub struct GlobSearchTool {
    base_dir: Option<PathBuf>,
}

impl GlobSearchTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_dir(mut self, dir: PathBuf) -> Self {
        self.base_dir = Some(dir);
        self
    }
}

#[async_trait]
impl Tool for GlobSearchTool {
    fn name(&self) -> &str {
        "glob_search"
    }

    fn description(&self) -> &str {
        "Search for files matching a glob pattern (e.g. '**/*.rs', 'src/**/*.ts'). \
         Returns a list of matching file paths."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (supports *, **, ?)"
                },
                "path": {
                    "type": "string",
                    "description": "Root directory to search from (default: current directory)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches (default: 100, max: 1000)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &JobContext,
    ) -> Result<ToolOutput, ToolError> {
        let start = std::time::Instant::now();

        let pattern = require_str(&params, "pattern")?;
        let path_str = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS)
            .min(HARD_MAX_RESULTS);

        let effective = super::path_utils::effective_base_dir(self.base_dir.as_deref(), ctx);
        let search_root = validate_path(path_str, effective.as_deref())?;

        let matcher = compile_glob(pattern)?;
        let mut results = Vec::new();
        collect_matches(
            &search_root,
            &search_root,
            &matcher,
            max_results,
            0,
            &mut results,
        )?;

        let output = format_results(&results, &search_root);
        Ok(ToolOutput::text(output, start.elapsed()))
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Never
    }
}

// ── Glob matching ───────────────────────────────────────────────────────

/// Compiled glob pattern — converted to a regex for matching.
struct GlobMatcher {
    re: regex::Regex,
}

fn compile_glob(pattern: &str) -> Result<GlobMatcher, ToolError> {
    let mut regex_str = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '*' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                // ** matches any path segment(s)
                if i + 2 < chars.len() && chars[i + 2] == '/' {
                    regex_str.push_str("(?:.*/)?");
                    i += 3;
                } else {
                    regex_str.push_str(".*");
                    i += 2;
                }
            }
            '*' => {
                // * matches anything except /
                regex_str.push_str("[^/]*");
                i += 1;
            }
            '?' => {
                regex_str.push_str("[^/]");
                i += 1;
            }
            '.' | '+' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '$' | '|' | '\\' => {
                regex_str.push('\\');
                regex_str.push(chars[i]);
                i += 1;
            }
            c => {
                regex_str.push(c);
                i += 1;
            }
        }
    }
    regex_str.push('$');

    let re = regex::Regex::new(&regex_str)
        .map_err(|e| ToolError::InvalidParameters(format!("Invalid glob pattern: {}", e)))?;
    Ok(GlobMatcher { re })
}

impl GlobMatcher {
    fn matches(&self, path: &str) -> bool {
        self.re.is_match(path)
    }
}

// ── Directory walk ──────────────────────────────────────────────────────

fn collect_matches(
    root: &Path,
    dir: &Path,
    matcher: &GlobMatcher,
    max: usize,
    depth: usize,
    results: &mut Vec<PathBuf>,
) -> Result<(), ToolError> {
    if depth > MAX_DEPTH || results.len() >= max {
        return Ok(());
    }

    let entries = std::fs::read_dir(dir).map_err(|e| {
        ToolError::ExecutionFailed(format!("Cannot read dir {}: {}", dir.display(), e))
    })?;

    let mut sorted: Vec<_> = entries.filter_map(Result::ok).collect();
    sorted.sort_by_key(|e| e.file_name());

    for entry in sorted {
        if results.len() >= max {
            break;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden and ignored directories
        if name_str.starts_with('.') || is_ignored_dir(&name_str) {
            continue;
        }

        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy();

        if path.is_file() && matcher.matches(&rel_str) {
            results.push(path.clone());
        }

        if path.is_dir() {
            collect_matches(root, &path, matcher, max, depth + 1, results)?;
        }
    }
    Ok(())
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

fn format_results(results: &[PathBuf], root: &Path) -> String {
    if results.is_empty() {
        return "No files matched.".to_string();
    }
    let mut out = String::new();
    out.push_str(&format!("Found {} files:\n", results.len()));
    for p in results {
        let rel = p.strip_prefix(root).unwrap_or(p);
        out.push_str(&format!("  {}\n", rel.display()));
    }
    out
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup() -> tempfile::TempDir {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
        fs::write(dir.path().join("lib.rs"), "pub mod foo;").expect("write");
        fs::write(dir.path().join("readme.md"), "# Hello").expect("write");
        fs::create_dir(dir.path().join("src")).expect("mkdir");
        fs::write(dir.path().join("src/foo.rs"), "pub fn foo() {}").expect("write");
        fs::write(dir.path().join("src/bar.ts"), "export {}").expect("write");
        dir
    }

    #[test]
    fn match_all_rs_files() {
        let dir = setup();
        let m = compile_glob("**/*.rs").expect("glob");
        let mut results = Vec::new();
        collect_matches(dir.path(), dir.path(), &m, 100, 0, &mut results).expect("walk");
        assert_eq!(results.len(), 3); // main.rs, lib.rs, src/foo.rs
    }

    #[test]
    fn match_ts_in_subdir() {
        let dir = setup();
        let m = compile_glob("src/*.ts").expect("glob");
        let mut results = Vec::new();
        collect_matches(dir.path(), dir.path(), &m, 100, 0, &mut results).expect("walk");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn match_with_question_mark() {
        let dir = setup();
        let m = compile_glob("*.?s").expect("glob");
        let mut results = Vec::new();
        collect_matches(dir.path(), dir.path(), &m, 100, 0, &mut results).expect("walk");
        assert_eq!(results.len(), 2); // main.rs, lib.rs
    }

    #[test]
    fn max_results_respected() {
        let dir = setup();
        let m = compile_glob("**/*").expect("glob");
        let mut results = Vec::new();
        collect_matches(dir.path(), dir.path(), &m, 2, 0, &mut results).expect("walk");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn no_matches_message() {
        let dir = setup();
        let m = compile_glob("**/*.java").expect("glob");
        let mut results = Vec::new();
        collect_matches(dir.path(), dir.path(), &m, 100, 0, &mut results).expect("walk");
        assert_eq!(format_results(&results, dir.path()), "No files matched.");
    }

    #[test]
    fn ignored_dirs_skipped() {
        let dir = setup();
        fs::create_dir(dir.path().join("node_modules")).expect("mkdir");
        fs::write(dir.path().join("node_modules/dep.rs"), "").expect("write");

        let m = compile_glob("**/*.rs").expect("glob");
        let mut results = Vec::new();
        collect_matches(dir.path(), dir.path(), &m, 100, 0, &mut results).expect("walk");
        // Should NOT include node_modules/dep.rs
        assert!(
            results
                .iter()
                .all(|p| !p.to_string_lossy().contains("node_modules"))
        );
    }
}

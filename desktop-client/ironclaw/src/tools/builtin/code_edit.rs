//! Precise code editing tool with count verification and diff preview.
//!
//! Complements `ApplyPatchTool`: that tool consumes the codex `apply_patch`
//! lark envelope (multi-file, multi-hunk, with surrounding context) and is the
//! correct surface for batch refactors and renames. **This** tool covers the
//! single-point-replacement-within-one-file niche by adding **replacement
//! count verification** (the caller specifies how many replacements they
//! expect) and returning a **unified-diff-style preview** of the change. This
//! prevents accidental bulk replacements when `old_string` appears more times
//! than expected.

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::fs;

use crate::context::JobContext;
use crate::tools::builtin::path_utils::{AccessMode, PathPolicy, validate_path_with_policy};
use crate::tools::tool::{
    ApprovalRequirement, Tool, ToolDomain, ToolError, ToolOutput, require_str,
};

use super::file_guard;

/// Maximum file size for editing (10MB).
const MAX_EDIT_SIZE: u64 = 10 * 1024 * 1024;

/// Number of context lines around each replacement site in the diff preview.
const DIFF_CONTEXT_LINES: usize = 3;

/// Precise code editing tool with replacement count verification.
#[derive(Debug, Default)]
pub struct CodeEditTool {
    base_dir: Option<PathBuf>,
    policy: Option<PathPolicy>,
}

impl CodeEditTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_dir(mut self, dir: PathBuf) -> Self {
        self.base_dir = Some(dir);
        self
    }

    pub fn with_policy(mut self, policy: PathPolicy) -> Self {
        self.policy = Some(policy);
        self
    }
}

#[async_trait]
impl Tool for CodeEditTool {
    fn name(&self) -> &str {
        "code_edit"
    }

    fn description(&self) -> &str {
        "Precise code editing: search for old_string and replace with new_string. \
         Use expected_count to verify the number of matches before applying. \
         Returns a diff preview showing the change context. \
         Safer than apply_patch for multi-occurrence strings."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find (must match including whitespace)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string"
                },
                "expected_count": {
                    "type": "integer",
                    "description": "Expected number of occurrences. If set, edit is rejected when actual count differs. If omitted, replaces the first occurrence only."
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &JobContext,
    ) -> Result<ToolOutput, ToolError> {
        let path_str = require_str(&params, "file_path")?;
        let old_string = require_str(&params, "old_string")?;
        let new_string = require_str(&params, "new_string")?;
        let expected_count = params.get("expected_count").and_then(|v| v.as_u64());

        let start = std::time::Instant::now();
        let effective = super::path_utils::effective_base_dir(self.base_dir.as_deref(), ctx);
        let path = validate_path_with_policy(
            path_str,
            effective.as_deref(),
            self.policy.as_ref(),
            AccessMode::Write,
        )?;

        file_guard::check_size_limit(&path, Some(MAX_EDIT_SIZE))?;

        let raw = fs::read(&path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read file: {}", e)))?;

        if file_guard::is_binary(&raw) {
            return Err(ToolError::ExecutionFailed(
                "Cannot edit binary files".to_string(),
            ));
        }

        let content = String::from_utf8(raw)
            .map_err(|_| ToolError::ExecutionFailed("File contains invalid UTF-8".to_string()))?;

        let actual_count = content.matches(old_string).count();

        if actual_count == 0 {
            return Err(ToolError::ExecutionFailed(format!(
                "old_string not found in {}. Ensure it matches exactly (including whitespace).",
                path.display()
            )));
        }

        // Count verification: reject if mismatch
        if let Some(expected) = expected_count {
            if actual_count != expected as usize {
                return Err(ToolError::ExecutionFailed(format!(
                    "Expected {} occurrence(s) of old_string but found {}. \
                     Edit aborted to prevent unintended changes.",
                    expected, actual_count
                )));
            }
        }

        // Determine how many to replace
        let replace_count = expected_count.map(|n| n as usize).unwrap_or(1);

        let new_content = replace_n(&content, old_string, new_string, replace_count);

        // Generate diff preview
        let diff = generate_diff_preview(&content, &new_content, &path);

        fs::write(&path, &new_content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

        let result = serde_json::json!({
            "path": path.display().to_string(),
            "replaced_count": replace_count,
            "diff_preview": diff,
            "success": true
        });

        Ok(ToolOutput::success(result, start.elapsed()))
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::UnlessAutoApproved
    }

    fn requires_sanitization(&self) -> bool {
        false
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn rate_limit_config(&self) -> Option<crate::tools::tool::ToolRateLimitConfig> {
        Some(crate::tools::tool::ToolRateLimitConfig::new(20, 200))
    }
}

/// Replace exactly `count` occurrences of `old` with `new` in `content`.
fn replace_n(content: &str, old: &str, new: &str, count: usize) -> String {
    if count == 0 {
        return content.to_string();
    }

    let mut result = String::with_capacity(content.len());
    let mut remaining = content;
    let mut replaced = 0;

    while replaced < count {
        match remaining.find(old) {
            Some(pos) => {
                result.push_str(&remaining[..pos]);
                result.push_str(new);
                remaining = &remaining[pos + old.len()..];
                replaced += 1;
            }
            None => break,
        }
    }
    result.push_str(remaining);
    result
}

/// Generate a unified-diff-style preview of the changes.
fn generate_diff_preview(old_content: &str, new_content: &str, path: &std::path::Path) -> String {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();

    let mut diff = format!("--- a/{}\n+++ b/{}\n", path.display(), path.display());

    // Find first differing line region
    let first_diff = old_lines
        .iter()
        .zip(new_lines.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(old_lines.len().min(new_lines.len()));

    // Find last differing line from the end
    let last_diff_old = find_last_diff_from_end(&old_lines, &new_lines);
    let last_diff_new = find_last_diff_from_end(&new_lines, &old_lines);

    let ctx_start = first_diff.saturating_sub(DIFF_CONTEXT_LINES);
    let ctx_end_old = (last_diff_old + DIFF_CONTEXT_LINES + 1).min(old_lines.len());
    let ctx_end_new = (last_diff_new + DIFF_CONTEXT_LINES + 1).min(new_lines.len());

    diff.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        ctx_start + 1,
        ctx_end_old - ctx_start,
        ctx_start + 1,
        ctx_end_new - ctx_start,
    ));

    // Context before change
    for line in &old_lines[ctx_start..first_diff] {
        diff.push_str(&format!(" {line}\n"));
    }
    // Removed lines
    for line in &old_lines[first_diff..last_diff_old + 1] {
        diff.push_str(&format!("-{line}\n"));
    }
    // Added lines
    for line in &new_lines[first_diff..last_diff_new + 1] {
        diff.push_str(&format!("+{line}\n"));
    }
    // Context after change
    let after_old = last_diff_old + 1;
    for line in &old_lines[after_old..ctx_end_old] {
        diff.push_str(&format!(" {line}\n"));
    }

    diff
}

/// Find the last line index (from the start) that differs, comparing from the end.
fn find_last_diff_from_end(primary: &[&str], other: &[&str]) -> usize {
    let mut pi = primary.len();
    let mut oi = other.len();

    while pi > 0 && oi > 0 {
        pi -= 1;
        oi -= 1;
        if primary[pi] != other[oi] {
            return pi;
        }
    }
    // If lengths differ, the extra lines at the start are "different"
    if pi > 0 { pi } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn single_replacement_default() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("test.rs");
        fs::write(&file, "fn foo() {}\nfn bar() {}\nfn foo() {}\n")
            .await
            .expect("write");

        let tool = CodeEditTool::new().with_base_dir(dir.path().to_path_buf());
        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": file.to_str().expect("path"),
                    "old_string": "fn foo() {}",
                    "new_string": "fn baz() {}"
                }),
                &JobContext::default(),
            )
            .await
            .expect("execute");

        assert_eq!(result.result["replaced_count"], 1);
        let content = fs::read_to_string(&file).await.expect("read");
        // Only first occurrence replaced
        assert!(content.starts_with("fn baz() {}"));
        assert!(content.contains("fn foo() {}"));
    }

    #[tokio::test]
    async fn expected_count_mismatch_rejects() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("test.rs");
        fs::write(&file, "fn foo() {}\nfn foo() {}\n")
            .await
            .expect("write");

        let tool = CodeEditTool::new().with_base_dir(dir.path().to_path_buf());
        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": file.to_str().expect("path"),
                    "old_string": "fn foo() {}",
                    "new_string": "fn baz() {}",
                    "expected_count": 1
                }),
                &JobContext::default(),
            )
            .await;

        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Expected 1") && err.contains("found 2"));
    }

    #[tokio::test]
    async fn expected_count_matches_replaces_all() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("test.rs");
        fs::write(&file, "fn foo() {}\nfn foo() {}\n")
            .await
            .expect("write");

        let tool = CodeEditTool::new().with_base_dir(dir.path().to_path_buf());
        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": file.to_str().expect("path"),
                    "old_string": "fn foo() {}",
                    "new_string": "fn baz() {}",
                    "expected_count": 2
                }),
                &JobContext::default(),
            )
            .await
            .expect("execute");

        assert_eq!(result.result["replaced_count"], 2);
        let content = fs::read_to_string(&file).await.expect("read");
        assert!(!content.contains("fn foo()"));
    }

    #[tokio::test]
    async fn old_string_not_found() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("test.rs");
        fs::write(&file, "fn bar() {}\n").await.expect("write");

        let tool = CodeEditTool::new().with_base_dir(dir.path().to_path_buf());
        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": file.to_str().expect("path"),
                    "old_string": "fn foo() {}",
                    "new_string": "fn baz() {}"
                }),
                &JobContext::default(),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn diff_preview_included() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("test.txt");
        fs::write(&file, "line1\nline2\nline3\nline4\nline5\n")
            .await
            .expect("write");

        let tool = CodeEditTool::new().with_base_dir(dir.path().to_path_buf());
        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": file.to_str().expect("path"),
                    "old_string": "line3",
                    "new_string": "LINE_THREE"
                }),
                &JobContext::default(),
            )
            .await
            .expect("execute");

        let diff = result.result["diff_preview"].as_str().expect("diff");
        assert!(diff.contains("-line3"));
        assert!(diff.contains("+LINE_THREE"));
    }

    #[tokio::test]
    async fn binary_file_rejected() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("binary.bin");
        fs::write(&file, b"hello\x00world").await.expect("write");

        let tool = CodeEditTool::new().with_base_dir(dir.path().to_path_buf());
        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": file.to_str().expect("path"),
                    "old_string": "hello",
                    "new_string": "goodbye"
                }),
                &JobContext::default(),
            )
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn replace_n_replaces_exactly_n() {
        let content = "aaa bbb aaa bbb aaa";
        assert_eq!(replace_n(content, "aaa", "xxx", 2), "xxx bbb xxx bbb aaa");
        assert_eq!(replace_n(content, "aaa", "xxx", 1), "xxx bbb aaa bbb aaa");
        assert_eq!(replace_n(content, "aaa", "xxx", 3), "xxx bbb xxx bbb xxx");
    }
}

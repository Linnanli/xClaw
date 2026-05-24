//! File operation tools for reading, writing, and navigating the filesystem.
//!
//! These tools provide controlled access to the filesystem with:
//! - Path validation and sandboxing
//! - Size limits on read/write operations
//! - Support for common development tasks

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;

use crate::path_utils::{AccessMode, PathPolicy, validate_path_with_policy};
use dasclaw_runtime::Tool;
use dasclaw_tool::{ApprovalRequirement, ToolDomain, ToolError, ToolOutput, require_str};
use dasclaw_workspace_cap::document::paths as ws_paths;

use crate::file_guard;

/// Well-known workspace filenames that must go through memory_write, not write_file.
///
/// If the LLM tries to write one of these via the filesystem tool we reject
/// immediately and point it at the correct tool.
const WORKSPACE_FILES: &[&str] = &[
    ws_paths::HEARTBEAT,
    ws_paths::MEMORY,
    ws_paths::IDENTITY,
    ws_paths::SOUL,
    ws_paths::AGENTS,
    ws_paths::USER,
    ws_paths::README,
];

/// Check whether `path` resolves to a workspace file that should be written
/// through `memory_write` instead of `write_file`.
fn is_workspace_path(path: &str) -> bool {
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    WORKSPACE_FILES.contains(&filename)
        || path.starts_with("daily/")
        || path.starts_with("context/")
}

/// Maximum file size for reading (10MB). Enforced by file_guard::check_size_limit.
const MAX_READ_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum file size for writing (5MB).
const MAX_WRITE_SIZE: usize = 5 * 1024 * 1024;

/// Maximum directory listing entries.
const MAX_DIR_ENTRIES: usize = 500;

/// Read file contents tool.
#[derive(Debug, Default)]
pub struct ReadFileTool {
    base_dir: Option<PathBuf>,
    policy: Option<PathPolicy>,
}

impl ReadFileTool {
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
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a file from the LOCAL FILESYSTEM. NOT for workspace memory paths \
         (use memory_read for those). Returns file content as text. \
         For large files, you can specify offset and limit to read a portion."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-indexed, optional)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (optional)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let path_str = require_str(&params, "path")?;

        let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = params.get("limit").and_then(|v| v.as_u64());

        let start = std::time::Instant::now();

        let effective = crate::path_utils::effective_base_dir(self.base_dir.as_deref(), ctx);
        let path = validate_path_with_policy(
            path_str,
            effective.as_deref(),
            self.policy.as_ref(),
            AccessMode::Read,
        )?;

        // File size limit (10MB default)
        file_guard::check_size_limit(&path, Some(MAX_READ_SIZE))?;

        // Read as raw bytes for binary detection
        let raw = fs::read(&path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read file: {}", e)))?;

        // Binary file detection — return info instead of garbled output
        if file_guard::is_binary(&raw) {
            let result = serde_json::json!({
                "content": "Binary file, cannot display. Use shell commands to inspect binary files.",
                "binary": true,
                "path": path.display().to_string(),
                "size_bytes": raw.len()
            });
            return Ok(ToolOutput::success(result, start.elapsed()));
        }

        let content = String::from_utf8(raw).map_err(|_| {
            ToolError::ExecutionFailed(
                "File contains invalid UTF-8. It may be a binary file.".to_string(),
            )
        })?;

        // Apply offset and limit
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start_line = if offset > 0 {
            offset.saturating_sub(1)
        } else {
            0
        };
        let end_line = if let Some(lim) = limit {
            (start_line + lim as usize).min(total_lines)
        } else {
            total_lines
        };

        let selected_lines: Vec<String> = lines[start_line..end_line]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>6}│ {}", start_line + i + 1, line))
            .collect();

        let result = serde_json::json!({
            "content": selected_lines.join("\n"),
            "total_lines": total_lines,
            "lines_shown": end_line - start_line,
            "path": path.display().to_string()
        });

        Ok(ToolOutput::success(result, start.elapsed()))
    }

    fn requires_sanitization(&self) -> bool {
        true // File content could contain anything
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::UnlessAutoApproved
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }
}

/// Write file contents tool.
#[derive(Debug, Default)]
pub struct WriteFileTool {
    base_dir: Option<PathBuf>,
    policy: Option<PathPolicy>,
}

impl WriteFileTool {
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
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file on the LOCAL FILESYSTEM. NOT for workspace memory \
         (use memory_write for that). Creates the file if it doesn't exist, overwrites if it does. \
         Parent directories are created automatically. Use apply_patch for targeted edits."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let path_str = require_str(&params, "path")?;

        // Reject workspace paths: these live in the database, not on disk.
        if is_workspace_path(path_str) {
            return Err(ToolError::InvalidParameters(format!(
                "'{}' is a workspace memory file. Use the memory_write tool instead of write_file. \
                 For HEARTBEAT.md use target='heartbeat', for MEMORY.md use target='memory'.",
                path_str
            )));
        }

        let content = require_str(&params, "content")?;

        let start = std::time::Instant::now();

        // Check content size
        if content.len() > MAX_WRITE_SIZE {
            return Err(ToolError::InvalidParameters(format!(
                "Content too large ({} bytes). Maximum is {} bytes.",
                content.len(),
                MAX_WRITE_SIZE
            )));
        }

        let effective = crate::path_utils::effective_base_dir(self.base_dir.as_deref(), ctx);
        let path = validate_path_with_policy(
            path_str,
            effective.as_deref(),
            self.policy.as_ref(),
            AccessMode::Write,
        )?;

        // Create parent directories
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create directories: {}", e))
            })?;
        }

        // Write file
        fs::write(&path, content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

        let result = serde_json::json!({
            "path": path.display().to_string(),
            "bytes_written": content.len(),
            "success": true
        });

        Ok(ToolOutput::success(result, start.elapsed()))
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::UnlessAutoApproved
    }

    fn requires_sanitization(&self) -> bool {
        false // We're writing, not reading external data
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn rate_limit_config(&self) -> Option<dasclaw_tool::ToolRateLimitConfig> {
        Some(dasclaw_tool::ToolRateLimitConfig::new(20, 200))
    }
}

/// List directory contents tool.
#[derive(Debug, Default)]
pub struct ListDirTool {
    base_dir: Option<PathBuf>,
    policy: Option<PathPolicy>,
}

impl ListDirTool {
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
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List contents of a directory on the LOCAL FILESYSTEM. NOT for workspace memory \
         (use memory_tree for that). Shows files and subdirectories with their sizes."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list (defaults to current directory)"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "If true, list contents recursively (default false)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth for recursive listing (default 3)"
                }
            },
            "required": []
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let path_str = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_depth = params
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;

        let start = std::time::Instant::now();

        let effective = crate::path_utils::effective_base_dir(self.base_dir.as_deref(), ctx);
        let path = validate_path_with_policy(
            path_str,
            effective.as_deref(),
            self.policy.as_ref(),
            AccessMode::Read,
        )?;

        let mut entries = Vec::new();
        list_dir_inner(&path, &path, recursive, max_depth, 0, &mut entries).await?;

        // Sort entries
        entries.sort_by(|a, b| {
            let a_is_dir = a.ends_with('/');
            let b_is_dir = b.ends_with('/');
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.cmp(b),
            }
        });

        let truncated = entries.len() > MAX_DIR_ENTRIES;
        if truncated {
            entries.truncate(MAX_DIR_ENTRIES);
        }

        let result = serde_json::json!({
            "path": path.display().to_string(),
            "entries": entries,
            "count": entries.len(),
            "truncated": truncated
        });

        Ok(ToolOutput::success(result, start.elapsed()))
    }

    fn requires_sanitization(&self) -> bool {
        false // Directory listings are safe
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }
}

/// Recursively list directory contents.
async fn list_dir_inner(
    base: &Path,
    path: &Path,
    recursive: bool,
    max_depth: usize,
    current_depth: usize,
    entries: &mut Vec<String>,
) -> Result<(), ToolError> {
    if entries.len() >= MAX_DIR_ENTRIES {
        return Ok(());
    }

    let mut dir = fs::read_dir(path)
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read directory: {}", e)))?;

    while let Some(entry) = dir
        .next_entry()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read entry: {}", e)))?
    {
        if entries.len() >= MAX_DIR_ENTRIES {
            break;
        }

        let entry_path = entry.path();
        let relative = entry_path
            .strip_prefix(base)
            .unwrap_or(&entry_path)
            .to_string_lossy();

        let metadata = entry.metadata().await.ok();
        let is_dir = metadata.as_ref().is_some_and(|m| m.is_dir());

        let display = if is_dir {
            format!("{}/", relative)
        } else {
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            format!("{} ({})", relative, format_size(size))
        };

        entries.push(display);

        if recursive && is_dir && current_depth < max_depth {
            // Skip common non-essential directories
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !matches!(
                name_str.as_ref(),
                "node_modules" | "target" | ".git" | "__pycache__" | "venv" | ".venv"
            ) {
                Box::pin(list_dir_inner(
                    base,
                    &entry_path,
                    recursive,
                    max_depth,
                    current_depth + 1,
                    entries,
                ))
                .await?;
            }
        }
    }

    Ok(())
}

/// Format file size in human-readable form.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Apply patch tool — codex `apply_patch` lark-grammar protocol.
///
/// Consumes a single envelope (`*** Begin Patch ... *** End Patch`) describing
/// one or more `Add File` / `Delete File` / `Update File` hunks, and routes
/// parsing + application through the shared [`dasclaw_apply_patch`] crate.
///
/// Boundary with `code_edit`:
///
/// - Use `apply_patch` for **multi-file** patches and **multi-hunk** edits
///   carrying their own context lines (`@@`, `-`, `+`, ` `). The envelope is
///   the only correct surface for batch refactors and renames.
/// - Use `code_edit` for **single-point string replacement within one file**
///   when you want explicit replacement-count verification (it surfaces an
///   `expected_count` field). It is *not* a substitute for multi-hunk patches.
///
/// Path safety: each hunk's destination path is validated through the
/// configured [`PathPolicy`] *before* any filesystem mutation, mirroring the
/// rest of the file-tool family. Move-rename hunks (`*** Move to:`) are
/// parsed but not yet applied — they currently surface as a structured error
/// so callers can fall back to a separate add/delete sequence (see W3
/// follow-up).
#[derive(Debug, Default)]
pub struct ApplyPatchTool {
    base_dir: Option<PathBuf>,
    policy: Option<PathPolicy>,
}

impl ApplyPatchTool {
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
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a multi-file patch using the apply_patch envelope format. The `input` field must \
         start with `*** Begin Patch` and end with `*** End Patch`, containing one or more hunks: \
         `*** Add File: <path>` followed by `+`-prefixed contents; `*** Delete File: <path>`; or \
         `*** Update File: <path>` followed by `@@` chunks of context (` `), removed (`-`), and \
         added (`+`) lines. Use `apply_patch` for multi-file or multi-hunk edits with surrounding \
         context. For surgical single-point replacement within one file, prefer `code_edit` \
         (which verifies an explicit replacement count)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "The full apply_patch envelope, beginning with `*** Begin Patch` and ending with `*** End Patch`."
                }
            },
            "required": ["input"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let input = require_str(&params, "input")?;
        let start = std::time::Instant::now();

        let args = dasclaw_apply_patch::parse_patch(input)
            .map_err(|e| ToolError::ExecutionFailed(format!("invalid patch: {e}")))?;

        let effective = crate::path_utils::effective_base_dir(self.base_dir.as_deref(), ctx);

        // Validate every destination path through the configured policy *before*
        // touching the filesystem; the apply layer then enforces base_dir again
        // as a defence-in-depth check (absolute / parent-dir escape).
        for hunk in &args.hunks {
            let raw = hunk.path().to_string_lossy().into_owned();
            validate_path_with_policy(
                &raw,
                effective.as_deref(),
                self.policy.as_ref(),
                AccessMode::Write,
            )?;
        }

        let opts = dasclaw_apply_patch::ApplyOptions {
            base_dir: effective.clone(),
        };

        let report = dasclaw_apply_patch::apply(&args, &opts)
            .map_err(|e| ToolError::ExecutionFailed(format!("apply_patch failed: {e}")))?;

        let to_str = |v: &Vec<PathBuf>| -> Vec<String> {
            v.iter().map(|p| p.display().to_string()).collect()
        };
        let result = serde_json::json!({
            "success": true,
            "files_added": to_str(&report.files_added),
            "files_deleted": to_str(&report.files_deleted),
            "files_updated": to_str(&report.files_updated),
            "hunks_applied": report.hunks_applied,
        });

        Ok(ToolOutput::success(result, start.elapsed()))
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::UnlessAutoApproved
    }

    fn requires_sanitization(&self) -> bool {
        false // We're writing, not reading external data
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn rate_limit_config(&self) -> Option<dasclaw_tool::ToolRateLimitConfig> {
        Some(dasclaw_tool::ToolRateLimitConfig::new(20, 200))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_utils::{normalize_lexical, validate_path};
    use dasclaw_runtime::context::JobContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line 1\nline 2\nline 3\n").unwrap();

        let tool = ReadFileTool::new().with_base_dir(dir.path().to_path_buf());
        let mut ctx = JobContext::default();

        let result = tool
            .execute(
                serde_json::json!({"path": file_path.to_str().unwrap()}),
                &mut ctx,
            )
            .await
            .unwrap();

        let content = result.result.get("content").unwrap().as_str().unwrap();
        assert!(content.contains("line 1"));
        assert!(content.contains("line 2"));
    }

    #[tokio::test]
    async fn test_write_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let tool = WriteFileTool::new().with_base_dir(dir.path().to_path_buf());
        let mut ctx = JobContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "path": file_path.to_str().unwrap(),
                    "content": "hello world"
                }),
                &mut ctx,
            )
            .await
            .unwrap();

        assert!(result.result.get("success").unwrap().as_bool().unwrap());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");
    }

    #[tokio::test]
    async fn test_apply_patch_codex_protocol() {
        let dir = TempDir::new().unwrap();
        // Pre-existing file that the patch will Update; the patch also Adds a
        // brand new file in a nested directory. This exercises the full
        // codex-protocol envelope (multi-file, mixed Add/Update).
        let code_path = dir.path().join("code.rs");
        std::fs::write(&code_path, "fn main() {\n    println!(\"old\");\n}\n").unwrap();

        let tool = ApplyPatchTool::new().with_base_dir(dir.path().to_path_buf());
        let mut ctx = JobContext::default();

        let envelope = [
            "*** Begin Patch",
            "*** Update File: code.rs",
            "@@",
            " fn main() {",
            "-    println!(\"old\");",
            "+    println!(\"new\");",
            " }",
            "*** Add File: docs/notes.md",
            "+# Notes",
            "+hello",
            "*** End Patch",
        ]
        .join("\n");

        let result = tool
            .execute(serde_json::json!({ "input": envelope }), &mut ctx)
            .await
            .unwrap();

        assert!(result.result.get("success").unwrap().as_bool().unwrap());
        assert_eq!(
            result
                .result
                .get("hunks_applied")
                .unwrap()
                .as_u64()
                .unwrap(),
            2
        );
        assert!(
            std::fs::read_to_string(&code_path)
                .unwrap()
                .contains("println!(\"new\")")
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
            "# Notes\nhello\n"
        );
    }

    #[tokio::test]
    async fn test_apply_patch_rejects_invalid_envelope() {
        let dir = TempDir::new().unwrap();
        let tool = ApplyPatchTool::new().with_base_dir(dir.path().to_path_buf());
        let mut ctx = JobContext::default();

        let result = tool
            .execute(serde_json::json!({ "input": "not a real patch" }), &mut ctx)
            .await;
        assert!(matches!(result, Err(ToolError::ExecutionFailed(_))));
    }

    #[tokio::test]
    async fn test_write_file_rejects_workspace_paths() {
        let dir = TempDir::new().unwrap();
        let tool = WriteFileTool::new().with_base_dir(dir.path().to_path_buf());
        let mut ctx = JobContext::default();

        let workspace_files = &[
            "HEARTBEAT.md",
            "MEMORY.md",
            "IDENTITY.md",
            "SOUL.md",
            "AGENTS.md",
            "USER.md",
            "README.md",
        ];

        for filename in workspace_files {
            let path = dir.path().join(filename);
            let err = tool
                .execute(
                    serde_json::json!({
                        "path": path.to_str().unwrap(),
                        "content": "test"
                    }),
                    &mut ctx,
                )
                .await
                .unwrap_err();

            let msg = err.to_string();
            assert!(
                msg.contains("memory_write"),
                "Rejection for {} should mention memory_write, got: {}",
                filename,
                msg
            );
        }

        // daily/ and context/ prefixes should also be rejected
        for prefix_path in &["daily/2024-01-15.md", "context/vision.md"] {
            let err = tool
                .execute(
                    serde_json::json!({
                        "path": prefix_path,
                        "content": "test"
                    }),
                    &mut ctx,
                )
                .await
                .unwrap_err();

            assert!(
                err.to_string().contains("memory_write"),
                "Rejection for {} should mention memory_write",
                prefix_path
            );
        }

        // Regular files should still work
        let regular_path = dir.path().join("normal.txt");
        let result = tool
            .execute(
                serde_json::json!({
                    "path": regular_path.to_str().unwrap(),
                    "content": "fine"
                }),
                &mut ctx,
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file1.txt"), "content").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();

        let tool = ListDirTool::new();
        let mut ctx = JobContext::default();

        let result = tool
            .execute(
                serde_json::json!({"path": dir.path().to_str().unwrap()}),
                &mut ctx,
            )
            .await
            .unwrap();

        let entries = result.result.get("entries").unwrap().as_array().unwrap();
        assert!(entries.len() >= 2);
    }

    #[test]
    fn test_normalize_lexical() {
        // Basic .. resolution
        assert_eq!(
            normalize_lexical(Path::new("/a/b/../c")),
            PathBuf::from("/a/c")
        );
        // Multiple .. components
        assert_eq!(
            normalize_lexical(Path::new("/a/b/c/../../d")),
            PathBuf::from("/a/d")
        );
        // . components stripped
        assert_eq!(
            normalize_lexical(Path::new("/a/./b/./c")),
            PathBuf::from("/a/b/c")
        );
        // Cannot escape root
        assert_eq!(
            normalize_lexical(Path::new("/a/../../..")),
            PathBuf::from("/")
        );
    }

    #[test]
    fn test_validate_path_rejects_traversal_nonexistent_parent() {
        // The critical test: writing to ../../outside/newdir/file with base_dir
        // set should be rejected even when the parent directory does not exist
        // (i.e. canonicalize() cannot resolve it).
        let dir = TempDir::new().unwrap();
        let evil_path = format!(
            "{}/../../outside/newdir/file.txt",
            dir.path().to_str().unwrap()
        );
        let result = validate_path(&evil_path, Some(dir.path()));
        assert!(
            result.is_err(),
            "Should reject traversal via non-existent parent, got: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_path_rejects_relative_traversal() {
        let dir = TempDir::new().unwrap();
        let result = validate_path("../../etc/passwd", Some(dir.path()));
        assert!(
            result.is_err(),
            "Should reject relative traversal, got: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_path_allows_valid_nested_write() {
        let dir = TempDir::new().unwrap();
        let result = validate_path("subdir/newfile.txt", Some(dir.path()));
        assert!(
            result.is_ok(),
            "Should allow nested writes within sandbox: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_path_allows_dot_dot_within_sandbox() {
        // a/b/../c resolves to a/c which is still inside the sandbox
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        let result = validate_path("a/b/../c.txt", Some(dir.path()));
        assert!(
            result.is_ok(),
            "Should allow .. that stays within sandbox: {:?}",
            result
        );
    }
}

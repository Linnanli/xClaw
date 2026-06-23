use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use dasclaw_tool::ToolError;

const READ_ONLY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyGitOutput {
    pub stdout: String,
    pub exit_code: i32,
}

pub async fn run_read_only_git(
    args: &[&str],
    workdir: &Path,
) -> Result<ReadOnlyGitOutput, ToolError> {
    ensure_read_only_args(args)?;
    let safe_args = safe_git_args(args);
    run_safe_git(&safe_args, workdir).await
}

async fn run_safe_git(args: &[String], workdir: &Path) -> Result<ReadOnlyGitOutput, ToolError> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args)
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env_remove("GIT_EXTERNAL_DIFF");

    let child = cmd
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| ToolError::ExecutionFailed(format!("Failed to spawn git: {error}")))?;
    let result = tokio::time::timeout(READ_ONLY_TIMEOUT, child.wait_with_output()).await;
    let output = match result {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            return Err(ToolError::ExecutionFailed(format!(
                "git I/O error: {error}"
            )));
        }
        Err(_) => return Err(ToolError::Timeout(READ_ONLY_TIMEOUT)),
    };

    let exit_code = output.status.code().unwrap_or(-1);
    let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() {
            stdout.push_str("\n--- stderr ---\n");
        }
        stdout.push_str(&stderr);
    }
    Ok(ReadOnlyGitOutput {
        stdout: truncate(&stdout),
        exit_code,
    })
}

fn ensure_read_only_args(args: &[&str]) -> Result<(), ToolError> {
    let command = args
        .first()
        .copied()
        .ok_or_else(|| ToolError::InvalidParameters("git command must not be empty".to_string()))?;
    match command {
        "diff" => ensure_read_only_diff(args),
        "merge-base" | "rev-parse" | "status" => Ok(()),
        "branch" => ensure_read_only_branch(args),
        "config" => ensure_read_only_config(args),
        _ => Err(ToolError::InvalidParameters(format!(
            "git command is not read-only: {command}"
        ))),
    }
}

fn ensure_read_only_diff(args: &[&str]) -> Result<(), ToolError> {
    for arg in args.iter().skip(1).copied() {
        if arg == "--output"
            || arg.starts_with("--output=")
            || arg == "--ext-diff"
            || arg == "--no-index"
        {
            return Err(ToolError::InvalidParameters(format!(
                "git diff argument is not read-only: {arg}"
            )));
        }
    }
    Ok(())
}

fn ensure_read_only_branch(args: &[&str]) -> Result<(), ToolError> {
    let mut previous_was_color = false;
    for arg in args.iter().skip(1).copied() {
        if previous_was_color {
            previous_was_color = false;
            continue;
        }
        if arg == "--color" {
            previous_was_color = true;
            continue;
        }
        if !matches!(
            arg,
            "--show-current"
                | "--list"
                | "--all"
                | "--remotes"
                | "--verbose"
                | "-v"
                | "-vv"
                | "--no-color"
                | "--contains"
                | "--merged"
                | "--no-merged"
        ) {
            return Err(ToolError::InvalidParameters(format!(
                "git branch argument is not read-only: {arg}"
            )));
        }
    }
    Ok(())
}

fn ensure_read_only_config(args: &[&str]) -> Result<(), ToolError> {
    let values = args.iter().skip(1).copied().collect::<Vec<_>>();
    if values.iter().any(|arg| {
        matches!(
            *arg,
            "--add"
                | "--replace-all"
                | "--unset"
                | "--unset-all"
                | "--rename-section"
                | "--remove-section"
                | "--edit"
        )
    }) {
        return Err(ToolError::InvalidParameters(
            "git config write arguments are not read-only".to_string(),
        ));
    }
    if values.iter().filter(|arg| !arg.starts_with('-')).count() > 1 {
        return Err(ToolError::InvalidParameters(
            "git config must not set values in read-only mode".to_string(),
        ));
    }
    Ok(())
}

fn safe_git_args(args: &[&str]) -> Vec<String> {
    let mut safe_args = vec![
        "-c".to_string(),
        "diff.external=".to_string(),
        "-c".to_string(),
        "core.pager=cat".to_string(),
    ];
    safe_args.push(args[0].to_string());
    if args[0] == "diff" {
        safe_args.push("--no-ext-diff".to_string());
    }
    safe_args.extend(args.iter().skip(1).map(|arg| (*arg).to_string()));
    safe_args
}

fn truncate(s: &str) -> String {
    if s.len() <= MAX_OUTPUT_BYTES {
        return s.to_string();
    }
    let cut = &s[..MAX_OUTPUT_BYTES];
    format!(
        "{}\n\n--- output truncated ({} bytes total) ---",
        cut,
        s.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn read_only_git_rejects_mutating_commands() {
        assert!(ensure_read_only_args(&["commit"]).is_err());
        assert!(ensure_read_only_args(&["diff", "--output=/tmp/x"]).is_err());
        assert!(ensure_read_only_args(&["branch", "-D", "topic"]).is_err());
        assert!(ensure_read_only_args(&["config", "user.name", "Tester"]).is_err());
    }

    #[test]
    fn read_only_git_allows_read_only_commands() {
        assert!(ensure_read_only_args(&["diff", "--stat", "--patch", "HEAD", "--"]).is_ok());
        assert!(ensure_read_only_args(&["merge-base", "HEAD", "origin/main"]).is_ok());
        assert!(ensure_read_only_args(&["rev-parse", "HEAD"]).is_ok());
        assert!(ensure_read_only_args(&["status", "--porcelain=v1"]).is_ok());
        assert!(ensure_read_only_args(&["branch", "--show-current"]).is_ok());
        assert!(ensure_read_only_args(&["config", "user.name"]).is_ok());
    }

    #[tokio::test]
    async fn read_only_git_diff_rejects_output_file() {
        let temp = TestRepo::new();
        init_repo(temp.path());

        let output = temp.path().join("diff.patch");
        let error = run_read_only_git(
            &[
                "diff",
                &format!("--output={}", output.display()),
                "HEAD",
                "--",
            ],
            temp.path(),
        )
        .await
        .expect_err("output file diff should be rejected");

        assert!(matches!(error, ToolError::InvalidParameters(_)));
        assert!(!output.exists());
    }

    #[tokio::test]
    async fn read_only_git_diff_ignores_external_diff_config() {
        let temp = TestRepo::new();
        init_repo(temp.path());
        fs::write(temp.path().join("src.txt"), "changed\n").expect("write changed");
        let marker = temp.path().join("marker");
        let external = format!("sh -c 'touch {}'", marker.display());
        run_git(temp.path(), ["config", "diff.external", &external]);

        let output = run_read_only_git(&["diff", "--stat", "--patch", "HEAD", "--"], temp.path())
            .await
            .expect("read-only diff");

        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("changed"));
        assert!(!marker.exists());
    }

    fn init_repo(root: &Path) {
        fs::write(root.join("src.txt"), "base\n").expect("write base");
        run_git(root, ["init"]);
        run_git(root, ["config", "user.email", "test@example.com"]);
        run_git(root, ["config", "user.name", "Test User"]);
        run_git(root, ["add", "."]);
        run_git(root, ["commit", "-m", "base"]);
    }

    fn run_git<const N: usize>(root: &Path, args: [&str; N]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .expect("git command");
        assert!(status.success());
    }

    struct TestRepo {
        path: PathBuf,
    }

    impl TestRepo {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir().join(format!(
                "dasclaw-read-only-git-test-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("temp repo dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

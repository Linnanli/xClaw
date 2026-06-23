use std::path::Path;
use std::time::Duration;

use dasclaw_tool::ToolError;

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

    let output = crate::runner::run_git(args, workdir, Some(Duration::from_secs(30))).await?;
    Ok(ReadOnlyGitOutput {
        stdout: output.stdout,
        exit_code: output.exit_code,
    })
}

fn ensure_read_only_args(args: &[&str]) -> Result<(), ToolError> {
    let command = args
        .first()
        .copied()
        .ok_or_else(|| ToolError::InvalidParameters("git command must not be empty".to_string()))?;
    match command {
        "diff" | "merge-base" | "rev-parse" | "status" => Ok(()),
        "branch" => ensure_read_only_branch(args),
        "config" => ensure_read_only_config(args),
        _ => Err(ToolError::InvalidParameters(format!(
            "git command is not read-only: {command}"
        ))),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_git_rejects_mutating_commands() {
        assert!(ensure_read_only_args(&["commit"]).is_err());
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
}

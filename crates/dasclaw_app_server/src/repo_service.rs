use std::path::PathBuf;
use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    GitDiffToRemoteParams, GitDiffToRemoteResponse, ServiceHealth, ServiceName,
};
use dasclaw_fs_tools::path_utils::{normalize_lexical, validate_path};

use crate::AppServerError;
use crate::app_services::RepoService;
use crate::blocking_runtime::BlockingTokioRuntime;

const CAPABILITY: &str = "repo";

pub struct AppServerRepoService {
    root: PathBuf,
    runtime: Mutex<Option<Result<BlockingTokioRuntime, AppServerError>>>,
}

impl AppServerRepoService {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let lexical_root = normalize_lexical(&root);
        let root = root.canonicalize().unwrap_or(lexical_root);
        Self {
            root,
            runtime: Mutex::new(None),
        }
    }

    fn runtime(&self) -> Result<BlockingTokioRuntime, AppServerError> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let result = runtime.get_or_insert_with(|| {
            BlockingTokioRuntime::new("dasclaw-app-server-repo", CAPABILITY)
        });
        match result {
            Ok(runtime) => Ok(runtime.clone()),
            Err(error) => Err(error.clone()),
        }
    }

    fn resolve_cwd(&self, cwd: &str) -> Result<PathBuf, AppServerError> {
        if cwd.trim().is_empty() {
            return Err(AppServerError::invalid_request(
                CAPABILITY,
                "cwd must not be empty",
            ));
        }
        let resolved = validate_path(cwd, Some(&self.root)).map_err(|error| {
            AppServerError::capability_unavailable(
                CAPABILITY,
                format!("repo cwd is outside service root: {error}"),
            )
        })?;
        let canonical = resolved.canonicalize().map_err(|error| {
            AppServerError::capability_unavailable(
                CAPABILITY,
                format!("repo cwd is unavailable: {error}"),
            )
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(AppServerError::capability_unavailable(
                CAPABILITY,
                "repo cwd is outside service root",
            ));
        }
        Ok(canonical)
    }

    async fn git_stdout(args: Vec<String>, cwd: PathBuf) -> Result<String, AppServerError> {
        let display_args = args.join(" ");
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = dasclaw_git_tools::run_read_only_git(&refs, &cwd)
            .await
            .map_err(|error| {
                AppServerError::capability_unavailable(
                    CAPABILITY,
                    format!("git {display_args} failed: {error}"),
                )
            })?;
        if output.exit_code != 0 {
            return Err(AppServerError::capability_unavailable(
                CAPABILITY,
                format!(
                    "git {} exited with code {}: {}",
                    display_args,
                    output.exit_code,
                    output.stdout.trim()
                ),
            ));
        }
        Ok(output.stdout)
    }

    async fn diff_to_remote_async(cwd: PathBuf) -> Result<GitDiffToRemoteResponse, AppServerError> {
        let upstream = match Self::git_stdout(
            git_args([
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ]),
            cwd.clone(),
        )
        .await
        {
            Ok(stdout) => {
                let upstream = stdout.trim();
                (!upstream.is_empty()).then(|| upstream.to_string())
            }
            Err(_) => None,
        };

        let sha = if let Some(upstream) = upstream {
            Self::git_stdout(git_args(["merge-base", "HEAD", &upstream]), cwd.clone()).await?
        } else {
            Self::git_stdout(git_args(["rev-parse", "HEAD"]), cwd.clone()).await?
        };
        let sha = sha.trim().to_string();
        let diff =
            Self::git_stdout(git_args(["diff", "--stat", "--patch", &sha, "--"]), cwd).await?;

        Ok(GitDiffToRemoteResponse { sha, diff })
    }
}

impl RepoService for AppServerRepoService {
    fn health(&self) -> ServiceHealth {
        let mut health = ServiceHealth::ready(ServiceName::Repo);
        health.message = Some(format!(
            "root={} guard=canonical-root-containment",
            self.root.display()
        ));
        health
    }

    fn git_diff_to_remote(
        &self,
        params: GitDiffToRemoteParams,
    ) -> Result<GitDiffToRemoteResponse, AppServerError> {
        let cwd = self.resolve_cwd(&params.cwd)?;
        self.runtime()?
            .block_on("git_diff_to_remote", Self::diff_to_remote_async(cwd))
    }
}

fn git_args<const N: usize>(args: [&str; N]) -> Vec<String> {
    args.into_iter().map(str::to_string).collect()
}

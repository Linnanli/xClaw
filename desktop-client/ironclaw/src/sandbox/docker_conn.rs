//! Docker daemon connection helper.
//!
//! Extracted from the deleted `sandbox::container` module (W3.1c) so that
//! the orchestrator's background-job container layer
//! (`orchestrator/{job_manager,reaper}.rs`) and the daemon detection probe
//! ([`super::detect`]) can keep sharing a single, well-tested socket
//! discovery routine.
//!
//! The routine probes, in order:
//!
//! 1. bollard's local defaults (honors `$DOCKER_HOST`, then
//!    `/var/run/docker.sock`)
//! 2. `~/.docker/run/docker.sock` (Docker Desktop 4.13+ on macOS)
//! 3. `~/.colima/default/docker.sock` (Colima)
//! 4. `~/.rd/docker.sock` (Rancher Desktop)
//! 5. `$XDG_RUNTIME_DIR/docker.sock` (rootless Docker on Linux)
//! 6. `/run/user/$UID/docker.sock` (rootless Docker fallback)

use bollard::Docker;

use super::error::{Result, SandboxError};

/// Connect to the Docker daemon, trying multiple well-known socket paths.
pub async fn connect_docker() -> Result<Docker> {
    if let Ok(docker) = Docker::connect_with_local_defaults()
        && docker.ping().await.is_ok()
    {
        return Ok(docker);
    }

    #[cfg(unix)]
    {
        for sock in unix_socket_candidates() {
            if sock.exists() {
                let sock_str = sock.to_string_lossy();
                if let Ok(docker) =
                    Docker::connect_with_socket(&sock_str, 120, bollard::API_DEFAULT_VERSION)
                    && docker.ping().await.is_ok()
                {
                    return Ok(docker);
                }
            }
        }
    }

    Err(SandboxError::DockerNotAvailable {
        reason: "Could not connect to Docker daemon. Tried: $DOCKER_HOST, \
            /var/run/docker.sock, ~/.docker/run/docker.sock, \
            ~/.colima/default/docker.sock, ~/.rd/docker.sock, \
            $XDG_RUNTIME_DIR/docker.sock, /run/user/$UID/docker.sock"
            .to_string(),
    })
}

#[cfg(unix)]
fn unix_socket_candidates() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    unix_socket_candidates_from_env(
        std::env::var_os("HOME").map(PathBuf::from),
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        std::env::var("UID").ok(),
    )
}

#[cfg(unix)]
fn unix_socket_candidates_from_env(
    home: Option<std::path::PathBuf>,
    xdg_runtime_dir: Option<std::path::PathBuf>,
    uid: Option<String>,
) -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;

    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut push_unique = |path: PathBuf| {
        if !candidates.iter().any(|existing| existing == &path) {
            candidates.push(path);
        }
    };

    if let Some(home) = home {
        push_unique(home.join(".docker/run/docker.sock"));
        push_unique(home.join(".colima/default/docker.sock"));
        push_unique(home.join(".rd/docker.sock"));
    }

    if let Some(xdg_runtime_dir) = xdg_runtime_dir {
        push_unique(xdg_runtime_dir.join("docker.sock"));
    }

    if let Some(uid) = uid.filter(|value| !value.is_empty()) {
        push_unique(PathBuf::from(format!("/run/user/{uid}/docker.sock")));
    }

    candidates
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn unix_socket_candidates_include_rootless_paths() {
        let candidates = unix_socket_candidates_from_env(
            Some(PathBuf::from("/home/tester")),
            Some(PathBuf::from("/run/user/1000")),
            Some("1000".to_string()),
        );

        assert!(candidates.contains(&PathBuf::from("/home/tester/.docker/run/docker.sock")));
        assert!(candidates.contains(&PathBuf::from("/home/tester/.colima/default/docker.sock")));
        assert!(candidates.contains(&PathBuf::from("/home/tester/.rd/docker.sock")));
        assert!(candidates.contains(&PathBuf::from("/run/user/1000/docker.sock")));
    }
}

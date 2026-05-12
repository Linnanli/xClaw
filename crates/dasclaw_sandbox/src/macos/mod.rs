//! macOS Seatbelt sandbox backend — delegates sbpl synthesis to
//! [`dasclaw_sandboxing::seatbelt`] (ADR-135 §3 PR-C1 / Wave-C1).
//!
//! Before Wave-C1 this module composed its own sbpl from three vendored
//! `.sbpl` files plus inline `writable_roots` / `proxy_loopback_ports`
//! handling. That implementation **did not enforce `read_only_subpaths`
//! (洞中洞)** — see ADR-142 / ADR-135 §1.3 "内核层暂不强制". The new code
//! path defers all sbpl assembly to `dasclaw_sandboxing` so we inherit the
//! upstream `FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd`
//! logic that automatically protects `.git/`, `.dasclaw/`, `.codex/` and
//! other sensitive subpaths under a writable root.
//!
//! ## What this module still owns
//!
//! - `env_clear` parent-env scrubbing (W2.2c P1).
//! - `pre_exec` `setrlimit` resource limits (W3.3-2).
//! - Post-spawn `memorystatus_control` memory cap (W3.3-3b).
//! - `spawn` + `wait_with_output` lifecycle.
//!
//! ## Wave-C2a: proxy_loopback_ports threaded via `req.network`
//!
//! `req.network: Option<&NetworkProxy>` is now forwarded to
//! `create_seatbelt_command_args`. When `Some`, the sbpl includes
//! `(allow network-outbound (remote ip "127.0.0.1:N"))` rules for the
//! proxy's loopback HTTP/SOCKS endpoints (restores `HTTP_PROXY=http://
//! 127.0.0.1:PORT` "hole punching" under `allow_network=false`). When
//! `None`, no proxy holes are emitted (fail-closed, matches C1
//! behaviour). Callers (`dasclaw_exec` etc.) start populating the field
//! in Wave-C2b.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub mod memorystatus;

use crate::policy_bridge;
use crate::rlimit;
use crate::{Sandbox, SandboxError, SandboxExecRequest, SandboxType};

use dasclaw_sandboxing::seatbelt::{
    create_seatbelt_command_args, CreateSeatbeltCommandArgsParams,
    MACOS_PATH_TO_SEATBELT_EXECUTABLE,
};

pub struct SeatbeltSandbox;

impl SeatbeltSandbox {
    pub fn new() -> Self {
        Self
    }

    /// Build the `sandbox-exec` argument vector for `req` by delegating sbpl
    /// synthesis to `dasclaw_sandboxing`.
    ///
    /// Returns the *arguments* portion only — the leading
    /// `/usr/bin/sandbox-exec` executable path is left to the caller so the
    /// `Command` construction site can also wire `pre_exec`, env scrubbing,
    /// and `cwd`.
    fn build_seatbelt_args(&self, req: &SandboxExecRequest) -> Vec<String> {
        let cwd: PathBuf = req
            .command
            .get_current_dir()
            .map(std::path::Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("/"));

        let triple = policy_bridge::to_protocol_policy(&req.policy, &cwd);

        let program = req.command.get_program().to_string_lossy().into_owned();
        let mut command: Vec<String> = Vec::with_capacity(1 + req.command.get_args().len());
        command.push(program);
        for a in req.command.get_args() {
            command.push(a.to_string_lossy().into_owned());
        }

        create_seatbelt_command_args(CreateSeatbeltCommandArgsParams {
            command,
            file_system_sandbox_policy: &triple.file_system_policy,
            network_sandbox_policy: triple.network_policy,
            sandbox_policy_cwd: &cwd,
            enforce_managed_network: false,
            network: req.network.as_ref(),
            extra_allow_unix_sockets: &[],
        })
    }
}

impl Default for SeatbeltSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox for SeatbeltSandbox {
    fn kind(&self) -> SandboxType {
        SandboxType::MacosSeatbelt
    }

    fn execute(&self, req: SandboxExecRequest) -> Result<std::process::Output, SandboxError> {
        let args = self.build_seatbelt_args(&req);

        let mut wrapped = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE);
        wrapped.args(&args);

        // Env scrubbing (W2.2c P1 fix): clear inherited parent env, then
        // pass *only* what the caller explicitly set on `req.command`. This
        // prevents accidental secret leakage (AWS_*, OPENAI_API_KEY, etc.)
        // into the sandboxed child.
        wrapped.env_clear();
        for (k, v) in req.command.get_envs() {
            if let Some(val) = v {
                wrapped.env(k, val);
            }
            // Note: `None` means "remove" on the original command, but since
            // env_clear() already wiped everything we just skip.
        }
        if let Some(d) = req.command.get_current_dir() {
            wrapped.current_dir(d);
        }

        // W3.3-2: apply setrlimit in child pre_exec. rlimits inherit across
        // exec(2), so limits set on `sandbox-exec` flow through to the
        // sandboxed program it spawns.
        let limits = req.policy.resource_limits.clone();
        // SAFETY: `apply_in_pre_exec` only invokes async-signal-safe
        // libc::setrlimit calls and returns io::Result. No allocations,
        // no locks, no other Rust runtime dependencies. See module docs
        // on `crate::rlimit`.
        let pre_exec_limits = limits.clone();
        unsafe {
            wrapped.pre_exec(move || rlimit::apply_in_pre_exec(&pre_exec_limits));
        }

        // W3.3-3b: spawn 后用 memorystatus_control 补足内存限制（macOS 上
        // RLIMIT_AS 返回 EINVAL，详见 docs/plans/architecture-refactor/46）。
        // 为了能在 spawn 后拿到 child pid 再调 memorystatus_control，
        // 改用 spawn() + wait_with_output() 取代 output()。
        wrapped.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = wrapped.spawn()?;
        if let Some(bytes) = limits.max_memory_bytes {
            // 设限失败不中断 execute：rlimit 已在 pre_exec 套上 CPU/FD/NPROC，
            // memorystatus 是内存加固。失败记入 stderr observability、但不 failgte。
            // 当前阶段没有 logger 接入；静默忽略错误，后续引入 tracing 后补 warn!。
            let _ = memorystatus::set_memory_limit(child.id() as i32, bytes);
        }
        Ok(child.wait_with_output()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SandboxPolicy, SandboxablePreference};

    #[test]
    fn build_seatbelt_args_emits_sandbox_exec_argv_shape() {
        let sb = SeatbeltSandbox::new();
        let req = SandboxExecRequest {
            command: Command::new("/bin/echo"),
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
            network: None,
        };
        let args = sb.build_seatbelt_args(&req);
        // sandbox-exec args always start with `-p <policy>` and end with
        // `-- <command>...`.
        assert_eq!(args.first().map(String::as_str), Some("-p"));
        let sep = args.iter().position(|a| a == "--").expect("`--` separator");
        // The policy is at index 1.
        let policy = &args[1];
        assert!(
            policy.contains("(version 1)"),
            "policy must start with sbpl (version 1) opener"
        );
        // The command segment must contain `/bin/echo`.
        assert!(args[sep + 1..].iter().any(|a| a == "/bin/echo"));
    }

    #[test]
    fn build_seatbelt_args_propagates_writable_root_for_workspace_write() {
        let sb = SeatbeltSandbox::new();
        let req = SandboxExecRequest {
            command: Command::new("/bin/echo"),
            policy: SandboxPolicy::read_only_defaults().with_writable("/tmp/work"),
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
            network: None,
        };
        let args = sb.build_seatbelt_args(&req);
        // dasclaw_sandboxing materialises writable roots as `-D
        // WRITABLE_ROOT_<N>=<path>` definition args; the sbpl policy body
        // references them as `(param "WRITABLE_ROOT_<N>")`. Verify both
        // sides line up.
        let policy = &args[1];
        assert!(
            policy.contains("file-write"),
            "policy must grant file-write* under a writable root: {policy}"
        );
        assert!(
            policy.contains("WRITABLE_ROOT_"),
            "policy must reference at least one WRITABLE_ROOT_<N> param: {policy}"
        );
        // On macOS `/tmp` is a symlink to `/private/tmp`, so the writable
        // root is canonicalized before reaching the argv. Match the suffix
        // rather than the literal input to stay robust across platforms.
        assert!(
            args.iter().any(|a| a.starts_with("-DWRITABLE_ROOT_")
                && (a.ends_with("=/tmp/work") || a.ends_with("=/private/tmp/work"))),
            "argv must define a WRITABLE_ROOT_<N>=/tmp/work definition, got {args:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_scrubs_parent_env() {
        // Set a sentinel in parent env, run echo $SENTINEL under sandbox,
        // and verify it is empty (env_clear scrubbed it).
        std::env::set_var("DASCLAW_SBX_SENTINEL", "leaked");
        let sb = SeatbeltSandbox::new();
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg("echo \"v=${DASCLAW_SBX_SENTINEL:-empty}\"");
        let req = SandboxExecRequest {
            command: cmd,
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Require,
            windows_sandbox_enabled: false,
            network: None,
        };
        let out = sb.execute(req).expect("seatbelt should run sh");
        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(stdout.trim(), "v=empty", "parent env leaked into sandbox");
        std::env::remove_var("DASCLAW_SBX_SENTINEL");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_runs_echo_under_real_sandbox() {
        // Real integration: must actually invoke /usr/bin/sandbox-exec.
        let sb = SeatbeltSandbox::new();
        let mut cmd = Command::new("/bin/echo");
        cmd.arg("hello");
        let req = SandboxExecRequest {
            command: cmd,
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Require,
            windows_sandbox_enabled: false,
            network: None,
        };
        let out = sb.execute(req).expect("seatbelt should run echo");
        assert!(out.status.success(), "exit={:?}", out.status);
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_propagates_resource_limits_to_child() {
        // W3.3-2 integration: setrlimit applied in pre_exec on the
        // sandbox-exec wrapper must inherit through to the child process.
        // We cap RLIMIT_NOFILE to 64 and ask `sh -c 'ulimit -n'` to print
        // its soft fd limit; the child should see 64, not the parent's.
        use crate::ResourceLimits;

        let sb = SeatbeltSandbox::new();
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg("ulimit -n");

        let limits = ResourceLimits {
            max_memory_bytes: None,
            max_cpu_secs: None,
            max_open_files: Some(64),
            max_processes: None,
        };
        let policy = SandboxPolicy::read_only_defaults().with_resource_limits(limits);

        let req = SandboxExecRequest {
            command: cmd,
            policy,
            preference: SandboxablePreference::Require,
            windows_sandbox_enabled: false,
            network: None,
        };
        let out = sb.execute(req).expect("seatbelt should run sh");
        assert!(out.status.success(), "exit={:?}", out.status);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(
            stdout.trim(),
            "64",
            "child should see RLIMIT_NOFILE=64, got {stdout:?}"
        );
    }

    // ---------------------------------------------------------------------
    // ADR-135 §3 Wave-C2a — `proxy_loopback_ports` 透传契约测试。
    //
    // 验证 `SandboxExecRequest.network` 是否被 `build_seatbelt_args` 正确
    // 转发给 `dasclaw_sandboxing::seatbelt::create_seatbelt_command_args`。
    // PR-C1 的回归在于硬编码 `network: None`；C2a 改为 `req.network.as_ref()`，
    // 在 sbpl 中恢复 `(allow network-outbound (remote ip "localhost:<port>"))`
    // hole-punch 规则。端到端的真实子进程穿透测试留给 PR-C2b（届时
    // `dasclaw_exec` 才会真正下传 `NetworkProxy`）。
    // ---------------------------------------------------------------------

    use async_trait::async_trait;
    use dasclaw_net_proxy::{
        build_config_state, ConfigReloader, ConfigState, NetworkProxy, NetworkProxyConfig,
        NetworkProxyConstraints, NetworkProxyState,
    };
    use std::net::Ipv4Addr;
    use std::net::SocketAddr;
    use std::sync::Arc;

    #[derive(Clone)]
    struct StaticReloader {
        state: ConfigState,
    }

    #[async_trait]
    impl ConfigReloader for StaticReloader {
        async fn maybe_reload(&self) -> anyhow::Result<Option<ConfigState>> {
            Ok(None)
        }

        async fn reload_now(&self) -> anyhow::Result<ConfigState> {
            Ok(self.state.clone())
        }

        fn source_label(&self) -> String {
            "PR-C2a contract test reloader".to_string()
        }
    }

    /// Build a `NetworkProxy` pointing at a fixed loopback HTTP port without
    /// reserving real listeners. `managed_by_codex(false)` keeps the builder
    /// from allocating an ephemeral port so the test stays deterministic.
    async fn make_proxy(http_port: u16) -> NetworkProxy {
        let mut cfg = NetworkProxyConfig::default();
        cfg.network.enabled = true;
        cfg.network.enable_socks5 = false;
        cfg.network.proxy_url = format!("http://127.0.0.1:{http_port}");

        let state = build_config_state(cfg, NetworkProxyConstraints::default())
            .expect("build_config_state");
        let reloader = Arc::new(StaticReloader {
            state: state.clone(),
        });
        let proxy_state = Arc::new(NetworkProxyState::with_reloader(state, reloader));
        NetworkProxy::builder()
            .state(proxy_state)
            .managed_by_codex(false)
            .http_addr(SocketAddr::from((Ipv4Addr::LOCALHOST, http_port)))
            .build()
            .await
            .expect("NetworkProxy::build")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn req_macos_seatbelt_threads_proxy_loopback_port_into_sbpl() {
        const HTTP_PORT: u16 = 18887;
        let proxy = make_proxy(HTTP_PORT).await;

        let mut cmd = Command::new("/bin/true");
        cmd.current_dir("/tmp");
        let req = SandboxExecRequest {
            command: cmd,
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Require,
            windows_sandbox_enabled: false,
            network: Some(proxy),
        };

        let args = SeatbeltSandbox::new().build_seatbelt_args(&req);
        let policy = args
            .iter()
            .skip_while(|a| a.as_str() != "-p")
            .nth(1)
            .expect("sbpl policy follows -p");

        let expected_rule =
            format!("(allow network-outbound (remote ip \"localhost:{HTTP_PORT}\"))");
        assert!(
            policy.contains(&expected_rule),
            "expected sbpl to hole-punch HTTP_PROXY loopback port via {expected_rule}; got policy:\n{policy}"
        );
    }

    #[test]
    fn req_macos_seatbelt_omits_loopback_holes_when_network_is_none() {
        // Fail-safe negative test — defends against a future regression
        // where a default proxy / env sniff would silently inject a hole.
        let mut cmd = Command::new("/bin/true");
        cmd.current_dir("/tmp");
        let req = SandboxExecRequest {
            command: cmd,
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Require,
            windows_sandbox_enabled: false,
            network: None,
        };

        let args = SeatbeltSandbox::new().build_seatbelt_args(&req);
        let policy = args
            .iter()
            .skip_while(|a| a.as_str() != "-p")
            .nth(1)
            .expect("sbpl policy follows -p");

        assert!(
            !policy.contains("(allow network-outbound (remote ip \"localhost:"),
            "with network=None the sbpl must not emit any localhost:<port> hole; got:\n{policy}"
        );
    }
}

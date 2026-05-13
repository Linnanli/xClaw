//! W3.2-C2b 端到端契约：SandboxedExecutor::assemble_request 必须把构造期
//! 注入的 NetworkProxy 忠实透传到 SandboxExecRequest.network，下游
//! seatbelt/landlock 才能拿到 proxy 地址在 sandbox profile 里打洞。
//!
//! 此测试无法放在 `dasclaw_exec/src/lib.rs#tests` 内，因为 NetworkProxy 的
//! cfg(test) 构造 helper (`network_proxy_state_for_policy`) 被 codex 上游
//! drift guard 锁定为 `pub(crate) #[cfg(test)]`（见
//! crates/dasclaw_net_proxy/Cargo.toml 顶部 `DO NOT hand-edit src/*.rs`），
//! 所以这里改用 dasclaw_net_proxy 的全公开 API 自己拼一个 unmanaged
//! NetworkProxy（`managed_by_codex(false)` ⇒ 不绑 socket，纯地址壳子，
//! 满足 `assemble_request` 仅 clone 不调用的契约）。

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use dasclaw_exec::{ExecRequest, SandboxedExecutor};
use dasclaw_net_proxy::{
    ConfigReloader, ConfigState, NetworkMode, NetworkProxy, NetworkProxyConfig,
    NetworkProxyConstraints, NetworkProxyState, build_config_state,
};
use dasclaw_sandbox::SandboxablePreference;
use dasclaw_workspace_cap::SandboxPolicy;

struct NoopReloader;

#[async_trait]
impl ConfigReloader for NoopReloader {
    fn source_label(&self) -> String {
        "dasclaw_exec::tests::network_proxy_threading".to_string()
    }

    async fn maybe_reload(&self) -> Result<Option<ConfigState>> {
        Ok(None)
    }

    async fn reload_now(&self) -> Result<ConfigState> {
        Err(anyhow::anyhow!(
            "force reload is not supported in this contract test"
        ))
    }
}

async fn build_unmanaged_proxy() -> NetworkProxy {
    let mut cfg = NetworkProxyConfig::default();
    cfg.network.enabled = true;
    cfg.network.mode = NetworkMode::Full;
    let state =
        build_config_state(cfg, NetworkProxyConstraints::default()).expect("build_config_state");
    let state = Arc::new(NetworkProxyState::with_reloader(
        state,
        Arc::new(NoopReloader),
    ));
    NetworkProxy::builder()
        .state(state)
        .managed_by_codex(false)
        .build()
        .await
        .expect("build unmanaged proxy")
}

#[tokio::test]
async fn req_executor_assemble_threads_network_into_request() {
    let proxy = build_unmanaged_proxy().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    let executor = SandboxedExecutor::new(
        SandboxPolicy::new_workspace_write_policy(),
        SandboxablePreference::Forbid,
        false,
        Some(proxy.clone()),
    );
    assert!(
        executor.network().is_some(),
        "constructor must retain injected proxy"
    );

    let assembled = executor
        .assemble_request(ExecRequest {
            command: Command::new("true"),
            cwd: tmp.path().to_path_buf(),
        })
        .expect("policy gate should pass under workspace-write + writable cwd");

    assert_eq!(
        assembled.network.as_ref(),
        Some(&proxy),
        "NetworkProxy 必须忠实透传到 SandboxExecRequest.network（W3.2-C2b 契约）"
    );
}

#[tokio::test]
async fn req_executor_assemble_omits_network_when_constructed_without_proxy() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let executor = SandboxedExecutor::new(
        SandboxPolicy::new_workspace_write_policy(),
        SandboxablePreference::Forbid,
        false,
        None,
    );

    let assembled = executor
        .assemble_request(ExecRequest {
            command: Command::new("true"),
            cwd: tmp.path().to_path_buf(),
        })
        .expect("policy gate should pass");
    assert!(
        assembled.network.is_none(),
        "fail-safe: 缺省构造下不允许凭空合成 proxy"
    );

    // 抑制 unused warnings for PathBuf import
    let _ = PathBuf::new();
}

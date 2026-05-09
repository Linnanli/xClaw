//! W7 / ADR-137 PR-N23: Desktop-client adapter for the verbatim-ported
//! [`dasclaw_net_proxy`] (codex-network-proxy 8,876 LOC).
//!
//! # Scope
//!
//! Spawned tool containers receive `HTTPS_PROXY` / `HTTP_PROXY` / `NO_PROXY`
//! env vars pointing at the local proxy, which enforces the per-policy
//! domain allowlist and (optionally) MITM TLS audit.
//!
//! # Credential injection lives elsewhere
//!
//! The legacy 1,766 LOC ironclaw fork that lived here previously injected
//! `Authorization` / `x-api-key` / query-param secrets at the proxy layer.
//! Investigation (see ADR-137 §4 + 31-target-architecture.md §4.x) showed:
//!
//! - Default mappings (`OPENAI_API_KEY` / `ANTHROPIC_API_KEY` /
//!   `NEARAI_API_KEY`) all point at HTTPS hosts.
//! - The legacy code's own NOTE comment + `NETWORK_SECURITY.md` §"No MITM"
//!   acknowledged HTTPS injection was impossible without MITM.
//! - Therefore, sandbox-proxy credential injection was effectively
//!   dead code for the canonical use case.
//!
//! Real credential injection that LLMs cannot bypass lives in:
//!
//! - [`crate::tools::builtin::http`] — host-side `reqwest` builder layer
//!   (works for HTTPS because injection happens before TLS termination).
//! - [`crate::tools::wasm::credential_injector`] — wasm tool host
//!   injection (same property).
//! - [`crate::orchestrator::api`] `/worker/{id}/credentials` endpoint —
//!   per-job env-var injection for spawned containers.
//!
//! # Module surface
//!
//! - [`NetworkProxyHandle`] — keeps the proxy listener alive; dropping it
//!   shuts the proxy down.
//! - [`start_network_proxy`] — assemble + start the proxy from a
//!   [`SandboxConfig`].
//! - [`proxy_env_vars`] — produce the `HTTPS_PROXY` / `HTTP_PROXY` /
//!   `NO_PROXY` triplet to inject into spawned child processes.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_net_proxy::{
    ConfigReloader, ConfigState, NetworkDomainPermission, NetworkDomainPermissionEntry,
    NetworkDomainPermissions, NetworkMode, NetworkProxy, NetworkProxyConfig,
    NetworkProxyConstraints, NetworkProxyHandle as ProxyTaskHandle, NetworkProxyState,
    build_config_state,
};

use crate::sandbox::config::SandboxConfig;
use crate::sandbox::error::{Result, SandboxError};

/// `ConfigReloader` that never reloads. The desktop-client surfaces config
/// changes by restarting the sandbox manager (and therefore the proxy)
/// rather than hot-reloading proxy state in place.
struct StaticReloader;

#[async_trait]
impl ConfigReloader for StaticReloader {
    fn source_label(&self) -> String {
        "ironclaw sandbox::net_proxy::StaticReloader".to_string()
    }

    async fn maybe_reload(&self) -> anyhow::Result<Option<ConfigState>> {
        Ok(None)
    }

    async fn reload_now(&self) -> anyhow::Result<ConfigState> {
        Err(anyhow::anyhow!(
            "ironclaw sandbox proxy does not support runtime reload; restart the sandbox manager"
        ))
    }
}

/// Handle returned after starting the proxy. Holding it keeps the listener
/// alive; dropping it shuts down the proxy.
pub struct NetworkProxyHandle {
    /// Bound HTTP proxy address. Use this for `HTTPS_PROXY` / `HTTP_PROXY`.
    pub addr: SocketAddr,
    /// Bound SOCKS5 proxy address (if SOCKS5 is enabled).
    pub socks_addr: SocketAddr,
    /// The running [`NetworkProxy`] instance. Held to keep the runtime
    /// settings + state reference alive for as long as the handle exists.
    pub proxy: Arc<NetworkProxy>,
    /// Background task handle for the spawned HTTP / SOCKS5 listeners.
    /// Dropped on shutdown.
    _task: ProxyTaskHandle,
}

/// Build the [`NetworkProxyConfig`] for a given sandbox config.
///
/// - `policy.has_full_network()` (`FullAccess`) → `NetworkMode::Full`,
///   no allowlist enforcement (the env-var triplet is still injected for
///   `HTTPS_PROXY` consistency, but every host is accepted).
/// - Otherwise → `NetworkMode::Limited`, with `network_allowlist` mapped
///   to explicit `Allow` permissions (deny-by-default for everything else).
fn build_proxy_config(cfg: &SandboxConfig) -> NetworkProxyConfig {
    let mut proxy_config = NetworkProxyConfig::default();
    proxy_config.network.enabled = true;

    if cfg.policy.has_full_network() {
        proxy_config.network.mode = NetworkMode::Full;
    } else {
        proxy_config.network.mode = NetworkMode::Limited;
        if !cfg.network_allowlist.is_empty() {
            let entries = cfg
                .network_allowlist
                .iter()
                .map(|pattern| NetworkDomainPermissionEntry {
                    pattern: pattern.clone(),
                    permission: NetworkDomainPermission::Allow,
                })
                .collect();
            proxy_config.network.domains = Some(NetworkDomainPermissions { entries });
        }
    }

    proxy_config
}

/// Assemble + start a network proxy for the given sandbox config.
pub async fn start_network_proxy(cfg: &SandboxConfig) -> Result<NetworkProxyHandle> {
    let proxy_config = build_proxy_config(cfg);
    let state: ConfigState = build_config_state(proxy_config, NetworkProxyConstraints::default())
        .map_err(|e| SandboxError::Config {
        reason: format!("network proxy config invalid: {e}"),
    })?;

    let proxy_state = Arc::new(NetworkProxyState::with_reloader(
        state,
        Arc::new(StaticReloader),
    ));

    // `managed_by_codex(false)` lets us pick the bind address rather than
    // having codex's runtime auto-reserve loopback ephemeral listeners
    // through its CODEX_HOME state file. The desktop-client owns the port
    // (`SandboxConfig::proxy_port`), so we pass it through.
    let bind_http = SocketAddr::from(([127, 0, 0, 1], cfg.proxy_port));
    let bind_socks = SocketAddr::from(([127, 0, 0, 1], 0));

    let proxy = NetworkProxy::builder()
        .state(proxy_state)
        .http_addr(bind_http)
        .socks_addr(bind_socks)
        .managed_by_codex(false)
        .build()
        .await
        .map_err(|e| SandboxError::Config {
            reason: format!("network proxy build failed: {e}"),
        })?;

    let http_addr = proxy.http_addr();
    let socks_addr = proxy.socks_addr();
    let task = proxy.run().await.map_err(|e| SandboxError::Config {
        reason: format!("network proxy start failed: {e}"),
    })?;

    Ok(NetworkProxyHandle {
        addr: http_addr,
        socks_addr,
        proxy: Arc::new(proxy),
        _task: task,
    })
}

/// Env vars to inject into spawned child processes / containers so they
/// route HTTP(S) through the proxy. `NO_PROXY` excludes localhost so
/// containers can still reach side-cars.
pub fn proxy_env_vars(addr: SocketAddr) -> Vec<(String, String)> {
    let url = format!("http://{addr}");
    vec![
        ("HTTPS_PROXY".to_string(), url.clone()),
        ("HTTP_PROXY".to_string(), url.clone()),
        ("https_proxy".to_string(), url.clone()),
        ("http_proxy".to_string(), url),
        (
            "NO_PROXY".to_string(),
            "localhost,127.0.0.1,::1".to_string(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::config::{SandboxConfig, SandboxPolicy};

    #[test]
    fn proxy_env_vars_includes_https_http_no_proxy() {
        let addr: SocketAddr = "127.0.0.1:31280".parse().unwrap();
        let vars: std::collections::HashMap<_, _> = proxy_env_vars(addr).into_iter().collect();
        assert_eq!(
            vars.get("HTTPS_PROXY").map(String::as_str),
            Some("http://127.0.0.1:31280")
        );
        assert_eq!(
            vars.get("HTTP_PROXY").map(String::as_str),
            Some("http://127.0.0.1:31280")
        );
        assert_eq!(
            vars.get("NO_PROXY").map(String::as_str),
            Some("localhost,127.0.0.1,::1")
        );
    }

    #[test]
    fn build_proxy_config_full_access_disables_allowlist() {
        let cfg = SandboxConfig {
            policy: SandboxPolicy::FullAccess,
            network_allowlist: vec!["api.openai.com".to_string()],
            ..Default::default()
        };
        let proxy_cfg = build_proxy_config(&cfg);
        assert!(matches!(proxy_cfg.network.mode, NetworkMode::Full));
        assert!(proxy_cfg.network.domains.is_none());
    }

    #[test]
    fn build_proxy_config_sandboxed_maps_allowlist_to_allow_entries() {
        let cfg = SandboxConfig {
            policy: SandboxPolicy::ReadOnly,
            network_allowlist: vec![
                "api.openai.com".to_string(),
                "api.anthropic.com".to_string(),
            ],
            ..Default::default()
        };
        let proxy_cfg = build_proxy_config(&cfg);
        assert!(matches!(proxy_cfg.network.mode, NetworkMode::Limited));
        let domains = proxy_cfg.network.domains.expect("expected allowlist");
        assert_eq!(domains.entries.len(), 2);
        assert!(
            domains
                .entries
                .iter()
                .all(|e| e.permission == NetworkDomainPermission::Allow)
        );
    }

    #[tokio::test]
    async fn start_network_proxy_binds_loopback() {
        let cfg = SandboxConfig {
            policy: SandboxPolicy::ReadOnly,
            network_allowlist: vec!["api.openai.com".to_string()],
            proxy_port: 0,
            ..Default::default()
        };
        let handle = start_network_proxy(&cfg).await.expect("proxy starts");
        assert!(handle.addr.ip().is_loopback());
        assert_ne!(handle.addr.port(), 0);
    }
}

//! W3.2b-4: Desktop-client integration of [`dasclaw_net_proxy`].
//!
//! Bridges the OS-sandbox layer (`SandboxConfig` + `SecretsStore`) to the
//! audited HTTP egress proxy. Spawned tool containers receive `HTTPS_PROXY` /
//! `HTTP_PROXY` / `NO_PROXY` env vars pointing at the local proxy, which then
//! enforces the per-policy domain allowlist and injects credentials resolved
//! from the desktop-client `SecretsStore` (postgres-backed, master-key sealed
//! by the per-OS keychain — see [`crate::secrets::keychain`]).
//!
//! # Fail-safe contract
//!
//! - `IronclawSecretsResolver::resolve` returns `None` on **any** error
//!   (NotFound, decryption failure, db error). The proxy treats `None` as
//!   "no credential available" and either omits the header (when
//!   `optional: true`) or denies the request (when `optional: false`,
//!   the default).
//! - Conversion of an unknown / malformed mapping never panics; if the local
//!   `CredentialLocation` cannot be represented by the proxy crate, the
//!   mapping is silently dropped and the request will be allowed without
//!   credential injection (subject to allowlist).
//!
//! # Module layout
//!
//! - [`IronclawSecretsResolver`] — `dasclaw_net_proxy::CredentialResolver`
//!   impl that pulls decrypted secrets from a `SecretsStore`.
//! - [`to_proxy_mappings`] — translate `crate::secrets::CredentialMapping`
//!   into `dasclaw_net_proxy::CredentialMapping`.
//! - [`start_network_proxy`] — assemble a [`NetworkProxyBuilder`] from a
//!   [`SandboxConfig`] + `SecretsStore`, start it, return a handle.
//! - [`proxy_env_vars`] — produce the `HTTPS_PROXY` / `HTTP_PROXY` /
//!   `NO_PROXY` triplet to inject into spawned child processes.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_net_proxy::{
    CredentialLocation as ProxyLocation, CredentialMapping as ProxyMapping, CredentialResolver,
    HttpProxy, NetworkProxyBuilder, ProxyMode,
};

use crate::sandbox::config::SandboxConfig;
use crate::sandbox::error::{Result, SandboxError};
use crate::secrets::{
    CredentialLocation as LocalLocation, CredentialMapping as LocalMapping, SecretsStore,
};

/// Bridges `dasclaw_net_proxy::CredentialResolver` to the desktop-client
/// `SecretsStore`. All errors are coerced to `None` (fail-safe).
pub struct IronclawSecretsResolver {
    store: Arc<dyn SecretsStore + Send + Sync>,
    user_id: String,
}

impl IronclawSecretsResolver {
    pub fn new(store: Arc<dyn SecretsStore + Send + Sync>, user_id: impl Into<String>) -> Self {
        Self {
            store,
            user_id: user_id.into(),
        }
    }
}

#[async_trait]
impl CredentialResolver for IronclawSecretsResolver {
    async fn resolve(&self, name: &str) -> Option<String> {
        match self.store.get_decrypted(&self.user_id, name).await {
            Ok(decrypted) => Some(decrypted.expose().to_string()),
            Err(err) => {
                // Fail-safe: never leak the secret name in error logs at
                // info level, and never propagate the error — proxy will
                // deny the request via the `optional: false` default.
                tracing::debug!(secret = %name, error = %err, "secret resolution failed");
                None
            }
        }
    }
}

/// Convert a single desktop-client `CredentialMapping` into the proxy's
/// representation. Returns `None` if the mapping has no host patterns or
/// uses a `CredentialLocation` variant that the proxy crate cannot enforce.
pub fn to_proxy_mapping(local: &LocalMapping) -> Option<ProxyMapping> {
    if local.host_patterns.is_empty() {
        return None;
    }

    let location = match &local.location {
        LocalLocation::AuthorizationBearer => ProxyLocation::AuthorizationBearer,
        LocalLocation::AuthorizationBasic { username } => ProxyLocation::AuthorizationBasic {
            username: username.clone(),
        },
        LocalLocation::Header { name, prefix } => ProxyLocation::Header {
            name: name.clone(),
            prefix: prefix.clone(),
        },
        LocalLocation::QueryParam { name } => ProxyLocation::QueryParam { name: name.clone() },
        LocalLocation::UrlPath { placeholder } => ProxyLocation::UrlPath {
            placeholder: placeholder.clone(),
        },
    };

    Some(ProxyMapping {
        secret_name: local.secret_name.clone(),
        location,
        host_patterns: local.host_patterns.clone(),
        optional: false, // Fail-safe: missing secret denies the request.
    })
}

/// Translate a list of local mappings. Mappings with empty `host_patterns`
/// are dropped.
pub fn to_proxy_mappings(local: &[LocalMapping]) -> Vec<ProxyMapping> {
    local.iter().filter_map(to_proxy_mapping).collect()
}

/// Handle returned after starting the proxy. Holding it keeps the listener
/// alive; dropping it shuts down the proxy.
pub struct NetworkProxyHandle {
    pub proxy: Arc<HttpProxy>,
    pub addr: SocketAddr,
}

/// Assemble + start a network proxy for the given sandbox config.
///
/// Mode mapping:
/// - `SandboxPolicy::FullAccess` → `ProxyMode::AllowAll` (no enforcement;
///   policy bypasses the sandbox anyway, kept here for env-var consistency).
/// - Any other policy → `ProxyMode::Restricted` (allowlist + credential
///   injection enforced).
pub async fn start_network_proxy(
    cfg: &SandboxConfig,
    credential_mappings: Vec<LocalMapping>,
    store: Arc<dyn SecretsStore + Send + Sync>,
    user_id: impl Into<String>,
) -> Result<NetworkProxyHandle> {
    let mode = if cfg.policy.has_full_network() {
        ProxyMode::AllowAll
    } else {
        ProxyMode::Restricted
    };

    let resolver: Arc<dyn CredentialResolver> =
        Arc::new(IronclawSecretsResolver::new(store, user_id));

    let proxy = NetworkProxyBuilder::new()
        .with_mode(mode)
        .with_allowlist(cfg.network_allowlist.clone())
        .with_credentials(to_proxy_mappings(&credential_mappings))
        .with_credential_resolver(resolver)
        .build();

    let addr = proxy
        .start(cfg.proxy_port)
        .await
        .map_err(|e| SandboxError::Config {
            reason: format!("network proxy start failed: {e}"),
        })?;

    Ok(NetworkProxyHandle {
        proxy: Arc::new(proxy),
        addr,
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
    use crate::secrets::{CreateSecretParams, DecryptedSecret, Secret, SecretError, SecretRef};
    use async_trait::async_trait;
    use std::sync::Mutex;
    use uuid::Uuid;

    /// Minimal in-memory store sufficient for resolver tests. Only
    /// `get_decrypted` is exercised; other methods return `NotFound`.
    struct MemStore {
        entries: Mutex<Vec<(String, String, String)>>, // (user, name, value)
    }
    impl MemStore {
        fn with(entries: Vec<(&str, &str, &str)>) -> Self {
            Self {
                entries: Mutex::new(
                    entries
                        .into_iter()
                        .map(|(u, n, v)| (u.into(), n.into(), v.into()))
                        .collect(),
                ),
            }
        }
    }
    #[async_trait]
    impl SecretsStore for MemStore {
        async fn create(
            &self,
            _: &str,
            _: CreateSecretParams,
        ) -> std::result::Result<Secret, SecretError> {
            Err(SecretError::NotFound("not implemented".into()))
        }
        async fn get(&self, _: &str, name: &str) -> std::result::Result<Secret, SecretError> {
            Err(SecretError::NotFound(name.into()))
        }
        async fn get_decrypted(
            &self,
            user_id: &str,
            name: &str,
        ) -> std::result::Result<DecryptedSecret, SecretError> {
            let g = self.entries.lock().unwrap();
            for (u, n, v) in g.iter() {
                if u == user_id && n == name {
                    return DecryptedSecret::from_bytes(v.as_bytes().to_vec());
                }
            }
            Err(SecretError::NotFound(name.into()))
        }
        async fn exists(&self, _: &str, _: &str) -> std::result::Result<bool, SecretError> {
            Ok(false)
        }
        async fn list(&self, _: &str) -> std::result::Result<Vec<SecretRef>, SecretError> {
            Ok(vec![])
        }
        async fn delete(&self, _: &str, _: &str) -> std::result::Result<bool, SecretError> {
            Ok(false)
        }
        async fn record_usage(&self, _: Uuid) -> std::result::Result<(), SecretError> {
            Ok(())
        }
        async fn is_accessible(
            &self,
            _: &str,
            _: &str,
            _: &[String],
        ) -> std::result::Result<bool, SecretError> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn resolver_returns_value_when_present() {
        let store = Arc::new(MemStore::with(vec![("alice", "OPENAI_API_KEY", "sk-xyz")]));
        let r = IronclawSecretsResolver::new(store, "alice");
        assert_eq!(r.resolve("OPENAI_API_KEY").await.as_deref(), Some("sk-xyz"));
    }

    #[tokio::test]
    async fn resolver_returns_none_for_missing_secret() {
        let store = Arc::new(MemStore::with(vec![]));
        let r = IronclawSecretsResolver::new(store, "alice");
        assert!(r.resolve("MISSING").await.is_none());
    }

    #[tokio::test]
    async fn resolver_isolates_users() {
        let store = Arc::new(MemStore::with(vec![("alice", "K", "a-val")]));
        let bob = IronclawSecretsResolver::new(store.clone(), "bob");
        assert!(
            bob.resolve("K").await.is_none(),
            "bob must not see alice's secret"
        );
    }

    #[test]
    fn mapping_bearer_round_trips() {
        let local = LocalMapping::bearer("OPENAI_API_KEY", "api.openai.com");
        let proxy = to_proxy_mapping(&local).expect("convertible");
        assert_eq!(proxy.secret_name, "OPENAI_API_KEY");
        assert_eq!(proxy.host_patterns, vec!["api.openai.com".to_string()]);
        assert!(matches!(proxy.location, ProxyLocation::AuthorizationBearer));
        assert!(!proxy.optional, "must default to required (fail-safe)");
    }

    #[test]
    fn mapping_header_round_trips() {
        let local = LocalMapping::header("ANTHROPIC_API_KEY", "x-api-key", "api.anthropic.com");
        let proxy = to_proxy_mapping(&local).expect("convertible");
        match proxy.location {
            ProxyLocation::Header { name, prefix } => {
                assert_eq!(name, "x-api-key");
                assert!(prefix.is_none());
            }
            other => panic!("unexpected location {other:?}"),
        }
    }

    #[test]
    fn mapping_with_empty_hosts_is_dropped() {
        let local = LocalMapping {
            secret_name: "X".into(),
            location: LocalLocation::AuthorizationBearer,
            host_patterns: vec![],
        };
        assert!(to_proxy_mapping(&local).is_none());
    }

    #[test]
    fn mappings_expand_multi_host() {
        let local = LocalMapping {
            secret_name: "K".into(),
            location: LocalLocation::AuthorizationBearer,
            host_patterns: vec!["a.com".into(), "b.com".into()],
        };
        let out = to_proxy_mappings(&[local]);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].host_patterns,
            vec!["a.com".to_string(), "b.com".to_string()]
        );
    }

    #[test]
    fn proxy_env_vars_include_no_proxy() {
        let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let env = proxy_env_vars(addr);
        let map: std::collections::HashMap<_, _> = env.into_iter().collect();
        assert_eq!(map["HTTPS_PROXY"], "http://127.0.0.1:9999");
        assert_eq!(map["HTTP_PROXY"], "http://127.0.0.1:9999");
        assert!(map["NO_PROXY"].contains("127.0.0.1"));
    }

    #[tokio::test]
    async fn start_network_proxy_binds_and_serves() {
        let cfg = SandboxConfig {
            policy: SandboxPolicy::ReadOnly,
            network_allowlist: vec!["example.com".into()],
            proxy_port: 0,
            ..SandboxConfig::default()
        };
        let store = Arc::new(MemStore::with(vec![]));
        let handle = start_network_proxy(&cfg, vec![], store, "alice")
            .await
            .expect("proxy starts");
        assert!(handle.addr.port() > 0);
        assert!(handle.proxy.is_running());
    }

    #[tokio::test]
    async fn full_access_policy_uses_allow_all_mode() {
        // Sanity: FullAccess must not crash the builder even with empty allowlist.
        let cfg = SandboxConfig {
            policy: SandboxPolicy::FullAccess,
            network_allowlist: vec![],
            proxy_port: 0,
            ..SandboxConfig::default()
        };
        let store = Arc::new(MemStore::with(vec![]));
        let handle = start_network_proxy(&cfg, vec![], store, "alice")
            .await
            .expect("proxy starts in AllowAll");
        assert!(handle.proxy.is_running());
    }
}

//! Builder for assembling an [`HttpProxy`] from allowlist + credential mappings.
//!
//! Simplified port from `ironclaw-main/src/sandbox/proxy/mod.rs::NetworkProxyBuilder`.
//! The `from_config` constructor is intentionally **not** ported — callers in
//! W3.2b-4 wire the proxy from desktop-client `SandboxConfig` directly via
//! `with_allowlist`/`with_credentials`/`with_credential_resolver`, keeping
//! this crate decoupled from any specific config schema.

use std::sync::Arc;

use crate::allowlist::DomainAllowlist;
use crate::error::Result;
use crate::http::{CredentialResolver, EnvCredentialResolver, HttpProxy};
use crate::policy::{AllowAllDecider, DefaultPolicyDecider, DenyAllDecider, NetworkPolicyDecider};
use crate::types::CredentialMapping;

/// What policy mode to apply when no custom decider is supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProxyMode {
    /// Default: enforce allowlist + credential mappings.
    #[default]
    Restricted,
    /// Allow everything (use only when caller has its own enforcement).
    AllowAll,
    /// Deny everything (kill-switch).
    DenyAll,
}

/// Fluent builder for [`HttpProxy`].
pub struct NetworkProxyBuilder {
    allowlist: Vec<String>,
    credential_mappings: Vec<CredentialMapping>,
    credential_resolver: Arc<dyn CredentialResolver>,
    mode: ProxyMode,
    deny_reason: String,
}

impl NetworkProxyBuilder {
    /// Create a new builder with safe defaults: empty allowlist, no
    /// credentials, env-based resolver, [`ProxyMode::Restricted`].
    ///
    /// **Note**: an empty allowlist combined with `Restricted` mode denies
    /// every request. Callers must add at least one allowed domain via
    /// [`Self::with_allowlist`] or [`Self::allow_domain`].
    pub fn new() -> Self {
        Self {
            allowlist: Vec::new(),
            credential_mappings: Vec::new(),
            credential_resolver: Arc::new(EnvCredentialResolver),
            mode: ProxyMode::default(),
            deny_reason: "denied by policy".to_string(),
        }
    }

    pub fn with_allowlist(mut self, domains: Vec<String>) -> Self {
        self.allowlist = domains;
        self
    }

    pub fn allow_domain(mut self, domain: impl Into<String>) -> Self {
        self.allowlist.push(domain.into());
        self
    }

    pub fn with_credentials(mut self, mappings: Vec<CredentialMapping>) -> Self {
        self.credential_mappings = mappings;
        self
    }

    pub fn with_credential_resolver(mut self, resolver: Arc<dyn CredentialResolver>) -> Self {
        self.credential_resolver = resolver;
        self
    }

    pub fn with_mode(mut self, mode: ProxyMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_deny_reason(mut self, reason: impl Into<String>) -> Self {
        self.deny_reason = reason.into();
        self
    }

    /// Assemble the proxy without starting it.
    pub fn build(self) -> HttpProxy {
        let decider: Arc<dyn NetworkPolicyDecider> = match self.mode {
            ProxyMode::AllowAll => Arc::new(AllowAllDecider),
            ProxyMode::DenyAll => Arc::new(DenyAllDecider::new(&self.deny_reason)),
            ProxyMode::Restricted => Arc::new(DefaultPolicyDecider::new(
                DomainAllowlist::new(&self.allowlist),
                self.credential_mappings,
            )),
        };

        HttpProxy::new(decider, self.credential_resolver)
    }

    /// Build the proxy and start listening on the given port (0 = auto).
    pub async fn build_and_start(self, port: u16) -> Result<HttpProxy> {
        let proxy = self.build();
        proxy.start(port).await?;
        Ok(proxy)
    }
}

impl Default for NetworkProxyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_restricted_with_empty_allowlist() {
        let b = NetworkProxyBuilder::new();
        assert_eq!(b.mode, ProxyMode::Restricted);
        assert!(b.allowlist.is_empty());
    }

    #[test]
    fn allow_domain_appends() {
        let b = NetworkProxyBuilder::new()
            .allow_domain("a.com")
            .allow_domain("b.com");
        assert_eq!(b.allowlist, vec!["a.com".to_string(), "b.com".to_string()]);
    }

    #[tokio::test]
    async fn builds_proxy_in_each_mode() {
        for mode in [
            ProxyMode::Restricted,
            ProxyMode::AllowAll,
            ProxyMode::DenyAll,
        ] {
            let proxy = NetworkProxyBuilder::new()
                .with_mode(mode)
                .allow_domain("example.com")
                .build();
            assert!(
                !proxy.is_running(),
                "mode {:?}: proxy must not auto-start",
                mode
            );
        }
    }
}

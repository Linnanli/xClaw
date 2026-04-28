//! HTTP forward proxy with domain allowlist + credential injection.
//!
//! Ported from `ironclaw-main/src/sandbox/proxy/*` per ADR
//! `docs/plans/architecture-refactor/43-net-proxy-port-adr.md` (W3.2b-1).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Network Proxy                               │
//! │                                                                  │
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │
//! │  │ HTTP Proxy  │───▶│   Policy    │───▶│ Credential Resolver │  │
//! │  │   Server    │    │   Decider   │    │  (host-supplied)    │  │
//! │  └─────────────┘    └─────────────┘    └─────────────────────┘  │
//! │         │                  │                                     │
//! │         │                  ▼                                     │
//! │         │           ┌─────────────┐                             │
//! │         │           │  Allowlist  │                             │
//! │         │           │  Validator  │                             │
//! │         │           └─────────────┘                             │
//! │         ▼                                                        │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │                    Internet                               │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Decoupling
//!
//! This crate is intentionally decoupled from any secrets-store backend:
//! credential **resolution** is performed via the [`CredentialResolver`]
//! trait, and credential **types** ([`CredentialMapping`], [`CredentialLocation`])
//! are pure data structures. Wire a real backend (e.g. `secrets::SecretsStore`)
//! by implementing [`CredentialResolver`] in the consuming crate.
//!
//! [`CredentialResolver`]: http::CredentialResolver

pub mod allowlist;
pub mod builder;
pub mod error;
pub mod http;
pub mod policy;
pub mod types;

pub use allowlist::{DomainAllowlist, DomainPattern, DomainValidationResult};
pub use builder::{NetworkProxyBuilder, ProxyMode};
pub use error::{ProxyError, Result};
pub use http::{CredentialResolver, EnvCredentialResolver, HttpProxy, NoCredentialResolver};
pub use policy::{
    AllowAllDecider, DefaultPolicyDecider, DenyAllDecider, NetworkDecision, NetworkPolicyDecider,
    NetworkRequest,
};
pub use types::{CredentialLocation, CredentialMapping};

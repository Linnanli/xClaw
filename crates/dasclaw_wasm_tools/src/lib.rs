//! WASM sandbox primitives for tool execution.
//!
//! This crate hosts the verbatim port of `desktop-client/ironclaw/src/tools/wasm/`
//! infrastructure modules per [ADR-152 §4.2 addendum] (F4.5.3 + F4.5.4). The following
//! files moved here unchanged (only `use` paths and `pub(crate)` visibility were
//! adjusted per ADR-129 §1.3.1 fork-able boundary):
//!
//! - `error` — `WasmError` + `From<WasmError> for dasclaw_tool::ToolError`
//! - `limits` — wasmtime `ResourceLimiter`, fuel/memory/timeout defaults
//! - `capabilities` — `Capabilities` descriptor + sub-capabilities
//! - `capabilities_schema` — YAML schema for `capabilities.yaml`
//! - `host` — `HostState` for wasmtime store (logs, capabilities)
//! - `allowlist` — HTTP endpoint allowlist validator
//! - `storage` — `WasmToolStore` trait + Postgres / libSQL implementations
//! - `credential_injector` — secret injection into outbound HTTP requests
//! - `runtime` — `WasmToolRuntime` engine/store lifecycle (F4.5.4)
//! - `wrapper` — `WasmToolWrapper` Tool impl + WIT bindings (F4.5.4)
//! - `test_credentials` — shared fixtures consumed by wrapper tests (F4.5.4)
//!
//! The modules `rate_limiter` and `loader` remain in
//! `desktop-client/ironclaw/src/tools/wasm/` because they couple to the desktop
//! `ToolRegistry` or shim into desktop modules outside `wasm/`. See
//! `tools/wasm/mod.rs` for the re-export shim that keeps `crate::*` working from
//! the desktop side.

pub mod allowlist;
pub mod capabilities;
pub mod capabilities_schema;
pub mod credential_injector;
pub mod error;
pub mod host;
pub mod limits;
pub mod runtime;
pub mod storage;
pub mod test_credentials;
pub mod wrapper;

/// WIT interface version exposed to tools (matches `wit/tool.wit`).
pub const WIT_TOOL_VERSION: &str = "0.3.0";

/// Channel WIT interface version (matches `desktop-client/ironclaw/wit/channel.wit`).
pub const WIT_CHANNEL_VERSION: &str = "0.3.0";

// Core re-exports (mirror of the historical `tools/wasm/mod.rs` block, minus
// the two modules that stay in the desktop crate).
pub use error::WasmError;
pub use host::{HostState, LogEntry, LogLevel};
pub use limits::{
    DEFAULT_FUEL_LIMIT, DEFAULT_MEMORY_LIMIT, DEFAULT_TIMEOUT, FuelConfig, ResourceLimits,
    WasmResourceLimiter,
};
pub use runtime::{PreparedModule, WasmRuntimeConfig, WasmToolRuntime, enable_compilation_cache};
pub use wrapper::{OAuthRefreshConfig, WasmToolWrapper};

pub use capabilities::{
    Capabilities, EndpointPattern, HttpCapability, RateLimitConfig, SecretsCapability,
    ToolInvokeCapability, WebhookCapability, WorkspaceCapability, WorkspaceReader,
};

pub use allowlist::{AllowlistResult, AllowlistValidator, DenyReason};
pub use credential_injector::{
    CredentialInjector, InjectedCredentials, InjectionError, SharedCredentialRegistry,
    host_matches_pattern, inject_credential,
};

#[cfg(feature = "libsql")]
pub use storage::LibSqlWasmToolStore;
#[cfg(feature = "postgres")]
pub use storage::PostgresWasmToolStore;
pub use storage::{
    StoreToolParams, StoredCapabilities, StoredWasmTool, StoredWasmToolWithBinary, ToolStatus,
    TrustLevel, WasmStorageError, WasmToolStore, compute_binary_hash, verify_binary_integrity,
};

pub use capabilities_schema::{
    AuthCapabilitySchema, CapabilitiesFile, OAuthConfigSchema, RateLimitSchema,
    ToolFieldSetupSchema, ToolSetupFieldInputType, ToolSetupSchema, ValidationEndpointSchema,
};

//! WASM sandbox for untrusted tool execution.
//!
//! Most primitives (allowlist, capabilities, capabilities_schema,
//! credential_injector, error, host, limits, storage, runtime, wrapper) have
//! been moved to the `dasclaw_wasm_tools` crate under ADR-152 F4.5.3 + F4.5.4.
//! This module re-exports them so existing `crate::tools::wasm::*` call sites
//! keep compiling, and keeps the two still-desktop-coupled modules (loader,
//! rate_limiter) in place.

/// Host WIT version for tool extensions.
///
/// Extensions declaring a `wit_version` in their capabilities file are checked
/// against this at load time: same major, not greater than host.
pub use dasclaw_wasm_tools::{WIT_CHANNEL_VERSION, WIT_TOOL_VERSION};

// Re-export moved modules so `crate::tools::wasm::<mod>::*` paths still resolve.
pub use dasclaw_wasm_tools::{
    allowlist, capabilities, capabilities_schema, credential_injector, error, host, limits,
    runtime, storage, wrapper,
};

// Modules still kept in the desktop crate.
pub(crate) mod loader;
mod rate_limiter;

// Core types
pub use dasclaw_wasm_tools::WasmError;
pub use dasclaw_wasm_tools::{
    DEFAULT_FUEL_LIMIT, DEFAULT_MEMORY_LIMIT, DEFAULT_TIMEOUT, FuelConfig, ResourceLimits,
    WasmResourceLimiter,
};
pub use dasclaw_wasm_tools::{HostState, LogEntry, LogLevel};
pub use dasclaw_wasm_tools::{
    OAuthRefreshConfig, PreparedModule, WasmRuntimeConfig, WasmToolRuntime, WasmToolWrapper,
    enable_compilation_cache,
};

// Capabilities (V2)
pub use dasclaw_wasm_tools::{
    Capabilities, EndpointPattern, HttpCapability, RateLimitConfig, SecretsCapability,
    ToolInvokeCapability, WebhookCapability, WorkspaceCapability, WorkspaceReader,
};

// Security components (V2)
pub use dasclaw_wasm_tools::inject_credential;
pub use dasclaw_wasm_tools::{AllowlistResult, AllowlistValidator, DenyReason};
pub use dasclaw_wasm_tools::{
    CredentialInjector, InjectedCredentials, InjectionError, SharedCredentialRegistry,
};
pub use rate_limiter::{LimitType, RateLimitError, RateLimitResult, RateLimiter};

// Storage (V2)
#[cfg(feature = "libsql")]
pub use dasclaw_wasm_tools::LibSqlWasmToolStore;
#[cfg(feature = "postgres")]
pub use dasclaw_wasm_tools::PostgresWasmToolStore;
pub use dasclaw_wasm_tools::{
    StoreToolParams, StoredCapabilities, StoredWasmTool, StoredWasmToolWithBinary, ToolStatus,
    TrustLevel, WasmStorageError, WasmToolStore, compute_binary_hash, verify_binary_integrity,
};

// Loader
pub use loader::{
    DiscoveredTool, LoadResults, WasmLoadError, WasmToolLoader, check_wit_version_compat,
    discover_dev_tools, discover_tools, load_dev_tools, resolve_wasm_target_dir,
    wasm_artifact_path,
};

// Capabilities schema (for parsing *.capabilities.json files)
pub use dasclaw_wasm_tools::{
    AuthCapabilitySchema, CapabilitiesFile, OAuthConfigSchema, RateLimitSchema,
    ToolFieldSetupSchema, ToolSetupFieldInputType, ToolSetupSchema, ValidationEndpointSchema,
};

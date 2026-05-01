# P0-A — Sandbox Activation Inventory

> **Scope**: Inventory only. This document records the **current static facts**
> about how `OsExecutor`, `start_network_proxy`, and `ShellTool` sandbox/proxy
> configuration are (or are not) wired into the desktop-client app boot path.
>
> **Non-scope**: This document **does not propose defaults, policy, or any
> behavior change**. All decision points are deferred to the redline ADR
> tracked in [#127](https://github.com/Linnanli/xClaw/issues/127). Implementation
> is deferred to [#128](https://github.com/Linnanli/xClaw/issues/128).
>
> **Source issue**: [#126](https://github.com/Linnanli/xClaw/issues/126) (parent
> [#28](https://github.com/Linnanli/xClaw/issues/28)).
>
> **Three-layer evidence**: Per AGENTS.md §"分析工具使用规范", every "X has Y" /
> "X lacks Y" claim below carries a Level 1 (semantic_search) and Level 3
> (`grep_search` / `rg`) reference. Level 2 (`vscode_listCodeUsages`) is
> **unsupported for Rust** in this workspace; documented as a known limitation,
> not a gap.

---

## 1. Source-of-truth file map

The activation path involves five files. All paths are workspace-relative.

| File | Role |
|---|---|
| [desktop-client/ironclaw/src/sandbox/os_executor.rs](../../../desktop-client/ironclaw/src/sandbox/os_executor.rs) | `OsExecutor::new(timeout, allow_full_access)` — OS-process sandbox executor (macOS Seatbelt / Linux Landlock+seccomp via `dasclaw_sandbox`). |
| [desktop-client/ironclaw/src/sandbox/net_proxy.rs](../../../desktop-client/ironclaw/src/sandbox/net_proxy.rs) | `start_network_proxy(cfg, mappings, store, user_id)` + `proxy_env_vars(addr)` — local HTTP forward proxy with allowlist + `SecretsStore`-backed credential resolver. |
| [desktop-client/ironclaw/src/tools/builtin/shell.rs](../../../desktop-client/ironclaw/src/tools/builtin/shell.rs) | `ShellTool::new()` + builders `with_sandbox(Arc<OsExecutor>)`, `with_sandbox_policy(SandboxPolicy)`, `with_extra_env(HashMap<String, String>)`. |
| [desktop-client/ironclaw/src/tools/registry.rs](../../../desktop-client/ironclaw/src/tools/registry.rs) | `register_dev_tools()` — single production callsite that constructs `ShellTool::new()` with **no** sandbox/policy/env wiring. |
| [desktop-client/ironclaw/src/app.rs](../../../desktop-client/ironclaw/src/app.rs) | App boot orchestrator. Owns `Option<Arc<dyn SecretsStore>>` and dispatches `bootstrap_tools(&BootstrapContext)`. |

---

## 2. ShellTool public builder surface

`ShellTool` already exposes the three builders required by the W3.2b-5 caller
pattern documented in [44-net-proxy-port-completion.md §"Caller integration pattern"](./44-net-proxy-port-completion.md):

| Method | Signature | Default if unset |
|---|---|---|
| `pub fn new()` | `fn new() -> Self` | Returns a `ShellTool` with `sandbox: None`, `sandbox_policy: SandboxPolicy::ReadOnly`, `extra_env: HashMap::new()`. |
| `pub fn with_sandbox(...)` | `fn with_sandbox(self, sandbox: Arc<OsExecutor>) -> Self` | Field stays `None` → `execute_direct()` path. |
| `pub fn with_sandbox_policy(...)` | `fn with_sandbox_policy(self, policy: SandboxPolicy) -> Self` | `SandboxPolicy::ReadOnly` (only meaningful when `sandbox` is `Some`). |
| `pub fn with_extra_env(...)` | `fn with_extra_env(self, env: HashMap<String, String>) -> Self` | Empty map (no proxy env merged into spawned commands). |

**Evidence**:
- L3: `grep "pub fn (new|with_sandbox|with_extra_env|with_sandbox_policy)|impl ShellTool"` matches `desktop-client/ironclaw/src/tools/builtin/shell.rs:578, 580, 596, 614, 620`.
- L1: `semantic_search "ShellTool builder with_sandbox extra_env"` returns the same 3 builder methods + the W3.2b-5 caller-integration excerpt from `44-net-proxy-port-completion.md`.

---

## 3. Production callsite map

### 3.1 `OsExecutor::new(...)` — production callers: 0

**Claim**: Outside `#[cfg(test)]`, no production code in this workspace
constructs an `OsExecutor`.

| Callsite | File | Line | Context |
|---|---|---|---|
| Test | `desktop-client/ironclaw/src/sandbox/os_executor.rs` | 224 | inside `#[cfg(test)] mod tests` (line 195). |
| Test | `desktop-client/ironclaw/src/sandbox/os_executor.rs` | 238 | inside the same test module. |
| Doc | `docs/plans/architecture-refactor/44-net-proxy-port-completion.md` | 71 | example caller integration pattern (markdown, not code). |
| Doc | `docs/plans/architecture-refactor/48-agent-framework-readiness-and-task-audit.md` | 87 | source of #28's gap claim. |

**Evidence**:
- L1: `semantic_search "app boot constructs OS sandbox executor"` returns codex
  caller paths (`codex-rs/cli/src/debug_sandbox.rs`, `core/src/session/mod.rs`)
  and **no** desktop-client production path. Codex paths are out of scope for
  W3 P0-A activation.
- L3: `grep_search "OsExecutor::new\("` (regex) → 4 matches total, of which 2
  are `#[cfg(test)]` and 2 are markdown docs. **Zero production callsites.**
- L2: Not available — `vscode_listCodeUsages` returns "No reference provider
  available for this file's language" for Rust in this workspace.

### 3.2 `start_network_proxy(...)` — production callers: 0

**Claim**: Outside `#[cfg(test)]`, no production code in this workspace calls
`start_network_proxy`.

| Callsite | File | Line | Context |
|---|---|---|---|
| Definition | `desktop-client/ironclaw/src/sandbox/net_proxy.rs` | 131 | `pub async fn start_network_proxy(...)`. |
| Test | `desktop-client/ironclaw/src/sandbox/net_proxy.rs` | 347 | inside `#[cfg(test)] mod tests` (line 183). |
| Test | `desktop-client/ironclaw/src/sandbox/net_proxy.rs` | 364 | inside the same test module. |
| Doc | `docs/plans/architecture-refactor/44-net-proxy-port-completion.md` | 68 | example caller integration. |
| Doc | `docs/plans/architecture-refactor/48-agent-framework-readiness-and-task-audit.md` | 87, 281 | source of #28's gap claim. |

**Evidence**:
- L1: same `semantic_search` as §3.1 returns codex `Session::start_managed_network_proxy` and `NetworkProxySpec::start_proxy` (both production-wired) but **only test callers** for desktop-client `start_network_proxy`.
- L3: `grep_search "start_network_proxy\("` (regex) → 6 matches total, of which 1 is the definition, 2 are `#[cfg(test)]`, 3 are markdown docs. **Zero production callsites.**

### 3.3 `ShellTool::new()` production registration — single dormant callsite

**Claim**: There is exactly **one** production callsite that constructs and
registers `ShellTool` for tool dispatch, and it does **not** call
`with_sandbox`, `with_sandbox_policy`, or `with_extra_env`.

The callsite:

```rust
// desktop-client/ironclaw/src/tools/registry.rs:479-480
fn register_dev_tools(&self) {
    self.register_sync(Arc::new(ShellTool::new()));
    self.register_sync(Arc::new(ReadFileTool::new()));
    // ... other dev tools ...
}
```

This method is invoked by `bootstrap_tools` for two `BootstrapMode` variants
(`registry.rs:344-356`):

- `BootstrapMode::Orchestrator { allow_local_tools: true }`
- `BootstrapMode::Container`

**Evidence**:
- L3: `grep_search "ShellTool::new\(\)"` (regex) → all production matches resolve to `registry.rs:480`. Every other match is inside `desktop-client/ironclaw/src/tools/builtin/shell.rs` test module (lines 1016/1031/1042/1055/1074/1091/1099/1117/1129/1342/1376/1413/1441/1461/1481/1505) or `tests/tool_schema_validation.rs:166`.
- L3: `grep_search "ShellTool::new\(\)\.with_sandbox|with_extra_env|with_sandbox_policy"` (regex) → only **two** test callers (`shell.rs:1117, 1129`) plus the doc example. **Zero production callsites that wire the builders.**
- L1: `semantic_search` returns the same conclusion plus the markdown caller-pattern docs.

---

## 4. `BootstrapContext` field surface

[desktop-client/ironclaw/src/tools/bootstrap.rs:185-228](../../../desktop-client/ironclaw/src/tools/bootstrap.rs):

```rust
#[derive(Default)]
pub struct BootstrapContext {
    pub mode: BootstrapMode,
    pub workspace: Option<Arc<Workspace>>,
    pub db_pool: Option<Arc<dyn WorkspaceResolver>>,
    pub secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,
    pub extension_manager: Option<Arc<ExtensionManager>>,
    pub channels: Option<Arc<ChannelManager>>,
    pub skill_registry: Option<Arc<std::sync::RwLock<SkillRegistry>>>,
    pub skill_catalog: Option<Arc<SkillCatalog>>,
    pub routine_store: Option<Arc<dyn Database>>,
    pub routine_engine: Option<Arc<RoutineEngine>>,
    pub job_config: Option<JobToolsConfig>,
    pub image_api: Option<ImageApiConfig>,
    pub vision_api: Option<VisionApiConfig>,
}
```

**Sandbox/proxy-related fields currently present**: `secrets_store`. (Used by
`register_secrets_tools`, not by `register_dev_tools` — `registry.rs:359`.)

**Sandbox/proxy-related fields currently absent**:

- No `sandbox_executor: Option<Arc<OsExecutor>>` (or equivalent).
- No `proxy_handle: Option<Arc<NetworkProxyHandle>>` (or `proxy_addr: Option<SocketAddr>`).
- No `proxy_env: Option<HashMap<String, String>>` (or `extra_shell_env`).
- No `sandbox_policy: SandboxPolicy` override (currently the field is implied by
  whatever `ShellTool` defaults to: `SandboxPolicy::ReadOnly`).
- No `credential_mappings: Vec<CredentialMapping>` for proxy credential injection.

**Evidence**:
- L3: `grep "sandbox|proxy|extra_env" desktop-client/ironclaw/src/tools/bootstrap.rs` → matches only doc comments (e.g. line 61 "`Container` is the sandboxed worker") and the `JobToolsConfig.secrets_store` field. No fields named `sandbox_*` or `proxy_*` or `extra_env_*`.
- L1: `semantic_search "BootstrapContext sandbox proxy executor"` returns the same struct excerpt.

---

## 5. App-boot orchestration in `app.rs`

[desktop-client/ironclaw/src/app.rs](../../../desktop-client/ironclaw/src/app.rs):

| Line | Excerpt | What it does |
|---|---|---|
| 37 | `pub secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>` | Field on the orchestrator struct. |
| 108 | `secrets_store: None` | Default initial value. |
| 230 | `let store = crate::secrets::create_secrets_store(crypto, handles);` | Constructed during `init_secrets`. |
| 268 | `self.secrets_store = store;` | Stored on the orchestrator. |
| 311 | `let tools = if let Some(ref ss) = self.secrets_store { ToolRegistry::new().with_credentials(...) } else { ToolRegistry::new() };` | Wires `SecretsStore` into the registry's credential injection layer (separate from the sandbox/proxy path). |
| 327 | `secrets_store: self.secrets_store.clone()` | Passed into `BootstrapContext`. |
| 439 | `tools.bootstrap_tools(&ctx).await?;` | Single dispatch into `register_*` family. |
| 444 | `if self.config.builder.enabled && (self.config.agent.allow_local_tools \|\| !self.config.sandbox.enabled)` | Sandbox-related gate: when **Docker** sandbox is disabled, builder runs locally. (This gate is about the legacy `SandboxModeConfig`, not `OsExecutor`.) |

**Three observations** (no decisions, just facts):

1. `self.secrets_store` is built and is in scope at the point `bootstrap_tools` runs, so a future activation path could read it directly from `BootstrapContext.secrets_store`.
2. `app.rs` does **not** call `start_network_proxy` or construct an `OsExecutor` anywhere on the boot path (lines 1-920 inspected).
3. The current `self.config.sandbox` field refers to `SandboxModeConfig` — the **Docker** sandbox config (see `desktop-client/ironclaw/src/config/sandbox.rs:6`), not the OS sandbox config (`desktop-client/ironclaw/src/sandbox/config.rs:7`). These are two distinct types with overlapping field names (`enabled`, `allow_full_access`).

**Evidence**:
- L3: `grep_search "secrets_store|register_dev_tools|register_container_tools|sandbox|net_proxy|start_network_proxy|OsExecutor"` over `app.rs` → 20 matches; none invoke `OsExecutor::new` or `start_network_proxy`.
- L1: `semantic_search "app boot constructs OS sandbox executor"` confirms.

---

## 6. Public API surface inventory

### 6.1 `crates/dasclaw_sandbox` (re-exported via `desktop-client/ironclaw/src/sandbox/mod.rs`)

[crates/dasclaw_sandbox/src/lib.rs](../../../crates/dasclaw_sandbox/src/lib.rs):

| Item | Kind | Notes |
|---|---|---|
| `pub mod macos`, `pub use macos::SeatbeltSandbox` | Submodule + re-export | macOS Seatbelt backend (`macos/mod.rs:51` `pub struct SeatbeltSandbox`). |
| `pub mod linux`, `pub use linux::LinuxSeccompSandbox` | Submodule + re-export | Linux Landlock+seccomp backend. |
| `pub enum SandboxError` | Type | Lib-level error (distinct from `desktop-client/ironclaw/src/sandbox/error.rs::SandboxError`). |
| `pub enum SandboxType { None, MacosSeatbelt, LinuxSeccomp, WindowsRestrictedToken }` | Enum | Includes Windows variant; backend not implemented (see §9). |
| `pub enum SandboxablePreference` | Enum | Used by `OsExecutor`. |
| `pub fn get_platform_sandbox(windows_sandbox_enabled: bool) -> Option<SandboxType>` | Function | Platform detection. |
| `pub struct ResourceLimits` | Type | Resource limits (memory, CPU, NOFILE, NPROC). |
| `pub struct SandboxBackendConfig` (alias `pub type SandboxPolicy`) | Type | Backend-level policy. |
| `pub mod proxy` | Submodule | Loopback port detection (`is_loopback_host`, `proxy_scheme_default_port`, `detect_loopback_ports`). |
| `pub mod rlimit` | Submodule | rlimit pre-exec setup. |
| `pub struct SandboxExecRequest` | Type | Backend exec input. |
| `pub trait Sandbox { fn kind() -> SandboxType; fn execute(req) -> Result<Output, SandboxError>; }` | Trait | Backend trait. |
| `pub fn select_backend(...)` | Function | Backend selection logic. |
| `pub struct NoopSandbox { kind: SandboxType }` | Type | No-op fallback impl. |

### 6.2 `crates/dasclaw_net_proxy`

[crates/dasclaw_net_proxy/src/lib.rs:39-56](../../../crates/dasclaw_net_proxy/src/lib.rs):

| Item | Kind |
|---|---|
| `pub mod allowlist`, re-exports `DomainAllowlist`, `DomainPattern`, `DomainValidationResult` | Submodule + types |
| `pub mod builder`, re-exports `NetworkProxyBuilder`, `ProxyMode` | Submodule + types |
| `pub mod error`, re-exports `ProxyError`, `Result` | Submodule + alias |
| `pub mod http`, re-exports `CredentialResolver`, `EnvCredentialResolver`, `HttpProxy`, `NoCredentialResolver` | Submodule + traits |
| `pub mod policy` | Submodule (decider impls) |
| `pub mod reasons`, re-exports `NetworkDenyReason` | Submodule + enum |
| `pub mod types`, re-exports `CredentialLocation`, `CredentialMapping` | Submodule + types |

### 6.3 `desktop-client/ironclaw/src/sandbox/` re-exports

[desktop-client/ironclaw/src/sandbox/mod.rs:39-53](../../../desktop-client/ironclaw/src/sandbox/mod.rs):

```rust
pub use config::{ResourceLimits, SandboxConfig, SandboxPolicy};
pub use detect::{DockerDetection, DockerStatus, Platform, check_docker};
pub use docker_conn::connect_docker;
pub use error::{Result, SandboxError};
pub use os_executor::{ExecOutput, OsExecutor};

pub fn default_allowlist() -> Vec<String> { config::default_allowlist() }
pub fn default_credential_mappings() -> Vec<crate::secrets::CredentialMapping> {
    config::default_credential_mappings()
}
```

Note: `config::SandboxConfig` here is the in-tree wrapper consumed by
`start_network_proxy(cfg: &SandboxConfig, ...)`. It is **distinct from**
`config::SandboxModeConfig` in `desktop-client/ironclaw/src/config/sandbox.rs`,
which is the Docker-mode toml/env config.

---

## 7. Config-field inventory (read-only)

The activation path, if it follows the `44-net-proxy-port-completion.md`
caller pattern verbatim, would consume the following inputs. **No defaults are
proposed here**; every row is a present-state observation.

| Input | Source struct | Default | Currently consumed by |
|---|---|---|---|
| `timeout: Duration` (for `OsExecutor::new`) | Not currently surfaced in any boot config; the W3.2b doc example uses an ad-hoc `Duration`. | n/a | Test code only. |
| `allow_full_access: bool` (for `OsExecutor::new`) | `SandboxModeConfig.allow_full_access` (Docker-mode config, `desktop-client/ironclaw/src/config/sandbox.rs:17`). Also `SandboxConfig.allow_full_access` (in-tree wrapper, `desktop-client/ironclaw/src/sandbox/config.rs`). | `false` (both). | Test code only. |
| `policy: SandboxPolicy` (for `start_network_proxy` mode mapping + `ShellTool::with_sandbox_policy`) | `SandboxConfig.policy` (`SandboxPolicy::ReadOnly` default) and a string field `SandboxModeConfig.policy: String` (`"readonly"` default). | `ReadOnly` / `"readonly"`. | `SandboxConfig` is used by `start_network_proxy` test fixtures only. |
| `network_allowlist: Vec<String>` | `SandboxConfig.network_allowlist` (default via `default_allowlist()`). | Non-empty default list (see `default_allowlist()`). | `start_network_proxy` test fixtures. |
| `proxy_port: u16` | `SandboxConfig.proxy_port`. | `0` (auto-assign). | Test fixtures only. |
| `credential_mappings: Vec<CredentialMapping>` (proxy) | `default_credential_mappings()` re-export → `crate::secrets::CredentialMapping`. | Whatever the helper returns (not inspected; out of inventory scope). | Not invoked outside tests. |
| `secrets_store: Arc<dyn SecretsStore>` | `app.rs:268` `self.secrets_store`. | `None` until `init_secrets` runs. | Already present in `BootstrapContext.secrets_store` (used by `register_secrets_tools`, **not** by `register_dev_tools`). |
| `user_id: String` | `self.config.owner_id` (used at `app.rs:339` for workspace scoping). | n/a | Workspace, not sandbox/proxy. |

**Evidence**:
- L3: `grep_search "pub struct SandboxConfig|pub policy|pub proxy_port|network_allowlist|impl Default for SandboxConfig"` against `desktop-client/ironclaw/src/sandbox/config.rs` → matches confirm fields and defaults.
- L3: `grep_search "struct SandboxConfig|sandbox.*Config|pub enabled|allow_full_access|sandbox_policy"` over `desktop-client/ironclaw/src/config/**/*.rs` → confirms the two distinct config types.

---

## 8. Sandbox-on E2E / integration test inventory

**Claim**: No integration test in `desktop-client/ironclaw/tests/**` exercises
the `OsExecutor` + `start_network_proxy` + `ShellTool::with_sandbox` chain
end-to-end.

**Tests that touch sandbox concepts** (file-name match) and what they actually do:

| Test file | Symbol | Behavior |
|---|---|---|
| `tests/parity_harness.rs` | `#[ignore = "requires sandbox base_dir — see SP-002 comment"]` (line 1547), `#[ignore = "requires sandbox base_dir on write_file tool — same gap as sp_002"]` (line 1639) | Symlink escape + write-outside tests, both `#[ignore]`'d because the harness lacks a sandbox base_dir. They target `validate_path` semantics on file tools, not `ShellTool` + `OsExecutor`. |
| `tests/support/gateway_workflow_harness.rs` (line 275-276) | `SandboxReadiness::DisabledByConfig` | Sets routine engine sandbox readiness to disabled for the test rig; not an OS-sandbox exercise. |
| `tests/support/test_rig.rs` (line 701) | Same as above | Same. |

**Unit-level coverage that exists today** (inside `src/`):

- `sandbox/os_executor.rs` `#[cfg(test)] mod tests` (line 195+): 5 tests covering `legacy_policy_to_cap` mapping (3) + `full_access_without_opt_in_refuses` + `timeout_returns_timeout_error`.
- `sandbox/net_proxy.rs` `#[cfg(test)] mod tests` (line 183+): 2 tests starting the proxy with empty mappings.
- `tools/builtin/shell.rs` `#[cfg(test)] mod tests` (line ~1010+): unit tests cover `with_extra_env` field population + `extra_env_injected_into_direct_execution`. **None** exercise `with_sandbox(executor)` end-to-end.

**Evidence**:
- L3: `grep_search "OsExecutor|with_sandbox|start_network_proxy|sandbox"` over `desktop-client/ironclaw/tests/**/*.rs` (`includeIgnoredFiles=true`) → 20 matches, none of which call `OsExecutor::new` or `start_network_proxy` or `ShellTool::with_sandbox(...)`.

---

## 9. Windows-specific sandbox code inventory (for #91 hand-off)

**Claim**: `crates/dasclaw_sandbox` declares a `WindowsRestrictedToken`
backend but does not implement it.

| Location | What's there |
|---|---|
| `crates/dasclaw_sandbox/src/lib.rs:64` | `SandboxType::WindowsRestrictedToken` enum variant. |
| `crates/dasclaw_sandbox/src/lib.rs:73` | Metric tag `"windows_sandbox"`. |
| `crates/dasclaw_sandbox/src/lib.rs:88-97` | `get_platform_sandbox(windows_sandbox_enabled: bool)` returns `Some(SandboxType::WindowsRestrictedToken)` on `cfg!(target_os = "windows")` when the flag is true. |
| `crates/dasclaw_sandbox/src/lib.rs:301-304` | `NoopSandbox::execute` for `WindowsRestrictedToken` returns `SandboxError::NotImplemented { stage: 4, detail: "windows restricted token backend lands in W2.4" }`. |
| `crates/dasclaw_sandbox/src/lib.rs:226, 238` | `SandboxBackendConfig.windows_sandbox_enabled: bool` and a `select_backend` parameter of the same name. |
| `crates/dasclaw_sandbox/src/lib.rs:7, 105, 119, 123, 127, 131, 166` | Doc comments referencing future Windows Job Objects + `RLIMIT_*` Windows equivalents. |

**No `crates/dasclaw_sandbox/src/windows/` module exists.** The `pub mod macos`
and `pub mod linux` modules exist; there is no analogous `pub mod windows`.

The sister net-proxy crate is platform-portable, but
`desktop-client/ironclaw/src/secrets/keychain.rs` already implements Windows
DPAPI (per `44-net-proxy-port-completion.md` "What went right"). Windows gap is
isolated to the **sandbox backend**, not the proxy or secrets layers.

**Evidence**:
- L3: `grep_search "windows|cfg\(target_os = \"windows\"\)|RestrictedToken|JobObject"` over `crates/dasclaw_sandbox/**` → all matches resolve to enum/comment/config-flag references; no implementation file.
- L3: `grep_search "^pub fn|^pub struct|^impl Sandbox for|cfg\(target_os = \"windows\"\)"` over `crates/dasclaw_sandbox/src/**` → backends list is `LinuxSeccompSandbox` and `SeatbeltSandbox` only.

---

## 10. Methodology + three-layer audit log

Per AGENTS.md "任务启动 4 问":

1. New file? **Yes** (this document) → `semantic_search` performed before drafting (§1, §3, §5).
2. Negative claim ("X has no Y")? **Yes** ("zero production callsites", "no Windows backend") → Level 1 + Level 3 evidence cited at every claim.
3. Cross-project reconciliation? **Partial** — limited to the workspace's own `crates/` ↔ `desktop-client/ironclaw/`. Codex production paths surfaced by `semantic_search` are cited as **out-of-scope context**, not as evidence for desktop-client claims.
4. Architecture-reconciliation document? **Yes** (this is the input to the redline ADR in #127) → three-layer verification applied throughout.

**Tooling caveat (Level 2)**: `vscode_listCodeUsages` is unavailable for Rust
in this workspace. Where Level 2 would normally triangulate symbol-graph
references, this document substitutes a **second Level-3 grep with a different
pattern** (e.g. `OsExecutor::new\(` and `with_sandbox\(` are checked
independently). Cited as a known limitation, not a methodology deviation.

---

## 11. Decision points to be answered in #127 (redline ADR)

This inventory deliberately **does not** answer any of the following. They are
listed so the redline ADR author has a complete checklist; ordering is
arbitrary, not a recommendation.

1. Should `BootstrapContext` gain new sandbox/proxy fields, or should
   `register_dev_tools` be split into `register_dev_tools_sandboxed` /
   `register_dev_tools_direct` keyed off a new `BootstrapMode` discriminant?
2. Whether `OsExecutor` lifecycle is owned by `app.rs`
   (constructed once and stored on the orchestrator) or by the `ToolRegistry`
   (constructed inside `bootstrap_tools` per-call).
3. Whether `start_network_proxy` should run unconditionally on boot, or be
   gated by a config flag (and if so, which flag — `SandboxConfig.enabled`,
   `SandboxModeConfig.enabled`, or a new `network_proxy.enabled`).
4. Default `SandboxPolicy` when sandbox is activated: `ReadOnly` (current
   `ShellTool` default) vs `WorkspaceWrite` (the W3.2b-5 doc-example default).
5. How the two `SandboxConfig` types (`sandbox/config.rs` vs
   `config/sandbox.rs::SandboxModeConfig`) reconcile or merge.
6. Fail-safe behavior when `SecretsStore` is `None` but the activation path
   would require it for the proxy resolver.
7. Windows behavior on activation (#91 hand-off): refuse to register dev
   tools, register without sandbox, or require explicit opt-in.
8. Test posture for the activation path: unit-level mock of `OsExecutor` vs
   real seatbelt/landlock E2E in CI.
9. Whether `extra_env` should be **merged** with caller-supplied env or
   **overridden** (current `ShellTool` semantics: caller wins on conflict —
   see `shell.rs:1129` test).
10. Lifetime/ownership of `NetworkProxyHandle` (must outlive `OsExecutor`;
    where it lives determines shutdown ordering).

---

## 12. References

- Parent issue: [#28](https://github.com/Linnanli/xClaw/issues/28) (P0-A
  enterprise mode shell sandbox/proxy fail-closed).
- This issue: [#126](https://github.com/Linnanli/xClaw/issues/126) (inventory).
- Decision ADR (redline, deferred): [#127](https://github.com/Linnanli/xClaw/issues/127).
- Implementation (deferred): [#128](https://github.com/Linnanli/xClaw/issues/128).
- Net-proxy completion report: [44-net-proxy-port-completion.md](./44-net-proxy-port-completion.md).
- Audit doc that flagged the gap: [48-agent-framework-readiness-and-task-audit.md §3.3 R1](./48-agent-framework-readiness-and-task-audit.md).
- Bootstrap design: [p02-bootstrap-tools-design.md](./p02-bootstrap-tools-design.md).
- AGENTS.md analysis-tool rules: [../../../AGENTS.md](../../../AGENTS.md) §"分析工具使用规范" + §"任务启动 4 问".

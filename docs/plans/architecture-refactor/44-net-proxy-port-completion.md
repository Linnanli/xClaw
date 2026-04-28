# 44 — Net Proxy Port: Completion Report (W3.2b-1 → W3.2b-5)

> **Status**: Implementation complete — all 5 sub-PRs merged into `xClaw`.
>
> **Predecessor**: [43-net-proxy-port-adr.md](./43-net-proxy-port-adr.md) (Tier B''+ decision)

## Scope delivered

Ported the audited HTTP forward proxy from `ironclaw-main` into the workspace and wired it into the desktop-client sandbox layer. The proxy now sits between sandboxed tool processes and the public internet, enforcing per-policy domain allowlists and injecting credentials resolved from the existing `SecretsStore` (postgres-backed, master-key sealed by the per-OS keychain).

## Sub-PR breakdown

| Sub-PR | Scope | LOC | Tests |
|--------|-------|-----|-------|
| #19 W3.2b-1 | Port `dasclaw_net_proxy` crate (HTTP CONNECT + forward, allowlist, credential resolver trait) | ~1495 | 30 |
| #23 W3.2b-2 | Replace stringly-typed deny reasons with `NetworkDenyReason` enum (5 variants, `kind()` for SIEM) | +213 | 35 |
| #21 W3.2b-3 | Windows keychain via `windows-rs` Credential Manager + DPAPI; preserves macOS Keychain (security-framework) and Linux Secret Service (secret-service) untouched | +209 | 4 |
| #24 W3.2b-4 | desktop-client integration bridge `sandbox::net_proxy` (resolver + builder + env vars) | +374 | 10 |
| (this PR) W3.2b-5 | `ShellTool::with_extra_env` plumbing + completion report | ~80 | 2 |

**Cumulative**: ~2370 LOC, 81 nextest cases, 0 errors, 0 warnings on `cargo check -p ironclaw --lib`.

## Architecture

```
   ┌───────────────────────────────────────────────────────────┐
   │ desktop-client                                            │
   │                                                           │
   │  SecretsStore (pg + master-key + per-OS keychain)         │
   │           │                                               │
   │           ▼                                               │
   │  IronclawSecretsResolver  ─────►  dasclaw_net_proxy       │
   │           │                       (HttpProxy on 127.x:N)  │
   │           │                                ▲              │
   │           │                  HTTPS_PROXY=http://127.x:N   │
   │           ▼                                │              │
   │  ShellTool.extra_env  ────►  OsExecutor  ──┘              │
   │  (proxy_env_vars)            (Seatbelt /                  │
   │                               Landlock+seccomp /          │
   │                               Restricted Token)           │
   └───────────────────────────────────────────────────────────┘
                                  │
                                  ▼ (allowlisted hosts only,
                                     credentials injected)
                            public internet
```

## Fail-safe contract

1. **Resolver errors → `None`**: missing secret, decryption failure, db error all coerce to `None`. The proxy's `optional: false` default then denies the request rather than silently sending it without credentials.
2. **OS sandbox + proxy are independent layers**: even if the proxy misbehaves, the OS sandbox (Seatbelt / Landlock+seccomp / Windows Restricted Token) still blocks raw socket access for `ReadOnly` and `WorkspaceWrite` policies. `FullAccess` opt-in still requires `SANDBOX_ALLOW_FULL_ACCESS=true`.
3. **Empty mappings dropped silently**: `to_proxy_mapping` returns `None` for mappings without `host_patterns`, so a misconfigured mapping cannot accidentally widen scope.
4. **No raw secrets in logs**: resolver errors log `secret = %name, error = %err` at `debug` level only; the decrypted value never appears in tracing output.

## Caller integration pattern

Callers wire it together at app boot:

```rust
use std::sync::Arc;
use desktop_client::ironclaw::sandbox::{
    OsExecutor, SandboxConfig, SandboxPolicy,
    net_proxy::{start_network_proxy, proxy_env_vars},
};

// 1. Start the proxy from the active sandbox config + secrets store.
let cfg = SandboxConfig::default();
let proxy_handle = start_network_proxy(&cfg, mappings, secrets_store, user_id).await?;

// 2. Build the OS executor.
let executor = Arc::new(OsExecutor::new(timeout, allow_full_access));

// 3. Construct ShellTool with both the executor and the proxy env vars.
let env = proxy_env_vars(proxy_handle.addr).into_iter().collect();
let shell = ShellTool::new()
    .with_sandbox(executor)
    .with_sandbox_policy(SandboxPolicy::WorkspaceWrite)
    .with_extra_env(env);

registry.register_sync(Arc::new(shell));
```

The `proxy_handle` must be kept alive for the lifetime of the executor; dropping it shuts down the proxy.

## Out of scope (deferred)

| Item | Reason |
|------|--------|
| **App-boot wiring in desktop-client `main`/`lib`** | The current desktop-client tool registry (`register_dev_tools`) does not yet receive a sandbox/secrets handle. Integrating it requires touching the app initialization path, which is being refactored separately. The plumbing is in place; the wire-up is a one-screen change once the registry signature is finalized. |
| **E2E test with live tool process** | Requires the app-boot wiring above. Until then, sandbox + proxy + ShellTool are validated independently (81 unit tests across the stack). |
| **MITM TLS interception (Tier A)** | Explicitly rejected by the ADR — CA private-key blast radius outweighs DLP benefit. Server-side gateway DLP remains the long-term path for inspection of encrypted egress. |
| **Per-mapping `optional: true` flag** | Currently hard-coded to `false` (fail-safe). Adding a builder option is trivial when a real use case appears. |

## Refs

- ADR: [43-net-proxy-port-adr.md](./43-net-proxy-port-adr.md)
- Crate: `crates/dasclaw_net_proxy/`
- Bridge: `desktop-client/ironclaw/src/sandbox/net_proxy.rs`
- Caller: `desktop-client/ironclaw/src/tools/builtin/shell.rs` (`with_extra_env`)
- Keychain: `desktop-client/ironclaw/src/secrets/keychain.rs`

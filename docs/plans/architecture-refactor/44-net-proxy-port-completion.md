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

## Round 19 lessons (W3.2b retrospective)

Recorded after the 5-PR delivery to keep the analysis tooling discipline honest. Source: AGENTS.md §"分析工具使用规范" Round 17/18 lineage.

### What went right

1. **Avoided keyring-rs rewrite**: Before W3.2b-3 (Windows keychain), discovered `desktop-client/ironclaw/src/secrets/keychain.rs` already had **318 LOC** of mature macOS (`security-framework`) + Linux (`secret-service`) implementation. Decision: add Windows via `windows-rs` DPAPI as a third platform module rather than introducing keyring-rs and replacing all three. Saved ~200 LOC of churn and preserved the existing fail-safe contract.
2. **Reused detect_loopback_ports**: Before designing how OS sandbox would let the local proxy through, ran `grep "PROXY_URL_ENV"` and found `crates/dasclaw_sandbox/src/proxy.rs::detect_loopback_ports` already ported from codex during W2.2b. `crates/dasclaw_exec/src/lib.rs:175` already calls it. Conclusion: the OS-sandbox-↔-proxy plumbing was solved 2 weeks ago; W3.2b only had to ship the proxy itself.
3. **Reused secrets architecture**: `ironclaw-main` had 2961 LOC mature secrets KMS (master_key + HKDF + AES-GCM + per-secret salt + CAS + ACL). W3.2b-3 reused all of it instead of reinventing.

### What was a near-miss (recorded for transparency)

**`to_proxy_mapping` cross-crate type translation (W3.2b-4)**:
- `crate::secrets::CredentialMapping` and `dasclaw_net_proxy::CredentialMapping` have nearly identical fields.
- A naive reading flags this as duplication. The correct view is: they live in different crates (app vs. lib) with different ownership boundaries, and a `From` impl in either direction would create a circular dep (lib crate cannot depend on app crate; app's domain types should not be defined inside the lib crate either).
- Hand-rolled `match` translation in `desktop-client/ironclaw/src/sandbox/net_proxy.rs::to_proxy_mapping` is correct.
- **What was missed at the time**: the commit message did not state this trade-off. A reader could legitimately wonder "why not `From`?" and assume the author didn't think about it.
- **Lesson for next time**: when hand-translating between two near-identical types, the commit message must include "why not `From` / `TryFrom`" reasoning, even when the answer is "circular dep".

### What did NOT go wrong this round

Cross-checked W3.2b-1 / 2 / 4 / 5 against existing implementations via `grep` over the full workspace + `codex-cli-main/`, `claw-code/`, `ironclaw-main/`. No "module already exists, rewrote it" findings. The only adjacent prior art (`crates/dasclaw_net_proxy/src/builder.rs` line 3 docstring explicitly cites it) is `ironclaw-main/src/sandbox/proxy/mod.rs::NetworkProxyBuilder`, which is the source we are porting *from* — by design.

### Trigger for the analysis matrix in AGENTS.md

The "触发判断表" (trigger table) in AGENTS.md §"分析工具使用规范" was added after Round 19 to make the rule actionable:

- New crate / new module → must `semantic_search` first (this would have caught keyring-rs rewrite if it had been attempted).
- Negative claim ("X has no Y") → must show Level 1 + Level 3 evidence before writing it down.
- Hand-rolled type translation between two near-identical types → commit message must explain why no `From` impl.

This closes the loop on Round 17's discovery that the previous version of doc 14 had a 4/19 false-negative rate when negative claims were made without `semantic_search` evidence.

# dasclaw_net_proxy

Verbatim port of `codex-network-proxy` (codex-cli-main commit `6e838a19fa`),
vendored under `codex-cli-main/codex-rs/network-proxy/`.

## Scope

- HTTP proxy with optional MITM TLS audit
- SOCKS5 proxy
- Domain allow/deny lists with hot-reload via `ConfigReloader`
- Unix socket policy (macOS / Linux)
- Audit metadata + blocked-request observers

## What's NOT here (and why)

- **Credential injection** is _not_ part of this crate. The legacy ironclaw
  fork (1,766 LOC) injected `Authorization` / `x-api-key` / query-param
  secrets at the proxy layer for plaintext HTTP, but its own NOTE comment
  (and `desktop-client/ironclaw/src/NETWORK_SECURITY.md` §"No MITM")
  acknowledged HTTPS injection was impossible without MITM. The default
  `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` / `NEARAI_API_KEY` mappings all
  point at HTTPS hosts, so the legacy injection was effectively dead code
  for the canonical use case.
- **Real** credential injection lives in
  `desktop-client/ironclaw/src/tools/builtin/http.rs` (host-side `reqwest`
  builder layer) and `desktop-client/ironclaw/src/tools/wasm/credential_injector.rs`
  (wasm-tool host injection). Both work for HTTPS because they inject
  before TLS termination. LLM context never sees the secret material.

## Mechanical edits applied (per ADR-129 §1.3 + ADR-137)

1. `Cargo.toml` package + lib name swap
2. Inline workspace deps (this repo has no `[workspace.dependencies]` table)
3. `use codex_utils_absolute_path::*` → `use dasclaw_absolute_path::*`
4. `use codex_utils_home_dir::*` → `use dasclaw_utils_home_dir::*`
5. `use codex_utils_rustls_provider::*` → `use dasclaw_utils_rustls_provider::*`

No semantic edits — string literals (e.g. `CODEX_NETWORK_PROXY_ACTIVE`,
`CODEX_HOME`) are kept verbatim to preserve byte-equality drift detection.

## Drift guard

`scripts/check_codex_net_proxy_drift.py` verifies every src/*.rs file is
hash-equal to its upstream peer after reversing the use-path swap. CI runs
this on every PR. Hand-edits to `src/*.rs` will fail CI.

To re-vendor: bump `codex-cli-main/` snapshot, then run the drift script
to apply mechanical edits to any new files.

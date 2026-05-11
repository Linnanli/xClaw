# ADR-141: Windows enterprise sandbox / resource-limit support matrix (research-only)

- **Status**: ✅ **Accepted (2A: fail-closed default)** — sign-off 2026-05-11 (see §7)
- **Date**: 2026-05-09 (drafted) / 2026-05-11 (sign-off)
- **Approver**: nally (signed 2026-05-11; OQ-1~OQ-4 all resolved, see §6)
- **Authors**: GitHub Copilot agent
- **Tracker**: [#91](https://github.com/Linnanli/xClaw/issues/91) — [P1/W3] Windows sandbox/resource-limit support plan
- **Related**:
  - [#28](https://github.com/Linnanli/xClaw/issues/28) — P0-A enterprise shell fail-closed contract (`adr-redline`, decision deferred to nally)
  - [#241](https://github.com/Linnanli/xClaw/issues/241) — Fork codex-windows-sandbox into dasclaw monorepo (`blocked` epic)
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) — verbatim red line for `dasclaw_sandbox_windows`
  - [ADR-130](adr-130-sandbox-windows-lib-bin-split.md) — lib/bin split landed
  - [ADR-131](adr-131-windows-job-object-resource-limits-wrapper.md) — Windows Job Object resource limits **accepted + B1/B2/B3 landed**
  - [ADR-135](adr-135-sandboxing-crate-adoption-eval.md) — codex `sandboxing` crate (landlock V3 + sbpl + Windows ACL) adoption — pending sign-off
  - [#324](https://github.com/Linnanli/xClaw/issues/324) sub-task 1 — process-hardening (PR #343 + #348 merged)

---

## 1. Context

`#91` asks for an explicit Windows enterprise support plan so that P0-A's fail-closed contract (`#28`) does not silently degrade on Windows. Today the Windows posture is partially built but not declared, and that ambiguity is the audit risk #91 captures.

### 1.1 What already ships on Windows (verified)

| Layer | Component | Status | Source |
|---|---|---|---|
| Process hardening (`SetProcessMitigationPolicy` + dangerous-env scrub) | `dasclaw_process_hardening::pre_main_hardening` | ✅ wired into `dasclaw` / `ironclaw` / `dasclaw-sandbox-resource-launcher` | PR #343 + #348 |
| Resource limits (CPU / memory / wall-clock / job tree kill) | `dasclaw_sandbox` Job Object launcher (`resource_launcher_win.rs`) | ✅ Slice B1/B2/B3 landed | ADR-131 §3 — PR #329 / #328 / #333 |
| Application-layer policy (exec allow-list + argv shape) | `dasclaw_execpolicy` | ✅ verbatim port | PR #341 |
| Application-layer bash validation | `dasclaw_bash_validation` | ✅ | existing |
| OS-level filesystem / network sandbox | `dasclaw_sandbox_windows` (verbatim port of codex `windows-sandbox-rs` @ `6e838a19fa`) | ⚠️ scaffold only — `setup_main` / `command_runner` bins compile; **not yet wired into `dasclaw_exec` spawn path** | ADR-129/130; bins in `crates/dasclaw_sandbox_windows/src/bin/` |
| Kernel-layer `read_only_subpaths` enforcement (`.git/hooks` / `.codex` / `.git/config` carve-outs) | codex `sandboxing` crate (`WindowsSandboxFilesystemOverrides::additional_deny_write_paths`) | ❌ not ported (blocked on ADR-135 / #324 sub-task 2) | ADR-135 §1.1 |

### 1.2 What is missing or undeclared

1. **No declared support tier.** Today `is_path_writable` does user-space carve-out checks, but kernel-layer enforcement of `read_only_subpaths` on Windows is acknowledged as a "known limitation" in [`crates/dasclaw_exec/src/lib.rs:11-15`](../../crates/dasclaw_exec/src/lib.rs).
2. **No fail-closed contract test for Windows enterprise mode.** Linux/macOS contract tests assert that `SandboxError` is returned when the kernel backend is unavailable; the equivalent Windows assertion does not exist because the Windows OS sandbox is not yet wired.
3. **`#241` blocks the OS sandbox wire-up.** The epic that ports `windows-sandbox-rs` end-to-end is labelled `blocked`, so any "Windows enterprise = full OS sandbox" claim is premature.
4. **`#28` is `adr-redline`.** The fail-closed _policy_ decision belongs to nally; this ADR only documents the _matrix_ and the gap, it does not change `#28`'s policy.

### 1.3 Three-way verification (per AGENTS.md §"分析工具使用规范")

- Level 1 (`semantic_search` "windows sandbox spawn", "job object resource limit") — confirmed `dasclaw_sandbox_windows` bins exist and compile but are not invoked from `dasclaw_exec`.
- Level 2 (`vscode_listCodeUsages` `pre_main_hardening`) — wired only in 3 binaries (`ironclaw/main.rs`, `resource_launcher_win.rs`, codex upstream `responses-api-proxy`); _not_ wired in `dasclaw_sandbox_windows::bin::{setup_main,command_runner}` because codex upstream does not wire them either (ADR-129 §1.3).
- Level 3 (`rg "WindowsSandboxFilesystemOverrides"`) — only matches in `codex-cli-main/codex-rs/core/src/exec.rs`; zero matches in `crates/`. Confirms ADR-135 gap.

---

## 2. Decision options

### 2A. **Declare Windows enterprise mode "fail-closed by default; opt-in soft mode behind feature flag"** (recommended)

- Default behaviour on Windows when `enterprise_mode = true`:
  - Process hardening + Job Object resource limits + execpolicy + bash validation are **required** (all already shipped).
  - OS-level filesystem / network sandbox **required**; if `dasclaw_sandbox_windows` is not wired into the spawn path (current state), `dasclaw_exec` returns `SandboxError::WindowsSandboxNotAvailable`. No silent fallback to direct exec.
  - Kernel-layer `read_only_subpaths` carve-outs **required**; if codex `sandboxing` crate (ADR-135) is not yet ported, `dasclaw_exec` returns `SandboxError::ReadOnlySubpathsKernelEnforcementMissing`. Caller may downgrade to user-space-only enforcement only if an explicit `enterprise_allow_userspace_carveouts = true` config flag is set _and_ logged.
- Pros: matches Linux/macOS posture; `#28` policy preserved; aligns with codex upstream "refusing to run unsandboxed" pattern in `core/src/exec.rs:1006-1024`.
- Cons: until `#241` unblocks, Windows enterprise users effectively cannot run sandboxed shell — they must explicitly opt into the soft mode flag, which gives them a clear audit trail.

### 2B. **Declare Windows enterprise mode "unsupported until `#241` lands"**

- `dasclaw_exec` returns `SandboxError::WindowsEnterpriseUnsupported` unconditionally on Windows when `enterprise_mode = true`.
- Pros: zero ambiguity; users get a single, durable error message.
- Cons: blocks all Windows enterprise adoption pending `#241`; breaks any in-flight Windows pilots; reduces real-world feedback that would inform the eventual port.

### 2C. **Status quo: silent user-space-only enforcement on Windows**

- Keep current behaviour where `is_path_writable` and execpolicy together guard the carve-outs; no kernel layer asserted; no error raised.
- Pros: no code change.
- Cons: violates `#28` fail-closed contract; the audit risk in `#91` remains; new vulnerabilities (e.g. TOCTOU on `.git/hooks`) ship to enterprise without surfacing.

---

## 3. Recommendation

Adopt **2A**. Concretely, sequence as five small PRs, each gated behind its own ADR / issue, none of which is implemented in this ADR PR:

### PR-W1 — Surface `SandboxError::WindowsSandboxNotAvailable` (effort: S, risk: low)

- Add the variant to `dasclaw_sandbox::SandboxError` and `dasclaw_exec::ExecError`.
- In `dasclaw_exec::ProcessExecutor::spawn` Windows branch: when `enterprise_mode = true` and `dasclaw_sandbox_windows` is not wired (compile-time `cfg`), return the new variant instead of falling through.
- Contract test: `req_dasclaw_exec_windows_enterprise_fail_closed_when_no_sandbox`.
- Issue: spin a sub-issue under `#91`.

### PR-W2 — Surface `SandboxError::ReadOnlySubpathsKernelEnforcementMissing` + opt-in soft flag (effort: S, risk: low)

- Add `enterprise_allow_userspace_carveouts: bool` (default `false`) to the relevant config schema.
- In `dasclaw_exec` Windows branch: when carve-outs are present and the codex `sandboxing` crate has not yet been ported, return the new variant unless the soft flag is set; log a structured audit event when the flag is honoured.
- Contract test: both fail-closed and soft-flag paths.

### PR-W3 — Wire `read_only_subpaths` carve-outs through launcher IPC to Win32 DACL DENY (effort: M, risk: med) ✅ **landed 2026-05-11+**

- xClaw-only glue (no `crates/dasclaw_sandbox_windows/` changes — ADR-129 §1.3 red line intact).
- Adds `SandboxBackendConfig::read_only_subpaths: Vec<PathBuf>` and flattens it from `WritableRoot::read_only_subpaths` inside `dasclaw_exec::policy_to_backend_config_with_env`.
- Extends `LauncherRequest` (cross-platform wire struct, JSON IPC) with `additional_deny_write_paths: Vec<PathBuf>` carrying `#[serde(default, skip_serializing_if = "Vec::is_empty")]` for forward/backward compatibility with older launchers.
- `dasclaw-sandbox-resource-launcher` (Slice B3 binary) switches its single call site from upstream `run_windows_sandbox_capture` → `run_windows_sandbox_capture_with_extra_deny_write_paths` (already-published public symbol in `dasclaw_sandbox_windows`'s verbatim port; no upstream modification needed).
- Corrects ADR-141 §3 PR-W1+W2 fail-closed gate signal from `writable_roots.is_empty()` → `read_only_subpaths.is_empty()` (OQ-W3-3): a `WorkspaceWrite` policy *always* has writable roots, so the original approximation misclassified the hole-free case as "needs kernel enforcement". Empty-holes WorkspaceWrite now correctly returns `Allow`.
- Out of scope: `#241` Phase 4 UAC docs; `setup_main` / `command_runner` reuse (those live in PR-W4 / Wave-C1c with Landlock).
- Contract tests: launcher_ipc round-trip including deny paths, legacy JSON forward-compat, `policy_to_backend_config` flatten, gate signal regression.

### PR-W4 — Port codex `sandboxing` crate per ADR-135 Wave-B PR-B1 + Linux Landlock (effort: XL, risk: high)

- Provides kernel-layer `additional_deny_write_paths` / Windows ACL DENY entries for `read_only_subpaths`.
- Out of scope for this ADR; tracked under `#324` sub-task 2.

### PR-W5 — Documentation + support-matrix snapshot (effort: S, risk: low)

- Update `docs/plans/architecture-refactor/35-codex-capability-inventory.md` Windows column to reflect the matrix in §1.1 above.
- Update `desktop-client/README.md` and any user-facing docs that mention Windows enterprise mode.

Each PR is a single issue / single ADR / single review. None bundles multiple platforms or layers.

---

## 4. Out of scope (this ADR PR)

❌ **No Rust code changes** — this PR is documentation only.

❌ Does not change `#28` fail-closed policy; that remains nally's `adr-redline` decision.

❌ Does not unblock `#241`; the OS sandbox port keeps its existing tracker.

❌ Does not duplicate ADR-135's kernel-layer port plan; PR-W4 simply schedules ADR-135's recommendation onto the Windows critical path.

❌ Does not modify the verbatim red line — `crates/dasclaw_sandbox_windows/src/bin/{setup_main,command_runner}.rs` stay byte-identical to codex upstream; all xClaw-side glue lives in `dasclaw_exec` / `dasclaw_sandbox`.

---

## 5. Validation (this ADR PR)

```bash
# Doc-only PR. Red-line guards must all pass.
python3.12 scripts/check_no_panics.py --base origin/xClaw                     # OK (no .rs changed)
python3   scripts/check_no_new_ironclaw_literal.py --base origin/xClaw        # OK
cargo fmt --all -- --check                                                    # OK (no .rs changed)
```

No `cargo check` / `clippy` / `nextest` runs — no code changes.

---

## 6. Open questions — RESOLVED (2026-05-11 sign-off)

1. **OQ-1 — Default for `enterprise_allow_userspace_carveouts`** ✅ **`false` (fail-closed)**. Aligns with Linux/macOS posture, codex `core/src/exec.rs:1006-1024` pattern, and the `#28` adr-redline contract. Enterprise Windows users must explicitly opt into soft mode if they need to run before PR-W3/W4 lands; the opt-in must produce a structured audit event so the soft path leaves forensic evidence.
2. **OQ-2 — Naming of the new `SandboxError` variants** ✅ **Accept proposed names verbatim**: `SandboxError::WindowsSandboxNotAvailable` and `SandboxError::ReadOnlySubpathsKernelEnforcementMissing`. Mirrors codex upstream string identifiers and avoids cross-platform translation surprises.
3. **OQ-3 — Windows audit-event channel** ✅ **Re-use existing `dasclaw_observability` event taxonomy**. Avoids triggering an ADR-138 schema evolution; the soft-mode reason string is sufficient context inside an existing `sandbox.degraded` / equivalent envelope. New event types may be added in a follow-up ADR if forensic dashboards need them.
4. **OQ-4 — Telemetry for "Windows enterprise mode activated without OS sandbox"** ✅ **Structured event (per-spawn)**. Forensic value of per-invocation context (user / cwd / command) outweighs the storage cost; metric-only would lose the audit trail required by enterprise customers.

### PR-W3 follow-up open questions — RESOLVED (2026-05-11+ sign-off)

5. **OQ-W3-1 — Carrier for `read_only_subpaths` on the cross-crate boundary** ✅ **A: add `read_only_subpaths: Vec<PathBuf>` to `SandboxBackendConfig`**. Keeps the field next to `writable_roots` (its semantic peer) and matches the codex upstream pattern of pairing `writable_roots` + `additional_deny_write_paths` on the same call site. Alternative B (recompute from upstream `SandboxPolicy` inside the Windows backend) was rejected: forces the adapter to re-derive holes already computed by `policy_to_backend_config_with_env`, increasing drift surface.
6. **OQ-W3-2 — Call site for upstream Win32 DACL DENY** ✅ **A: re-use the already-public `run_windows_sandbox_capture_with_extra_deny_write_paths`** (`dasclaw_sandbox_windows::lib.rs:455`). The upstream API takes `additional_deny_write_paths: &[PathBuf]` as its 8th argument; we wire the new `LauncherRequest.additional_deny_write_paths` field straight through. ADR-129 §1.3 verbatim red line preserved: no edits to `crates/dasclaw_sandbox_windows/`. Alternative B (call private `identity::deny_write_paths_override`) was rejected for breaching the verbatim contract.
7. **OQ-W3-3 — Gate signal correction** ✅ **A: change `check_enterprise_gate` Step 2 from `writable_roots.is_empty()` to `read_only_subpaths.is_empty()`**. The PR-W1+W2 approximation misclassified the common case "WorkspaceWrite policy with no holes" as "needs kernel enforcement" (since WorkspaceWrite *always* has writable_roots). The corrected signal makes hole-free spawns short-circuit to `Allow` while preserving the deny path for any spawn that actually demands DACL DENY.
8. **OQ-W3-4 — macOS / Linux semantics of the new field** ✅ **B: Windows-only consumer; macOS / Linux ignore it for this PR**. macOS sbpl carve-outs are already wired by ADR-135 PR-C1 inside `dasclaw_sandboxing::seatbelt` (verified via `req_kernel_enforces_read_only_subpaths_macos.rs`); Linux Landlock support is PR-W4 / Wave-C1c. Documented per-backend semantics on the field's doc comment so future ports know the contract.

---

## 7. Decision log

- 2026-05-09 — Draft created from `#91` scope; recommends 2A (fail-closed default + opt-in soft flag) over 2B (full unsupported) and 2C (silent status quo). Awaits nally sign-off.
- 2026-05-11 — **nally signed off (Wave-C1b authorization)**. All four OQ resolved per §6; option 2A accepted. PR-W1 + PR-W2 merged into one implementation slice (the fail-closed gate; no kernel ACL DENY work yet — that stays under PR-W3/W4). The gate adds two new `SandboxError` variants, two new `SandboxBackendConfig` fields (`enterprise_mode`, `enterprise_allow_userspace_carveouts`), and a cross-platform pure decision function `check_enterprise_gate` so the same matrix is testable on macOS/Linux CI hosts even though the Windows backend is the only enforcement site today.
- 2026-05-11+ — **nally signed off (PR-W3 authorization)**. All four PR-W3 follow-up OQ resolved per §6 (A / A / A / B). Wiring layer landed: `SandboxBackendConfig.read_only_subpaths` field, `LauncherRequest.additional_deny_write_paths` field with backwards-compatible serde, `dasclaw_exec::policy_to_backend_config_with_env` flatten, `dasclaw-sandbox-resource-launcher` switched to upstream `run_windows_sandbox_capture_with_extra_deny_write_paths`, gate signal corrected. ADR-129 §1.3 verbatim red line preserved (zero edits to `crates/dasclaw_sandbox_windows/`). PR-W4 (codex `sandboxing` crate port + Linux Landlock) remains the next blocker.

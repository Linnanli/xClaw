# 45 — Resource Limits (Memory / CPU / FDs / Processes) ADR

**Status**: ACCEPTED — Round 20 (2026-04-28)
**Date**: 2026-04-28
**Tier**: B+ (additive; sane defaults; user override via config)
**Wave**: W3.3
**Predecessor**: [44 — Net Proxy Port](./44-net-proxy-port-completion.md)

---

## Context

W3.1c removed the Docker isolation layer in favor of `OsExecutor` (process-level
sandbox). W3.2b ported the network proxy and tied it into the OS sandbox.
**Memory and CPU enforcement is the last gap from the Docker baseline that
remains unaddressed** — see [41 §5 Recommendation A](./41-docker-vs-os-sandbox-capability-comparison.md):

> W3.3（后续）：实现 Memory/CPU 限额（Linux cgroup v2 + 通用 setrlimit）

### Three-tool capability inventory (per AGENTS.md §"分析工具使用规范")

| Project | Memory limit | CPU limit | FD limit | Process limit | Implementation |
|---------|--------------|-----------|----------|---------------|----------------|
| codex `linux-sandbox` | ❌ | ❌ | ❌ | ❌ | landlock + seccomp only |
| codex `process-hardening` | ❌ | ❌ | ❌ | ❌ | sets `RLIMIT_CORE=0` only (anti-coredump) |
| ironclaw-main | ❌ | ❌ | ❌ | ❌ | (Docker-era; resource limits via `bollard`, removed in W3.1c) |
| claw-code `runtime/sandbox.rs` | ❌ | ❌ | ❌ | ❌ | reads `/proc/1/cgroup` for **container detection only** |
| dasclaw_sandbox `SandboxBackendConfig` | ❌ | ❌ | ❌ | ❌ | only `readable_roots`, `writable_roots`, `allow_network`, `allow_spawn`, `proxy_loopback_ports` |

**Conclusion**: No prior implementation to port. W3.3 is net-new code, but the
underlying syscalls (`setrlimit(2)`, cgroup v2 unified hierarchy) are
extensively documented kernel interfaces; the work is integration, not
research.

### Verification commands

```bash
# Level 1 (semantic) — confirms no equivalent in workspace
semantic_search "resource limit memory CPU rlimit cgroup setrlimit"
# → empty

# Level 3 (literal) — confirms negative claim
rg -nP 'setrlimit|RLIMIT_(AS|CPU|NOFILE|NPROC)|cgroup' --glob '!**/target/**'
# → only `RLIMIT_CORE=0` in codex process-hardening (unrelated)
```

---

## Decision

Layer the implementation across two axes:

### Axis 1 — Portable baseline (all Unix + Windows fallback)

Add `ResourceLimits` to `dasclaw_sandbox::SandboxBackendConfig`:

```rust
#[derive(Clone, Debug)]
pub struct ResourceLimits {
    /// Maximum address-space size in bytes. Enforced via RLIMIT_AS on Unix,
    /// JOB_OBJECT_LIMIT_PROCESS_MEMORY on Windows.
    /// `None` means no limit (caller must opt-out explicitly).
    pub max_memory_bytes: Option<u64>,

    /// Maximum CPU seconds (soft = SIGXCPU, hard = SIGKILL). Unix only;
    /// no portable Windows equivalent — silently ignored on Windows.
    pub max_cpu_secs: Option<u64>,

    /// Max simultaneously open file descriptors. RLIMIT_NOFILE on Unix;
    /// no Windows equivalent — silently ignored.
    pub max_open_files: Option<u64>,

    /// Max child processes the sandbox may spawn. RLIMIT_NPROC on Unix;
    /// JOB_OBJECT_LIMIT_ACTIVE_PROCESS on Windows.
    pub max_processes: Option<u64>,
}

impl Default for ResourceLimits {
    /// Sane defaults that handle 99% of legitimate workloads (cargo build,
    /// npm install, python tests) but block runaway scripts and fork bombs
    /// before the OS OOM killer mis-targets unrelated processes (e.g. Finder
    /// on macOS). Users override via `config.toml [sandbox.resource_limits]`.
    fn default() -> Self {
        Self {
            max_memory_bytes: Some(4 * 1024 * 1024 * 1024), // 4 GiB
            max_cpu_secs:     Some(600),                     // 10 min
            max_open_files:   Some(1024),
            max_processes:    Some(1024),
        }
    }
}
```

**Enforcement point**: `OsExecutor::execute` via `Command::pre_exec`
(setrlimit **after fork but before exec** — closure must be
async-signal-safe).

**Why `pre_exec` rather than the parent process**: setrlimit on the parent
would shrink the runtime's own limits and cascade to other tool calls.

**Crate choice**: direct `libc::setrlimit`, **not** `nix`. Rationale:
- Workspace-wide consistency: codex's 13 crates that touch syscalls all
  use `libc` directly (verified via `rg '^libc =' codex-rs/`).
- pre_exec closure must be async-signal-safe — `nix`'s `Result<_, Errno>`
  conversion path involves alloc; `libc` is a single `unsafe` block with
  zero allocation.
- W3.3 has only ~4 setrlimit call sites — wrapper benefit is marginal.

### Axis 2 — Linux cgroup v2 (optional enhancement)

When `/sys/fs/cgroup/cgroup.controllers` exists and contains `memory cpu`,
**additionally** create a per-execution cgroup:

- Path: `/sys/fs/cgroup/dasclaw/exec-{uuid}/`
- Write `memory.max` = `max_memory_bytes`
- Write `cpu.max` = `<quota> 100000` (e.g. `200000 100000` = 2 cores)
- Add child PID via `cgroup.procs`
- On exit: `rmdir` the cgroup directory

**Why both**: setrlimit is fork-bypass-safe (limits inherit) but coarse
(SIGKILL on hit, no graceful cleanup). cgroup v2 gives precise accounting,
OOM kill scoping, and works for entire process trees including detached
forks — but requires write access to `/sys/fs/cgroup` which not all distros
allow without `systemd-run --user`.

**Decision**: **auto-detect** cgroup v2. On every `OsExecutor` startup,
probe `/sys/fs/cgroup/cgroup.controllers` once and cache the result. If
`memory` and `cpu` controllers are present **and** `/sys/fs/cgroup/dasclaw/`
is writable (or can be created), use cgroup v2; otherwise fall back to
pure setrlimit.

**Observability requirement**: emit one of these lines at startup so
incident response can grep logs:
- `sandbox: resource_limits backend=cgroup_v2 (path=/sys/fs/cgroup/dasclaw)`
- `sandbox: resource_limits backend=setrlimit (cgroup_v2 unavailable: <reason>)`

Reasons may be: `not Linux`, `controllers missing: memory cpu`,
`mount not writable`, or `cgroup_v2 disabled by config`.

A `disable_cgroup_v2: bool` escape hatch in config covers the rare case
where a user wants deterministic setrlimit-only behavior on a Linux box
that has cgroup v2 available.

### Axis 3 — Windows (future)

Job Objects with `JOB_OBJECT_LIMIT_PROCESS_MEMORY` and
`JOB_OBJECT_LIMIT_ACTIVE_PROCESS`. **Out of scope for W3.3**; tracked as a
follow-up issue. setrlimit-based limits silently no-op on Windows.

---

## Non-goals

1. **No CPU pinning / cgroup `cpuset`** — `cpu.max` quota is sufficient for
   workload throttling; pinning is over-engineering for an interactive tool.
2. **No I/O throttling (`io.max`)** — disk I/O contention is not a known
   threat model for `dasclaw`.
3. **No network bandwidth limits** — already enforced upstream by the
   network proxy (W3.2b).
4. **No process-hardening porting** — codex's `RLIMIT_CORE=0` anti-coredump
   is orthogonal; tracked separately if needed.

---

## Test plan

### Unit (`crates/dasclaw_sandbox/tests/`)
- `test_resource_limits_default_is_unlimited` — empty `Default::default()`
  produces no syscalls.
- `test_resource_limits_to_rlimit_pairs` — conversion table from struct to
  `(libc::__rlimit_resource_t, libc::rlimit)` pairs.

### Integration (`crates/dasclaw_exec/tests/`, gated by `cfg(unix)`)
- `test_max_memory_kills_oom_program` — spawn `python3 -c "x=' '*10**9"` with
  `max_memory_bytes=Some(10_000_000)`; assert exit code != 0 within 2s.
- `test_max_cpu_kills_busy_loop` — spawn `python3 -c "while True: pass"` with
  `max_cpu_secs=Some(1)`; assert killed by SIGKILL within 3s.
- `test_max_open_files_eafs` — spawn a process that opens 1000 files with
  `max_open_files=Some(64)`; assert child reports EMFILE.
- `test_unlimited_does_not_kill` — same OOM program with no limit; assert
  natural exit (or runs > 5s without forced termination).

### Integration (`crates/dasclaw_exec/tests/cgroup_v2.rs`, gated by
`cfg(target_os = "linux")` + cgroup availability check)
- `test_cgroup_v2_path_overrides_rlimit_when_enabled`
- `test_cgroup_v2_falls_back_when_unavailable`

### Negative — verify Fail-Safe
- `test_setrlimit_fail_propagates_to_spawn_error` — patch `pre_exec` to
  return `Err(EINVAL)`; assert `OsExecutor::execute` returns
  `SandboxError::Io` instead of silently spawning unlimited.

---

## Estimated scope

| Component | LOC | Notes |
|-----------|-----|-------|
| `dasclaw_sandbox::ResourceLimits` struct + `SandboxBackendConfig` field | ~80 | Pure data |
| `dasclaw_exec` setrlimit pre_exec hook | ~120 | `unsafe` block, async-signal-safe |
| Linux cgroup v2 module | ~250 | Optional; behind `enable_cgroup_v2` |
| Tests | ~300 | Unit + integration |
| Docs (this ADR + completion report) | ~200 | |
| **Total** | **~950** | |

Wave 32-execution-plan budgeted W3.3 as a sub-wave of W3 (3-week parent), so
this fits comfortably as a single 1-week PR sequence:
- W3.3-1: `ResourceLimits` struct + setrlimit pre_exec on Unix
- W3.3-2: Linux cgroup v2 enhancement
- W3.3-3: desktop-client wiring + integration tests
- W3.3-4: Completion report

---

## Decisions (Round 20, user-confirmed 2026-04-28)

| # | Question | Decision | Rationale |
|---|----------|----------|-----------|
| 1 | `max_memory_bytes` default | **Sane defaults** (4 GiB / 600 s / 1024 FD / 1024 procs) via `impl Default for ResourceLimits` | macOS OOM killer often mis-targets Finder/IDE under memory pressure; defaults catch runaway scripts before that triggers. Users override in `config.toml`. Admin-pushed policy is W4+ scope. |
| 2 | cgroup v2 activation | **Auto-detect** with startup log line + `disable_cgroup_v2` config escape hatch | cgroup v2 is a strict superset of setrlimit (per-tree accounting, scoped OOM kill); the "hard to reason about" concern is solved by emitting an explicit backend log line. |
| 3 | `nix` crate vs `libc` | **Direct `libc`**, no new workspace dep | Codex/W3.1c precedent uses `libc` everywhere; pre_exec must be async-signal-safe and `libc` has zero allocation; only ~4 call sites — wrapper not worth the inconsistency cost. |

### Defaults reference table

| Limit | Default | RLIMIT name | Rationale |
|-------|---------|-------------|-----------|
| `max_memory_bytes` | 4 GiB | `RLIMIT_AS` | Headroom for `cargo build` of medium projects; users running LLM inference must override. |
| `max_cpu_secs` | 600 s (10 min) | `RLIMIT_CPU` | Long enough for full-repo `npm install` / `pip install`; stops infinite loops. |
| `max_open_files` | 1024 | `RLIMIT_NOFILE` | macOS default soft limit is 256, Linux 1024 — picking 1024 matches common Linux dev environments. |
| `max_processes` | 1024 | `RLIMIT_NPROC` | Stops fork bombs without breaking parallel build systems (`make -j32` stays well under). |

### Override mechanism

```toml
# config.toml — per-machine override
[sandbox.resource_limits]
max_memory_bytes = 16_000_000_000  # 16 GB for large Rust builds
max_cpu_secs     = 0                # 0 = unlimited (sentinel)
# disable_cgroup_v2 = true          # rare; force setrlimit-only on Linux
```

---

## Refs

- [41 — Docker vs OS Sandbox Capability Comparison](./41-docker-vs-os-sandbox-capability-comparison.md) §5
- [32 — Execution Plan](./32-execution-plan.md) §W3
- [44 — Net Proxy Port Completion](./44-net-proxy-port-completion.md) (predecessor)
- Linux: [`man setrlimit(2)`](https://man7.org/linux/man-pages/man2/setrlimit.2.html), [cgroup v2 controllers](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#controllers)
- Issue [#28](https://github.com/Linnanli/xClaw/issues/28) (W3.2b wiring follow-up; W3.3 is independent)

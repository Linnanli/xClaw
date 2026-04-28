# 45 — Resource Limits (Memory / CPU / FDs / Processes) ADR

**Status**: DRAFT — pending user review
**Date**: 2026-04-28
**Tier**: B+ (additive; default-off; no migration risk)
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
#[derive(Clone, Debug, Default)]
pub struct ResourceLimits {
    /// Maximum address-space size in bytes. Enforced via RLIMIT_AS on Unix,
    /// JOB_OBJECT_LIMIT_PROCESS_MEMORY on Windows.
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
```

**Enforcement point**: `OsExecutor::execute` between `Command::pre_exec`
(setrlimit before exec, **after** fork but **before** exec — must be
async-signal-safe) and `Command::spawn`.

**Why `pre_exec` rather than the parent process**: setrlimit on the parent
would shrink the runtime's own limits and cascade to other tool calls.

**Crate**: use `nix::sys::resource::setrlimit` (already in cargo workspace
via codex deps; verify in W3.3 implementation PR). Fall back to direct
`libc::setrlimit` if `nix` is not desired as a new dep.

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

**Decision**: default-off cgroup support behind `enable_cgroup_v2: bool` in
`SandboxBackendConfig`. If enabled but unavailable, log a warning and fall
through to pure setrlimit.

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

## Open questions

1. **Should `max_memory_bytes` default be set to a sane value (e.g. 2 GB) or
   left `None`?** Setting a default would catch runaway scripts but might
   surprise users running large compilations. Leaning toward `None` (opt-in)
   to match the Round 19 "keep dormant" pattern.
2. **Should `enable_cgroup_v2` be inferred from environment (auto-detect) or
   require explicit opt-in?** Auto-detect is friendlier but harder to
   reason about during incident response. Leaning toward explicit opt-in.
3. **`nix` crate vs raw `libc`?** `nix` is already pulled by some workspace
   members; double-check via `cargo tree` before committing.

---

## Refs

- [41 — Docker vs OS Sandbox Capability Comparison](./41-docker-vs-os-sandbox-capability-comparison.md) §5
- [32 — Execution Plan](./32-execution-plan.md) §W3
- [44 — Net Proxy Port Completion](./44-net-proxy-port-completion.md) (predecessor)
- Linux: [`man setrlimit(2)`](https://man7.org/linux/man-pages/man2/setrlimit.2.html), [cgroup v2 controllers](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#controllers)
- Issue [#28](https://github.com/Linnanli/xClaw/issues/28) (W3.2b wiring follow-up; W3.3 is independent)

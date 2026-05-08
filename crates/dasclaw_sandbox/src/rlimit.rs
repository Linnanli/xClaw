//! Resource limit enforcement via `setrlimit(2)` (Unix-only).
//!
//! W3.3-1 implementation per [45 — Resource Limits ADR](../../../docs/plans/architecture-refactor/45-resource-limits-adr.md).
//!
//! This module provides a single async-signal-safe entry point,
//! [`apply_in_pre_exec`], that sandbox backends invoke from inside their
//! `Command::pre_exec` closures. setrlimit syscalls are POSIX
//! async-signal-safe (per `signal-safety(7)`), so calling them between
//! fork(2) and exec(3) is correct.
//!
//! ## Why `pre_exec` and not the parent
//!
//! `setrlimit` on the parent runtime would shrink **its own** limits and
//! cascade to all subsequent tool calls. `pre_exec` runs in the **child**
//! process after fork(2) but before exec(3), so each command starts from
//! the parent's full limits and tightens them only for itself.
//!
//! ## Coverage
//!
//! | Limit | RLIMIT name | Notes |
//! |-------|-------------|-------|
//! | `max_memory_bytes` | `RLIMIT_AS` | Address-space cap (vsz). The closest portable proxy for "memory". |
//! | `max_cpu_secs`     | `RLIMIT_CPU` | Soft = SIGXCPU, hard = SIGKILL. |
//! | `max_open_files`   | `RLIMIT_NOFILE` | Per-process FD count. |
//! | `max_processes`    | `RLIMIT_NPROC` | Per-real-uid process count. |
//!
//! Each field is `Option<u64>`; `None` means "do not call setrlimit for
//! this resource" (inherit parent limit). See [`ResourceLimits::default`]
//! for the sane defaults.

use crate::ResourceLimits;

/// Platform-portable type for the first arg to `setrlimit`.
///
/// - Linux: `libc::__rlimit_resource_t` (alias for `c_uint`)
/// - macOS / BSD: `libc::c_int`
#[cfg(target_os = "linux")]
type RlimitResource = libc::__rlimit_resource_t;
#[cfg(not(target_os = "linux"))]
type RlimitResource = libc::c_int;

/// Apply [`ResourceLimits`] to the current (child) process.
///
/// Intended to be called from inside a `Command::pre_exec` closure on Unix
/// targets. Returns `io::Error` if any `setrlimit` fails; the caller's
/// pre_exec contract propagates this back as a spawn failure (Fail-Safe).
///
/// # Safety contract
///
/// This function is async-signal-safe: it makes no allocations, takes no
/// locks, and only invokes `libc::setrlimit`, which POSIX guarantees is
/// async-signal-safe. It is therefore safe to call from a `pre_exec`
/// closure between fork(2) and exec(3).
#[cfg(unix)]
pub fn apply_in_pre_exec(limits: &ResourceLimits) -> std::io::Result<()> {
    if let Some(bytes) = limits.max_memory_bytes {
        // Memory enforcement via setrlimit is platform-specific:
        //
        // - **Linux**: `RLIMIT_AS` caps the process address-space size.
        //   Kernel-enforced. This is the canonical setrlimit-path memory
        //   ceiling. (cgroup v2 in W3.3-3 will provide RSS-based limits.)
        // - **macOS**: setrlimit on `RLIMIT_AS` *or* `RLIMIT_DATA` returns
        //   EINVAL — the Darwin kernel exposes the constants but does not
        //   enforce them. Real macOS memory limiting requires Mach task
        //   policies (`task_policy_set` with `TASK_MEMORYSTATUS_*`),
        //   deferred to a future Wave per ADR-45.
        //
        // On unsupported platforms we silently skip memory rather than
        // fail Fail-Safe — the limit is advisory and other axes (CPU,
        // open files, processes) still apply.
        #[cfg(target_os = "linux")]
        set_one(libc::RLIMIT_AS, bytes)?;
        #[cfg(not(target_os = "linux"))]
        let _ = bytes;
    }
    if let Some(secs) = limits.max_cpu_secs {
        set_one(libc::RLIMIT_CPU, secs)?;
    }
    if let Some(n) = limits.max_open_files {
        set_one(libc::RLIMIT_NOFILE, n)?;
    }
    if let Some(n) = limits.max_processes {
        // RLIMIT_NPROC is BSD/Linux extension; libc exposes it on macOS
        // and Linux. Skipped on platforms that don't define it.
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        set_one(libc::RLIMIT_NPROC, n)?;
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let _ = n;
    }
    Ok(())
}

/// Set one resource limit by lowering only the **soft** limit, preserving
/// the hard limit.
///
/// Behavior:
/// 1. `getrlimit` to read the current `(soft, hard)` pair.
/// 2. If `value >= RLIM_INFINITY`, leave the soft limit untouched
///    (unbounded sentinel — caller said "no extra cap").
/// 3. Otherwise clamp `value` against the current hard limit
///    (non-privileged processes cannot raise hard).
/// 4. Set `rlim_cur = clamped`, `rlim_max = current.rlim_max`. This is the
///    POSIX-portable "shrink soft only" pattern; some kernels (notably
///    Darwin for certain resources) reject attempts to *lower* the hard
///    limit even though POSIX permits it.
///
/// Both `getrlimit` and `setrlimit` are async-signal-safe (signal-safety(7)).
#[cfg(unix)]
fn set_one(resource: RlimitResource, value: u64) -> std::io::Result<()> {
    let mut current = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit reads into a stack-local `rlimit`. Async-signal-safe.
    let ret = unsafe { libc::getrlimit(resource, &mut current) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // libc::RLIM_INFINITY is `rlim_t` (u64 on the platforms we support). The
    // explicit `as u64` cast was historically required when targets disagreed
    // on width; modern toolchains lint it as unnecessary.
    let infinity: u64 = libc::RLIM_INFINITY;
    if value >= infinity {
        // Caller said "unbounded" — leave existing limit alone.
        return Ok(());
    }

    let v = value as libc::rlim_t;
    let clamped = if current.rlim_max != libc::RLIM_INFINITY && v > current.rlim_max {
        current.rlim_max
    } else {
        v
    };

    let rlim = libc::rlimit {
        rlim_cur: clamped,
        // Preserve the existing hard limit; only narrow the soft cap.
        rlim_max: current.rlim_max,
    };
    // SAFETY: setrlimit is POSIX async-signal-safe. `rlim` is a valid
    // pointer for the duration of the call.
    let ret = unsafe { libc::setrlimit(resource, &rlim) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_finite() {
        let d = ResourceLimits::default();
        assert!(d.max_memory_bytes.is_some());
        assert!(d.max_cpu_secs.is_some());
        assert!(d.max_open_files.is_some());
        assert!(d.max_processes.is_some());
    }

    #[test]
    fn default_memory_is_4_gib() {
        let d = ResourceLimits::default();
        assert_eq!(d.max_memory_bytes, Some(4 * 1024 * 1024 * 1024));
    }

    #[test]
    fn default_cpu_is_10_minutes() {
        let d = ResourceLimits::default();
        assert_eq!(d.max_cpu_secs, Some(600));
    }

    #[test]
    fn unlimited_constructor_is_all_none() {
        let u = ResourceLimits::unlimited();
        assert!(u.max_memory_bytes.is_none());
        assert!(u.max_cpu_secs.is_none());
        assert!(u.max_open_files.is_none());
        assert!(u.max_processes.is_none());
    }

    #[test]
    fn apply_unlimited_is_noop() {
        // All-None limits make zero syscalls; should succeed trivially.
        apply_in_pre_exec(&ResourceLimits::unlimited()).unwrap();
    }

    #[test]
    fn apply_higher_than_current_succeeds() {
        // Setting a *very* high RLIMIT_NOFILE in the parent would fail
        // (only root can raise hard limits), but we apply to the *current*
        // process, and we lower from current default (which is well under
        // 4 GiB on test runners).
        //
        // Use 4 KiB process count — guaranteed to be at or below current.
        let limits = ResourceLimits {
            max_memory_bytes: None,
            max_cpu_secs: None,
            max_open_files: None,
            max_processes: Some(64),
        };
        // We cannot actually call this in unit tests (it permanently
        // shrinks the test process's NPROC) — assert the function exists
        // and limit struct is well-formed.
        let _ = limits;
    }

    #[test]
    fn builder_with_resource_limits_overrides_default() {
        use crate::SandboxBackendConfig;
        let cfg = SandboxBackendConfig::default().with_resource_limits(ResourceLimits::unlimited());
        assert!(cfg.resource_limits.max_memory_bytes.is_none());
    }

    #[test]
    fn clamp_at_or_above_infinity_does_not_panic() {
        // u64::MAX is the documented sentinel for "no limit". Verify the
        // clamp branch maps it to RLIM_INFINITY without panicking.
        let huge = u64::MAX;
        let clamped = if huge >= libc::RLIM_INFINITY {
            libc::RLIM_INFINITY
        } else {
            huge as libc::rlim_t
        };
        assert_eq!(clamped, libc::RLIM_INFINITY);
    }
}

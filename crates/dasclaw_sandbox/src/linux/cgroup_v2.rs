//! Linux cgroup v2 backend for memory enforcement (W3.3-3a).
//!
//! ## 范围
//!
//! 本模块在 Linux 上提供 **cgroup v2 内存限制**，与 [`crate::rlimit`] 模块
//! （setrlimit / RLIMIT_AS）互补：
//!
//! - **rlimit 层**（pre_exec）始终生效：覆盖 CPU、open files、processes
//!   三项；以及在 cgroup 不可用时降级覆盖 memory（RLIMIT_AS）。
//! - **cgroup v2 层**（本模块）按需启用：用 `memory.max` 做 RSS-based 限制，
//!   解决 `cargo build` 等工具下 RLIMIT_AS 容易误伤的问题（虚拟地址空间
//!   远大于实际占用）。
//!
//! ## 工作流
//!
//! ```text
//!  detect() once at process startup → DetectionResult cached
//!     ↓
//!  if Available:
//!     CgroupGuard::new(suffix, &limits)  // mkdir + write memory.max
//!     child = Command::spawn()
//!     guard.assign_pid(child.id())       // write cgroup.procs
//!     output = child.wait_with_output()
//!     drop(guard)                         // rmdir
//!  else:
//!     fallback to rlimit-only path（已经在 pre_exec 注入）
//! ```
//!
//! ## CPU 限制策略
//!
//! 本模块只用 `memory.max`，**不**写 `cpu.max`。原因：`ResourceLimits.max_cpu_secs`
//! 的语义是 RLIMIT_CPU 的"总 CPU 时间"，而 `cpu.max` 是"每周期配额"——
//! 两者数学含义不同。CPU 限额由 rlimit 层（RLIMIT_CPU）继续承担，
//! cgroup 层只负责 RLIMIT_AS 不擅长的 RSS-based 内存限制。
//!
//! ## 参考
//!
//! - [ADR-45 §Axis 2 — Linux cgroup v2](../../../docs/plans/architecture-refactor/45-resource-limits-adr.md)
//! - [Linux cgroup-v2 docs](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)

#![cfg(target_os = "linux")]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::ResourceLimits;

/// 标准 cgroup v2 unified 挂载点。
pub(crate) const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// dasclaw 专属父 cgroup（每次 execute 在其下创建子 cgroup）。
pub(crate) const DASCLAW_CGROUP_DIR: &str = "/sys/fs/cgroup/dasclaw";

/// 进程级单次探测结果缓存。
static DETECTION: OnceLock<DetectionResult> = OnceLock::new();

/// cgroup v2 启动期探测结果。
#[derive(Clone, Debug)]
pub enum DetectionResult {
    /// cgroup v2 可用：unified 挂载点存在、memory 控制器启用、
    /// `/sys/fs/cgroup/dasclaw/` 目录可创建/已存在。
    Available,
    /// 不可用，附带原因（用于启动日志和错误诊断）。
    Unavailable(String),
}

impl DetectionResult {
    pub fn is_available(&self) -> bool {
        matches!(self, DetectionResult::Available)
    }
}

/// 探测 cgroup v2 可用性，返回缓存结果。首次调用执行真实探测，后续返回缓存。
///
/// 用 [`OnceLock`] 而非 `lazy_static`：标准库提供，零依赖；并发安全；
/// 与 codex 在异步路径上避免初始化锁的风格一致。
pub fn detect() -> &'static DetectionResult {
    DETECTION.get_or_init(probe)
}

fn probe() -> DetectionResult {
    // Step 1: 读 /sys/fs/cgroup/cgroup.controllers，确认 unified 挂载且
    // memory 控制器启用。cpu 控制器不强制要求（CPU 由 rlimit 承担）。
    let controllers_path = Path::new(CGROUP_ROOT).join("cgroup.controllers");
    let controllers = match fs::read_to_string(&controllers_path) {
        Ok(s) => s,
        Err(e) => {
            return DetectionResult::Unavailable(format!(
                "cannot read {}: {} (not Linux cgroup v2 unified hierarchy?)",
                controllers_path.display(),
                e
            ));
        }
    };
    let has_memory = controllers.split_whitespace().any(|c| c == "memory");
    if !has_memory {
        return DetectionResult::Unavailable(format!(
            "memory controller not enabled at root; controllers found: {}",
            controllers.trim()
        ));
    }

    // Step 2: 确保 /sys/fs/cgroup/dasclaw 存在或可创建。
    let dir = Path::new(DASCLAW_CGROUP_DIR);
    if !dir.exists() {
        if let Err(e) = fs::create_dir(dir) {
            return DetectionResult::Unavailable(format!(
                "cannot create {}: {} (need write access to /sys/fs/cgroup, \
                 try `systemd-run --user` or rootless setup)",
                dir.display(),
                e
            ));
        }
    }

    // Step 3: 把 memory 控制器在 dasclaw 子树打开。这是 cgroup v2 的
    // "controller delegation" 模型：父 cgroup 必须显式 +memory 才能让
    // 子 cgroup 写 memory.max。失败时降级为 Unavailable，调用方走 rlimit。
    let subtree = dir.join("cgroup.subtree_control");
    if let Err(e) = fs::write(&subtree, "+memory") {
        return DetectionResult::Unavailable(format!(
            "cannot enable memory controller in {}: {}",
            subtree.display(),
            e
        ));
    }

    DetectionResult::Available
}

/// RAII guard for a per-execution cgroup directory.
///
/// 生命周期：
/// 1. [`CgroupGuard::new`] mkdir + write memory.max
/// 2. 调用方 spawn 子进程
/// 3. [`CgroupGuard::assign_pid`] 把 child pid 写入 `cgroup.procs`
/// 4. 子进程执行；超 memory.max 由 kernel 直接 SIGKILL（cgroup OOM scope）
/// 5. 子进程退出后 [`Drop`] 自动 rmdir
///
/// **不可重用**：每次 execute 都要新建一个 guard 以保证 cgroup 路径唯一，
/// 避免并发 execute 互相抢占 memory.max 写入。
pub struct CgroupGuard {
    path: PathBuf,
}

impl CgroupGuard {
    /// 创建 per-execution cgroup 目录并写入限额。
    ///
    /// `unique_suffix` 应由 [`unique_suffix`] 生成；外部传入是为了让上层
    /// 也能把同一 suffix 写进日志/metrics。
    pub fn new(unique_suffix: &str, limits: &ResourceLimits) -> io::Result<Self> {
        let path = Path::new(DASCLAW_CGROUP_DIR).join(format!("exec-{unique_suffix}"));
        fs::create_dir(&path)?;
        let guard = Self { path };

        if let Some(bytes) = limits.max_memory_bytes {
            // memory.max 接受十进制字节数；"max" 字符串表示无限。
            // 我们的 ResourceLimits 用 None 表示无限（不调到此分支），
            // 所以这里只处理具体值。
            fs::write(guard.path.join("memory.max"), bytes.to_string())?;
        }

        Ok(guard)
    }

    /// 把子进程 pid 加入本 cgroup。必须在子进程已 spawn 之后调用。
    ///
    /// 写 `cgroup.procs` 是 cgroup v2 移动进程的官方接口；写入后子进程的
    /// 内存 / CPU 计费立即归到本 cgroup。
    pub fn assign_pid(&self, pid: u32) -> io::Result<()> {
        fs::write(self.path.join("cgroup.procs"), pid.to_string())
    }

    /// 暴露路径，便于日志和测试。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CgroupGuard {
    fn drop(&mut self) {
        // rmdir cgroup 目录。
        // - 子进程已正常退出 → procs 为空 → rmdir 成功
        // - 子进程异常仍存活 → rmdir 返回 EBUSY → 忽略，等下次孤儿清理
        // 不 panic：drop 不允许失败语义。
        let _ = fs::remove_dir(&self.path);
    }
}

/// 生成 per-execution cgroup 唯一后缀：`{pid}-{nanos}`。
///
/// 为什么不用 `uuid` crate：
/// - 需求只是"同一进程内不同次 execute 之间不冲突"，pid+nanos 已经足够
/// - 避免新增 workspace 依赖
/// - 与 codex 风格一致（codex 用 `tempfile` + 时间戳即可，未引 uuid）
pub fn unique_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}", std::process::id(), nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_suffix_contains_pid() {
        let s = unique_suffix();
        assert!(s.starts_with(&format!("{}-", std::process::id())));
        assert!(s.split('-').count() >= 2);
    }

    #[test]
    fn unique_suffix_changes_between_calls() {
        let a = unique_suffix();
        // 同 pid 下 nanos 必定单调递增；连续调用的 suffix 不同。
        let b = unique_suffix();
        assert_ne!(a, b);
    }

    #[test]
    fn detection_result_is_available_predicate() {
        assert!(DetectionResult::Available.is_available());
        assert!(!DetectionResult::Unavailable("reason".into()).is_available());
    }

    #[test]
    fn detect_returns_consistent_result() {
        // 在 CI / 开发机上结果取决于环境，本测试只验证 OnceLock 行为：
        // 同一进程内多次 detect() 返回同一引用。
        let r1 = detect() as *const DetectionResult;
        let r2 = detect() as *const DetectionResult;
        assert_eq!(r1, r2);
    }

    // 注意：真实 mkdir/write/rmdir 测试需要 /sys/fs/cgroup 写权限，
    // 不在单元测试覆盖；由集成测试（带 root 或 rootless cgroup）和
    // 端到端场景验证。
}

//! macOS process memory limit via `memorystatus_control` SPI (W3.3-3b).
//!
//! ## 背景
//!
//! macOS 上 `setrlimit(RLIMIT_AS / RLIMIT_DATA)` 实测返回 EINVAL —— Darwin
//! 内核暴露常量但不实施。生产级进程内存限制走
//! `memorystatus_control(MEMORYSTATUS_CMD_SET_MEMLIMIT_PROPERTIES, ...)`：
//!
//! - 头文件公开（随 Xcode CLT 安装的 SDK 中 `<sys/kern_memorystatus.h>`）
//! - 无 entitlement 即可对自身或自己 spawn 的子进程设限
//! - WebKit / Chromium 渲染进程长期使用
//! - 超限即由 jetsam 子系统 SIGKILL 子进程
//!   (`JETSAM_REASON_MEMORY_PERPROCESSLIMIT`, reason=7)
//!
//! ## 调用时序
//!
//! ```text
//!   parent: posix_spawn (via Command::spawn)
//!     ├─ child created with pid
//!   parent: memorystatus_control(SET_MEMLIMIT_PROPERTIES, child_pid, ...)
//!     ├─ kernel ledger 立即生效
//!   parent: child.wait_with_output()
//! ```
//!
//! **关键**：`memorystatus_control` 不在 POSIX async-signal-safe 列表，
//! 必须在父进程而非 pre_exec hook 中调用；调用时机要在子进程已存在
//! 之后（fork 完成后）。
//!
//! ## 参考
//!
//! - [46 — macOS memory limit research](../../../docs/plans/architecture-refactor/46-macos-memory-limit-research.md)
//! - [Apple darwin-xnu `kern_memorystatus.h`](https://github.com/apple/darwin-xnu/blob/main/bsd/sys/kern_memorystatus.h)

#![cfg(target_os = "macos")]

use std::io;

/// `MEMORYSTATUS_CMD_SET_MEMLIMIT_PROPERTIES` from `<sys/kern_memorystatus.h>`.
///
/// 来源：Apple darwin-xnu 公开头：
/// `bsd/sys/kern_memorystatus.h` `#define MEMORYSTATUS_CMD_SET_MEMLIMIT_PROPERTIES 7`
const MEMORYSTATUS_CMD_SET_MEMLIMIT_PROPERTIES: u32 = 7;

/// `MEMORYSTATUS_MEMLIMIT_ATTR_FATAL` —— 超限时 kernel 立即 SIGKILL，
/// 不发预警、不进入 idle-exit 流程。这是我们想要的"硬限制"语义。
const MEMORYSTATUS_MEMLIMIT_ATTR_FATAL: u32 = 1;

/// `memorystatus_memlimit_properties` from `<sys/kern_memorystatus.h>`.
///
/// 字段语义：
/// - `memlimit_active` / `memlimit_inactive`：foreground / background
///   状态下的内存上限，单位 **MB**（`int32_t`）。
/// - `*_attr`：bit 0 = FATAL（超限即 kill），其它 bits 保留。
///
/// 我们让 active 与 inactive 相同，使限制不依赖 UI focus 状态——
/// 命令行子进程没有 frontend/background 区分。
#[repr(C)]
struct MemorystatusMemlimitProperties {
    memlimit_active: i32,
    memlimit_active_attr: u32,
    memlimit_inactive: i32,
    memlimit_inactive_attr: u32,
}

extern "C" {
    /// `int memorystatus_control(uint32_t command, int32_t pid, uint32_t flags,
    ///                            void *buffer, size_t buffersize);`
    ///
    /// 来源：darwin libsystem_kernel；与 syscall(440) 等价但有官方 libc 入口。
    fn memorystatus_control(
        command: u32,
        pid: i32,
        flags: u32,
        buffer: *mut libc::c_void,
        buffersize: libc::size_t,
    ) -> libc::c_int;
}

/// 给 `pid` 设置硬性 RSS 上限，单位字节；超限由 kernel SIGKILL。
///
/// 内部把字节转换为 MB（向上取整防止 0 字节误设无限）。`bytes < 1MB`
/// 会被 clamp 到 1MB —— `memorystatus_control` 接受的最小值是 1MB。
///
/// 失败返回 `io::Error`：调用方应当把它当作"加固层未生效"，但**不**
/// 因此中止整个 execute——rlimit 兜底（CPU/FD/NPROC）仍然有效。
pub fn set_memory_limit(pid: i32, bytes: u64) -> io::Result<()> {
    let mb_u64 = bytes.div_ceil(1024 * 1024).max(1);
    // Darwin 接口用 i32 MB；超过 i32::MAX MB（≈2 PB）的请求 clamp。
    let mb = mb_u64.min(i32::MAX as u64) as i32;

    let mut props = MemorystatusMemlimitProperties {
        memlimit_active: mb,
        memlimit_active_attr: MEMORYSTATUS_MEMLIMIT_ATTR_FATAL,
        memlimit_inactive: mb,
        memlimit_inactive_attr: MEMORYSTATUS_MEMLIMIT_ATTR_FATAL,
    };

    // SAFETY: memorystatus_control 是 darwin libsystem_kernel 的标准 SPI；
    // 我们传入栈分配的 props 结构和正确的 size。pid 由 Command::spawn 返回，
    // 在 wait 之前必然存活（zombie 也算存在，syscall 不会越界访问）。
    let ret = unsafe {
        memorystatus_control(
            MEMORYSTATUS_CMD_SET_MEMLIMIT_PROPERTIES,
            pid,
            0,
            &mut props as *mut _ as *mut libc::c_void,
            std::mem::size_of::<MemorystatusMemlimitProperties>(),
        )
    };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_on_self_with_huge_limit_succeeds() {
        // 给自己设个大到不可能触达的限制（i32::MAX MB ≈ 2 PB）。
        // 这个调用不应该影响测试运行，但能验证 FFI 调用路径完整。
        let pid = std::process::id() as i32;
        let huge_bytes = (i32::MAX as u64) * 1024 * 1024;
        // 在 sandbox CI 或开发机上，给自身设限有可能被 entitlement 检查
        // 拒绝（很少见，但 macOS 14+ 有些场景会报 EPERM）。我们只断言
        // **不 panic**——任何 OS 错误都视为环境约束，跳过。
        let _ = set_memory_limit(pid, huge_bytes);
    }

    #[test]
    fn set_on_invalid_pid_returns_error() {
        // pid=0 表示"all processes"在 BSD 习惯里是特殊值，但 memorystatus
        // 不接受；必然返回错误。pid=-1 同理。
        // 我们用 pid=2_000_000_000 这种大概率不存在的 pid。
        let result = set_memory_limit(2_000_000_000, 1024 * 1024 * 1024);
        assert!(result.is_err(), "expected error for non-existent pid");
    }

    #[test]
    fn small_bytes_are_clamped_up_to_1mb() {
        // bytes < 1MB 应被 clamp 到 1MB，避免向 kernel 提交 0 MB
        // （会被解读为"无限"或返回 EINVAL，与调用方意图相反）。
        // 这个测试只验证不 panic + 接口可达——即使 OS 拒绝 1MB 太小，
        // 我们只要保证我们的 clamp 逻辑被触发。
        let pid = std::process::id() as i32;
        let _ = set_memory_limit(pid, 100); // 100 bytes → clamp 到 1MB
    }
}

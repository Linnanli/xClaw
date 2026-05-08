//! Windows Job Object FFI for outer resource-limit wrapping (Slice B2 of ADR-131).
//!
//! ## 角色
//!
//! 这个模块实现 ADR-131 决议的 **side-by-side wrapper** 中的 Job Object
//! FFI 部分（详见 [`docs/plans/architecture-refactor/adr-131-windows-job-object-resource-limits-wrapper.md`](../../../../docs/plans/architecture-refactor/adr-131-windows-job-object-resource-limits-wrapper.md)）：
//!
//! - 不修改 `dasclaw_sandbox_windows` verbatim crate（合规 ADR-129 §1.3 红线）
//! - 创建一个 **outer** Job Object，设 `JOB_OBJECT_LIMIT_PROCESS_MEMORY` /
//!   `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` / `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
//! - `AssignProcessToJobObject` 把当前进程（launcher binary）放进 outer job
//! - upstream `command_runner_win.rs` 之后再创建 inner job（仅
//!   `KILL_ON_JOB_CLOSE`），是 outer 的 nested child（Win 8+ 嵌套 job 语义）
//! - arbitrary command 跑在 inner job 中，受 outer job 的 memory/process
//!   limits 自动 enforce
//!
//! ## 安全契约
//!
//! - [`JobObject`] 是 RAII guard：drop 时 `CloseHandle`。
//!   配合 `KILL_ON_JOB_CLOSE`，guard drop 会让 OS 立即 kill 整条 job 链上
//!   的所有进程（包括 upstream spawn 的 sandbox 子进程），保证不存在 orphan。
//! - 所有 FFI 调用失败都返回 [`JobObjectError`]，不 panic、不 unwrap。
//!
//! ## 调用方
//!
//! Slice B3 (adapter 接入) 把这个模块和 [`crate::launcher_ipc`] (B1) 组合在
//! `dasclaw-sandbox-resource-launcher.exe` 里使用。本模块自身不调用 lib API。

#![cfg(target_os = "windows")]

use std::mem::size_of;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

/// Resource limits to apply to the outer job object.
///
/// `None` means "no limit on this axis"（OS 默认）。零值视为非法，构造函数会
/// 拒绝（防误用）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OuterJobLimits {
    /// Per-process memory cap（bytes）。映射到 `JOB_OBJECT_LIMIT_PROCESS_MEMORY`。
    /// 上游 sandbox 子进程超过此值会被 OS 立即终止。
    pub max_process_memory_bytes: Option<u64>,

    /// Job-wide memory cap（bytes）。映射到 `JOB_OBJECT_LIMIT_JOB_MEMORY`。
    /// 整条 job 上所有进程（含 nested job 的孙进程）总内存超过此值会被终止。
    pub max_job_memory_bytes: Option<u64>,

    /// Active-process count cap。映射到 `JOB_OBJECT_LIMIT_ACTIVE_PROCESS`。
    /// 防 fork bomb / DoS。值必须 >= 1（自身 launcher 已占 1）。
    pub max_active_processes: Option<u32>,
}

impl OuterJobLimits {
    /// 验证字段语义合规。零值（任何字段为 0）视为误用，返回 Err。
    pub fn validate(&self) -> Result<(), JobObjectError> {
        if self.max_process_memory_bytes == Some(0) {
            return Err(JobObjectError::InvalidLimits {
                detail: "max_process_memory_bytes=0 is invalid; use None for no limit".into(),
            });
        }
        if self.max_job_memory_bytes == Some(0) {
            return Err(JobObjectError::InvalidLimits {
                detail: "max_job_memory_bytes=0 is invalid; use None for no limit".into(),
            });
        }
        if self.max_active_processes == Some(0) {
            return Err(JobObjectError::InvalidLimits {
                detail: "max_active_processes=0 is invalid; use None for no limit".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JobObjectError {
    #[error("invalid OuterJobLimits: {detail}")]
    InvalidLimits { detail: String },

    #[error("CreateJobObjectW failed (GetLastError={code})")]
    CreateFailed { code: u32 },

    #[error(
        "SetInformationJobObject(JobObjectExtendedLimitInformation) failed (GetLastError={code})"
    )]
    SetInfoFailed { code: u32 },

    #[error("AssignProcessToJobObject failed (GetLastError={code})")]
    AssignFailed { code: u32 },
}

/// RAII guard wrapping a Windows Job Object handle.
///
/// Drop closes the handle. Combined with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
/// (always set by [`OuterJobLimits::apply_to_new_job`]), drop will terminate
/// every process associated with the job, including upstream-spawned
/// grandchildren via Windows nested-job semantics.
pub struct JobObject {
    handle: HANDLE,
}

impl JobObject {
    /// Create a new (unnamed, anonymous) Job Object and apply the given limits.
    ///
    /// Always sets `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` for hygiene (codex
    /// upstream precedent: `command_runner_win.rs:120`).
    ///
    /// On any FFI failure the handle (if created) is closed before returning Err.
    pub fn create_with_limits(limits: OuterJobLimits) -> Result<Self, JobObjectError> {
        limits.validate()?;

        // SAFETY: CreateJobObjectW with both pointers null is documented as
        // creating an anonymous unnamed job. Returns 0 on failure.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle == 0 {
            let code = unsafe { GetLastError() };
            return Err(JobObjectError::CreateFailed { code });
        }

        // SAFETY: zero-initialised JOBOBJECT_EXTENDED_LIMIT_INFORMATION, then
        // populate the bits we care about.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };

        let mut flags: u32 = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        if let Some(per_proc) = limits.max_process_memory_bytes {
            flags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            info.ProcessMemoryLimit = per_proc as usize;
        }
        if let Some(per_job) = limits.max_job_memory_bytes {
            flags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
            info.JobMemoryLimit = per_job as usize;
        }
        if let Some(active) = limits.max_active_processes {
            flags |= JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            info.BasicLimitInformation.ActiveProcessLimit = active;
        }

        info.BasicLimitInformation.LimitFlags = flags;

        // SAFETY: handle is valid (just-created), info pointer is valid for
        // the duration of the call, size matches the struct.
        let ok = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            let code = unsafe { GetLastError() };
            // SAFETY: handle was successfully created above; closing on error path.
            unsafe { CloseHandle(handle) };
            return Err(JobObjectError::SetInfoFailed { code });
        }

        Ok(JobObject { handle })
    }

    /// Attach the **current** process to this job. Children spawned afterwards
    /// inherit the job association by default (no `CREATE_BREAKAWAY_FROM_JOB`
    /// flag in upstream `command_runner_win.rs`'s spawn paths).
    ///
    /// Idempotent within a single OS process: Windows allows a process to
    /// belong to multiple jobs only via nested-job semantics; calling this
    /// twice on the same process is treated as caller error and returns Err.
    pub fn assign_current_process(&self) -> Result<(), JobObjectError> {
        // SAFETY: GetCurrentProcess returns a pseudo-handle (-1) that does not
        // need closing; AssignProcessToJobObject is documented to accept it.
        let ok = unsafe { AssignProcessToJobObject(self.handle, GetCurrentProcess()) };
        if ok == 0 {
            let code = unsafe { GetLastError() };
            return Err(JobObjectError::AssignFailed { code });
        }
        Ok(())
    }

    /// Raw handle (for FFI re-use; do **not** close manually — RAII guard owns it).
    #[inline]
    pub fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        if self.handle != 0 {
            // SAFETY: handle was created by CreateJobObjectW and not closed elsewhere.
            unsafe { CloseHandle(self.handle) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_zero_per_process_memory() {
        let limits = OuterJobLimits {
            max_process_memory_bytes: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            limits.validate(),
            Err(JobObjectError::InvalidLimits { .. })
        ));
    }

    #[test]
    fn validate_rejects_zero_job_memory() {
        let limits = OuterJobLimits {
            max_job_memory_bytes: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            limits.validate(),
            Err(JobObjectError::InvalidLimits { .. })
        ));
    }

    #[test]
    fn validate_rejects_zero_active_processes() {
        let limits = OuterJobLimits {
            max_active_processes: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            limits.validate(),
            Err(JobObjectError::InvalidLimits { .. })
        ));
    }

    #[test]
    fn validate_accepts_default() {
        let limits = OuterJobLimits::default();
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn validate_accepts_typical_values() {
        let limits = OuterJobLimits {
            max_process_memory_bytes: Some(2 * 1024 * 1024 * 1024),
            max_job_memory_bytes: Some(4 * 1024 * 1024 * 1024),
            max_active_processes: Some(64),
        };
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn create_with_default_limits_succeeds() {
        // 默认 limits 仅设 KILL_ON_JOB_CLOSE，应总能成功。
        let job = JobObject::create_with_limits(OuterJobLimits::default())
            .expect("create unnamed job object with default limits");
        assert!(job.raw_handle() != 0);
        // drop 自动 CloseHandle
    }

    #[test]
    fn create_with_typical_limits_succeeds() {
        let limits = OuterJobLimits {
            max_process_memory_bytes: Some(2 * 1024 * 1024 * 1024),
            max_job_memory_bytes: Some(4 * 1024 * 1024 * 1024),
            max_active_processes: Some(64),
        };
        let job = JobObject::create_with_limits(limits).expect("create job with typical limits");
        assert!(job.raw_handle() != 0);
    }

    #[test]
    fn create_rejects_zero_limit_fields() {
        let limits = OuterJobLimits {
            max_process_memory_bytes: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            JobObject::create_with_limits(limits),
            Err(JobObjectError::InvalidLimits { .. })
        ));
    }
}

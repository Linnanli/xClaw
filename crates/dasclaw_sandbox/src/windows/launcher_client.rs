//! Spawns `dasclaw-sandbox-resource-launcher.exe` and brokers IPC between
//! adapter and launcher (Slice B3 of ADR-131).
//!
//! ## 角色
//!
//! 这个模块是 [ADR-131](../../../../docs/plans/architecture-refactor/adr-131-windows-job-object-resource-limits-wrapper.md)
//! 决议 **side-by-side wrapper** 在 adapter 端的客户端：
//!
//! 1. 用 [`std::process::Command`] spawn launcher binary
//! 2. 把 [`crate::launcher_ipc::LauncherRequest`] JSON 写到 stdin
//! 3. wait + 读 stdout 全量 → 反序列化 [`crate::launcher_ipc::LauncherResponse`]
//! 4. base64 解码 stdout / stderr → 组装 [`std::process::Output`]
//!
//! ## launcher 路径解析
//!
//! [`resolve_launcher_path`] 按优先级尝试：
//!
//! 1. `DASCLAW_SANDBOX_LAUNCHER_PATH` 环境变量（部署 / 测试 override）
//! 2. 当前 exe 同目录下的 `dasclaw-sandbox-resource-launcher.exe`
//!    （生产部署形态：把 launcher 与主 binary 一起 ship）
//! 3. fallback：仅文件名（让 OS 走 PATH 查找，dev 场景）
//!
//! ## 失败语义
//!
//! 所有 IPC / spawn / 解码错误统一映射到
//! [`crate::SandboxError::WindowsLauncherFailed`]，与
//! [`crate::SandboxError::WindowsSetupPending`] 严格区分（详见 ADR-131 §7 Q2）。

#![cfg(target_os = "windows")]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use crate::launcher_ipc::{LauncherRequest, LauncherResponse, PROTOCOL_VERSION};
use crate::SandboxError;

const LAUNCHER_BIN: &str = "dasclaw-sandbox-resource-launcher.exe";
const LAUNCHER_PATH_ENV: &str = "DASCLAW_SANDBOX_LAUNCHER_PATH";

/// Spawn launcher，发请求，等响应，转 [`std::process::Output`]。
pub(super) fn spawn_and_capture(
    request: LauncherRequest,
) -> Result<std::process::Output, SandboxError> {
    debug_assert_eq!(request.protocol_version, PROTOCOL_VERSION);

    let exe = resolve_launcher_path();

    let req_json =
        serde_json::to_vec(&request).map_err(|e| SandboxError::WindowsLauncherFailed {
            detail: format!("serialize LauncherRequest: {e}"),
        })?;

    let mut child = Command::new(&exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| SandboxError::WindowsLauncherFailed {
            detail: format!("spawn launcher {}: {e}", exe.display()),
        })?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| SandboxError::WindowsLauncherFailed {
                detail: "launcher child stdin not piped".into(),
            })?;
        stdin
            .write_all(&req_json)
            .map_err(|e| SandboxError::WindowsLauncherFailed {
                detail: format!("write LauncherRequest to launcher stdin: {e}"),
            })?;
    }

    let raw = child
        .wait_with_output()
        .map_err(|e| SandboxError::WindowsLauncherFailed {
            detail: format!("wait launcher: {e}"),
        })?;

    let response: LauncherResponse =
        serde_json::from_slice(&raw.stdout).map_err(|e| SandboxError::WindowsLauncherFailed {
            detail: format!(
                "parse LauncherResponse: {e} (launcher stderr: {})",
                String::from_utf8_lossy(&raw.stderr).trim()
            ),
        })?;

    if response.protocol_version != PROTOCOL_VERSION {
        return Err(SandboxError::WindowsLauncherFailed {
            detail: format!(
                "launcher protocol mismatch: response v{}, expected v{PROTOCOL_VERSION}",
                response.protocol_version
            ),
        });
    }

    if let Some(err) = response.error {
        return Err(SandboxError::WindowsLauncherFailed { detail: err });
    }

    let stdout =
        BASE64
            .decode(&response.stdout_b64)
            .map_err(|e| SandboxError::WindowsLauncherFailed {
                detail: format!("decode stdout_b64: {e}"),
            })?;
    let stderr =
        BASE64
            .decode(&response.stderr_b64)
            .map_err(|e| SandboxError::WindowsLauncherFailed {
                detail: format!("decode stderr_b64: {e}"),
            })?;

    use std::os::windows::process::ExitStatusExt;
    let status = std::process::ExitStatus::from_raw(response.exit_code as u32);

    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn resolve_launcher_path() -> PathBuf {
    if let Ok(p) = std::env::var(LAUNCHER_PATH_ENV) {
        if !p.is_empty() {
            let pb = PathBuf::from(p);
            if pb.exists() {
                return pb;
            }
        }
    }

    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            let candidate = dir.join(LAUNCHER_BIN);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    PathBuf::from(LAUNCHER_BIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_path_falls_back_to_bin_name() {
        // safety: tests-only env mutation; restored at end.
        let prev = std::env::var_os(LAUNCHER_PATH_ENV);
        unsafe {
            std::env::remove_var(LAUNCHER_PATH_ENV);
        }
        let p = resolve_launcher_path();
        // 至少包含 launcher 可执行文件名；可能是 sibling-of-exe 真实路径或裸文件名
        assert!(
            p.file_name().is_some_and(|n| n == LAUNCHER_BIN),
            "expected path to end with {LAUNCHER_BIN}, got {}",
            p.display()
        );
        if let Some(v) = prev {
            unsafe {
                std::env::set_var(LAUNCHER_PATH_ENV, v);
            }
        }
    }

    #[test]
    fn launcher_path_env_override_when_exists() {
        // 用 current_exe 自身（一定存在）做 override target
        let exe = std::env::current_exe().expect("current_exe");
        let prev = std::env::var_os(LAUNCHER_PATH_ENV);
        // safety: tests-only env mutation; restored at end.
        unsafe {
            std::env::set_var(LAUNCHER_PATH_ENV, &exe);
        }
        let p = resolve_launcher_path();
        assert_eq!(p, exe);
        unsafe {
            match prev {
                Some(v) => std::env::set_var(LAUNCHER_PATH_ENV, v),
                None => std::env::remove_var(LAUNCHER_PATH_ENV),
            }
        }
    }

    #[test]
    fn launcher_path_env_ignored_when_path_missing() {
        let prev = std::env::var_os(LAUNCHER_PATH_ENV);
        // safety: tests-only env mutation; restored at end.
        unsafe {
            std::env::set_var(
                LAUNCHER_PATH_ENV,
                r"C:\definitely\not\there\dasclaw-sandbox-resource-launcher.exe",
            );
        }
        let p = resolve_launcher_path();
        // override 路径不存在 → 跌回 sibling-of-exe 或裸文件名
        assert!(
            p.file_name().is_some_and(|n| n == LAUNCHER_BIN),
            "expected fallback to {LAUNCHER_BIN}, got {}",
            p.display()
        );
        unsafe {
            match prev {
                Some(v) => std::env::set_var(LAUNCHER_PATH_ENV, v),
                None => std::env::remove_var(LAUNCHER_PATH_ENV),
            }
        }
    }
}

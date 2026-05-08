// Derived from openai/codex commit 6e838a19fa
//   path: codex-rs/windows-sandbox-rs/src/lib.rs
// SPDX-License-Identifier: Apache-2.0

//! Windows-specific sandbox runtime for x-claw.
//!
//! Ported (mechanically, with import rewrites) from
//! `openai/codex` `codex-rs/windows-sandbox-rs/` at commit `6e838a19fa`.
//! See `vendor/codex-windows-sandbox/README.md` for the reference snapshot
//! and the porting plan in tracker issue #250.
//!
//! ## Crate status (Phase 1.1.4i-9)
//!
//! V'-b "port-on-demand" subset. The cross-platform PTY/process plumbing
//! (`pty::process`) and the `cfg(windows)`-only ConPTY backend
//! (`pty::win::*`, `RawConPty`) are in tree, plus the leaf path-key,
//! sandbox setup, and logging utilities consumed across the upstream crate.
//!
//! Currently exposed:
//!
//! - Sandbox policy types (`types::SandboxPolicy`, `types::NetworkAccess`,
//!   `types::WritableRoot`).
//! - String / path utilities (`string_util`, `absolute_path`,
//!   `path_normalization`).
//! - Sandbox setup helpers (`sandbox_utils::ensure_codex_home_exists`,
//!   `sandbox_utils::inject_git_safe_directory`).
//! - Sandbox audit log (`logging::log_start` / `log_success` / `log_failure`
//!   / `log_note` / `debug_log`).
//! - DPAPI wrappers (`dpapi::protect` / `dpapi::unprotect`, `cfg(windows)`).
//! - Process/thread attribute list builder
//!   (`proc_thread_attr::ProcThreadAttributeList`, `cfg(windows)`).
//! - Setup error report types & redaction
//!   (`setup_error::SetupFailure`, `setup_error::SetupErrorCode`,
//!   `setup_error::write_setup_error_report`,
//!   `setup_error::read_setup_error_report`,
//!   `setup_error::sanitize_setup_metric_tag_value`).
//! - Per-workspace capability SID store (`cap::CapSids`,
//!   `cap::load_or_create_cap_sids`, `cap::workspace_cap_sid_for_cwd`).
//! - Sandbox environment scrubbers (`env::normalize_null_device_env`,
//!   `env::ensure_non_interactive_pager`, `env::inherit_path_env`,
//!   `env::apply_no_network_to_env`).
//! - Win32 utility helpers (`winutil::to_wide`, `winutil::format_last_error`,
//!   `winutil::resolve_sid`, `winutil::string_from_sid_bytes`,
//!   `winutil::quote_windows_arg`, `winutil::argv_to_command_line`,
//!   `cfg(windows)`).
//! - Hide newly-created sandbox users from Winlogon and hide the current
//!   user's profile dir (`hide_users::hide_newly_created_users`,
//!   `hide_users::hide_current_user_profile_dir`, `cfg(windows)`).
//! - Cross-platform PTY/process driver adapter (`pty::ProcessDriver`,
//!   `pty::SpawnedProcess`, `pty::TerminalSize`, `pty::spawn_from_driver`).
//! - Restricted-token builders for AppContainer / capability sandboxing
//!   (`token::world_sid`, `token::convert_string_sid_to_sid`,
//!   `token::get_current_token_for_restriction`,
//!   `token::get_logon_sid_bytes`, `token::create_readonly_token_with_cap`,
//!   `token::create_readonly_token_with_cap_from`,
//!   `token::create_readonly_token_with_caps_from`,
//!   `token::create_workspace_write_token_with_caps_from`, `cfg(windows)`).
//! - Private-desktop launcher (`desktop::LaunchDesktop::prepare`,
//!   `desktop::LaunchDesktop::startup_info_desktop`, `cfg(windows)`).
//! - Sandbox policy parser (`policy::parse_policy`, re-export
//!   `policy::SandboxPolicy`).
//! - Restricted-token-aware process spawner
//!   (`process::create_process_as_user`, `process::spawn_process_with_pipes`,
//!   `process::read_handle_loop`, `cfg(windows)`).
//! - Read-ACL named-mutex guard
//!   (`read_acl_mutex::acquire_read_acl_mutex`,
//!   `read_acl_mutex::read_acl_mutex_exists`,
//!   `read_acl_mutex::ReadAclMutexGuard`, `cfg(windows)`).
//! - Filesystem ACL helpers (`acl::*`, `cfg(windows)`).
//! - Sandbox user account creation / lookup (`sandbox_users::*`,
//!   `cfg(windows)`).
//! - SSH client config dependency resolver
//!   (`ssh_config_dependencies::ssh_config_dependency_paths`).
//! - Allow/deny path computation for sandbox policy
//!   (`allow::AllowDenyPaths`, `allow::compute_allow_paths`).
//! - Workspace ACL protection helpers
//!   (`workspace_acl::is_command_cwd_root`,
//!   `workspace_acl::protect_workspace_codex_dir`,
//!   `workspace_acl::protect_workspace_agents_dir`, `cfg(windows)`).
//! - Sandbox audit / world-writable scan
//!   (`audit::apply_world_writable_scan_and_denies`,
//!   `audit::gather_candidates`, `cfg(windows)`).
//! - Sandbox helper executable materialisation
//!   (`helper_materialization::resolve_current_exe_for_launch`,
//!   `cfg(windows)`).
//! - Sandbox setup orchestrator (`setup::sandbox_dir`,
//!   `setup::sandbox_bin_dir`, `setup::sandbox_secrets_dir`,
//!   `setup::SETUP_VERSION`, plus `SandboxSetupRequest`,
//!   `SetupRootOverrides`, `run_elevated_setup`, `run_setup_refresh`,
//!   `run_setup_refresh_with_extra_read_roots`; re-exported at the crate
//!   root, `cfg(windows)`).
//! - On non-Windows targets, [`unsupported`] returns a typed error indicating
//!   that the Windows sandbox runtime is unavailable.
//!
//! The actual Win32 sandbox logic (AppContainer, JobObject, network ACLs)
//! lands in subsequent PRs under tracker issue #263 / #250.

pub mod absolute_path;
#[cfg(windows)]
pub mod acl;
pub mod allow;
#[cfg(windows)]
pub mod audit;
pub mod cap;
#[cfg(windows)]
pub mod desktop;
#[cfg(windows)]
pub mod dpapi;
pub mod env;
#[cfg(windows)]
pub mod helper_materialization;
#[cfg(windows)]
pub mod hide_users;
#[cfg(windows)]
pub mod identity;
pub mod logging;
pub mod path_normalization;
pub mod policy;
#[cfg(windows)]
pub mod proc_thread_attr;
#[cfg(windows)]
pub mod process;
pub mod pty;
#[cfg(windows)]
pub mod read_acl_mutex;
#[cfg(windows)]
pub mod sandbox_users;
pub mod sandbox_utils;
#[cfg(windows)]
#[path = "setup_orchestrator.rs"]
pub mod setup;
#[cfg(windows)]
pub mod setup_error;
#[cfg(windows)]
pub mod spawn_prep;
// Consumed by setup (setup_orchestrator) on Windows; on non-Windows targets
// the consumer is cfg-gated out, so allow dead_code to keep the cross-platform
// build clean.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod ssh_config_dependencies;
pub mod string_util;
#[cfg(windows)]
pub mod token;
pub mod types;
#[cfg(windows)]
pub mod winutil;
#[cfg(windows)]
pub mod workspace_acl;

// `elevated/` subdirectory — modules used by the elevated command runner /
// IPC bootstrap path. Mirrors codex `windows-sandbox-rs` `lib.rs` declarations
// 1:1 (visibility kept verbatim: `pub(crate)` for `ipc_framed`, private for
// `runner_pipe` / `runner_client`). The remaining file in that subdirectory,
// `cwd_junction.rs`, is intentionally NOT registered here: upstream consumes
// it only from the `command_runner_win.rs` bin source via `mod cwd_junction;`,
// which lands in Wave i-7b. See ADR-130 §2.
#[cfg(windows)]
#[path = "elevated/ipc_framed.rs"]
pub(crate) mod ipc_framed;
#[cfg(windows)]
#[path = "elevated/runner_client.rs"]
mod runner_client;
#[cfg(windows)]
#[path = "elevated/runner_pipe.rs"]
mod runner_pipe;

#[cfg(windows)]
pub use helper_materialization::resolve_current_exe_for_launch;
#[cfg(windows)]
pub use identity::require_logon_sandbox_creds;
#[cfg(windows)]
pub use identity::sandbox_setup_is_complete;
#[cfg(windows)]
pub use setup::SETUP_VERSION;
#[cfg(windows)]
pub use setup::SandboxSetupRequest;
#[cfg(windows)]
pub use setup::SetupRootOverrides;
#[cfg(windows)]
pub use setup::run_elevated_setup;
#[cfg(windows)]
pub use setup::run_setup_refresh;
#[cfg(windows)]
pub use setup::run_setup_refresh_with_extra_read_roots;
#[cfg(windows)]
pub use setup::sandbox_bin_dir;
#[cfg(windows)]
pub use setup::sandbox_dir;
#[cfg(windows)]
pub use setup::sandbox_secrets_dir;

// ---------------------------------------------------------------------------
// Re-exports consumed by `bin/setup_main.rs` (Wave i-7a).
//
// These mirror codex `windows-sandbox-rs/src/lib.rs` (commit 6e838a19fa)
// 1:1 so the verbatim port of `setup_main_win.rs` compiles with only an
// `codex_windows_sandbox::` -> `dasclaw_sandbox_windows::` crate-name
// rewrite. See ADR-130 §2.
// ---------------------------------------------------------------------------
#[cfg(windows)]
pub use acl::add_deny_write_ace;
#[cfg(windows)]
pub use acl::ensure_allow_mask_aces_with_inheritance;
#[cfg(windows)]
pub use acl::ensure_allow_write_aces;
#[cfg(windows)]
pub use acl::path_mask_allows;
#[cfg(windows)]
pub use cap::load_or_create_cap_sids;
#[cfg(windows)]
pub use cap::workspace_cap_sid_for_cwd;
#[cfg(windows)]
pub use hide_users::hide_newly_created_users;
pub use logging::LOG_FILE_NAME;
pub use logging::log_note;
pub use path_normalization::canonicalize_path;
#[cfg(windows)]
pub use setup_error::SetupErrorCode;
#[cfg(windows)]
pub use setup_error::SetupErrorReport;
#[cfg(windows)]
pub use setup_error::SetupFailure;
#[cfg(windows)]
pub use setup_error::extract_failure as extract_setup_failure;
#[cfg(windows)]
pub use setup_error::write_setup_error_report;
#[cfg(windows)]
pub use token::convert_string_sid_to_sid;

// ---------------------------------------------------------------------------
// Re-exports consumed by `bin/command_runner.rs` (Wave i-7b).
//
// Same provenance as the i-7a block above: mirrors codex
// `windows-sandbox-rs/src/lib.rs` (commit 6e838a19fa) 1:1 so the verbatim
// port of `elevated/command_runner_win.rs` compiles with only an
// `codex_windows_sandbox::` -> `dasclaw_sandbox_windows::` crate-name
// rewrite. See ADR-130 §2.
// ---------------------------------------------------------------------------
#[cfg(windows)]
pub use acl::allow_null_device;
#[cfg(windows)]
pub use desktop::LaunchDesktop;
#[cfg(windows)]
pub use hide_users::hide_current_user_profile_dir;
#[cfg(windows)]
pub use ipc_framed::ErrorPayload;
#[cfg(windows)]
pub use ipc_framed::ExitPayload;
#[cfg(windows)]
pub use ipc_framed::FramedMessage;
#[cfg(windows)]
pub use ipc_framed::Message;
#[cfg(windows)]
pub use ipc_framed::OutputPayload;
#[cfg(windows)]
pub use ipc_framed::OutputStream;
#[cfg(windows)]
pub use ipc_framed::ResizePayload;
#[cfg(windows)]
pub use ipc_framed::SpawnReady;
#[cfg(windows)]
pub use ipc_framed::SpawnRequest;
#[cfg(windows)]
pub use ipc_framed::decode_bytes;
#[cfg(windows)]
pub use ipc_framed::encode_bytes;
#[cfg(windows)]
pub use ipc_framed::read_frame;
#[cfg(windows)]
pub use ipc_framed::write_frame;
pub use policy::SandboxPolicy;
pub use policy::parse_policy;
#[cfg(windows)]
pub use process::PipeSpawnHandles;
#[cfg(windows)]
pub use process::StderrMode;
#[cfg(windows)]
pub use process::StdinMode;
#[cfg(windows)]
pub use process::read_handle_loop;
#[cfg(windows)]
pub use process::spawn_process_with_pipes;
#[cfg(windows)]
pub use token::create_readonly_token_with_caps_from;
#[cfg(windows)]
pub use token::create_workspace_write_token_with_caps_from;
#[cfg(windows)]
pub use token::get_current_token_for_restriction;

// ---------------------------------------------------------------------------
// elevated_impl + inline windows_impl/stub blocks (Wave i-6b)
//
// Verbatim port of upstream `codex-rs/windows-sandbox-rs/src/{elevated_impl.rs,
// lib.rs}` at commit 6e838a19fa. Per ADR-130 §2.2 the upstream-inline
// `mod windows_impl { ... }` and `mod stub { ... }` blocks must remain inline
// in our lib.rs (extracting them into separate files would be a "patch-style"
// refactor and is forbidden).
//
// Mechanical edits applied:
//   * `// safety: verbatim from codex 6e838a19fa ...` line-end annotations are
//     `scripts/check_no_panics.py` suppression markers on `.unwrap()` /
//     `.expect()` calls. They preserve upstream behaviour byte-for-byte; the
//     upstream already gates each call with `#[allow(clippy::unwrap_used)]`.
//   * `mod stub` import rewrite: `codex_protocol::protocol::SandboxPolicy`
//     -> `crate::types::SandboxPolicy` (codex_protocol is not a dependency
//     of this crate; `crate::types::SandboxPolicy` is the equivalent type).
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod elevated_impl;
#[cfg(windows)]
pub use elevated_impl::ElevatedSandboxCaptureRequest;
#[cfg(windows)]
pub use elevated_impl::run_windows_sandbox_capture as run_windows_sandbox_capture_elevated;

#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_capture;
#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_capture_with_extra_deny_write_paths;
#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_legacy_preflight;
#[cfg(target_os = "windows")]
pub use winutil::quote_windows_arg;
#[cfg(target_os = "windows")]
pub use winutil::string_from_sid_bytes;
#[cfg(target_os = "windows")]
pub use winutil::to_wide;
#[cfg(target_os = "windows")]
pub use workspace_acl::is_command_cwd_root;

#[cfg(not(target_os = "windows"))]
pub use stub::CaptureResult;
#[cfg(not(target_os = "windows"))]
pub use stub::apply_world_writable_scan_and_denies;
#[cfg(not(target_os = "windows"))]
pub use stub::run_windows_sandbox_capture;
#[cfg(not(target_os = "windows"))]
pub use stub::run_windows_sandbox_legacy_preflight;

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::acl::add_allow_ace;
    use super::acl::add_deny_write_ace;
    use super::acl::allow_null_device;
    use super::acl::revoke_ace;
    use super::allow::AllowDenyPaths;
    use super::allow::compute_allow_paths;
    use super::cap::load_or_create_cap_sids;
    use super::cap::workspace_cap_sid_for_cwd;
    use super::logging::log_failure;
    use super::logging::log_success;
    use super::path_normalization::canonicalize_path;
    use super::policy::SandboxPolicy;
    use super::process::create_process_as_user;
    use super::sandbox_utils::ensure_codex_home_exists;
    use super::spawn_prep::prepare_legacy_spawn_context;
    use super::token::convert_string_sid_to_sid;
    use super::token::create_workspace_write_token_with_caps_from;
    use super::workspace_acl::is_command_cwd_root;
    use anyhow::Result;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::io;
    use std::path::Path;
    use std::path::PathBuf;
    use std::ptr;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
    use windows_sys::Win32::Foundation::SetHandleInformation;
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::GetExitCodeProcess;
    use windows_sys::Win32::System::Threading::INFINITE;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    type PipeHandles = ((HANDLE, HANDLE), (HANDLE, HANDLE), (HANDLE, HANDLE));

    unsafe fn setup_stdio_pipes() -> io::Result<PipeHandles> {
        let mut in_r: HANDLE = 0;
        let mut in_w: HANDLE = 0;
        let mut out_r: HANDLE = 0;
        let mut out_w: HANDLE = 0;
        let mut err_r: HANDLE = 0;
        let mut err_w: HANDLE = 0;
        if CreatePipe(&mut in_r, &mut in_w, ptr::null_mut(), 0) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if CreatePipe(&mut out_r, &mut out_w, ptr::null_mut(), 0) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if CreatePipe(&mut err_r, &mut err_w, ptr::null_mut(), 0) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if SetHandleInformation(in_r, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if SetHandleInformation(out_w, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if SetHandleInformation(err_w, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        Ok(((in_r, in_w), (out_r, out_w), (err_r, err_w)))
    }

    pub struct CaptureResult {
        pub exit_code: i32,
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
        pub timed_out: bool,
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture(
        policy_json_or_preset: &str,
        sandbox_policy_cwd: &Path,
        codex_home: &Path,
        command: Vec<String>,
        cwd: &Path,
        env_map: HashMap<String, String>,
        timeout_ms: Option<u64>,
        use_private_desktop: bool,
    ) -> Result<CaptureResult> {
        run_windows_sandbox_capture_with_extra_deny_write_paths(
            policy_json_or_preset,
            sandbox_policy_cwd,
            codex_home,
            command,
            cwd,
            env_map,
            timeout_ms,
            &[],
            use_private_desktop,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture_with_extra_deny_write_paths(
        policy_json_or_preset: &str,
        sandbox_policy_cwd: &Path,
        codex_home: &Path,
        command: Vec<String>,
        cwd: &Path,
        mut env_map: HashMap<String, String>,
        timeout_ms: Option<u64>,
        additional_deny_write_paths: &[PathBuf],
        use_private_desktop: bool,
    ) -> Result<CaptureResult> {
        let common = prepare_legacy_spawn_context(
            policy_json_or_preset,
            codex_home,
            cwd,
            &mut env_map,
            &command,
            /*inherit_path*/ false,
            /*add_git_safe_directory*/ false,
        )?;
        let policy = common.policy;
        let current_dir = common.current_dir;
        let logs_base_dir = common.logs_base_dir.as_deref();
        let is_workspace_write = common.is_workspace_write;
        if !policy.has_full_disk_read_access() {
            anyhow::bail!(
                "Restricted read-only access requires the elevated Windows sandbox backend"
            );
        }
        let caps = load_or_create_cap_sids(codex_home)?;
        let (h_token, psid_generic, psid_workspace): (HANDLE, *mut c_void, Option<*mut c_void>) = unsafe {
            match &policy {
                SandboxPolicy::ReadOnly { .. } => {
                    #[allow(clippy::expect_used)]
                    let psid =
                        convert_string_sid_to_sid(&caps.readonly).expect("valid readonly SID"); // safety: verbatim from codex 6e838a19fa lib.rs windows_impl
                    let (h, _) = super::token::create_readonly_token_with_cap(psid)?;
                    (h, psid, None)
                }
                SandboxPolicy::WorkspaceWrite { .. } => {
                    #[allow(clippy::expect_used)]
                    let psid_generic =
                        convert_string_sid_to_sid(&caps.workspace).expect("valid workspace SID"); // safety: verbatim from codex 6e838a19fa lib.rs windows_impl
                    let ws_sid = workspace_cap_sid_for_cwd(codex_home, cwd)?;
                    #[allow(clippy::expect_used)]
                    let psid_workspace =
                        convert_string_sid_to_sid(&ws_sid).expect("valid workspace SID"); // safety: verbatim from codex 6e838a19fa lib.rs windows_impl
                    let base = super::token::get_current_token_for_restriction()?;
                    let h_res = create_workspace_write_token_with_caps_from(
                        base,
                        &[psid_generic, psid_workspace],
                    );
                    windows_sys::Win32::Foundation::CloseHandle(base);
                    let h = h_res?;
                    (h, psid_generic, Some(psid_workspace))
                }
                SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                    unreachable!("DangerFullAccess handled above")
                }
            }
        };

        unsafe {
            if is_workspace_write
                && let Ok(base) = super::token::get_current_token_for_restriction()
            {
                if let Ok(bytes) = super::token::get_logon_sid_bytes(base) {
                    let mut tmp = bytes;
                    let psid2 = tmp.as_mut_ptr() as *mut c_void;
                    allow_null_device(psid2);
                }
                windows_sys::Win32::Foundation::CloseHandle(base);
            }
        }

        let persist_aces = is_workspace_write;
        let AllowDenyPaths { allow, mut deny } =
            compute_allow_paths(&policy, sandbox_policy_cwd, &current_dir, &env_map);
        for path in additional_deny_write_paths {
            if path.exists() {
                deny.insert(path.clone());
            }
        }
        let canonical_cwd = canonicalize_path(&current_dir);
        let mut guards: Vec<(PathBuf, *mut c_void)> = Vec::new();
        unsafe {
            for p in &allow {
                let psid = if is_workspace_write && is_command_cwd_root(p, &canonical_cwd) {
                    psid_workspace.unwrap_or(psid_generic)
                } else {
                    psid_generic
                };
                if let Ok(added) = add_allow_ace(p, psid)
                    && added
                {
                    if persist_aces {
                        if p.is_dir() {
                            // best-effort seeding omitted intentionally
                        }
                    } else {
                        guards.push((p.clone(), psid));
                    }
                }
            }
            for p in &deny {
                if let Ok(added) = add_deny_write_ace(p, psid_generic)
                    && added
                    && !persist_aces
                {
                    guards.push((p.clone(), psid_generic));
                }
            }
            allow_null_device(psid_generic);
            if let Some(psid) = psid_workspace {
                allow_null_device(psid);
            }
        }
        let (stdin_pair, stdout_pair, stderr_pair) = unsafe { setup_stdio_pipes()? };
        let ((in_r, in_w), (out_r, out_w), (err_r, err_w)) = (stdin_pair, stdout_pair, stderr_pair);
        let spawn_res = unsafe {
            create_process_as_user(
                h_token,
                &command,
                cwd,
                &env_map,
                logs_base_dir,
                Some((in_r, out_w, err_w)),
                use_private_desktop,
            )
        };
        let created = match spawn_res {
            Ok(v) => v,
            Err(err) => {
                unsafe {
                    CloseHandle(in_r);
                    CloseHandle(in_w);
                    CloseHandle(out_r);
                    CloseHandle(out_w);
                    CloseHandle(err_r);
                    CloseHandle(err_w);
                    CloseHandle(h_token);
                }
                return Err(err);
            }
        };
        let pi = created.process_info;
        let _desktop = created;

        unsafe {
            CloseHandle(in_r);
            // Close the parent's stdin write end so the child sees EOF immediately.
            CloseHandle(in_w);
            CloseHandle(out_w);
            CloseHandle(err_w);
        }

        let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
        let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
        let t_out = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            loop {
                let mut read_bytes: u32 = 0;
                let ok = unsafe {
                    windows_sys::Win32::Storage::FileSystem::ReadFile(
                        out_r,
                        tmp.as_mut_ptr(),
                        tmp.len() as u32,
                        &mut read_bytes,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 || read_bytes == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..read_bytes as usize]);
            }
            let _ = tx_out.send(buf);
        });
        let t_err = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            loop {
                let mut read_bytes: u32 = 0;
                let ok = unsafe {
                    windows_sys::Win32::Storage::FileSystem::ReadFile(
                        err_r,
                        tmp.as_mut_ptr(),
                        tmp.len() as u32,
                        &mut read_bytes,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 || read_bytes == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..read_bytes as usize]);
            }
            let _ = tx_err.send(buf);
        });

        let timeout = timeout_ms.map(|ms| ms as u32).unwrap_or(INFINITE);
        let res = unsafe { WaitForSingleObject(pi.hProcess, timeout) };
        let timed_out = res == 0x0000_0102;
        let mut exit_code_u32: u32 = 1;
        if !timed_out {
            unsafe {
                GetExitCodeProcess(pi.hProcess, &mut exit_code_u32);
            }
        } else {
            unsafe {
                windows_sys::Win32::System::Threading::TerminateProcess(pi.hProcess, 1);
            }
        }

        unsafe {
            if pi.hThread != 0 {
                CloseHandle(pi.hThread);
            }
            if pi.hProcess != 0 {
                CloseHandle(pi.hProcess);
            }
            CloseHandle(h_token);
        }
        let _ = t_out.join();
        let _ = t_err.join();
        let stdout = rx_out.recv().unwrap_or_default();
        let stderr = rx_err.recv().unwrap_or_default();
        let exit_code = if timed_out {
            128 + 64
        } else {
            exit_code_u32 as i32
        };

        if exit_code == 0 {
            log_success(&command, logs_base_dir);
        } else {
            log_failure(&command, &format!("exit code {exit_code}"), logs_base_dir);
        }

        if !persist_aces {
            unsafe {
                for (p, sid) in guards {
                    revoke_ace(&p, sid);
                }
            }
        }
        Ok(CaptureResult {
            exit_code,
            stdout,
            stderr,
            timed_out,
        })
    }

    pub fn run_windows_sandbox_legacy_preflight(
        sandbox_policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        codex_home: &Path,
        cwd: &Path,
        env_map: &HashMap<String, String>,
    ) -> Result<()> {
        let is_workspace_write = matches!(sandbox_policy, SandboxPolicy::WorkspaceWrite { .. });
        if !is_workspace_write {
            return Ok(());
        }

        ensure_codex_home_exists(codex_home)?;
        let caps = load_or_create_cap_sids(codex_home)?;
        #[allow(clippy::expect_used)]
        let psid_generic =
            unsafe { convert_string_sid_to_sid(&caps.workspace) }.expect("valid workspace SID"); // safety: verbatim from codex 6e838a19fa lib.rs windows_impl
        let ws_sid = workspace_cap_sid_for_cwd(codex_home, cwd)?;
        #[allow(clippy::expect_used)]
        let psid_workspace =
            unsafe { convert_string_sid_to_sid(&ws_sid) }.expect("valid workspace SID"); // safety: verbatim from codex 6e838a19fa lib.rs windows_impl
        let current_dir = cwd.to_path_buf();
        let AllowDenyPaths { allow, deny } =
            compute_allow_paths(sandbox_policy, sandbox_policy_cwd, &current_dir, env_map);
        let canonical_cwd = canonicalize_path(&current_dir);
        unsafe {
            for p in &allow {
                let psid = if is_command_cwd_root(p, &canonical_cwd) {
                    psid_workspace
                } else {
                    psid_generic
                };
                let _ = add_allow_ace(p, psid);
            }
            for p in &deny {
                let _ = add_deny_write_ace(p, psid_generic);
            }
            allow_null_device(psid_generic);
            allow_null_device(psid_workspace);
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use crate::policy::SandboxPolicy;
        use crate::spawn_prep::should_apply_network_block;

        fn workspace_policy(network_access: bool) -> SandboxPolicy {
            SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                network_access,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }
        }

        #[test]
        fn applies_network_block_when_access_is_disabled() {
            assert!(should_apply_network_block(&workspace_policy(
                /*network_access*/ false
            )));
        }

        #[test]
        fn skips_network_block_when_access_is_allowed() {
            assert!(!should_apply_network_block(&workspace_policy(
                /*network_access*/ true
            )));
        }

        #[test]
        fn applies_network_block_for_read_only() {
            assert!(should_apply_network_block(
                &SandboxPolicy::new_read_only_policy()
            ));
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod stub {
    use crate::types::SandboxPolicy;
    use anyhow::Result;
    use anyhow::bail;
    use std::collections::HashMap;
    use std::path::Path;

    #[derive(Debug, Default)]
    pub struct CaptureResult {
        pub exit_code: i32,
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
        pub timed_out: bool,
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture(
        _policy_json_or_preset: &str,
        _sandbox_policy_cwd: &Path,
        _codex_home: &Path,
        _command: Vec<String>,
        _cwd: &Path,
        _env_map: HashMap<String, String>,
        _timeout_ms: Option<u64>,
        _use_private_desktop: bool,
    ) -> Result<CaptureResult> {
        bail!("Windows sandbox is only available on Windows")
    }

    pub fn apply_world_writable_scan_and_denies(
        _codex_home: &Path,
        _cwd: &Path,
        _env_map: &HashMap<String, String>,
        _sandbox_policy: &SandboxPolicy,
        _logs_base_dir: Option<&Path>,
    ) -> Result<()> {
        bail!("Windows sandbox is only available on Windows")
    }

    pub fn run_windows_sandbox_legacy_preflight(
        _sandbox_policy: &SandboxPolicy,
        _sandbox_policy_cwd: &Path,
        _codex_home: &Path,
        _cwd: &Path,
        _env_map: &HashMap<String, String>,
    ) -> Result<()> {
        bail!("Windows sandbox is only available on Windows")
    }
}

/// Returned by sandbox entry points on platforms where the Windows sandbox is
/// unavailable.
///
/// Until the Win32 implementation lands (tracker #250), this is the only
/// surface the rest of the workspace can call into.
pub fn unsupported() -> anyhow::Error {
    anyhow::anyhow!(
        "dasclaw_sandbox_windows: Windows sandbox runtime is not yet implemented on this build (Phase 1.1.0 skeleton)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_message_is_stable() {
        let err = unsupported();
        let msg = err.to_string();
        assert!(msg.contains("dasclaw_sandbox_windows"));
        assert!(msg.contains("Phase 1.1.0"));
    }
}

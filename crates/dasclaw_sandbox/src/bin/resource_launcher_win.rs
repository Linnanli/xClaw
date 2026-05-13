//! `dasclaw-sandbox-resource-launcher` binary — Slice B3 of ADR-131.
//!
//! ## 角色
//!
//! 这个 binary 是 [ADR-131](../../../../docs/plans/architecture-refactor/adr-131-windows-job-object-resource-limits-wrapper.md)
//! 决议的 **side-by-side wrapper** 入口（Option B）：
//!
//! 1. 从 stdin 读 [`LauncherRequest`] JSON
//! 2. 若 `outer_limits` 非空：创建 outer [`JobObject`] + 设资源限制 +
//!    `AssignProcessToJobObject(GetCurrentProcess())` —— 之后任何子孙进程
//!    （包括 `run_windows_sandbox_capture` spawn 出来的沙箱目标）会**默认**
//!    继承到该外层 Job Object（Win 8+ 嵌套 Job Object 语义）
//! 3. 同进程内调 `dasclaw_sandbox_windows::run_windows_sandbox_capture`
//!    —— `dasclaw_sandbox_windows` crate 是 codex `windows-sandbox-rs` 的 1:1
//!    verbatim port（ADR-129 §1.3 红线，禁止修改）
//! 4. 把 [`LauncherResponse`] JSON 写到 stdout
//!
//! Slice B1 起骨架，B2 补 [`JobObject`] FFI，本 slice (B3) 把它们组合并接
//! `run_windows_sandbox_capture`，同时把内嵌 base64 替换为 `base64` crate。
//!
//! ## 平台
//!
//! 真实逻辑只在 `cfg(target_os = "windows")` 下编译。其他平台编 stub main，
//! 退出码 1 + stderr 提示。理由见 ADR-131 §7 Q4：Cargo `[[bin]]` 不支持
//! `[target.'cfg(...)'.bin]`；stub 让全平台 `cargo build` 可靠通过。

/// #324 sub-task 1 — Pre-main process hardening hook.
///
/// Runs before `fn main()` (via `#[ctor::ctor]`) to disable core dumps,
/// block ptrace attach, and scrub dangerous environment variables. Applies to
/// both the Windows real-impl and the cross-platform stub branches below.
/// Pattern mirrors codex `responses-api-proxy/src/main.rs:4-7`.
#[ctor::ctor]
fn pre_main() {
    dasclaw_process_hardening::pre_main_hardening();
}

#[cfg(target_os = "windows")]
fn main() -> std::process::ExitCode {
    use std::io::{Read, Write};

    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine as _;

    use dasclaw_sandbox::launcher_ipc::{LauncherRequest, LauncherResponse, PROTOCOL_VERSION};
    use dasclaw_sandbox::windows::job_object::{JobObject, OuterJobLimits};
    use dasclaw_sandbox_windows::run_windows_sandbox_capture_elevated;
    use dasclaw_sandbox_windows::run_windows_sandbox_capture_with_extra_deny_write_paths;
    use dasclaw_sandbox_windows::ElevatedSandboxCaptureRequest;

    fn write_response(resp: &LauncherResponse) -> std::io::Result<()> {
        let json = serde_json::to_vec(resp)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(&json)?;
        handle.flush()
    }

    fn fail(detail: impl Into<String>, code: u8) -> std::process::ExitCode {
        let _ = write_response(&LauncherResponse::error(detail));
        std::process::ExitCode::from(code)
    }

    let mut buf = String::new();
    if let Err(e) = std::io::stdin().lock().read_to_string(&mut buf) {
        return fail(format!("read stdin: {e}"), 2);
    }

    let req: LauncherRequest = match serde_json::from_str(&buf) {
        Ok(r) => r,
        Err(e) => return fail(format!("parse LauncherRequest: {e}"), 2),
    };

    if req.protocol_version != PROTOCOL_VERSION {
        let _ = write_response(&LauncherResponse::error_protocol_mismatch(
            req.protocol_version,
            PROTOCOL_VERSION,
        ));
        return std::process::ExitCode::from(1);
    }

    // outer Job Object（仅当请求带限制时）。RAII guard 必须存活到
    // run_windows_sandbox_capture 返回之后，所以绑定到 main 局部。
    let _outer_guard = match req.outer_limits {
        None => None,
        Some(wire) => {
            let limits = OuterJobLimits {
                max_process_memory_bytes: wire.max_process_memory_bytes,
                max_job_memory_bytes: wire.max_job_memory_bytes,
                max_active_processes: wire.max_active_processes,
            };
            let job = match JobObject::create_with_limits(limits) {
                Ok(j) => j,
                Err(e) => return fail(format!("JobObject::create_with_limits: {e}"), 1),
            };
            if let Err(e) = job.assign_current_process() {
                return fail(format!("JobObject::assign_current_process: {e}"), 1);
            }
            Some(job)
        }
    };

    let capture = if req.use_elevated_backend {
        // Issue #462 (B4-7) — Elevated backend branch. v1: pass `None` for
        // every filesystem override so upstream uses the policy_json
        // defaults verbatim; future slices may surface real overrides
        // through the IPC. `proxy_enforced` here mirrors the dispatch
        // decision (helper returns `true` when either operator config or
        // proxy demand it), so plumbing `true` keeps the elevated path's
        // own firewall-mode logic consistent.
        let request = ElevatedSandboxCaptureRequest {
            policy_json_or_preset: req.policy_json.as_str(),
            sandbox_policy_cwd: req.cwd.as_path(),
            codex_home: req.dasclaw_home.as_path(),
            command: req.argv,
            cwd: req.cwd.as_path(),
            env_map: req.env,
            timeout_ms: None,
            use_private_desktop: req.use_private_desktop,
            proxy_enforced: true,
            read_roots_override: None,
            read_roots_include_platform_defaults: true,
            write_roots_override: None,
            deny_write_paths_override: req.additional_deny_write_paths.as_slice(),
        };
        match run_windows_sandbox_capture_elevated(request) {
            Ok(c) => c,
            Err(e) => return fail(format!("run_windows_sandbox_capture_elevated: {e}"), 1),
        }
    } else {
        match run_windows_sandbox_capture_with_extra_deny_write_paths(
            &req.policy_json,
            &req.cwd,
            &req.dasclaw_home,
            req.argv,
            &req.cwd,
            req.env,
            None, // timeout — adapter 上层（OsExecutor）用 tokio timeout 包裹
            // ADR-141 §3 PR-W3 / OQ-W3-2 (sign-off 2026-05-11): forward
            // `SandboxBackendConfig::read_only_subpaths` as upstream
            // `additional_deny_write_paths`, so upstream's `acl::add_deny_write_ace`
            // installs Win32 DACL DENY ACEs on every carve-out path. Empty slice
            // preserves Slice B1 behaviour bit-for-bit.
            &req.additional_deny_write_paths,
            req.use_private_desktop,
        ) {
            Ok(c) => c,
            Err(e) => return fail(format!("run_windows_sandbox_capture: {e}"), 1),
        }
    };

    let resp = LauncherResponse {
        protocol_version: PROTOCOL_VERSION,
        exit_code: capture.exit_code,
        stdout_b64: BASE64.encode(&capture.stdout),
        stderr_b64: BASE64.encode(&capture.stderr),
        error: None,
    };

    if let Err(e) = write_response(&resp) {
        eprintln!("write LauncherResponse: {e}");
        return std::process::ExitCode::from(2);
    }

    std::process::ExitCode::SUCCESS
}

#[cfg(not(target_os = "windows"))]
fn main() -> std::process::ExitCode {
    eprintln!(
        "dasclaw-sandbox-resource-launcher is Windows-only; \
         building on this platform only ensures workspace-wide cargo build passes. \
         See ADR-131 §7 Q4 for rationale."
    );
    std::process::ExitCode::from(1)
}

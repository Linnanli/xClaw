//! `dasclaw-sandbox-resource-launcher` binary — Slice B1 of ADR-131.
//!
//! ## 角色
//!
//! 这个 binary 是 ADR-131 决议的 **side-by-side wrapper** 入口（Option B）：
//!
//! 1. 从 stdin 读 [`LauncherRequest`] JSON
//! 2. （B3 后续）创建 outer Job Object（Slice B2 的 `JobObject`）+ 设资源限制 +
//!    `AssignProcessToJobObject(GetCurrentProcess())`
//! 3. （B3 后续）调 `dasclaw_sandbox_windows::run_windows_sandbox_capture`
//! 4. 把 [`LauncherResponse`] JSON 写到 stdout
//!
//! ## 当前 slice (B1) 范围
//!
//! 本 PR 只完成 **骨架 + IPC 协议**，**不**调任何 FFI / lib API：
//!
//! - 解析 stdin → [`LauncherRequest`]
//! - 校验 [`PROTOCOL_VERSION`] 匹配
//! - 写一个 dry-run 响应（`exit_code = 0`，`stdout_b64 = base64("dry-run B1")`）
//!   到 stdout
//! - 退出码 0 / 1 / 2，分别对应 OK / 协议不匹配 / 解析失败
//!
//! Slice B3 会替换这里的 dry-run 逻辑为真实的 Job Object + sandbox 调用。
//!
//! ## 平台
//!
//! 真实逻辑只在 `cfg(target_os = "windows")` 下编译。其他平台编 stub main，
//! 退出码 1 + stderr 提示。理由见 [ADR-131 §7 Q4](../../../docs/plans/architecture-refactor/adr-131-windows-job-object-resource-limits-wrapper.md#7-open-questions----已签字回答)：
//! Cargo `[[bin]]` 不支持 `[target.'cfg(...)'.bin]`；`required-features` 方案
//! 增加 CI 复杂度与误用风险。stub 让全平台 `cargo build` 可靠通过。

#[cfg(target_os = "windows")]
fn main() -> std::process::ExitCode {
    use std::io::{Read, Write};

    use dasclaw_sandbox::launcher_ipc::{LauncherRequest, LauncherResponse, PROTOCOL_VERSION};

    let stdin = std::io::stdin();
    let mut buf = String::new();
    if let Err(e) = stdin.lock().read_to_string(&mut buf) {
        let resp = LauncherResponse::error(format!("read stdin failed: {e}"));
        let _ = write_response(&resp);
        return std::process::ExitCode::from(2);
    }

    let req: LauncherRequest = match serde_json::from_str(&buf) {
        Ok(r) => r,
        Err(e) => {
            let resp = LauncherResponse::error(format!("parse LauncherRequest failed: {e}"));
            let _ = write_response(&resp);
            return std::process::ExitCode::from(2);
        }
    };

    if req.protocol_version != PROTOCOL_VERSION {
        let resp =
            LauncherResponse::error_protocol_mismatch(req.protocol_version, PROTOCOL_VERSION);
        let _ = write_response(&resp);
        return std::process::ExitCode::from(1);
    }

    // B1 dry-run: 不调 FFI / lib API。echo 一个 OK 响应。
    // B3 会把以下整段替换为：
    //   let outer = JobObject::create_with_limits(...)?;
    //   outer.assign_current_process()?;
    //   let capture = run_windows_sandbox_capture(...)?;
    //   build response from capture
    let resp = LauncherResponse {
        protocol_version: PROTOCOL_VERSION,
        exit_code: 0,
        stdout_b64: base64_encode(format!(
            "dry-run B1: argv={argv:?}, outer_limits={lim:?}",
            argv = req.argv,
            lim = req.outer_limits
        )),
        stderr_b64: String::new(),
        error: None,
    };

    if let Err(e) = write_response(&resp) {
        eprintln!("write response failed: {e}");
        return std::process::ExitCode::from(2);
    }

    std::process::ExitCode::SUCCESS
}

#[cfg(target_os = "windows")]
fn write_response(resp: &dasclaw_sandbox::launcher_ipc::LauncherResponse) -> std::io::Result<()> {
    let json = serde_json::to_string(resp)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    use std::io::Write;
    handle.write_all(json.as_bytes())?;
    handle.write_all(b"\n")?;
    handle.flush()
}

/// 极小的 base64 实现（避免本 slice 引入新依赖）。
/// dry-run 用，B3 会换 `base64` crate（已在 workspace 其他 crate 用过，复用）。
#[cfg(target_os = "windows")]
fn base64_encode(s: impl AsRef<[u8]>) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_ref();
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push(TABLE[(n & 0x3F) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = (bytes[i] as u32) << 16;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
    out
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

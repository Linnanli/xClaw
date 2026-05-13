//! IPC protocol for `dasclaw-sandbox-resource-launcher.exe` (Slice B1 of ADR-131).
//!
//! ## 角色
//!
//! 这个模块定义 ADR-131 决议的 **side-by-side wrapper** 中的进程间协议
//! （详见 [`docs/plans/architecture-refactor/adr-131-windows-job-object-resource-limits-wrapper.md`](../../../docs/plans/architecture-refactor/adr-131-windows-job-object-resource-limits-wrapper.md)）：
//!
//! - adapter（`WindowsRestrictedTokenSandbox::execute`）spawn launcher binary
//! - 把 [`LauncherRequest`] 用 JSON 写到子进程 stdin
//! - launcher 在自身进程上创建 outer Job Object（Slice B2）→ 设资源限制 →
//!   调 `dasclaw_sandbox_windows::run_windows_sandbox_capture`（不在本模块）
//! - launcher 把 [`LauncherResponse`] 用 JSON 写回 stdout
//! - adapter 读 stdout 反序列化 → 转 `std::process::Output`
//!
//! ## 平台
//!
//! 本模块**跨平台**（不 cfg-gate），便于 macOS / Linux 本地单元测试 IPC 数据
//! 结构 round-trip。真实使用只在 Windows binary 与 Windows adapter 之间发生。
//!
//! ## 二进制 vs 文本
//!
//! `stdout_b64` / `stderr_b64` 用 base64 编码原始字节，避免 JSON 字符串对
//! 非-UTF-8 输出（Windows 原生 GBK / UTF-16 命令）的损失。adapter 解码后
//! 回填 `std::process::Output { stdout: Vec<u8>, stderr: Vec<u8> }`。
//!
//! ## 协议版本
//!
//! [`LauncherRequest::protocol_version`] / [`LauncherResponse::protocol_version`]
//! 字段固定为 [`PROTOCOL_VERSION`]。launcher 收到不匹配的版本会立即 fail
//! 并回 `LauncherResponse::error_protocol_mismatch`，不尝试继续。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 当前协议版本。bump 时同时更新 launcher binary 的解析逻辑。
pub const PROTOCOL_VERSION: u32 = 1;

/// adapter → launcher 的请求。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LauncherRequest {
    /// 协议版本。固定为 [`PROTOCOL_VERSION`]。
    pub protocol_version: u32,

    /// 要执行的命令（含 program）。`argv[0]` 是 program，`argv[1..]` 是参数。
    pub argv: Vec<String>,

    /// 工作目录。launcher 直接传给 `run_windows_sandbox_capture`。
    pub cwd: PathBuf,

    /// 环境变量。launcher 直接传给 `run_windows_sandbox_capture`。
    pub env: HashMap<String, String>,

    /// dasclaw_home（DPAPI key / setup state 根目录）。
    pub dasclaw_home: PathBuf,

    /// 序列化后的上游 `windows_sandbox::SandboxPolicy` JSON。
    /// adapter 已经做完 `SandboxBackendConfig → UpstreamPolicy` 的转换。
    pub policy_json: String,

    /// 外层 Job Object 资源限制（None = 不施加 / 兼容现状）。
    /// launcher 收到 Some(...) 时调 Slice B2 的 `JobObject::create_with_limits`。
    pub outer_limits: Option<OuterJobLimitsWire>,

    /// 是否使用 Alternate Desktop（透传上游 API）。
    pub use_private_desktop: bool,

    /// 额外的 deny-write 路径列表 — 即 [`crate::SandboxBackendConfig::read_only_subpaths`]
    /// 的 wire 形式（ADR-141 §3 PR-W3 / OQ-W3-2 sign-off 2026-05-11）。
    ///
    /// launcher 收到非空列表时改调
    /// `dasclaw_sandbox_windows::run_windows_sandbox_capture_with_extra_deny_write_paths`，
    /// 把这些路径作为 `additional_deny_write_paths` 透传给 upstream，upstream
    /// 再通过 `acl::add_deny_write_ace` 下发 Win32 DACL DENY entries。
    ///
    /// 空 Vec = 维持 Slice B1 的旧行为（调 `run_windows_sandbox_capture`），
    /// 协议向后兼容：旧 launcher 反序列化新 request 时 serde 的 `#[serde(default)]`
    /// 兜底为空 Vec。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_deny_write_paths: Vec<PathBuf>,

    /// Issue #462 (B4-7) — select elevated Windows sandbox backend.
    ///
    /// - `false` (default; old-launcher wire compat) → launcher calls
    ///   `dasclaw_sandbox_windows::run_windows_sandbox_capture_with_extra_deny_write_paths`
    ///   (restricted-token / default / weaker isolation, no admin setup
    ///   needed at run time).
    /// - `true` → launcher calls
    ///   `dasclaw_sandbox_windows::run_windows_sandbox_capture_elevated`
    ///   (full elevated isolation: per-user firewall + dedicated logon
    ///   user; requires `dasclaw-sandbox-setup.exe` to have completed).
    ///
    /// Decided in the adapter by
    /// [`crate::windows_dispatch::windows_sandbox_uses_elevated_backend`].
    /// `#[serde(default)]` keeps the wire backward-compatible: old launcher
    /// binaries decoding a new request just see `false`, matching the
    /// pre-#462 behaviour.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub use_elevated_backend: bool,
}

/// `OuterJobLimits` 的 wire 形式。
///
/// 与 [`crate::windows::job_object::OuterJobLimits`] 字段一致，但**跨平台**
/// 可序列化（`OuterJobLimits` 本身在 macOS / Linux 不可见，因为它依赖
/// windows-sys 类型）。
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OuterJobLimitsWire {
    pub max_process_memory_bytes: Option<u64>,
    pub max_job_memory_bytes: Option<u64>,
    pub max_active_processes: Option<u32>,
}

/// launcher → adapter 的响应。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LauncherResponse {
    pub protocol_version: u32,

    /// 子进程退出码（i32 透传上游 `CaptureResult::exit_code`）。
    /// launcher 自身错误时为 -1。
    pub exit_code: i32,

    /// stdout 字节（base64）。空字符串表示空输出。
    pub stdout_b64: String,
    pub stderr_b64: String,

    /// launcher 自身错误时的人类可读详情。`Some(_)` ↔ `exit_code == -1`。
    pub error: Option<String>,
}

impl LauncherResponse {
    /// 构造一个表示 launcher 自身错误的响应。
    pub fn error(detail: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            exit_code: -1,
            stdout_b64: String::new(),
            stderr_b64: String::new(),
            error: Some(detail.into()),
        }
    }

    /// 构造一个协议版本不匹配的响应。
    pub fn error_protocol_mismatch(got: u32, expected: u32) -> Self {
        Self::error(format!(
            "launcher protocol version mismatch: got {got}, expected {expected}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> LauncherRequest {
        let mut env = HashMap::new();
        env.insert("PATH".into(), r"C:\Windows\System32".into());
        env.insert("DASCLAW_HOME".into(), r"C:\Users\u\.dasclaw".into());

        LauncherRequest {
            protocol_version: PROTOCOL_VERSION,
            argv: vec![
                "powershell.exe".into(),
                "-NoProfile".into(),
                "Get-Date".into(),
            ],
            cwd: PathBuf::from(r"C:\Users\u\workspace"),
            env,
            dasclaw_home: PathBuf::from(r"C:\Users\u\.dasclaw"),
            policy_json: r#"{"ReadOnly":{"network_access":false}}"#.into(),
            outer_limits: Some(OuterJobLimitsWire {
                max_process_memory_bytes: Some(2 * 1024 * 1024 * 1024),
                max_job_memory_bytes: None,
                max_active_processes: Some(64),
            }),
            use_private_desktop: false,
            additional_deny_write_paths: vec![],
            use_elevated_backend: false,
        }
    }

    #[test]
    fn request_round_trips_through_json() {
        let req = sample_request();
        let json = serde_json::to_string(&req).expect("serialize");
        let back: LauncherRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req, back);
    }

    #[test]
    fn request_with_no_outer_limits_round_trips() {
        let mut req = sample_request();
        req.outer_limits = None;
        let json = serde_json::to_string(&req).expect("serialize");
        let back: LauncherRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req, back);
    }

    #[test]
    fn request_with_additional_deny_write_paths_round_trips() {
        // ADR-141 §3 PR-W3 / OQ-W3-2 contract: deny paths must survive the
        // adapter ↔ launcher IPC boundary intact so that upstream
        // `run_windows_sandbox_capture_with_extra_deny_write_paths` can DENY
        // them at the Win32 DACL layer.
        let mut req = sample_request();
        req.additional_deny_write_paths = vec![
            PathBuf::from(r"C:\Users\u\workspace\.git"),
            PathBuf::from(r"C:\Users\u\workspace\.dasclaw"),
            PathBuf::from(r"C:\Users\u\workspace\.codex"),
        ];
        let json = serde_json::to_string(&req).expect("serialize");
        let back: LauncherRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req, back);
        assert_eq!(back.additional_deny_write_paths.len(), 3);
    }

    #[test]
    fn request_omits_additional_deny_write_paths_when_empty() {
        // skip_serializing_if keeps the wire payload backwards-compatible
        // with launchers that predate ADR-141 PR-W3.
        let req = sample_request();
        assert!(req.additional_deny_write_paths.is_empty());
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(
            !json.contains("additional_deny_write_paths"),
            "empty list must be omitted from wire payload, got: {json}",
        );
    }

    #[test]
    fn request_legacy_json_without_deny_paths_field_deserializes() {
        // Forward-compat: an older adapter that does not yet emit the
        // `additional_deny_write_paths` field must still be parseable.
        let legacy_json = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION,
            "argv": ["cmd.exe"],
            "cwd": "C:\\workspace",
            "env": {},
            "dasclaw_home": "C:\\Users\\u\\.dasclaw",
            "policy_json": "{\"ReadOnly\":{\"network_access\":false}}",
            "outer_limits": null,
            "use_private_desktop": false,
        })
        .to_string();
        let parsed: LauncherRequest =
            serde_json::from_str(&legacy_json).expect("legacy json must deserialize");
        assert!(parsed.additional_deny_write_paths.is_empty());
    }

    #[test]
    fn response_round_trips_through_json() {
        let resp = LauncherResponse {
            protocol_version: PROTOCOL_VERSION,
            exit_code: 0,
            stdout_b64: "aGVsbG8=".into(), // "hello"
            stderr_b64: String::new(),
            error: None,
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        let back: LauncherResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(resp, back);
    }

    #[test]
    fn response_error_helper_marks_exit_code_minus_one() {
        let resp = LauncherResponse::error("CreateJobObjectW failed: GetLastError=5");
        assert_eq!(resp.exit_code, -1);
        assert!(resp.error.is_some());
        assert!(resp.error.as_ref().unwrap().contains("CreateJobObjectW"));
    }

    #[test]
    fn response_protocol_mismatch_helper_describes_versions() {
        let resp = LauncherResponse::error_protocol_mismatch(2, PROTOCOL_VERSION);
        assert_eq!(resp.exit_code, -1);
        assert_eq!(resp.protocol_version, PROTOCOL_VERSION);
        let msg = resp.error.unwrap();
        assert!(msg.contains("got 2"));
        assert!(msg.contains(&format!("expected {PROTOCOL_VERSION}")));
    }

    #[test]
    fn protocol_version_is_one() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }
}

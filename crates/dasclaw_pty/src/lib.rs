//! `dasclaw_pty` — a minimal, portable PTY abstraction (W2.5).
//!
//! ## 设计目标
//!
//! 为 W3+ 的 ShellTool / hooks BeforeToolCall 提供一个**与具体 PTY 后端解耦**
//! 的 trait + 默认实装。当前默认实装基于 `portable-pty`（macOS/Linux 用
//! `posix_openpt`，Windows 用 ConPTY），覆盖 W2 验收要求的
//! `spawn / write / read / resize / kill / wait_for_exit`。
//!
//! ## 范围
//!
//! - **同步 API**：`PtyChild` 暴露阻塞读写接口。异步 wrapper 留 W3
//!   `dasclaw_hooks::BeforeToolCall` 真正接入时再加，避免现在锁死 tokio 版本。
//! - **不做**：进程组 kill（codex `process_group.rs` 184 LOC 处理 shell
//!   子进程逃逸，本切片只 kill 直接子进程，shell 子进程逃逸由
//!   `dasclaw_sandbox` seccomp/Seatbelt 拦截）。
//! - **不做**：信号转发的 SIGINT/SIGTERM 链路（W3 ShellTool 集成时按
//!   实际工具策略接）。
//! - **Windows**：靠 `portable-pty` ConPTY 后端原生支持，无需自实现。
//!
//! ## 与 dasclaw_sandbox 的关系
//!
//! `Pty` trait 不耦合 sandbox。调用方按需求叠加：
//!
//! ```text
//! sandbox.execute(cmd)   ←  非交互工具（cargo test 等一次性命令）
//! pty.spawn(opts)        ←  交互工具（bash / python REPL / ssh）
//! ```
//!
//! 真要把交互式 shell 跑在 sandbox 里（W3 ShellTool 默认策略），调用方
//! 自行用 sandbox.execute 包装 PTY child 的 program 路径。本 crate 不
//! 强制耦合避免循环依赖。

#![deny(missing_docs)]

mod portable;

#[cfg(test)]
mod tests;

pub use portable::PortablePtyBackend;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// PTY 终端尺寸。所有字段都使用 `u16`，与 `portable-pty` / POSIX 一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtySize {
    /// 行数（字符）。
    pub rows: u16,
    /// 列数（字符）。
    pub cols: u16,
    /// 像素宽度（可选；现代终端常用 0）。
    pub pixel_width: u16,
    /// 像素高度（可选；现代终端常用 0）。
    pub pixel_height: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Spawn 一个 PTY child 时需要的参数集合。
#[derive(Debug, Clone)]
pub struct PtySpawnOptions {
    /// 可执行文件路径或 PATH 上的程序名。
    pub program: String,
    /// argv（不含 program 自身）。
    pub args: Vec<String>,
    /// 工作目录。`None` = 继承父进程 cwd。
    pub cwd: Option<PathBuf>,
    /// 环境变量。**不会**继承父进程 env — 调用方必须显式提供。
    /// 这与 `dasclaw_sandbox` 的 env 透传策略一致，防止意外泄漏。
    pub env: HashMap<String, String>,
    /// 初始终端尺寸。
    pub size: PtySize,
}

impl PtySpawnOptions {
    /// 用最小参数构造：program + 默认 24x80 + 空 env。
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            size: PtySize::default(),
        }
    }

    /// 链式追加 args。
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// 链式设置 cwd。
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// 链式追加单个 env 变量。
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// 链式设置尺寸。
    pub fn with_size(mut self, size: PtySize) -> Self {
        self.size = size;
        self
    }
}

/// PTY 错误类型。
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    /// `portable_pty::PtySystem::openpty` 失败（fd 耗尽、权限等）。
    #[error("failed to open pty: {0}")]
    Open(String),

    /// 启动子进程失败（program 不存在、权限不足）。
    #[error("failed to spawn child: {0}")]
    Spawn(String),

    /// 读 PTY master 失败。
    #[error("pty read error: {0}")]
    Read(#[source] std::io::Error),

    /// 写 PTY master 失败。
    #[error("pty write error: {0}")]
    Write(#[source] std::io::Error),

    /// 调整尺寸失败。
    #[error("pty resize error: {0}")]
    Resize(String),

    /// kill child 失败（进程已退出 = NotFound 时调用方应当作幂等成功）。
    #[error("pty kill error: {0}")]
    Kill(#[source] std::io::Error),

    /// `wait_for_exit` 超时。
    #[error("pty wait timed out after {0:?}")]
    WaitTimeout(Duration),

    /// child 状态不可查询（portable-pty `try_wait` 返回错误）。
    #[error("pty wait error: {0}")]
    Wait(String),
}

/// 退出状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtyExitStatus {
    /// 进程正常退出，附带 exit code（约定 Unix 0=ok，非 0=失败）。
    Exited(i32),
    /// 进程被信号杀死（仅 Unix 有意义）。
    Signaled,
    /// 进程仍在运行。
    Running,
}

impl PtyExitStatus {
    /// 是否已完成（不再 Running）。
    pub fn is_finished(&self) -> bool {
        !matches!(self, PtyExitStatus::Running)
    }

    /// 是否成功退出（Exited(0)）。
    pub fn is_success(&self) -> bool {
        matches!(self, PtyExitStatus::Exited(0))
    }
}

/// PTY 后端 trait。**对外**只暴露 `spawn`，其余动作通过 `PtyChild` 进行。
pub trait Pty: Send + Sync {
    /// 启动一个进程，attach 到 PTY，返回可控的 child handle。
    fn spawn(&self, options: PtySpawnOptions) -> Result<Box<dyn PtyChild>, PtyError>;
}

/// 已 spawn 的 PTY child 控制句柄。
///
/// 所有方法都是同步阻塞的；上层异步 runtime（tokio 等）应包到
/// `spawn_blocking` 里。
pub trait PtyChild: Send {
    /// 向 child stdin 写字节。返回实际写入字节数（短写需上层循环）。
    fn write(&mut self, buf: &[u8]) -> Result<usize, PtyError>;

    /// 从 child stdout/stderr（PTY 不区分）读字节。EOF 返回 `Ok(0)`。
    /// 阻塞直到至少 1 字节或 EOF。
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError>;

    /// 调整终端尺寸。子进程通常会收到 SIGWINCH（Unix）。
    fn resize(&mut self, size: PtySize) -> Result<(), PtyError>;

    /// 强制 kill child（SIGKILL on Unix / TerminateProcess on Windows）。
    /// 如果 child 已退出，**返回 Ok(())**（幂等）。
    fn kill(&mut self) -> Result<(), PtyError>;

    /// 非阻塞查询 child 当前状态。
    fn try_wait(&mut self) -> Result<PtyExitStatus, PtyError>;

    /// 阻塞等待 child 退出，最多 `timeout`。`None` = 无限等。
    /// 超时时**不**自动 kill；调用方决定是否 kill 后再等。
    fn wait_for_exit(&mut self, timeout: Option<Duration>) -> Result<PtyExitStatus, PtyError>;
}

/// 工厂：返回当前平台默认 PTY 后端。
///
/// 默认 = `PortablePtyBackend`（macOS/Linux: posix_openpt; Windows: ConPTY）。
pub fn default_backend() -> PortablePtyBackend {
    PortablePtyBackend::new()
}

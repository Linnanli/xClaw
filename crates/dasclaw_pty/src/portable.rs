//! Default PTY backend backed by the `portable-pty` crate.
//!
//! Wraps `portable_pty::native_pty_system()` so that the rest of `dasclaw_pty`
//! can program against the [`Pty`] / [`PtyChild`] traits instead of the
//! concrete `portable_pty::*` types. Keeps Windows/macOS/Linux on a single
//! code path (ConPTY / posix_openpt) without per-platform branches in the
//! call sites.

use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtyPair, PtySystem};

use crate::{Pty, PtyChild, PtyError, PtyExitStatus, PtySize, PtySpawnOptions};

/// 默认实装：基于 `portable-pty`。
///
/// 多线程安全；`spawn` 内部每次都新建 `PtySystem`，无共享可变状态。
#[derive(Default, Debug)]
pub struct PortablePtyBackend;

impl PortablePtyBackend {
    /// 构造默认后端。
    pub fn new() -> Self {
        Self
    }
}

impl Pty for PortablePtyBackend {
    fn spawn(&self, options: PtySpawnOptions) -> Result<Box<dyn PtyChild>, PtyError> {
        let pty_system: Box<dyn PtySystem> = native_pty_system();
        let pair: PtyPair = pty_system
            .openpty(to_portable_size(options.size))
            .map_err(|e| PtyError::Open(e.to_string()))?;

        let mut cmd = CommandBuilder::new(&options.program);
        cmd.args(&options.args);
        if let Some(cwd) = options.cwd.as_ref() {
            cmd.cwd(cwd);
        }
        // env_clear 等价：portable-pty 默认不继承 env，调用方显式设置即可。
        // 这里不主动调 env_clear() 因为 portable-pty 0.8 没有该 API；它
        // 默认就只用 CommandBuilder 上设的 env，无父进程继承。
        for (k, v) in &options.env {
            cmd.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Spawn(e.to_string()))?;

        // master 拆出 reader / writer。reader 是 try_clone_reader（可读端），
        // writer 是 take_writer（独占写端，take 后 master 不再可写）。
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Open(format!("clone reader: {e}")))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Open(format!("take writer: {e}")))?;

        // 持有 master 用于 resize；slave 显式 drop 以释放 fd（child 已 fork）。
        drop(pair.slave);

        Ok(Box::new(PortablePtyChild {
            master: pair.master,
            reader,
            writer,
            child,
        }))
    }
}

struct PortablePtyChild {
    master: Box<dyn MasterPty + Send>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl PtyChild for PortablePtyChild {
    fn write(&mut self, buf: &[u8]) -> Result<usize, PtyError> {
        self.writer.write(buf).map_err(PtyError::Write)
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        self.reader.read(buf).map_err(PtyError::Read)
    }

    fn resize(&mut self, size: PtySize) -> Result<(), PtyError> {
        self.master
            .resize(to_portable_size(size))
            .map_err(|e| PtyError::Resize(e.to_string()))
    }

    fn kill(&mut self) -> Result<(), PtyError> {
        // 先看 child 是否已自然退出。已退出时 kill(2) 会返回 ESRCH（Unix）
        // 或 invalid handle（Windows），跨平台错误 kind 不一致。最干净的
        // 幂等实现是：已退出 → 直接 Ok(())，不触发 syscall。
        if let Ok(Some(_)) = self.child.try_wait() {
            return Ok(());
        }
        match self.child.kill() {
            Ok(()) => Ok(()),
            // 边界情况：try_wait 之后 / kill 之前 child 在 race 窗口内退出。
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) if e.raw_os_error() == Some(3) /* ESRCH on Unix */ => Ok(()),
            Err(e) => Err(PtyError::Kill(e)),
        }
    }

    fn try_wait(&mut self) -> Result<PtyExitStatus, PtyError> {
        match self.child.try_wait() {
            Ok(None) => Ok(PtyExitStatus::Running),
            Ok(Some(status)) => Ok(map_exit_status(status)),
            Err(e) => Err(PtyError::Wait(e.to_string())),
        }
    }

    fn wait_for_exit(&mut self, timeout: Option<Duration>) -> Result<PtyExitStatus, PtyError> {
        // portable-pty 没有原生 timeout wait API；用 try_wait + 轮询模拟。
        // 轮询间隔 25ms 在交互响应（< 一帧）和 CPU 占用间取中庸值。
        const POLL_INTERVAL: Duration = Duration::from_millis(25);
        let deadline = timeout.map(|t| Instant::now() + t);

        loop {
            match self.try_wait()? {
                PtyExitStatus::Running => {}
                done => return Ok(done),
            }

            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    return Err(PtyError::WaitTimeout(
                        timeout.expect("deadline implies timeout"),
                    ));
                }
            }

            thread::sleep(POLL_INTERVAL);
        }
    }
}

fn to_portable_size(size: PtySize) -> portable_pty::PtySize {
    portable_pty::PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: size.pixel_width,
        pixel_height: size.pixel_height,
    }
}

fn map_exit_status(status: portable_pty::ExitStatus) -> PtyExitStatus {
    // portable-pty 抽象层不区分 "exit code N" 与 "killed by signal"，统一用
    // u32 exit_code 暴露。我们保持同一映射：success() → Exited(0)，否则
    // Exited(exit_code as i32)。`PtyExitStatus::Signaled` 留给未来更精细的
    // 后端（例如直接走 std::process::ExitStatus 的本地 backend）使用，本
    // backend 不会产出该值。
    if status.success() {
        PtyExitStatus::Exited(0)
    } else {
        PtyExitStatus::Exited(status.exit_code() as i32)
    }
}

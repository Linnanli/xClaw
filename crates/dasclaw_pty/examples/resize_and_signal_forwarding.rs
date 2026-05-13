//! `resize_and_signal_forwarding` — 演示 `dasclaw_pty` 的 resize + Ctrl-C 信号
//! 转发调用模板（Unix-only：依赖 POSIX `stty` 与 line-discipline INTR 翻译）。
//!
//! 跑这个例子：
//!
//! ```bash
//! cargo run -p dasclaw_pty --example resize_and_signal_forwarding
//! ```
//!
//! 预期输出（macOS / Linux）：
//!
//! ```text
//! [1/6] spawning bash -i with PtySize { rows: 24, cols: 80 }
//! [2/6] asking bash to record initial stty size to <tmpdir>/dasclaw_pty_b6_3.log
//! [3/6] resizing to PtySize { rows: 40, cols: 120 }
//! [4/6] asking bash to record post-resize stty size to the same file
//! [5/6] sending Ctrl-C (\x03); line discipline turns it into SIGINT for bash
//! [6/6] sending `exit` and waiting up to 5s; status = Exited(0)
//! recorded sizes:
//!   24 80
//!   40 120
//! ```
//!
//! ## 接入指南
//!
//! 这是 W3 `ShellTool` 集成 `dasclaw_pty` 时的最小调用模板：
//! - **resize**：UI 端的 terminal resize 事件 → 调 `child.resize(new_size)`。
//!   Unix 上 portable-pty 自动发 SIGWINCH 给前台进程组，`stty size` 立刻反映新值。
//! - **信号转发**：UI 端的 Ctrl-C 事件 → 直接 `child.write(b"\x03")`。PTY 把
//!   `\x03` 当作 INTR 字符送给 line discipline，再由 kernel 翻译成 SIGINT 投递
//!   给前台进程组。**不**要用 `child.kill()`——那走 SIGKILL，跳过 line discipline，
//!   破坏 readline / 嵌套 TUI 的交互。
//!
//! ## 验证策略：写文件 + 读文件，而不是读 PTY master
//!
//! 这例子刻意 **不** 从 PTY master 读输出来验证 stty 的回答。原因：
//! `PtyChild::read` 在 `portable-pty` 实装下是 **阻塞** 的（无 `set_nonblocking`
//! API），不分线程做超时控制就会卡死。而例子的本意是展示 _调用模板_，不是
//! 教人怎么解 PTY 阻塞读——后者属于 W3 ShellTool 自己的事（mio / async runtime）。
//! 所以这里让 bash 把 `stty size` 输出 **重定向到普通文件**，主线程在 child 退出
//! 后读文件做断言。简单可观测。
//!
//! ## 为什么 Unix-only
//!
//! - `stty` 是 POSIX 工具，Windows 没有；ConPTY 路径下要读尺寸得调
//!   Win32 `GetConsoleScreenBufferInfo`，与本例展示的"用户态 stty 验证"目的不符。
//! - Ctrl-C 在 ConPTY 下走 Console Control Event，不是 `\x03` line discipline
//!   翻译；行为本质不同，混在一个例子里只会增加噪音。
//! - Windows ConPTY 的 resize / 信号示例后续随 W3 Windows ShellTool 接入一起补。

#![cfg(unix)]

use std::env;
use std::fs;
use std::thread::sleep;
use std::time::Duration;

use dasclaw_pty::{default_backend, Pty, PtyExitStatus, PtySize, PtySpawnOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = default_backend();

    let initial_size = PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };

    // 落盘点：放在 std::env::temp_dir() 下，跨 macOS / Linux 都能找到。
    let log_path = env::temp_dir().join("dasclaw_pty_b6_3.log");
    // 先清掉残留，避免误读上一次运行的数据。
    let _ = fs::remove_file(&log_path);

    println!("[1/6] spawning bash -i with {initial_size:?}");
    // env 必须显式提供：PtySpawnOptions 文档约定不继承父进程 env。
    let mut child = backend.spawn(
        PtySpawnOptions::new("/bin/bash")
            .with_args(["--noprofile", "--norc", "-i"])
            .with_env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .with_env("TERM", "xterm-256color")
            .with_env("HISTFILE", "/dev/null")
            .with_size(initial_size),
    )?;

    // 给 bash 一点时间打印 prompt 完成初始化。
    sleep(Duration::from_millis(300));

    // 关键：让 bash 把它自己后续的 stdout / stderr 重定向到 /dev/null。
    // 否则 PTY master 端的内核环形缓冲区会被 bash 的 echo / prompt 填满，
    // 主线程不读 master 时 bash 的 write 会阻塞，最终 `exit` 无法正常退出，
    // `wait_for_exit` 超时。这是 example 不接 mio / async runtime 的代价。
    // disable echo 同时减少回写量，并把 fd 1/2 重定向走。
    child.write(b"stty -echo; exec >/dev/null 2>&1\n")?;
    sleep(Duration::from_millis(200));

    println!(
        "[2/6] asking bash to record initial stty size to {}",
        log_path.display()
    );
    child.write(format!("stty size >> {}\n", log_path.display()).as_bytes())?;
    sleep(Duration::from_millis(200));

    let new_size = PtySize {
        rows: 40,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    };
    println!("[3/6] resizing to {new_size:?}");
    child.resize(new_size)?;
    // SIGWINCH 已发出；让 bash 处理一下再问 stty。
    sleep(Duration::from_millis(200));

    println!("[4/6] asking bash to record post-resize stty size to the same file");
    child.write(format!("stty size >> {}\n", log_path.display()).as_bytes())?;
    sleep(Duration::from_millis(200));

    println!("[5/6] sending Ctrl-C (\\x03); line discipline turns it into SIGINT for bash");
    child.write(b"\x03")?;
    sleep(Duration::from_millis(200));

    println!("[6/6] sending `exit` and waiting up to 5s");
    child.write(b"exit\n")?;
    let status = child.wait_for_exit(Some(Duration::from_secs(5)))?;

    let log = fs::read_to_string(&log_path).unwrap_or_default();
    println!("        status = {status:?}");
    println!("recorded sizes:");
    for line in log.lines() {
        println!("  {line}");
    }

    // 断言：必须看到两行 stty size 记录，且 resize 真生效（行数 / 列数变了）。
    let lines: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return Err(format!(
            "expected at least 2 stty-size records in {}, got {}: {:?}",
            log_path.display(),
            lines.len(),
            lines
        )
        .into());
    }
    let first = lines[0].trim();
    let second = lines[1].trim();
    if first == second {
        return Err(format!(
            "resize did not take effect; both records report `{first}`. \
             expected first=`24 80`, second=`40 120`"
        )
        .into());
    }

    match status {
        // 接受任何 Exited(_)：`exit\n` 不带参数会沿用 `$?`，而前一步刚发过
        // Ctrl-C，bash 把 SIGINT 记成 130，于是 `$?` 是非 0。这是符合预期
        // 的副作用，不是 bug。关键是 child 走到了 Exited（没卡死）。
        PtyExitStatus::Exited(_) => Ok(()),
        other => Err(format!("unexpected exit status: {other:?}").into()),
    }
}

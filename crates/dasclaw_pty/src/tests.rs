//! Integration + unit tests for [`crate::PortablePtyBackend`].
//!
//! 真集成测试只在 Unix 上跑（依赖 `/bin/echo` `/bin/cat` `/bin/sh`）。
//! Windows CI 由 portable-pty 上游测试覆盖；本 crate 在 Windows 上仅
//! 验证编译与构造路径，不验真实 spawn — 等 W2.4 Windows sandbox 成型后
//! 一起补 ConPTY smoke 测试。

use std::io::{Read, Write};
use std::time::Duration;

use crate::{
    default_backend, PortablePtyBackend, Pty, PtyExitStatus, PtySize, PtySpawnOptions, StreamingPty,
};

#[test]
fn pty_size_default_is_24x80() {
    let s = PtySize::default();
    assert_eq!(s.rows, 24);
    assert_eq!(s.cols, 80);
    assert_eq!(s.pixel_width, 0);
    assert_eq!(s.pixel_height, 0);
}

#[test]
fn spawn_options_builder_chain() {
    let opts = PtySpawnOptions::new("/bin/echo")
        .with_args(["hello", "world"])
        .with_cwd("/tmp")
        .with_env("FOO", "bar")
        .with_size(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        });

    assert_eq!(opts.program, "/bin/echo");
    assert_eq!(opts.args, vec!["hello", "world"]);
    assert_eq!(opts.cwd.as_deref(), Some(std::path::Path::new("/tmp")));
    assert_eq!(opts.env.get("FOO").map(String::as_str), Some("bar"));
    assert_eq!(opts.size.rows, 40);
    assert_eq!(opts.size.cols, 120);
}

#[test]
fn exit_status_helpers() {
    assert!(PtyExitStatus::Exited(0).is_finished());
    assert!(PtyExitStatus::Exited(0).is_success());
    assert!(PtyExitStatus::Exited(1).is_finished());
    assert!(!PtyExitStatus::Exited(1).is_success());
    assert!(PtyExitStatus::Signaled.is_finished());
    assert!(!PtyExitStatus::Signaled.is_success());
    assert!(!PtyExitStatus::Running.is_finished());
    assert!(!PtyExitStatus::Running.is_success());
}

#[test]
fn default_backend_is_portable() {
    // 仅验证工厂能构造，不 spawn — 多平台都安全跑。
    let _backend: PortablePtyBackend = default_backend();
}

// ---------- Unix-only real spawn tests ----------

#[cfg(unix)]
#[test]
fn echo_runs_to_completion_and_reports_exit_zero() {
    let backend = PortablePtyBackend::new();
    let mut child = backend
        .spawn(PtySpawnOptions::new("/bin/echo").with_args(["dasclaw_pty:hello"]))
        .expect("spawn /bin/echo");

    let status = child
        .wait_for_exit(Some(Duration::from_secs(5)))
        .expect("echo should exit within 5s");
    assert_eq!(status, PtyExitStatus::Exited(0));
    assert!(status.is_success());
}

#[cfg(unix)]
#[test]
fn child_stdout_is_readable_via_pty() {
    let backend = PortablePtyBackend::new();
    let mut child = backend
        .spawn(PtySpawnOptions::new("/bin/echo").with_args(["dasclaw_pty:greeting"]))
        .expect("spawn /bin/echo");

    // PTY 在 child 退出 + slave fd 关闭后，master 读会立即返回 EIO。
    // 所以必须**先**循环读直到看到 sentinel，再 wait。echo 通常在
    // < 50ms 内输出完毕，2 秒 deadline 足够。
    let mut buf = [0u8; 256];
    let mut collected = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        match child.read(&mut buf) {
            Ok(0) => break, // 干净 EOF
            Ok(n) => {
                collected.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&collected).contains("dasclaw_pty:greeting") {
                    break;
                }
            }
            Err(_) => break, // PTY EOF on Unix 表现为 EIO
        }
    }

    let _ = child.wait_for_exit(Some(Duration::from_secs(2)));

    let out = String::from_utf8_lossy(&collected);
    assert!(
        out.contains("dasclaw_pty:greeting"),
        "expected greeting in PTY output, got {out:?}",
    );
}

#[cfg(unix)]
#[test]
fn write_to_cat_is_echoed_back() {
    let backend = PortablePtyBackend::new();
    let mut child = backend
        .spawn(PtySpawnOptions::new("/bin/cat"))
        .expect("spawn /bin/cat");

    // 写入一行；cat 在 PTY 模式下默认 line buffer + echo on，所以单次
    // 写应该立即收到 echo + 输出双份。我们只断言至少出现一次 sentinel。
    let line = b"sentinel\n";
    let mut written = 0;
    while written < line.len() {
        let n = child.write(&line[written..]).expect("write to cat");
        assert!(n > 0, "PTY write must make progress");
        written += n;
    }

    // 读最多 1 秒，找 sentinel 子串就停。
    let mut buf = [0u8; 256];
    let mut collected = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        match child.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                collected.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&collected).contains("sentinel") {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    // 完事 kill cat（它在等更多 stdin）。
    child.kill().expect("kill cat");
    let _ = child.wait_for_exit(Some(Duration::from_secs(2)));

    let observed = String::from_utf8_lossy(&collected);
    assert!(
        observed.contains("sentinel"),
        "expected sentinel echoed back, got {observed:?}",
    );
}

#[cfg(unix)]
#[test]
fn split_process_allows_read_write_and_wait_without_shared_child_lock() {
    let backend = PortablePtyBackend::new();
    let process = backend
        .spawn_process(
            PtySpawnOptions::new("/bin/sh")
                .with_args(["-c", "printf ready; read line; printf \"got:$line\""]),
        )
        .expect("spawn split process");

    let mut reader = process.reader;
    let mut writer = process.writer;
    let mut control = process.control;

    let reader_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 256];
        let mut collected = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => collected.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }
        collected
    });

    writer.write_all(b"hello\n").expect("write to split writer");
    drop(writer);

    let status = control
        .wait_for_exit(Some(Duration::from_secs(5)))
        .expect("split process exits");
    let output = reader_thread.join().expect("reader thread joins");
    let output = String::from_utf8_lossy(&output);

    assert!(status.is_success(), "expected success, got {status:?}");
    assert!(
        output.contains("ready"),
        "expected ready in PTY output, got {output:?}",
    );
    assert!(
        output.contains("got:hello"),
        "expected got:hello in PTY output, got {output:?}",
    );
}

#[cfg(unix)]
#[test]
fn resize_does_not_error_on_running_child() {
    let backend = PortablePtyBackend::new();
    let mut child = backend
        .spawn(PtySpawnOptions::new("/bin/cat"))
        .expect("spawn /bin/cat");

    // 三档常见尺寸都应该能 resize 成功。
    for size in [
        PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        },
        PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        },
        PtySize {
            rows: 80,
            cols: 200,
            pixel_width: 1920,
            pixel_height: 1080,
        },
    ] {
        child.resize(size).expect("resize should succeed");
    }

    child.kill().expect("kill cat");
    let _ = child.wait_for_exit(Some(Duration::from_secs(2)));
}

#[cfg(unix)]
#[test]
fn kill_then_wait_reports_finished() {
    let backend = PortablePtyBackend::new();
    let mut child = backend
        .spawn(PtySpawnOptions::new("/bin/sh").with_args(["-c", "sleep 30"]))
        .expect("spawn sleeping shell");

    // 进程应该还在跑。
    assert_eq!(child.try_wait().expect("try_wait"), PtyExitStatus::Running);

    child.kill().expect("kill running child");

    // wait 应该在 SIGKILL 后很快返回，不超过 2 秒。
    let status = child
        .wait_for_exit(Some(Duration::from_secs(2)))
        .expect("wait after kill");
    assert!(
        status.is_finished(),
        "kill should make child finished, got {status:?}"
    );
}

#[cfg(unix)]
#[test]
fn kill_is_idempotent_after_natural_exit() {
    let backend = PortablePtyBackend::new();
    let mut child = backend
        .spawn(PtySpawnOptions::new("/bin/echo").with_args(["bye"]))
        .expect("spawn echo");

    let _ = child
        .wait_for_exit(Some(Duration::from_secs(5)))
        .expect("echo exits");

    // child 已自然退出；kill 必须返回 Ok(())（不能 panic / 报错）。
    child.kill().expect("kill on already-dead child must be Ok");
    child.kill().expect("second kill must also be Ok");
}

#[cfg(unix)]
#[test]
fn wait_for_exit_times_out_when_child_blocks() {
    let backend = PortablePtyBackend::new();
    let mut child = backend
        .spawn(PtySpawnOptions::new("/bin/sh").with_args(["-c", "sleep 30"]))
        .expect("spawn sleeping shell");

    let result = child.wait_for_exit(Some(Duration::from_millis(150)));
    match result {
        Err(crate::PtyError::WaitTimeout(dur)) => {
            assert_eq!(dur, Duration::from_millis(150));
        }
        other => panic!("expected WaitTimeout, got {other:?}"),
    }

    // 清理：超时后调用方有责任 kill。
    child.kill().expect("kill after timeout");
    let _ = child.wait_for_exit(Some(Duration::from_secs(2)));
}

#[cfg(unix)]
#[test]
fn nonzero_exit_code_is_preserved() {
    let backend = PortablePtyBackend::new();
    let mut child = backend
        .spawn(PtySpawnOptions::new("/bin/sh").with_args(["-c", "exit 42"]))
        .expect("spawn shell with exit 42");

    let status = child
        .wait_for_exit(Some(Duration::from_secs(5)))
        .expect("shell exits");
    assert_eq!(status, PtyExitStatus::Exited(42));
    assert!(!status.is_success());
}

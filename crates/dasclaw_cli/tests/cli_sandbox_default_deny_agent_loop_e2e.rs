//! W6.6b — ADR-153 §1.1 A6 / e12: **binary-level** sandbox default-deny.
//!
//! Companion to `safety_sandbox_default_deny_e2e.rs`. That sibling pins
//! the **library** contract (`run_with_tools_and_hooks` + a custom
//! sandboxed tool executor) on a write attempt under the headless
//! `ReadOnly` policy. This file pins the **shipped `dasclaw` binary**
//! contract: spawn the real CLI via `assert_cmd`, drive it against a
//! `wiremock` OpenAI-Chat-Completions server that asks the agent to run
//! a `shell` tool call that writes to disk, and verify three independent
//! invariants:
//!
//! 1. The binary does **not** panic / crash — `assert_cmd::Assert::success()`
//!    confirms it reached the LLM's second-turn reply and printed it.
//! 2. The sandbox actually **enforced** the deny — the target file does
//!    not exist on disk after the run.
//! 3. The denial signal **propagated to the model loop** — the 2nd
//!    `/chat/completions` request body (containing the prior tool_result)
//!    carries a Seatbelt / sandbox denial keyword. Without this, a buggy
//!    refactor could silently swallow the sandbox error and still
//!    produce a passing `.success()` run.
//!
//! ## Why we don't assert a non-zero exit code (departure from #868)
//!
//! Issue #868 originally asked for "exit code 非零 + stderr 含 sandbox 关键字".
//! Today `dasclaw-cli` exits 0 whenever the **agent loop** completes — tool
//! failures stay inside the loop as `is_error=true` `ToolResult`s and
//! never bubble up to `real_main()`. Forcing a non-zero exit for any
//! tool failure is a separate product decision (would mean every
//! transient tool error kills the binary), tracked outside this slice.
//! The B-route acceptance (file-not-created + denial keyword reaches the
//! model) still locks the e12 invariant end-to-end without coupling
//! this PR to that decision. Negotiation recorded on issue #868.
//!
//! ## Platform gate
//!
//! macOS only, matching the library sibling: Seatbelt is the only fully
//! wired backend in the test image (`/usr/bin/sandbox-exec` is on `PATH`),
//! Linux + Windows are blocked behind #481.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const TEST_API_KEY: &str = "sk-test";

/// Sniffer that records every request body seen by the mock server.
///
/// `wiremock::Respond` is invoked synchronously for every match; we
/// stash the JSON-decoded body in a shared `Vec` so the test can later
/// assert that the 2nd-turn request (which carries the prior
/// `tool_result`) contains the sandbox denial signal.
struct BodySniffer {
    bodies: Arc<Mutex<Vec<serde_json::Value>>>,
    response: serde_json::Value,
}

impl Respond for BodySniffer {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        // `Respond::respond` is invoked synchronously from inside the
        // wiremock dispatcher (which is itself running on a tokio
        // runtime), so we must use a `std::sync::Mutex` here — a
        // `tokio::sync::Mutex::blocking_lock` panics with
        // "Cannot block the current thread from within a runtime."
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&request.body)
            && let Ok(mut guard) = self.bodies.lock()
        {
            guard.push(value);
        }
        ResponseTemplate::new(200).set_body_json(self.response.clone())
    }
}

fn tool_call_body(target_path: &str) -> serde_json::Value {
    // First-turn assistant reply asking the agent to run the `shell` tool
    // with a write command. The path is deliberately a tempdir entry so
    // we can assert post-run that the sandbox prevented the write.
    json!({
        "id": "chatcmpl-cli-sandbox-1",
        "object": "chat.completion",
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_shell_1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": serde_json::to_string(&json!({
                            "command": format!("echo blocked > {target_path}")
                        })).unwrap()
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": { "prompt_tokens": 5, "completion_tokens": 8 }
    })
}

const FINAL_REPLY: &str = "understood: the sandbox refused the write.";

fn final_text_body() -> serde_json::Value {
    json!({
        "id": "chatcmpl-cli-sandbox-2",
        "object": "chat.completion",
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": FINAL_REPLY },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 50, "completion_tokens": 8 }
    })
}

/// e12 binary-level main assertion.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn req_dasclaw_cli_a6_binary_sandbox_denies_write_under_read_only_policy() {
    // Per-run tempdir so concurrent test runs do not collide. macOS puts
    // this under $TMPDIR (/var/folders/...) which is outside any
    // ReadOnly carve-out → writes here MUST be denied.
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let target: PathBuf = tempdir.path().join("a6_should_not_exist.txt");
    let target_path_str = target.to_string_lossy().into_owned();

    let server = MockServer::start().await;
    let bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));

    // Turn 1: assistant emits the shell tool_call.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(BodySniffer {
            bodies: bodies.clone(),
            response: tool_call_body(&target_path_str),
        })
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Turn 2: assistant finalizes after seeing the (denied) tool_result.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(BodySniffer {
            bodies: bodies.clone(),
            response: final_text_body(),
        })
        .expect(1)
        .mount(&server)
        .await;

    // Run the **shipped** binary, not a library entry point. The
    // `--enable-shell-tool` flag wires `dasclaw_shell_tools::ShellTool`
    // through `ToolToExecutorAdapter` with the default ReadOnly policy
    // (see `crates/dasclaw_cli/src/main.rs::run_run_subcommand`).
    let mut cmd = Command::cargo_bin("dasclaw-cli").expect("dasclaw-cli binary builds");
    cmd.args([
        "run",
        "--provider",
        "openai-compat",
        "--model",
        "gpt-4o-mini",
        "--base-url",
        &server.uri(),
        "--api-key",
        TEST_API_KEY,
        "--enable-shell-tool",
        "--prompt",
        "please write the word `blocked` to the file",
        "--system",
        "you are running under a sandboxed CLI; obey the user.",
    ]);

    let assert = cmd.assert();
    // Invariant 1: binary did not crash / panic. `assert_cmd::Assert::success()`
    // verifies exit code == 0 *and* the process actually terminated cleanly.
    let assert = assert.success();
    // Invariant 1b: final LLM reply made it to stdout (proves the loop ran
    // to completion past the tool error rather than aborting mid-turn).
    assert.stdout(predicate::str::contains(FINAL_REPLY));

    // Invariant 2: the sandbox actually enforced the deny. If this fires,
    // either the kernel-level Seatbelt profile regressed or the
    // `--enable-shell-tool` wiring lost its ReadOnly default.
    assert!(
        !target.exists(),
        "sandbox must prevent the write; file was created at {target:?}"
    );

    // Invariant 3: the denial signal reached the model loop. We inspect
    // the body of the 2nd /chat/completions request — it carries the
    // prior tool_result as a `role: tool` message. The body must contain
    // either the kernel-level Seatbelt phrase (`Operation not permitted`)
    // OR the ShellTool's structured `"sandboxed":true` flag with a
    // non-zero exit code — both prove the loop observed an enforced
    // denial rather than a silent success.
    let captured = bodies.lock().expect("poisoned body collector");
    assert!(
        captured.len() >= 2,
        "expected at least 2 /chat/completions requests, got {} — {:?}",
        captured.len(),
        captured
    );
    // We inspect the *last* captured body rather than `captured[1]` to be
    // robust against an extra retry on turn 1: regardless of retries, the
    // final request the binary sent before exiting must carry the prior
    // tool_result as a `role: tool` message with the enforced-denial signal.
    let second_body_str = captured
        .last()
        .expect("captured non-empty per assert above")
        .to_string();
    // The denial signal must be one of two *precise* shapes — kernel-level
    // Seatbelt phrase (`Operation not permitted`) OR the ShellTool's
    // structured `"sandboxed":true` flag. We deliberately do NOT accept the
    // bare substring "sandbox" as a fallback: that would let any field name
    // containing the word pass, weakening the fail-safe invariant.
    let denial_signal = second_body_str.contains("Operation not permitted")
        || second_body_str.contains("\\\"sandboxed\\\":true");
    let exit_signal =
        second_body_str.contains("\\\"success\\\":false") || second_body_str.contains("exit_code");
    assert!(
        denial_signal && exit_signal,
        "2nd request body must carry an enforced-denial signal; got {second_body_str}"
    );
}

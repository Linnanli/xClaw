//! Build script: compile the minimal HTTP fixture WASM component used by
//! capability opt-in tests (#854).
//!
//! Reproducible build:
//!   cargo build -p dasclaw_wasm_tools
//! (this script invokes the fixture build automatically; product lands in
//!  `OUT_DIR/minimal_http_component.wasm` and is included via `include_bytes!`.)
//!
//! Prerequisites:
//!   rustup target add wasm32-wasip2
//!   (wasm-tools optional — only used for stripping; build still succeeds without it.)

use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Helper: borrow a path as `&str` or produce a typed build-script error
/// instead of panicking via `unwrap()`. Cargo-supplied paths are UTF-8 in
/// practice, but we still want a clean error if a host violates that.
fn path_str<'a>(label: &str, p: &'a Path) -> Result<&'a str, Box<dyn Error>> {
    p.to_str()
        .ok_or_else(|| format!("{label} path is not valid UTF-8: {}", p.display()).into())
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|e| format!("CARGO_MANIFEST_DIR not set (cargo invariant): {e}"))?;
    let root = PathBuf::from(&manifest_dir);
    let out_dir = PathBuf::from(
        env::var("OUT_DIR").map_err(|e| format!("OUT_DIR not set (cargo invariant): {e}"))?,
    );

    let fixture_dir = root.join("tests/fixtures/minimal_http_component");
    let out_wasm = out_dir.join("minimal_http_component.wasm");

    println!("cargo:rerun-if-changed=tests/fixtures/minimal_http_component/src/lib.rs");
    println!("cargo:rerun-if-changed=tests/fixtures/minimal_http_component/Cargo.toml");
    println!("cargo:rerun-if-changed=wit/tool.wit");

    // Use a sub-crate-local target dir so the fixture build does not collide
    // with the parent workspace target. The sub-crate has [workspace] of its
    // own (standalone), but pointing CARGO_TARGET_DIR explicitly is safer in CI.
    let fixture_target = out_dir.join("fixture-target");
    let fixture_manifest = fixture_dir.join("Cargo.toml");

    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip2",
            "--manifest-path",
            path_str("fixture Cargo.toml", &fixture_manifest)?,
            "--target-dir",
            path_str("fixture target dir", &fixture_target)?,
        ])
        // Avoid inheriting RUSTFLAGS that could break wasm builds (e.g. -C link-arg=...)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .status();

    let built = matches!(status, Ok(s) if s.success());

    if !built {
        return Err(format!(
            "Failed to build fixture WASM component at {}. \
             Run: rustup target add wasm32-wasip2",
            fixture_dir.display()
        )
        .into());
    }

    let raw_wasm =
        fixture_target.join("wasm32-wasip2/release/dasclaw_wasm_tools_fixture_minimal_http.wasm");
    if !raw_wasm.exists() {
        return Err(format!(
            "Fixture WASM not found at {}. Inspect the fixture sub-crate.",
            raw_wasm.display()
        )
        .into());
    }

    // wasm32-wasip2 already emits a component — no `wasm-tools component new`
    // needed. Strip debug info when wasm-tools is available; otherwise copy
    // the raw component.
    let strip_ok = Command::new("wasm-tools")
        .args([
            "strip",
            path_str("raw wasm", &raw_wasm)?,
            "-o",
            path_str("output wasm", &out_wasm)?,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !strip_ok {
        std::fs::copy(&raw_wasm, &out_wasm).map_err(|e| {
            format!(
                "Failed to copy fixture WASM from {} to {}: {}",
                raw_wasm.display(),
                out_wasm.display(),
                e
            )
        })?;
    }

    Ok(())
}

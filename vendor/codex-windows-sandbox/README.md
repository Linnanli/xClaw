# vendor/codex-windows-sandbox

Phase 1.0 vendor scaffolding for **[ADR-121](../../docs/plans/architecture-refactor/adr-121-p0a-sandbox-activation-decision.md) D3-3**
(Windows OS sandbox via fork of `codex-windows-sandbox`).

Tracking: epic [#241](https://github.com/Linnanli/xClaw/issues/241)

## What is in this directory

- `upstream/` — verbatim copy of selected crates from
  [openai/codex](https://github.com/openai/codex) at commit
  [`6e838a19fa52f2c30442c5bd2913acd1e6fe4c9d`](https://github.com/openai/codex/commit/6e838a19fa52f2c30442c5bd2913acd1e6fe4c9d).
  No file under `upstream/` has been modified relative to that commit.
- `LICENSE-APACHE-2.0` — full text of the upstream Apache License 2.0.
- `NOTICE-upstream` — verbatim copy of the upstream `NOTICE` file.
- `vendor-pin.json` — machine-readable manifest of what was vendored, from where,
  and at which upstream commit.

## What is **not** in this PR (PR-1.0)

- ❌ Workspace registration — root `Cargo.toml` adds `vendor` to `[workspace] exclude`,
  so vendored `Cargo.toml` files are intentionally **not** picked up by `cargo build`.
- ❌ Edits to vendored `Cargo.toml` (no `[workspace.dependencies]` rewrites yet).
- ❌ Brand renaming (`CodexSandboxUsers` → `DasclawSandboxUsers`, etc.).
- ❌ Adapter glue from `crates/dasclaw_sandbox` to the vendored crate.
- ❌ Windows CI matrix.

These are deliberately deferred to **PR-1.1** so this PR can be reviewed strictly
on three axes:

1. License attribution correctness (Apache 2.0 + NOTICE preserved, upstream commit pinned).
2. Vendor scope (which 5 crates were copied and why — see `vendor-pin.json`).
3. No accidental coupling to the workspace build graph.

## Vendored crates (5 total, ~32.8k LOC)

| Upstream path | Role | Local path |
| --- | --- | --- |
| `codex-rs/windows-sandbox-rs` | primary (12 253 LOC) | `upstream/windows-sandbox-rs/` |
| `codex-rs/protocol` | transitive (`SandboxPolicy`, `NetworkAccess`) | `upstream/protocol/` |
| `codex-rs/utils/pty` | transitive (ConPTY wrapper) | `upstream/utils/pty/` |
| `codex-rs/utils/string` | transitive (2 helpers) | `upstream/utils/string/` |
| `codex-rs/utils/absolute-path` | transitive (`AbsolutePathBuf`) | `upstream/utils/absolute-path/` |

The full motivation for vendoring (vs. submodule / cargo path-dep) and the exact
upstream surface used by `windows-sandbox-rs` is recorded in `vendor-pin.json`.

## Refresh procedure

To re-vendor against a newer upstream commit:

1. `cd codex-cli-main && git fetch && git checkout <new-commit>`
2. From repo root, re-run the rsync used in PR-1.0 (one line per crate):
   ```bash
   for d in windows-sandbox-rs protocol utils/pty utils/string utils/absolute-path; do
     rsync -a --delete --exclude='target' --exclude='.git' \
       "codex-cli-main/codex-rs/$d/" "vendor/codex-windows-sandbox/upstream/$d/"
   done
   ```
3. Update `vendor-pin.json` (`upstream.commit`, `commit_subject`, `captured_at`).
4. Re-copy `LICENSE` / `NOTICE` from `codex-cli-main/` if the upstream files changed.

## Why a separate `vendor/` tree (rather than a fresh `crates/` member)

ADR-121 D3-3 explicitly chose **fork** over rewrite. Keeping the vendored tree
isolated under `vendor/` preserves a clean delta against upstream so future
cherry-picks stay tractable. PR-1.1 will introduce a `crates/dasclaw_sandbox_windows/`
shim that depends on (or wraps) the vendored crate; the vendored sources
themselves will continue to live under `vendor/` and remain branded `codex-*`
until Phase 2 (Brand & Naming).

## License

Code under `upstream/` is © OpenAI and licensed under the Apache License 2.0
(see `LICENSE-APACHE-2.0` and `NOTICE-upstream`). Any modifications introduced
by later phases of this fork will preserve the Apache 2.0 license and add a
modification notice as required by the license.

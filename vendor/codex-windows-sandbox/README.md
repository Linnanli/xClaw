# vendor/codex-windows-sandbox

> **Status (Phase 1.1.0)**: Read-only **reference snapshot**. The actual workspace crate
> being built is [`crates/dasclaw_sandbox_windows`](../../crates/dasclaw_sandbox_windows).
> Files under this directory are **never compiled** — root `Cargo.toml` keeps
> `vendor` in `[workspace] exclude` and that exclusion will not be lifted.

This snapshot exists to make the porting plan in [tracker #250](https://github.com/Linnanli/xClaw/issues/250) auditable:
each module ported into `crates/dasclaw_sandbox_windows/` carries a header that
points back to the corresponding file under `upstream/`.

## Background

Per [ADR-121](../../docs/plans/architecture-refactor/adr-121-p0a-sandbox-activation-decision.md) D3-3
(epic [#241](https://github.com/Linnanli/xClaw/issues/241)), x-claw forks
`codex-windows-sandbox`. Phase 1.0 (PR #248) vendored selected upstream crates
verbatim. Phase 1.1 then re-evaluated build wiring and concluded that a verbatim
build would drag in 30+ transitive `codex-*` crates. The decision (**design
path V'-b**) is to:

1. Keep the verbatim snapshot under `upstream/` as a **reference only**, and
2. Hand-port the parts we actually need into a brand-new main-workspace crate
   `crates/dasclaw_sandbox_windows/`, mechanically rewriting imports to local
   equivalents, with each ported file carrying an Apache-2.0 attribution header.

## Layout

| Path | Purpose |
| --- | --- |
| `upstream/` | Verbatim copy of selected upstream crates at the pinned commit. **Never built.** |
| `LICENSE-APACHE-2.0` | Full text of the upstream Apache License 2.0. |
| `NOTICE-upstream` | Verbatim copy of the upstream `NOTICE` file. |
| `vendor-pin.json` | Machine-readable manifest: upstream commit, vendored paths, current usage status. |

The vendored crates and the upstream commit are documented in
[`vendor-pin.json`](./vendor-pin.json).

## Refreshing the snapshot

Refresh is optional and only needed when porting a new upstream feature. To
re-snapshot against a newer upstream commit:

1. `cd codex-cli-main && git fetch && git checkout <new-commit>`
2. From repo root, re-run the rsync (one line per crate):
   ```bash
   for d in windows-sandbox-rs protocol utils/pty utils/string utils/absolute-path; do
     rsync -a --delete --exclude='target' --exclude='.git' \
       "codex-cli-main/codex-rs/$d/" "vendor/codex-windows-sandbox/upstream/$d/"
   done
   ```
3. Update `vendor-pin.json` (`upstream.commit`, `commit_subject`, `captured_at`).
4. Re-copy `LICENSE` / `NOTICE` from `codex-cli-main/` if the upstream files changed.

Because the build does not depend on this directory, refresh **never affects
CI**. It only affects the diff readers see when inspecting future ports.

## License

Code under `upstream/` is © OpenAI and licensed under the Apache License 2.0
(see `LICENSE-APACHE-2.0` and `NOTICE-upstream`). Files ported into
`crates/dasclaw_sandbox_windows/` carry per-file attribution headers crediting
the upstream source path and commit, in compliance with Apache 2.0 §4.

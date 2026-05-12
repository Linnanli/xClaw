# third_party/bubblewrap — pointer

This directory exists to satisfy the convention that third-party licensed
components are discoverable from the repository root via `third_party/`.

The actual vendored bubblewrap source tree lives at:

> [`crates/vendor/bubblewrap/`](../../crates/vendor/bubblewrap/)

It is placed as a **sibling** of the consuming crate `crates/dasclaw_sandbox_linux/`
to exactly mirror upstream codex's sibling layout
(`codex-cli-main/codex-rs/vendor/bubblewrap/` is sibling of
`codex-cli-main/codex-rs/linux-sandbox/`), per the ADR-129 §1.3 verbatim-port
red line. The path `../vendor/bubblewrap` referenced from
[`crates/dasclaw_sandbox_linux/build.rs`](../../crates/dasclaw_sandbox_linux/build.rs)
resolves to this directory. See [`NOTICE`](../../NOTICE) (top of
repository) for the LGPL-2.0-or-later attribution, upstream URL, version
(0.11.0), and §6 compliance posture.

Relevant references:

- ADR-144 §6 OQ-C1c-7 — Linux WritableRoot kernel enforcement (Wave-C1c
  Phase 0 spike): [`docs/plans/architecture-refactor/adr-144-linux-writable-root-kernel-enforcement.md`](../../docs/plans/architecture-refactor/adr-144-linux-writable-root-kernel-enforcement.md)
- Tracking issue: [#440 — chore(legal): LGPL v2 compliance for vendored bubblewrap](https://github.com/Linnanli/xClaw/issues/440)
- Upstream: <https://github.com/containers/bubblewrap>
- Vendored version: **0.11.0** (per `vendor/bubblewrap/NEWS.md` and `meson.build`)
- SPDX-License-Identifier: `LGPL-2.0-or-later` (declared per-file in C/H headers)

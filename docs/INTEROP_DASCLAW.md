# claw-code → dasclaw_* interop map

> **Status**: Documentation-only (B5 deliverable for Issue #911).
> **Purpose**: Prove that the three lineages (codex / claw-code / ironclaw) already
> compose on the GUI path by mapping every claw-code concept to its concrete
> landing in the workspace `dasclaw_*` crates, with an executable reference at
> [`crates/dasclaw_cli/examples/claw_code_interop.rs`](../crates/dasclaw_cli/examples/claw_code_interop.rs).
>
> **Location note (deviation from Issue #911 brief)**: the original task brief
> placed this file at `claw-code/INTEROP_DASCLAW.md`, but `claw-code/` is a git
> submodule in this workspace (see `.gitmodules`), so any file added there
> would belong to a different repo. To honour the rule "do not modify the
> claw-code source tree" while still satisfying the B5 grep contract from this
> repo, the file lives at `docs/INTEROP_DASCLAW.md` and the e2e grep anchor
> test reads it from that path.

## Why this file exists

Issue #911 closes the B5 gap from
[`docs/plans/architecture-refactor/49-gui-readiness-assessment.md`](plans/architecture-refactor/49-gui-readiness-assessment.md)
§5 / §9: "三库融合在 GUI 路径未端到端证明". Rather than re-implement claw-code's
runtime, we show that its primitives are already absorbed into the dasclaw stack
and can be driven from a single headless agent loop.

## Mapping table

| claw-code concept | dasclaw_* landing | 接入难度 |
|---|---|---|
| Headless agent loop (`runtime/agent.ts`) | [`dasclaw_runtime::Agent`](../crates/dasclaw_runtime/src/agent.rs) (`AgentResponder` + `ToolExecutor` + `HookBundle`) | trivial — already drives `dasclaw_cli`'s e2e tests |
| `bash` tool gating ("Are you sure?" / refuse) | [`dasclaw_bash_validation`](../crates/dasclaw_bash_validation) (`validate_command`) wrapped by [`dasclaw_hooks::BashValidationHook`](../crates/dasclaw_hooks/src/bash_validation_hook.rs) (`EgressGate`) | trivial — `BashValidationHook::new(PermissionMode, workspace)` plugs into `HookBundle.egress` |
| `apply_patch` tool (model emits `*** Begin Patch …`) | [`dasclaw_apply_patch::parse_patch`](../crates/dasclaw_apply_patch/src/parser.rs) + [`apply`](../crates/dasclaw_apply_patch/src/apply.rs) | trivial — pure function over `ApplyPatchArgs` and `ApplyOptions { base_dir }` |
| Hook/event bus (BeforeToolCall / AfterToolCall) | [`dasclaw_hooks`](../crates/dasclaw_hooks) (`HookBundle` re-exports `EgressGate` from `dasclaw_core::hooks`, plus declarative `HookBundleConfig`) | trivial — the agent loop already calls `hooks.egress.check` before every tool dispatch |
| 9-layer defence stack (sandbox / approval / secrets / audit) | `HookBundle { egress, sandbox, secrets, approval }` in [`dasclaw_core::hooks`](../crates/dasclaw_core/src/hooks.rs) | already wired — this PR only exercises the `egress` lane; sandbox/approval are out of scope for the demo |

## Executable proof

[`crates/dasclaw_cli/examples/claw_code_interop.rs`](../crates/dasclaw_cli/examples/claw_code_interop.rs)
runs a scripted four-turn agent loop in a tempdir, in this order:

1. Model calls `bash` with `rm -rf /` → `dasclaw_bash_validation` (via
   `dasclaw_hooks::BashValidationHook`) blocks; executor is **never** invoked.
2. Model calls `bash` with `ls` → gate allows; stub executor returns a fake
   listing.
3. Model calls `apply_patch` with an `*** Update File: hello.txt` hunk →
   `dasclaw_apply_patch::{parse_patch, apply}` writes `world\n` into the
   tempdir.
4. Model emits final text `done` and the loop exits.

The matching e2e test
[`crates/dasclaw_cli/tests/claw_code_interop_e2e.rs`](../crates/dasclaw_cli/tests/claw_code_interop_e2e.rs)
asserts the audit timeline, the rejection on step 1, the on-disk mutation on
step 3, and that **this very file** contains the string anchors that prove the
grep contract from Issue #911:
`dasclaw_runtime`, `dasclaw_bash_validation`, `dasclaw_apply_patch`,
`dasclaw_hooks`.

## Non-goals (explicit)

- We do not port claw-code source code into the workspace.
- We do not execute real shell or real network; both lanes are stubbed in the
  executor so the demo is hermetic and deterministic.
- We do not wire `dasclaw_governance`'s 6-pack on this path — that is covered
  by separate ADR-153 §1.1 B-series work, not B5.
- `claw-code/`, `vendor/`, `codex-cli-main/` source trees stay untouched.

## See also

- Issue #911 (B5 — claw-code interop demo on GUI path)
- [`docs/plans/architecture-refactor/49-gui-readiness-assessment.md`](plans/architecture-refactor/49-gui-readiness-assessment.md)
  §5 (GUI readiness gaps) and §9 (B5 row)
- ADR-153 §1.1 (B-series rollout for headless agent loop)
- [`docs/plans/architecture-refactor/53-headless-agent-capability-design.md`](plans/architecture-refactor/53-headless-agent-capability-design.md)
  (headless agent framework capability boundary; defines what is / is not part
  of the headless library surface)
- [`crates/dasclaw_runtime/README.md`](../crates/dasclaw_runtime/README.md)
  (library-side getting-started for embedders) and
  [`crates/dasclaw_cli/examples/headless_agent_starter.rs`](../crates/dasclaw_cli/examples/headless_agent_starter.rs)
  (minimum runnable embed example covering responder + tool executor + approval
  policy + event stream)

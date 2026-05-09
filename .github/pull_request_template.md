## Summary

<!-- 2-5 bullet points: what changed and why -->

-

## Change Type

<!-- Check one -->

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor
- [ ] Documentation
- [ ] CI/Infrastructure
- [ ] Security
- [ ] Dependencies

## Linked Issue

<!-- Closes #N, or "None" -->

## Sources read

<!--
强制：列出本 PR 派单 issue body 中 **Source 文档** 字段、**关联 ADR** 字段引用的全部文件 +
你实际打开过的 §/标题。Coding Agent 必须如实回声路径与小节，禁止只写 "已阅"。

例：
  - docs/plans/architecture-refactor/32-execution-plan.md §W7
  - docs/plans/architecture-refactor/adr-137-net-proxy-port-plan.md §3.2 PR-N23
  - AGENTS.md §"Skills 强制使用规范"
  - crates/dasclaw_net_proxy/src/lib.rs（模块文档）

如确实无 source（trivial chore PR），写 "None — chore PR with no architecture impact"。
-->

-

## Cross-cuts

<!--
ADR-114 (.ironclaw → .dasclaw rebrand) enforcement field. Required.
Pick one and delete the others:

  - ADR-114: 类A 无新增 .ironclaw / IRONCLAW_BASE_DIR 字面量
  - ADR-114: 类B issue#XXX (打 'adr-114-class-b' label，CI grep guard 整体跳过)
  - ADR-114: 不涉及

Reference: docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md §4.1
-->

-

## Validation

<!-- How did you verify this works? -->

- [ ] `cargo fmt`
- [ ] `cargo clippy --all --benches --tests --examples --all-features`
- [ ] Relevant tests pass: <!-- list specific tests -->
- [ ] Manual testing: <!-- describe what you tested -->

## Security Impact

<!-- Does this change affect: permissions, network calls, secrets, file access, tool execution, sandbox policy? If yes, describe. If no, write "None". -->

## Database Impact

<!-- Does this add/modify migrations, change schema, or affect both PostgreSQL and libSQL? If yes, describe. If no, write "None". -->

## Blast Radius

<!-- What subsystems does this touch? What could break? -->

## Rollback Plan

<!-- How to revert if this causes problems? For Track C changes, this is mandatory. -->

---

**Review track**: <!-- A (docs/tests/chore) | B (feature/refactor) | C (security/runtime/DB/CI) -->

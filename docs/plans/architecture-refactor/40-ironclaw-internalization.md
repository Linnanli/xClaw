# 40 — ironclaw 内联与长期淡化路径（ADR-110）

> **决策日期**：2026-04-27
> **状态**：已执行（v1.0）
> **影响**：`desktop-client/ironclaw/` 从嵌套独立 git 仓库 → x-claw monorepo 内的普通目录
> **相关 ADR**：ADR-101（吸收式重构）、ADR-105（fork 处理）、ADR-110（本文档）

---

## 1. 背景

`desktop-client/ironclaw/` 是 NEAR AI `nearai/ironclaw` v0.24 的 fork（`Linnanli/xclaw-ironclaw`），同时是 x-claw cargo workspace 的成员。这种"嵌套独立 git 仓库"结构在 W2 之前是合理的：

- 当时 fork 改动可控
- 期望长期同步上游
- ironclaw 与 x-claw 关注点分离

W2 完成后，状态发生质变：

- **42 fork-only commit**（dasclaw 接线核心成果，见 32-execution-plan §ADR-105）
- 已完成 sandbox / PTY / workspace_cap / exec 等多个核心模块替换
- 实际上 ironclaw fork 已经成为 x-claw 的内核，不再是"借用的上游"

继续维持双仓库结构产生明显成本：
- 每改 ironclaw 内文件需 2 个 PR（子仓库代码 + 外层 submodule pointer bump）
- IDE 跨仓库跳转破裂
- Cargo workspace path 依赖语义割裂
- release 协调成本翻倍

## 2. 决策：内联（方案 R / 做法 1）

**执行步骤**（v1.0 已完成）：

1. 子仓库当前状态推送到 `xclaw-ironclaw` 的 `archive/before-internalize-2026-04-27` 分支作 archive
2. 外层 `rm -rf desktop-client/ironclaw/.git`
3. 外层 `git add desktop-client/ironclaw/` + 单 commit `feat: internalize ironclaw as x-claw core (ADR-110)`
4. 后续所有 ironclaw 改动直接在外层 x-claw 仓库 PR

**保留**：
- `xclaw-ironclaw` GitHub 仓库继续存在，作为历史 archive 与 fork 出处证明
- `LICENSE-MIT` / `LICENSE-APACHE`（NEAR AI 双许可不变）
- `CONTRIBUTING.md` / `AUTHORS` / `NOTICE` 等归属文件

## 3. 长期淡化路径

将 `ironclaw` 从"上游 fork"逐步淡化为"x-claw 内部自研内核"：

| Phase | 内容 | 触发条件 |
|-------|------|---------|
| **P1（本次）** | 内联：删除嵌套 .git，目录与 path 依赖不变 | 已完成 2026-04-27 |
| **P2** | crate 重命名 `ironclaw` → `x_claw_core` | 模块替换达 50%+ 时 |
| **P3** | 模块逐步替换为 `dasclaw_*` / `x_claw_*`（已在做：sandbox/PTY/workspace_cap） | 持续执行（W3-W6） |
| **P4** | 移除上游标识：README/AUTHORS 重写、CHANGELOG 续接 x-claw 自身、上游 attribution 仅保留 LICENSE | dasclaw_* 替换达 80%+ |

**P2-P4 不强制时间表**，按 wave 推进自然演化。

## 4. 风险与缓解

| 风险 | 严重度 | 缓解 |
|------|--------|------|
| 失去上游 nearai/ironclaw 同步能力 | 中 | 已分叉太深（42 fork-only commit），回归上游成本本来就高 |
| ironclaw 旧 git history 不在外层 | 低 | history 仍存在 `xclaw-ironclaw` GitHub fork 可查；本地 history 对日常开发价值有限 |
| 巨型初始 commit (~50000+ LOC) | 中 | commit message 详注内联背景与 archive 分支；不可 review 但必要 |
| LICENSE / 作者归属合规 | 低 | 保留 LICENSE 文件不动；NOTICE 中追加"forked from nearai/ironclaw v0.24" |

## 5. 不做什么（明确边界）

- **不做** git subtree 保留 history：操作复杂、新人难维护，价值不抵成本
- **不做** crate 立刻重命名：现有 import 全部生效，破坏面太大，留给 P2
- **不做** 立刻删除 dasclaw_* 替换不掉的 ironclaw 模块（如 LLM provider）：保留并标 legacy

## 6. 验证

内联后 P0 验证：
- [x] `cargo build` 全 workspace 通过
- [x] `desktop-client/ironclaw/` 在外层 git 中显示为普通目录（非 modified content）
- [x] `xclaw-ironclaw` archive 分支 push 成功
- [x] `40-ironclaw-internalization.md` ADR 落地

## 7. 关联文档

- `docs/plans/architecture-refactor/30-architecture-truth.md` §"desktop-client/ironclaw 处理"
- `docs/plans/architecture-refactor/31-target-architecture.md` ADR-101 / ADR-105
- `docs/plans/architecture-refactor/38-desktop-client-ironclaw-fork-inventory.md` fork 全量清单

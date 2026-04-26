# architecture-refactor — 2026-04 吸收式重构（ADR-101）

> **v2.2 文档集合**（2026-04-25 ~ 2026-04-26）
> 本目录是本次「吸收式重构」决策与执行计划的主资产。9 份核心文档 + 2 份 ADR。

## 文档导航

### 📐 架构主文档（30-33）

| # | 文档 | 角色 | 当前版本 |
|---|------|------|---------|
| 30 | [30-architecture-truth.md](30-architecture-truth.md) | **事实矩阵**：四方 + ironclaw fork 现状盘点（一切结论的事实底盘） | **v2.2** |
| 31 | [31-target-architecture.md](31-target-architecture.md) | **目标架构**：15 crate 蓝图 + ADR-101 ~ ADR-110（含 dasclaw_bridge_lite） | **v2.2** |
| 32 | [32-execution-plan.md](32-execution-plan.md) | **执行计划**：W1-W9 共 9 个 Wave，~8-10 月（v2.2 W6 扩 5 周） | **v2.2** |
| 33 | [33-feasibility-and-validation.md](33-feasibility-and-validation.md) | **可行性 + 验证**：18 个目标 + 10 个风险 + 三层 E2E（含 §3.8 tauri-driver vs Computer Use vs Lima/Codespaces） | **v2.2** |

### 🔍 能力清单（35-38）

| # | 文档 | 范围 | 当前版本 |
|---|------|------|---------|
| 35 | [35-codex-capability-inventory.md](35-codex-capability-inventory.md) | codex-cli-main 全量盘点 + §Q 88 commit 增量 | **v0.3** |
| 36 | [36-claw-code-capability-inventory.md](36-claw-code-capability-inventory.md) | claw-code 全量盘点（治理 6 件套 / mock-anthropic 27 场景） | v1 |
| 37 | [37-ironclaw-main-capability-inventory.md](37-ironclaw-main-capability-inventory.md) | ironclaw-main 上游 v0.26 全量盘点（464,222 LOC） | v1 |
| 38 | [38-desktop-client-ironclaw-fork-inventory.md](38-desktop-client-ironclaw-fork-inventory.md) | desktop-client/ironclaw fork 全量盘点（295,658 LOC） | v1 |

### 📝 架构评审

| 文档 | 内容 |
|------|------|
| [34-architect-review.md](34-architect-review.md) | 资深架构师视角对 30-33 的批判性评审 |

### 🏷 历史 ADR（仍有效）

| ADR | 主题 |
|-----|------|
| [adr-001-sandbox-hook-not-wired-in-phase3.md](adr-001-sandbox-hook-not-wired-in-phase3.md) | Phase 3 不接线进程内 sandbox hook |
| [adr-002-sandbox-backend-layered-strategy.md](adr-002-sandbox-backend-layered-strategy.md) | 沙箱后端分层策略（cap-std + codex 三平台 + safety + Docker） |

### 📦 archive/

- `archive/01-08-*` — Phase 1-8 早期方案文档（已被 30-33 替代）
- `archive/legacy-v1/` — 09-13 旧版能力清单 + cleanup-proposal（已被 30-38 替代，2026-04-26 归档）

## 本次开发关键决策摘要

1. **ADR-101 吸收式重构**：不"升级 fork 到主仓"，而是把三库（codex / claw-code / ironclaw-main）+ desktop-client/ironclaw fork 熔合成新的 dasclaw_* crate 体系。
2. **dasclaw_core ≈ 14-16k LOC**（不是 v2 全套 30k）：v2 真资产是 410 LOC trait + 3,109 LOC types，剩余实现按 chat+job 两类 thread type 砍至 10-12k。
3. **dasclaw_bridge_lite ≈ 5-7k LOC**（vs ironclaw bridge 全套 25,369 LOC）：保 EffectExecutor + LlmBackend + AuthLite + CostGuard + UserFacingErrors；砍 router 9.6k + store 大半 + skill_migration。
4. **dasclaw_workspace ≈ 7-9k LOC**（vs ironclaw workspace 全套 12,857 LOC）：去多租户化 -3k LOC，保 chunker + embeddings + RRF k=60 + Hybrid Search。
5. **codex 88 commit 增量**：goal 系统五件套 / ThreadStore trait / permissions profiles / rollout-trace / Unix socket transport 五项 P0 增量已纳入 W6。
6. **全栈 Tauri E2E 三层**：L1 tauri-driver (Linux+Windows CI) + L2 Playwright over CDP (macOS nightly) + L3 Computer Use MCP (探索性 / UAT)；macOS 上 tauri-driver 不可用是已知缺口。

## 维护约定

- 任何新决策走 ADR-1xx（编号承接 ADR-110）
- 大版本号变更（如 v2.2 → v2.3）须同步更新 30/31/32/33 + 本 README
- 35-38 能力清单与上游 commit 同步：每收一次 upstream，必加 §Q-style 增量节

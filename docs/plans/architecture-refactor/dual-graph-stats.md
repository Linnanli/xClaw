# 双图谱基线统计（Issue #621 第一步）

> **状态**：accepted（数据快照，不阻塞决策）
> **生成时间**：2026-05-18
> **基线 commit**：`80d41c902c56`（xClaw HEAD）
> **工具**：`code-review-graph` CLI（Tree-sitter 解析 + 增量图谱）

## 1. 范围

| 范围 | 命名空间 | 物理路径 | 说明 |
|------|----------|---------|------|
| A | `core-side` (alias `xclaw-core`) | [crates/](../../../crates/) | 22 个 `dasclaw_*` + `ironclaw_auth` + `vendor/` 顶层 workspace crate 集合 |
| B | `ironclaw-side` (alias `ironclaw`) | [desktop-client/ironclaw/](../../../desktop-client/ironclaw/) | fork 内核（含 `src/`、`crates/ironclaw_safety/`、`crates/ironclaw_common/`、`tests/`） |

> Issue #621 §范围已固定：本统计**仅**覆盖上述两个目录。`codex-cli-main/`、`claw-code/`、`ironclaw-main/`、`decode-claude-code-main/`、`claude-code-main/` 作为参考库另有独立图谱（`codex-cli` / `claw-code` / `ironclaw-main`），不进入双图谱对账。

## 2. 图谱统计

| 维度 | core-side (`xclaw-core`) | ironclaw-side (`ironclaw`) |
|------|--------------------------|-----------------------------|
| **文件数** | 333 | 564 |
| **节点数** | 6,871 | 14,035 |
| **边数** | 45,882 | 135,778 |
| **节点/文件** | 20.6 | 24.9 |
| **边/节点** | 6.68 | 9.67 |
| **语言** | rust, powershell | bash, rust, python, javascript |
| **构建分支** | xClaw | xClaw |
| **构建 commit** | 80d41c902c56 | 80d41c902c56 |
| **构建时间** | 2026-05-18T17:10:08 | 2026-05-18T17:10:34 |

## 3. 解读要点

1. **规模比例约 1:2**：ironclaw-side 节点数约为 core-side 的 2.04 倍，边数约 2.96 倍。说明 ironclaw 侧不仅文件更多，且**模块间耦合度（边/节点比）显著更高**（9.67 vs 6.68）——这与 ADR-101/110 中关于 ironclaw bridge/workspace 是「双模式胶水层 + 多通道集成」的判断一致。
2. **core-side 几乎是纯 Rust crate 工厂**：仅一个 powershell 文件（属于 verbatim port 自 codex 的 `dasclaw_sandbox_windows`）。无 python/js 噪声，节点 hub 度较低，符合「23 个独立 workspace crate，每个职责单一」的设计目标。
3. **ironclaw-side 是多语言集成层**：含 bash（启动脚本 / sandbox shim）、python（迁移/工具 / scripts）、javascript（front-end glue / channel adapters），与 §31-target-architecture.md L5「LLM/通道/持久化/Routines/Secrets/文档抽取」描述对应。
4. **每文件节点密度**：ironclaw-side 24.9 vs core-side 20.6，差距不大；这意味着「ironclaw 文件平均更大」的直觉**部分成立**，但更显著的差异在跨文件耦合（边）。

## 4. 后续动作（不在本 PR 范围）

- **第二步**（独立 PR）：基于这两份图谱产出 [capability-comparison.md](capability-comparison.md)（18 条能力对比表）。
- **第三步**（独立 PR）：产出 [adr-149-agent-and-capability-fusion.md](adr-149-agent-and-capability-fusion.md)。

## 5. 复现命令

```bash
# 范围 A
cd crates && code-review-graph build --repo .
code-review-graph register $(pwd) --alias xclaw-core

# 范围 B（已注册，仅增量更新）
code-review-graph update --repo desktop-client/ironclaw

# 查看 stats
code-review-graph status --repo crates
code-review-graph status --repo desktop-client/ironclaw
```

## 6. 数据物理位置

| 范围 | 数据库 | git 状态 |
|------|--------|---------|
| A | `crates/.code-review-graph/graph.db` | ✅ 已在根 `.gitignore` 中（`.code-review-graph/` 规则全局生效）|
| B | `desktop-client/ironclaw/.code-review-graph/graph.db` | ✅ 同上 |

本 PR 仅引入此 markdown 报告；不提交图谱二进制（数据库属生成物，复现命令已列于 §5）。

## Sources read

- `docs/plans/architecture-refactor/31-target-architecture.md` §1（六层架构总览）+ §4（crate 清单）
- `docs/plans/architecture-refactor/32-execution-plan.md` Wave 概览
- `AGENTS.md` §「code-review-graph 增量图谱」
- Issue #621 body §「第一步：双图谱建立」

# Archive — Route A 时代历史文档

本目录保存 **Route A (边缘注入式重构)** 时代的规划文档,已被 [11-target-architecture-route-b.md](../11-target-architecture-route-b.md) 及后续 12-16 序列取代。

保留这些文档的目的是:
1. 追溯历史决策与废弃原因
2. 审计合规时提供完整演进证据
3. 避免 PR 合并冲突导致的历史丢失

**不要引用这些文档做新决策**。新决策必须基于 `docs/plans/architecture-refactor/11-16` 序列。

## 归档清单

| 文档 | Route A 角色 | 废弃原因 |
|-----|-----------|--------|
| 00-overview.md | 整体蓝图 | 被 [11-target-architecture-route-b.md](../11-target-architecture-route-b.md) 取代 |
| 01-current-gaps-analysis.md | 当前问题分析 | 被 [09-capability-inventory-and-migration-strategy.md](../09-capability-inventory-and-migration-strategy.md) + [13-security-capability-inventory.md](../13-security-capability-inventory.md) 取代 |
| 02-target-architecture.md | 目标架构 (Route A) | Route A 废弃,被 11 文档取代 |
| 03-phase1-ai-sdk-migration.md | Phase 1 AI-SDK 迁移 | Phase 制废弃,改 Wave 制 (见 11 §8) |
| 04-phase2-claw-code-api.md | Phase 2 claw-code API | 同上 |
| 04b-step-i-execution-plan.md | Step I 执行计划 | 同上 |
| 05-phase3-agent-extraction.md | Phase 3 Agent 抽取 | 部分内容并入 Wave 1 (dasclaw_agent_kernel) |
| 05b-phase3-execution-plan.md | Phase 3 执行计划 | 同上 |
| 06-phase4-claw-code-capability-port.md | Phase 4 claw-code 能力移植 | 改由 14/15/16 按 P0 清单驱动 |
| 06-safety-preservation.md | 安全保留承诺 | 被 13/15/16 具体化 (9 层纵深 + 15 条技术栈 + E2E 流程) |
| 07-rollback-plan.md | 回滚计划 | Wave 制自带回滚 (每 Wave 独立契约),不再单列 |
| 08-three-tree-framework-overview.md | 三库统管框架概览 | 被 11 §1-§3 取代 |

## 与当前基线的映射

```
Route A (archive/)                      Route B (当前基线)
────────────────────                  ─────────────────────
00 overview         ─────────────────►  11 target-architecture-route-b
01 gaps + 06 safety ─────────────────►  13 security-capability-inventory
02 target + 08 3-tree ───────────────►  11 §1-§3
03/04/04b/05/05b phases ─────────────►  11 §8 Wave 路线图
06 phase4 port ──────────────────────►  14 claude-code-capability-parity (P0 清单)
07 rollback ─────────────────────────►  各 Wave 契约 (内嵌)
                     (新增)           ►  15 product-north-star (产品定位)
                     (新增)           ►  16 end-to-end-flow (运行时流程)
                     (新增)           ►  17 crate-inventory-audit (crate 盘点)
```

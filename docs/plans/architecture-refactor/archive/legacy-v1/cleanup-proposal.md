# architecture-refactor 旧文档清理建议

> **v1.0 (2026-04-25)** · 配套 [30-architecture-truth.md](30-architecture-truth.md) ~ [33-feasibility-and-validation.md](33-feasibility-and-validation.md)。
> **重要**：本清单不直接删除任何文件。请用户审阅后批准，再执行。
> 用户原话："把非架构搬迁的需求整理类的内容评估一下删除掉"。

---

## 评估口径

每个文档判定：
- ✅ **保留** — 仍有架构价值
- 📦 **归档**（移到 archive/）— 历史价值在，但不再是当前主线
- 🗑️ **删除** — 内容已被 30-33 完全覆盖，或属于"产品需求/状态/调研"非架构内容
- ✏️ **重写**（在保留前提下精简）— 部分内容有用，但夹带过期/幻觉信息

---

## 文档清单与建议

| # | 文件 | 主题 | 建议 | 理由 |
|---|------|------|------|------|
| 09 | 09-capability-inventory-and-migration-strategy.md | 三库能力清单 + 迁移策略 | 📦 归档 | 内容被 30 文档替代；§1.1-§1.3 事实清单仍有参考价值，归档保留 |
| 10 | 10-agent-kernel-quality-assessment.md | Agent 内核质量评估 | 🗑️ 删除 | 已被 31 §3 ADR-101 覆盖，且属于"评估"非"架构" |
| 11 | 11-target-architecture-route-b.md | 路线 B 目标架构 | 📦 归档 | 被 31 文档替代，命名前缀从 dasclaw_ 改为 x_claw_，6 件套从必做改可选；保留作历史参考 |
| 12 | 12-porting-compliance.md | 移植合规 | 🗑️ 删除 | 27 文档已确认"合规延后"，本文档失效 |
| 13 | 13-security-capability-inventory.md | 安全能力清单 | ✏️ 重写 | 事实部分有价值（DLP/sandbox/secrets 路径），但与 30 §2.3 重复；建议精简后并入 30 文档附录 |
| 14 | 14-claude-code-capability-parity.md | Claude Code 能力对账 | 🗑️ 删除 | §0 §6 自承认 Round 18 错误率 4/19，且 30 文档已重做；保留增加幻觉风险 |
| 15 | 15-product-north-star.md | 产品北极星 | 🗑️ 删除 | 产品定位文档，非架构 |
| 16 | 16-end-to-end-flow.md | 端到端流程 | 📦 归档 | 流程描述，已被 31 §7 sequence diagram 替代 |
| 17 | 17-crate-inventory-audit.md | crate 盘点审计 | 📦 归档 | 三库 crate 归属表有价值，已融入 31 §4；归档保留作详细查询 |
| 18 | 18-execution-roadmap.md | 执行路线图 | 📦 归档 | 被 32 文档替代，归档保留 |
| 19 | 19-product-definition-questionnaire.md | 产品定义问卷 | 🗑️ 删除 | 产品需求问卷，非架构 |
| 20 | 20-technical-feasibility-report.md | 技术可行性报告 | 🗑️ 删除 | 被 33 文档替代 |
| 21 | 21-timeline-rebaseline.md | 时间表重设 | 🗑️ 删除 | 时间表，非架构 |
| 22 | 22-product-constraints-impact.md | 产品约束影响 | 🗑️ 删除 | 产品约束，非架构 |
| 23 | 23-ai-assisted-dev-reality-check.md | AI 辅助开发现实检查 | 🗑️ 删除 | 开发方式讨论，非架构 |
| 24 | 24-mvp-scope-and-deferred.md | MVP 范围与延后 | 🗑️ 删除 | 范围管理，被 27 文档替代 |
| 25 | 25-existing-capability-audit.md | 现有能力审计 | 🗑️ 删除 | 被 30 文档替代 |
| 26 | 26-capability-audit-matrix.md | 能力审计矩阵 | 🗑️ 删除 | 被 30 文档矩阵替代 |
| 27 | 27-mvp-repriority-architecture-first.md | MVP 重排架构优先 | ✅ 保留 | 方向锚点文档，本次架构调整的起点 |
| ADR-001 | adr-001-sandbox-hook-not-wired-in-phase3.md | ADR-001 | ✅ 保留 | ADR 本身需要保留 |
| ADR-002 | adr-002-sandbox-backend-layered-strategy.md | ADR-002 | ✅ 保留 | ADR 本身需要保留 |

---

## 汇总

| 处理 | 数量 | 文件 |
|------|------|------|
| ✅ 保留 | 3 | 27, ADR-001, ADR-002 |
| 📦 归档（移到 archive/） | 5 | 09, 11, 16, 17, 18 |
| ✏️ 重写后并入 30 附录 | 1 | 13 |
| 🗑️ 删除 | 12 | 10, 12, 14, 15, 19, 20, 21, 22, 23, 24, 25, 26, 30 之外的 14 |

**新增的 4 份文档**：
- 30-architecture-truth.md（事实矩阵）
- 31-target-architecture.md（目标架构）
- 32-execution-plan.md（执行计划）
- 33-feasibility-and-validation.md（可行性 + 验证）

---

## 执行步骤（用户确认后再做）

```bash
cd docs/plans/architecture-refactor

# 1. 归档 5 份
mv 09-capability-inventory-and-migration-strategy.md archive/
mv 11-target-architecture-route-b.md archive/
mv 16-end-to-end-flow.md archive/
mv 17-crate-inventory-audit.md archive/
mv 18-execution-roadmap.md archive/

# 2. 删除 12 份（请审慎）
rm 10-agent-kernel-quality-assessment.md
rm 12-porting-compliance.md
rm 14-claude-code-capability-parity.md
rm 15-product-north-star.md
rm 19-product-definition-questionnaire.md
rm 20-technical-feasibility-report.md
rm 21-timeline-rebaseline.md
rm 22-product-constraints-impact.md
rm 23-ai-assisted-dev-reality-check.md
rm 24-mvp-scope-and-deferred.md
rm 25-existing-capability-audit.md
rm 26-capability-audit-matrix.md

# 3. 重写 13 文档（精简后并入 30 附录）
# 手工操作：从 13 提取仍准确的事实，加入 30 文档"附录 B 安全能力详情"
rm 13-security-capability-inventory.md  # 完成并入后再删
```

---

## 风险与备份

- 删除前确保 git commit 干净（所有删除都可通过 git history 找回）
- 建议先在新分支 `architecture-refactor-cleanup` 操作，PR 评审后合并
- archive/ 目录已存在（含旧版 00-08），新归档的 5 份直接放进去

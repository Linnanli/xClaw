# Agent Rules

## 沟通规则

1. **不要假设用户清楚自己想要什么。** 当动机或目标不清晰时，停下来讨论，而不是猜测着往前冲。做错了再改的成本远高于多问一句。
2. **目标清晰但路径不是最短的，直接说并建议更好的办法。** 用户可能因为惯性选择了次优方案，AI 有责任指出更短的路径——但最终决定权在用户。
- 所有最终回复必须用中文。

## 任务启动 4 问（每次新任务必答）

开始任何代码、文档、架构任务前，先明确回答：

1. 是否新增模块、crate 或文件？
2. 是否包含否定性结论（如“X 没有 Y”“X 缺 Y”“X 独家”）？
3. 是否做跨项目对账（codex、claw-code、ironclaw、desktop-client）？
4. 是否写架构对账文档（ADR、对比矩阵、14-md）？

判定规则：

- 四问全否：可直接进入开发。
- 任一为是：必须先做三层验证。

三层验证顺序：

1. Level 1：semantic_search（概念语义检索）
2. Level 2：vscode_listCodeUsages（符号引用核验）
3. Level 3：rg 或 grep（字面量定位）

若触发三层验证，commit message 建议增加：已检查 X 是否已有，结论：…

## 沟通规则

1. 目标不清晰时先澄清，不靠猜测推进。
2. 若用户路径不是最短路径，应明确给出更优方案和理由。

## Skills 执行规范（默认）

非平凡实现后、提交前、PR 前，按以下顺序执行：

1. code-quality-audit
2. code-simplifier（verbatim port 场景跳过）
3. adr-compliance-check（仅触及 ADR 红线场景）
4. code-review-expert

可跳过场景：

- 单字符或单行 typo 修复
- 仅文档或注释修改
- 仅 fmt 或 lint 自动修复
- 用户明确要求先跳过 review

## 图谱工具协同规范（code-review-graph + Graphify）

结论：两者可配合使用，推荐“结构图谱用 code-review-graph，跨域语义图谱用 Graphify”的双轨模式。

1. code-review-graph（主）
- 适用：Rust/TS 代码结构、调用链、影响面、变更审查。
- 优势：增量快、查询稳定、结构化强（适合 PR review 与架构核验）。

2. Graphify（辅）
- 适用：跨仓库、跨类型资产（代码 + 文档 + 研究材料）的知识图谱与社区聚类。
- 优势：可输出 `graphify-out/graph.json`、`GRAPH_REPORT.md`、HTML 树图，适合探索性分析。

3. 协同使用顺序（默认）
- 第一步：先用 code-review-graph 做精确定位（callers/callees/importers/影响半径）。
- 第二步：再用 Graphify 做跨仓库语义探索（`query`/`path`/`explain`）。
- 第三步：结论落文档时，保留两类证据：结构证据（code-review-graph）+ 语义证据（Graphify）。

4. 本仓建议拓扑
- 分仓维护 Graphify 图：`crates/`、`desktop-client/ironclaw/`、`claude-code-main/`、`codex-cli-main/`、`ironclaw-main/`、`claw-code/`。
- 需要全局对比时，使用 Graphify merge 生成根目录 `graphify-out/merged-graph.json`。

5. Graphify skill 可用性（官方）
- 官方 skill 已可用，触发词：`/graphify`。
- Copilot skill 安装位置：`~/.copilot/skills/graphify/SKILL.md`。
- 在 agent 场景中可直接用 `/graphify <path>`、`/graphify <path> --update`、`/graphify query "..."`。

6. 产物管理
- `graphify-out/` 及其 cache 属可再生产物，默认不入库（见 `.gitignore` 规则）。

## GitHub PR 最小流程

1. 先完成当前切片实现与最小充分验证。
2. commit 后 push 分支，再开 PR。
3. PR 描述必须包含：背景目标、改动范围、非目标、验证命令结果、后续计划。
4. stacked PR 必须写清 base、merge 顺序、先看哪个 PR。
5. 合并 stacked 下层 PR 时二选一：
   - 路径 A：先不删下层分支，待上层全部完成后统一清理。
   - 路径 B：下层分支删除前，立即把上层 PR base 切回 xClaw（或仍存活的中间分支）。
6. 开 PR 后必须用 gh pr checks <PR#> --watch --fail-fast 等待 CI，不用 sleep 轮询。

## Rust 本地最小验证门

按顺序执行：

1. cargo check -p <touched-crate> --tests
2. cargo nextest run -p <touched-crate>
3. cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/<base-branch>
4. cargo clippy --no-deps -p <touched-crate> --all-targets -- -D warnings

补充约束：

- 默认使用 cargo nextest run，不用 cargo test（仅 nextest 缺失时回退）。
- 不在本地跑 workspace 级 heavy build、workspace clippy、workspace nextest（由 CI 兜底）。

## 测试与专项规则（索引）

- 通用测试策略与历史教训：docs/testing-guide.md
- 架构与复用指南：docs/architecture-guide.md
- Rust 编码规范：.kiro/steering/rust-coding-standards.md
- Admin Backend 冒烟：admin-backend/tests/integration_smoke_tests.rs
- Tauri 命令契约：desktop-client/tests/tauri_command_contract_tests.rs
- 启动时序测试：desktop-client/src/engine_startup_tests.rs

## 反模式（禁止）

- 跳过任务启动 4 问，直接凭感觉下结论。
- 用字面量搜索替代语义搜索得出架构结论。
- 在否定性结论场景缺少双证据就落笔。
- stacked PR 不维护 base 关系，导致上层 PR 被自动关闭。
- 明知已完成闭环里程碑，仍在同一会话无限追加阶段。

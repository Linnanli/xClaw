---
name: multi-agent-workflow-creator
description: "创建或更新 Copilot CLI 多 agent 工作流。用于根据开发者要并行开发的模块，生成或重配 orchestrator、worker custom agents、hooks、脚本骨架和状态模板。触发词：创建多 agent 工作流、生成 orchestrator、生成 worker agents、根据模块创建 agents、配置 hooks 监控 agent、Copilot CLI 并行开发、worktree agent 编排。"
argument-hint: "输入要并行开发的模块、是否启用 Copilot CLI/hooks、是否需要 foundation worker"
user-invocable: true
---

# Multi-Agent Workflow Creator

这个 skill 不负责运行多 agent 工作流，而是负责创建、更新和重配这套工作流资产。

适用场景：

- 用户想把一套多 agent 协作流程固化下来
- 用户要根据若干业务模块生成 orchestrator 和 worker agents
- 用户要增加 hooks、日志、脚本、状态模板
- 用户要把已有设计文档落成可执行工作流骨架

不适用场景：

- 直接开发某个业务模块
- 运行中的实时调度
- 单独修复某个 agent 的运行时故障

## 产出目标

根据用户需求，创建或更新以下资产中的一部分或全部：

1. 正式设计文档
2. orchestrator custom agent
3. 按模块拆分的 worker custom agents
4. hooks 配置
5. scripts/agents 下的启动、汇总、检查脚本骨架
6. docs/plans/agent-runs 下的状态模板

参考 [资产映射](./references/artifact-map.md)。

## 工作流程

### 第一步：检查现状

先检查仓库中是否已存在以下内容：

1. `.github/agents/`
2. `.github/hooks/`
3. `.github/skills/`
4. `scripts/agents/`
5. `docs/plans/`
6. 与多 agent 工作流相关的现有设计文档或模板

如果已有同名或高相似度工作流，优先更新而不是重复创建。

### 第二步：询问输入

必须先问清以下问题，不能跳过：

1. 要并行开发哪些模块或功能线？
2. 每条功能线是否需要单独 worker？
3. 是否使用 Copilot CLI 作为运行入口？
4. 是否需要 hooks 记录日志和阻断高冲突路径？
5. 是否已有固定 worktree 名称或路径？
6. 是否需要 foundation worker 处理共享抽象？
7. 这次是“只生成设计/配置”，还是“直接生成可执行骨架”？

如果当前对话已经提供了答案，直接提取，不重复追问。

参考 [输入与分支决策](./references/intake-and-decisions.md)。

### 第三步：确定拓扑

至少输出以下决策：

1. 是否需要 orchestrator
2. 需要几个 worker，以及每个 worker 对应哪个模块
3. 是否需要 foundation worker
4. 运行模式是 CLI 模式还是手动模式
5. 是否生成 hooks 和脚本骨架

默认规则：

1. 一个模块对应一个 worker
2. 只有共享抽象冲突明显时才引入 foundation worker
3. 若 CLI 条件未满足，生成手动模式骨架，不生成 CLI 专属运行脚本

### 第四步：生成资产

按以下顺序创建或更新资产：

1. 正式设计文档
2. orchestrator agent
3. worker agents
4. hooks 配置
5. scripts/agents 脚本骨架
6. 状态模板与审查模板

角色定义、模板字段和状态协议参考 [输出契约](./references/output-contracts.md)。

### 第五步：校验

生成后至少做以下检查：

1. skill、agent、hook 文件位置是否正确
2. YAML frontmatter 是否有效
3. skill 名称是否与目录名一致
4. agent 的职责边界是否互斥、是否覆盖所有模块
5. hooks 是否只承担日志和护栏，不承担高层决策
6. 是否避免重复生成相同文档或模板

### 第六步：交付说明

完成后必须向用户说明：

1. 本次创建了哪些资产
2. 哪些资产只是骨架，后续仍需接入 Copilot CLI 或脚本
3. 下一步建议先做什么

## 关键原则

1. skill 负责创建和重配工作流，不负责替代 orchestrator 运行工作流
2. custom agents 负责长期角色，不要把所有逻辑挤进一个 skill
3. hooks 只做确定性日志与护栏，不做高层调度决策
4. 优先生成最小可运行骨架，不一次性生成过多长期没人维护的文件
5. 若已有正式设计文档，优先引用并对齐，不要复制出第二份冲突规范

## 输出风格

输出应包含：

1. 工作流拓扑摘要
2. 生成资产清单
3. 未覆盖或待确认项
4. 建议的下一步

# 资产映射

## 推荐资产清单

### 1. 正式设计文档

- `docs/plans/copilot-cli-multi-agent-orchestration-design.md`

用途：定义工作流原则、阶段、角色边界、状态协议和测试闸门。

### 2. custom agents

- `.github/agents/fullstack-orchestrator.agent.md`
- `.github/agents/<module>-worker.agent.md`
- `.github/agents/foundation-worker.agent.md`（仅在需要时）

用途：定义长期角色，而不是一次性任务。

### 3. hooks

- `.github/hooks/copilot-cli-hooks.json`

用途：记录 preToolUse、postToolUse、agentStop，或执行高冲突路径护栏。

### 4. 脚本骨架

- `scripts/agents/start-orchestrator.sh`
- `scripts/agents/start-<module>-worker.sh`
- `scripts/agents/collect-agent-status.sh`
- `scripts/agents/check-conflicts.sh`

用途：承载启动、汇总、检查等确定性操作。

### 5. 状态模板

- `docs/plans/agent-runs/<run-id>/`
- `docs/plans/agent-runs/templates/`

用途：承载首轮 fullstack-audit 输出、轮次状态摘要、主控汇总结果。

## 最小生成集

如果用户只要最小骨架，至少生成：

1. 正式设计文档
2. orchestrator agent
3. 至少一个 worker agent
4. 一个状态模板

## 完整生成集

如果用户要求可执行骨架，生成：

1. 正式设计文档
2. orchestrator agent
3. worker agents
4. hooks
5. scripts/agents
6. 状态模板

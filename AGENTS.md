# Agent Rules

## Feature Parity Update Policy

- If you change implementation status for any feature tracked in `FEATURE_PARITY.md`, update that file in the same branch.
- Do not open a PR that changes feature behavior without checking `FEATURE_PARITY.md` for needed status updates (`❌`, `🚧`, `✅`, notes, and priorities).

## Skills System

本项目在 `.trae/skills/` 目录下定义了一系列可复用的技能模块，用于指导 AI Agent 执行特定类型的任务。每个技能都包含详细的指导原则、工具支持和最佳实践。

### 可用技能

当前可用的技能包括：

1. **Code Quality Gate** (`code-quality-gate`) - 全面的代码质量门禁检查
2. **Engineer Mindset Coding** (`engineer-mindset-coding`) - 符合工程最佳实践的生产级代码
3. **TDD Practitioner** (`tdd-practitioner`) - 测试驱动开发方法论

### 使用方式

- **查看技能详情**：每个技能在 `.trae/skills/<skill-name>/SKILL.md` 文件中都有完整的文档
- **调用技能**：在与 AI Agent 交互时，可以明确引用技能名称（如 "使用 TDD Practitioner 技能实现此功能"）
- **技能协作**：这些技能可以组合使用，例如：
  - `engineer-mindset-coding` 定义代码结构和质量标准
  - `tdd-practitioner` 通过测试驱动实现这些标准
  - `code-quality-gate` 验证最终代码符合所有质量要求

### 扩展技能

如需添加新技能，在 `.trae/skills/` 下创建新目录，包含 `SKILL.md` 文件，遵循现有技能的 YAML front-matter 格式。

## MCP Interactive Feedback 规则

注意在任务完成前根据mcp-feedback-enhanced工具的要求进行工具调用

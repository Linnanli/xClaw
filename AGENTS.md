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

**强制要求**：每次AI Agent执行完成前必须调用MCP Interactive Feedback工具。

### 执行规范

1. **调用时机**：
   - 在任务开始执行时，首先调用MCP Interactive Feedback工具
   - 在任务执行过程中，根据用户反馈调整后再次调用
   - 在任务最终完成前，必须调用MCP Interactive Feedback工具进行确认

2. **工具调用**：
   - 使用 `mcp_mcp_feedback_enhanced_interactive_feedback` 工具（已验证可用）
   - 必需参数：`summary`（任务执行摘要）、`project_directory`（项目目录）
   - 可选参数：`timeout`（超时时间，默认600秒）
   - **验证**：该工具已在当前环境中多次成功调用，确保Agent能够正常使用

3. **反馈处理**：
   - 如果用户反馈包含具体指令，必须按照指令调整执行
   - 如果用户反馈为"好的执行"或类似确认，可以继续执行
   - 如果用户反馈指出问题，必须修正问题后重新调用

4. **验证机制**：
   - 每次Agent执行必须包含至少一次MCP Interactive Feedback调用
   - 未调用MCP Interactive Feedback的Agent执行视为不完整
   - 用户可以通过检查执行日志验证是否遵循此规则

### 目的
确保AI Agent与用户保持交互，及时获取反馈，避免遗漏关键步骤，提高任务执行质量。

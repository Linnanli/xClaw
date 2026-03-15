# Agent Rules

## 语言规则 (Language Rules)

**所有回答必须用中文。** (All responses must be in Chinese.)

## Feature Parity Update Policy

- If you change implementation status for any feature tracked in `FEATURE_PARITY.md`, update that file in the same branch.
- Do not open a PR that changes feature behavior without checking `FEATURE_PARITY.md` for needed status updates (`❌`, `🚧`, `✅`, notes, and priorities).

## Desktop Client Feature Implementation Policy

- Desktop client features should reuse existing web gateway API implementations when possible
- When implementing a new desktop client feature:
  1. Check if the corresponding web API endpoint already exists in `src/channels/web/handlers/`
  2. If yes, create a Tauri command wrapper that calls the web API
  3. Update `DESKTOP_CLIENT_FEATURE_CHECKLIST.md` with the implementation status
  4. Document the feature in the checklist with the date completed

### Available Web API Endpoints for Reuse

- **Memory APIs**: `memory_tree_handler`, `memory_list_handler`, `memory_read_handler`, `memory_write_handler`, `memory_search_handler`
- **Chat APIs**: `chat_send_handler`, `chat_history_handler`, `chat_threads_handler`, `chat_new_thread_handler`, `chat_events_handler`
- **Jobs APIs**: `jobs_list_handler`, `jobs_detail_handler`, `jobs_cancel_handler`, `jobs_restart_handler`, `jobs_events_handler`
- **Extensions APIs**: `extensions_list_handler`, `extensions_install_handler`, `extensions_uninstall_handler`
- **Skills APIs**: `skills_list_handler`, `skills_install_handler`, `skills_uninstall_handler`
- **Routines APIs**: `routines_list_handler`, `routines_create_handler`, `routines_delete_handler`, `routines_trigger_handler`

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

**强制要求**：每次AI Agent执行完成前必须调用MCP Interactive Feedback工具，确保与用户保持交互，及时获取反馈，避免遗漏关键步骤，提高任务执行质量。

### 执行规范

1. **调用时机**：
   - 在任务开始执行时，首先调用MCP Interactive Feedback工具
   - 在任务执行过程中，根据用户反馈调整后再次调用
   - 在任务最终完成前，必须调用MCP Interactive Feedback工具进行确认

2. **工具调用**：
   - 使用 `mcp_mcp_feedback_enhanced_interactive_feedback` 工具（已验证可用）
   - 必需参数：`summary`（任务执行摘要）、`project_directory`（项目目录）
   - 可选参数：`timeout`（超时时间，默认600秒）

3. **反馈处理**：
   - 如果用户反馈包含具体指令，必须按照指令调整执行
   - 如果用户反馈为"好的执行"或类似确认，可以继续执行
   - 如果用户反馈指出问题，必须修正问题后重新调用

4. **验证机制**：
   - 每次Agent执行必须包含至少一次MCP Interactive Feedback调用
   - 未调用MCP Interactive Feedback的Agent执行视为不完整
   - 用户可以通过检查执行日志验证是否遵循此规则

## Rust 代码开发规则

**强制要求**：每次为 desktop-client 或其他 Rust 项目添加新的功能或逻辑代码时，必须遵循以下技能和流程。

### 适用范围

- 新增 Rust 函数、方法、模块或结构体
- 修改现有逻辑的重大重构
- 添加新的 API 端点或命令处理器
- 不适用于：仅修复 bug 的最小改动、注释更新、格式调整

### 必须遵循的技能

1. **TDD Practitioner** (`tdd-practitioner`)
   - 先编写测试用例（Red 阶段）
   - 再实现功能代码（Green 阶段）
   - 最后重构优化（Refactor 阶段）
   - 所有单元测试必须通过

2. **Engineer Mindset Coding** (`engineer-mindset-coding`)
   - 代码结构清晰，易于维护
   - 完整的错误处理和边界情况处理
   - 充分的代码注释和文档
   - 遵循 Rust 最佳实践和项目约定

3. **Code Quality Gate** (`code-quality-gate`)
   - 代码审查检查清单
   - 性能和安全性评估
   - 编译警告检查（应为 0 个警告）
   - 测试覆盖率检查

### 执行流程

```
1. 理解需求
   ↓
2. 查看技能文档（.trae/skills/）
   ↓
3. 编写测试用例（TDD - Red）
   ↓
4. 实现功能代码（TDD - Green）
   ↓
5. 代码重构优化（TDD - Refactor）
   ↓
6. 运行 cargo build 完整编译验证
   ↓
7. 运行单元测试验证
   ↓
8. 执行代码质量检查
   ↓
9. 更新相关文档和检查清单
```

### 检查清单

- [ ] 已查看相关技能文档
- [ ] 已编写单元测试（测试优先）
- [ ] 已实现功能代码
- [ ] 已进行代码重构优化
- [ ] 已运行 cargo build 验证编译
- [ ] 编译通过且无错误（警告应为 0）
- [ ] 所有单元测试通过
- [ ] 代码符合项目约定和最佳实践
- [ ] 已更新 DESKTOP_CLIENT_FEATURE_CHECKLIST.md
- [ ] 已更新相关代码注释和文档

## 外部库API使用验证策略

**目的**：防止因使用错误的API版本或模式而导致大量编译错误。

### 问题案例

在desktop-client开发中，曾因使用错误的libSQL 0.6 API导致59个编译错误：
- 错误使用：`Connection::open()` 直接打开连接
- 正确使用：`libsql::Builder::new_local(path).build().await` 然后 `db.connect()`

### 预防策略

1. **查找现有用法**
   - 在实现新功能前，先搜索项目中该库的现有使用模式
   - 使用 `grep` 或 `rg` 搜索关键API调用
   - 示例：`rg "libsql::" src/` 查找libSQL的使用方式

2. **参考项目代码**
   - 优先参考项目内已有的实现，而不是外部文档
   - 项目代码反映了实际使用的版本和配置
   - 示例：参考 `src/db/libsql/mod.rs` 了解正确的libSQL使用模式

3. **完整编译验证**
   - 使用 `getDiagnostics` 进行快速检查
   - **必须**使用 `cargo build` 进行完整编译验证
   - `getDiagnostics` 可能遗漏跨模块的类型错误

4. **分阶段实现**
   - 先实现核心功能并验证编译
   - 再逐步添加辅助功能
   - 避免一次性添加大量未验证的代码

5. **依赖版本检查**
   - 检查 `Cargo.toml` 中的依赖版本
   - 注意主版本号变化可能带来的breaking changes
   - 查看依赖的CHANGELOG了解API变更

### 工作流程

```
1. 需要使用外部库API
   ↓
2. 搜索项目中该库的现有用法
   ↓
3. 参考现有代码实现新功能
   ↓
4. 使用 getDiagnostics 快速检查
   ↓
5. 使用 cargo build 完整验证
   ↓
6. 如有错误，对比现有用法找出差异
   ↓
7. 修正后重新验证
```

### 检查清单

- [ ] 已搜索项目中该库的现有使用模式
- [ ] 已参考项目内类似功能的实现
- [ ] 已检查 Cargo.toml 中的依赖版本
- [ ] 已使用 cargo build 完整编译验证
- [ ] 编译通过且无错误（警告可接受）
- [ ] 对于新增功能，已按照"Rust 代码开发规则"执行 TDD 和单元测试

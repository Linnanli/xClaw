# Agent Rules

## 语言规则

**所有回答必须用中文。**

---

## 架构规则：代码复用优先

### 核心原则

Desktop Client 和 Admin Backend 新增功能时，**禁止重复实现**主项目已有的能力。

### 复用优先级

**Desktop Client**：
1. 复用 Web Gateway API（`src/channels/web/handlers/`）→ 创建 Tauri 命令包装器
2. 复用共享 Crate（`crates/ironclaw_auth/` 等）
3. 从主项目提取新的共享 Crate
4. 仅 Desktop Client 特有功能才独立实现

**Admin Backend**：
1. 通过 `ironclaw` crate 依赖主项目核心功能
2. 复用共享 Crate
3. 创建新的共享 Crate
4. 仅管理后台特有功能才独立实现

### 共享 Crate 规范

参考 `crates/ironclaw_auth/` 的模式。共享 crate 不应依赖应用层代码，API 变更需同步更新所有使用方。

### 可复用的 Web Gateway API

实现 Desktop Client 功能前必须检查：Memory、Chat、Jobs、Extensions、Skills、Routines、Logs、Approval 相关 handler 是否已存在于 `src/channels/web/handlers/`。

详细的架构说明、流程图和代码示例见 `docs/architecture-guide.md`。

---

## Rust 开发规范

### 开发流程（TDD）

新增 Rust 函数/模块/结构体/API 端点时（不含最小 bug 修复）：
1. 编写测试用例（Red）
2. 实现功能代码（Green）
3. 重构优化（Refactor）
4. `cargo build` 完整编译验证（不只是 `cargo test`）
5. 0 编译错误，0 编译警告

### 外部库使用

- **先搜索项目中该库的现有用法**（`rg "libsql::" src/`），参考项目代码而非外部文档
- **优先使用成熟社区库**，避免重复造轮子（如 `config-rs`、`reqwest`、`serde`）
- 检查 `Cargo.toml` 中的依赖版本，注意 breaking changes
- 分阶段实现，先验证核心功能编译通过

### 技能参考

详细的编码规范见 `.kiro/steering/` 下的技能文档：
- `engineer-mindset-coding.md` — 工程最佳实践
- `tdd-practitioner.md` — TDD 方法论
- `code-quality-gate.md` — 代码质量门禁

---

## 测试规则

### 核心原则

> **测试的目标不是追求高覆盖率，而是确保系统在所有情况下都是安全的。**

1. **测试失败路径和成功路径同等重要** — 每个功能都要有失败路径测试
2. **安全功能必须"故障安全"（Fail-Safe）** — 失败时拒绝操作，而非允许操作（Fail-Open）
3. **真实环境测试不可或缺** — 模拟测试只能验证逻辑，无法验证集成
4. **契约测试验证业务目标可达成** — 不只是"能解析"，而是"够用"
5. **安全审计测试验证无泄露** — 日志、错误信息、网络响应中不得包含原始敏感数据
6. **`cargo test` 通过 ≠ `cargo build` 通过** — 测试可能绕过有问题的代码
7. **测试不应固化错误设计** — 测试断言必须从业务目标出发，而非从实现假设出发

### 测试维度矩阵

| 测试维度 | 正常路径 | 错误路径 | 降级逻辑 | 真实环境 | 契约测试 | 安全审计 |
|---------|---------|---------|---------|---------|---------|---------|
| 单元测试 | ✅ 必须 | ✅ 必须 | ⚠️ 如有 | N/A | N/A | ✅ 必须 |
| 集成测试 | ✅ 必须 | ✅ 必须 | ⚠️ 如有 | ✅ 推荐 | ✅ 必须 | ✅ 必须 |
| E2E 测试 | ✅ 必须 | ✅ 推荐 | ⚠️ 如有 | ✅ 必须 | N/A | ✅ 推荐 |

### 覆盖率目标

| 维度 | 功能模块 | 安全模块 |
|------|---------|---------|
| 单元测试 | >90% | 100% |
| 集成测试 | >80% | 100% |
| 失败路径 | >80% | 100% |
| 契约测试 | >90% | 100% |
| 安全审计 | — | 100% |

### 测试文件组织

```
tests/
├── {module}_unit_tests.rs           # 单元测试
├── {module}_failure_tests.rs        # 失败路径测试
├── {module}_integration_tests.rs    # 集成测试
├── {module}_contract_tests.rs       # 契约测试
├── {module}_security_audit_tests.rs # 安全审计测试
├── {module}_reliability_tests.rs    # 可靠性测试
├── {module}_regression_tests.rs     # 回归测试
└── integration_smoke_tests.rs       # 编译+迁移+路由冒烟（全局唯一）
```

### 测试命名规范

- 需求测试: `req_{module}_{id}_{description}`
- 安全测试: `test_security_{attack_type}`
- 失败路径: `test_failure_{scenario}`
- 契约测试: `test_contract_{interface}_{case}`
- 审计测试: `test_audit_{security_concern}`
- 可靠性: `test_{failure_scenario}_recovery`
- 回归测试: `test_{feature}_backward_compatibility`

### 历史教训摘要

以下是项目中真实发生过的测试盲区，新功能开发时必须警惕：

| 盲区 | 教训 | 防护措施 |
|------|------|---------|
| DLP 失败路径缺失 | 测试只覆盖成功路径，降级逻辑允许原始消息发送 | 每个功能必须有失败路径测试 |
| model-configs 404 | `cargo test` 通过但 `cargo build` 失败，迁移文件存在但未执行 | `integration_smoke_tests.rs` 三层防护 |
| client-models 缺字段 | 测试主动断言了错误行为，把错误设计固化为"规范" | 契约测试从业务目标出发 |
| SSE 事件名不匹配 | 测试用模拟数据，未验证真实后端事件格式 | 真实环境集成测试 + 参考已有实现 |
| Tauri state not managed | 单元测试绕过 Tauri 状态注入，运行时 panic | 启动时序测试 + 冒烟测试 |

详细案例分析、代码示例和 E2E 测试模式见 `docs/testing-guide.md`。

---

## Admin Backend 冒烟测试（integration_smoke_tests.rs）

**强制要求**：每次新增迁移文件时，必须同步在 `integration_smoke_tests.rs` 的 `required` 列表中追加对应条目。

三层防护：
1. **编译检查** — 调用 `create_router()` 强制编译所有 handler
2. **迁移完整性** — 查询 `information_schema` 验证每个迁移对应的表存在
3. **HTTP 路由冒烟** — 用 `tower::ServiceExt::oneshot` 验证路由不返回 404

---

## Tauri 命令注册契约测试

所有 Tauri 命令通过 `all_tauri_commands!()` 宏集中管理（`desktop-client/src/lib.rs`），`main.rs` 和测试共用同一份注册表。

新增命令流程：
1. 在 `src/ipc.rs` 或 `src/commands.rs` 实现命令
2. 在 `all_tauri_commands!()` 宏中添加
3. 在 `tests/tauri_command_contract_tests.rs` 的 `FRONTEND_INVOKED_COMMANDS` 中添加
4. 运行 `cargo test --test tauri_command_contract_tests`
5. 前端添加对应 `invoke()` 调用

> 普通单元测试直接调用 Rust 函数，绕过 IPC 路由，无法捕获 "Command not found" 错误。契约测试是唯一能在 CI 阶段验证 IPC 路由完整性的方法。

---

## 启动时序测试（Desktop Client）

涉及 Tauri 状态管理的改动，必须运行：
```bash
cargo test -p desktop-client --lib engine_startup_tests
```

覆盖维度：启动时序（状态转换）、失败路径（未就绪时友好错误）、并发安全（多线程初始化）、冒烟测试（模拟真实启动流程）。

> 单元测试通过 ≠ 运行时安全。涉及异步状态注入的场景，必须用时序测试和冒烟测试覆盖。

参考：`desktop-client/src/engine_startup_tests.rs`

---

## E2E 测试

前端功能完成后必须编写 E2E 测试。推荐 Cypress（UI 测试）和 Playwright（SSE/跨浏览器测试）。

关键要求：
- 同时覆盖模拟环境和真实环境
- 新功能先检查主项目是否有相同实现（如 `src/channels/web/static/app.js`）
- 验证前后端实际通信格式，不只依赖 mock

参考：`admin-backend/frontend/cypress/`、`docs/testing-guide.md`

---

## 统一检查清单

### 功能开发

- [ ] 已检查主项目/共享 crate 是否有可复用能力
- [ ] 已遵循 TDD 流程（测试优先）
- [ ] 单元测试覆盖正常路径 + 错误路径
- [ ] 已编写集成测试和契约测试
- [ ] `cargo build` 编译通过，0 错误 0 警告
- [ ] 所有测试通过

### 安全功能（额外）

- [ ] 失败路径测试覆盖率 100%
- [ ] 安全审计测试：敏感信息不泄露（日志、错误、网络）
- [ ] 降级逻辑采用 Fail-Safe 设计
- [ ] 契约测试验证业务目标可达成

### 新增迁移

- [ ] 已在 `integration_smoke_tests.rs` 的 `required` 列表追加表名验证

### 新增 Tauri 命令

- [ ] 已添加到 `all_tauri_commands!()` 宏
- [ ] 已添加到 `FRONTEND_INVOKED_COMMANDS`
- [ ] `cargo test --test tauri_command_contract_tests` 通过

---

## 参考文档索引

- `docs/testing-guide.md` — 测试代码示例、历史教训详细分析、E2E 测试模式
- `docs/architecture-guide.md` — 架构详细说明、复用流程图、代码示例
- `desktop-client/DLP_LESSONS_SUMMARY.md` — DLP 测试盲区教训
- `desktop-client/DLP_TESTING_LESSONS_LEARNED.md` — 详细经验教训
- `desktop-client/DLP_SUPPLEMENTARY_TEST_PLAN.md` — 补充测试计划
- `desktop-client/src/engine_startup_tests.rs` — 启动时序测试
- `admin-backend/tests/integration_smoke_tests.rs` — 冒烟测试
- `desktop-client/tests/tauri_command_contract_tests.rs` — Tauri 命令契约测试
- `admin-backend/frontend/cypress/` — Cypress E2E 测试
- `.kiro/steering/` — 编码技能文档（TDD、工程实践、质量门禁）

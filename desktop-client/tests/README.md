# Desktop Client 测试说明

> 本文档说明 Desktop Client 的测试组织和运行方式

## 测试结构

### Rust 测试 (`tests/` 目录)

```
tests/
├── auth_*.rs (7 个认证测试)
│   ├── auth_integration_tests.rs
│   ├── auth_property_tests.rs
│   ├── auth_regression_tests.rs
│   ├── auth_reliability_tests.rs
│   ├── auth_requirements_tests.rs
│   ├── auth_session_tests.rs
│   └── auth_token_manager_tests.rs
├── api_*.rs (5 个 API 测试)
│   ├── api_client_tests.rs
│   ├── api_error_tests.rs
│   ├── api_integration_tests.rs
│   └── api_property_tests.rs
├── chat_integration_tests.rs
├── sse_*.rs (2 个 SSE 测试)
│   ├── sse_browser_simulation_tests.rs
│   └── sse_integration_tests.rs
├── *_property_tests.rs (6 个属性测试)
│   ├── extension_manager_property_tests.rs
│   ├── offline_mode_property_tests.rs
│   ├── policy_sync_property_tests.rs
│   ├── routine_manager_property_tests.rs
│   ├── storage_property_tests.rs
│   └── ui_property_tests.rs
├── test_*.rs (10 个其他测试)
└── support/ (测试辅助代码)
    ├── mod.rs
    ├── generators.rs
    ├── test_fixture.rs
    └── test_server.rs
```

### 前端测试 (`ui/src/` 目录)

```
ui/src/app/
├── components/
│   ├── common/__tests__/ (7 个组件测试)
│   │   ├── ConnectionStatus.test.tsx
│   │   ├── DeleteConfirmDialog.test.tsx
│   │   ├── DynamicWatermark.test.tsx
│   │   ├── DynamicWatermark.requirements.test.tsx
│   │   ├── DynamicWatermark.security.test.tsx
│   │   ├── MessageActions.test.tsx
│   │   └── MessageEditor.test.tsx
│   └── tabs/__tests__/ (4 个 Tab 测试)
│       ├── JobsTab.test.tsx
│       ├── MemoryTab.test.tsx
│       ├── MemoryTab.requirements.test.tsx
│       └── MemoryTab.security.test.tsx
├── contexts/__tests__/ (1 个 Context 测试)
│   └── ThemeContext.test.tsx
├── hooks/__tests__/ (3 个 Hook 测试)
│   ├── useAiChatTauri.test.ts
│   ├── useSSEConnection.test.ts
│   └── useWatermark.test.ts
└── services/__tests__/ (2 个服务测试)
    ├── sseService.test.ts
    └── sseService.integration.test.ts
```

## 测试命名规范

### Rust 测试

- `{module}_unit_tests.rs` - 单元测试
- `{module}_integration_tests.rs` - 集成测试
- `{module}_property_tests.rs` - 属性测试 (使用 proptest)
- `{module}_reliability_tests.rs` - 可靠性测试
- `{module}_requirements_tests.rs` - 需求级测试
- `{module}_regression_tests.rs` - 回归测试
- `{module}_session_tests.rs` - 会话测试
- `test_{specific}.rs` - 特定功能测试

### 前端测试

- `{Component}.test.tsx` - 组件单元测试
- `{Component}.requirements.test.tsx` - 需求级测试
- `{Component}.security.test.tsx` - 安全测试
- `{hook}.test.ts` - Hook 测试
- `{service}.test.ts` - 服务单元测试
- `{service}.integration.test.ts` - 服务集成测试

## 运行测试

### 运行所有 Rust 测试

```bash
cd desktop-client
cargo test
```

### 运行特定模块的测试

```bash
# 认证测试
cargo test --test auth_integration_tests

# API 测试
cargo test --test api_client_tests

# 聊天测试
cargo test --test chat_integration_tests
```

### 运行所有前端测试

```bash
cd desktop-client/ui
npm run test
```

### 运行特定前端测试

```bash
# 运行特定文件
npm run test -- useAiChatTauri.test.ts

# 运行特定目录
npm run test -- components/common
```

### 运行 E2E 测试

```bash
cd desktop-client/ui
npm run test:e2e
```

## 测试覆盖率

### 生成 Rust 覆盖率报告

```bash
cd desktop-client
cargo tarpaulin --out Html --output-dir coverage/
```

### 生成前端覆盖率报告

```bash
cd desktop-client/ui
npm run test:coverage
```

## 测试类型说明

### 单元测试

测试独立的函数和模块,不依赖外部服务。

**示例**: `auth_token_manager_tests.rs`

### 集成测试

测试多个模块的交互,可能依赖外部服务。

**示例**: `auth_integration_tests.rs`, `api_integration_tests.rs`

### 属性测试

使用随机生成的输入测试属性和不变量。

**示例**: `auth_property_tests.rs`, `api_property_tests.rs`

### 可靠性测试

测试故障恢复、容错和并发场景。

**示例**: `auth_reliability_tests.rs`

### 需求级测试

验证功能需求是否满足。

**示例**: `auth_requirements_tests.rs`, `MemoryTab.requirements.test.tsx`

### 安全测试

测试安全相关功能,如恶意输入防护。

**示例**: `DynamicWatermark.security.test.tsx`

### 回归测试

防止已修复的 bug 再次出现。

**示例**: `auth_regression_tests.rs`

## 测试辅助工具

### Rust 测试辅助

位于 `tests/support/` 目录:

- `generators.rs` - 测试数据生成器
- `test_fixture.rs` - 测试夹具
- `test_server.rs` - 测试服务器

### 前端测试辅助

- `@testing-library/react` - React 组件测试
- `vitest` - 测试运行器
- `@testing-library/user-event` - 用户交互模拟

## 测试最佳实践

### 1. 测试命名

```rust
// ✅ 好的命名
#[test]
fn test_verify_password_with_correct_password() { }

// ❌ 不好的命名
#[test]
fn test1() { }
```

### 2. 测试隔离

每个测试应该独立,不依赖其他测试的状态。

```rust
// ✅ 好的做法
#[test]
fn test_feature() {
    let state = setup_test_state();
    // 测试逻辑
    cleanup(state);
}
```

### 3. 使用测试辅助函数

```rust
// 使用 support 模块中的辅助函数
use crate::support::generators::generate_test_user;

#[test]
fn test_user_creation() {
    let user = generate_test_user();
    // 测试逻辑
}
```

### 4. 测试覆盖率目标

根据 AGENTS.md 中的规则:

**功能模块**:
- 单元测试: >90% (正常路径), >80% (错误路径)
- 集成测试: >80% (正常路径), >70% (错误路径)
- E2E 测试: >75% (正常路径), >50% (错误路径)

**安全模块**:
- 单元测试: 100% (正常路径 + 错误路径)
- 集成测试: 100% (正常路径 + 错误路径)
- E2E 测试: >90% (正常路径), >80% (错误路径)

## 常见问题

### Q: 如何运行单个测试?

```bash
# Rust
cargo test test_name

# 前端
npm run test -- -t "test name"
```

### Q: 如何调试测试?

```bash
# Rust - 显示输出
cargo test -- --nocapture

# 前端 - 使用 VS Code 调试器
# 在测试文件中设置断点,按 F5
```

### Q: 测试失败怎么办?

1. 查看错误信息
2. 运行单个测试定位问题
3. 使用 `--nocapture` 查看输出
4. 检查测试数据和环境

### Q: 如何添加新测试?

1. 确定测试类型 (单元/集成/E2E)
2. 选择合适的文件或创建新文件
3. 遵循命名规范
4. 编写测试用例
5. 运行测试验证

## 相关文档

- [测试指南](../docs/guides/testing.md) - 详细的测试编写指南
- [AGENTS.md](../../AGENTS.md) - 测试覆盖率要求
- [测试覆盖率报告](../TEST_COVERAGE_REPORT.md) - 当前覆盖率

---

**注意**: 测试是代码质量的保证,请确保所有新功能都有相应的测试覆盖。

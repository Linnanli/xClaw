# 测试指南

> 如何为 Desktop Client 编写和运行测试

## 快速开始

### 运行测试

```bash
# Rust 测试
cd desktop-client
cargo test

# 前端测试
cd desktop-client/src-ui
npm run test

# E2E 测试
cd desktop-client/src-ui
npm run test:e2e
```

### 查看覆盖率

```bash
# Rust 覆盖率
cargo tarpaulin --out Html

# 前端覆盖率
npm run test:coverage
```

## 测试类型

### 1. 单元测试

测试独立的函数和模块。

**Rust 示例**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let token = generate_random_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
```

**前端示例**:
```typescript
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Button } from './Button';

describe('Button', () => {
  it('renders with text', () => {
    render(<Button>Click me</Button>);
    expect(screen.getByText('Click me')).toBeInTheDocument();
  });
});
```

### 2. 集成测试

测试多个模块的交互。

**Rust 示例**:
```rust
// tests/auth_integration_tests.rs
use desktop_client::auth::AuthManager;

#[tokio::test]
async fn test_auth_flow() {
    let mut auth = AuthManager::new();
    
    // 设置密码
    auth.setup_master_password("password123").unwrap();
    
    // 验证密码
    assert!(auth.verify_password("password123").is_ok());
    
    // 创建会话
    let session = auth.create_session("user".to_string()).unwrap();
    assert!(!session.is_expired());
}
```

**前端示例**:
```typescript
// src/app/hooks/__tests__/useAiChatTauri.test.ts
import { renderHook, waitFor } from '@testing-library/react';
import { useAiChatTauri } from '../useAiChatTauri';

describe('useAiChatTauri', () => {
  it('sends message and receives response', async () => {
    const { result } = renderHook(() => useAiChatTauri({
      threadId: 'test-thread',
    }));

    await result.current.sendMessage('Hello');
    
    await waitFor(() => {
      expect(result.current.messages).toHaveLength(2);
    });
  });
});
```

### 3. E2E 测试

测试完整的用户流程。

**Cypress 示例**:
```typescript
// cypress/e2e/chat_tauri_ipc.cy.ts
describe('Chat with Tauri IPC', () => {
  beforeEach(() => {
    cy.visit('/');
  });

  it('sends message and receives response', () => {
    cy.get('[data-testid="message-input"]').type('Hello');
    cy.get('[data-testid="send-button"]').click();
    
    cy.get('[data-testid="message-item"]')
      .should('have.length.greaterThan', 0);
  });
});
```

### 4. 属性测试

使用随机输入测试属性。

**Rust 示例**:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_token_always_64_chars(s in ".*") {
        let token = generate_random_token();
        prop_assert_eq!(token.len(), 64);
    }
}
```

## 测试覆盖率要求

根据 [AGENTS.md](../../AGENTS.md):

### 功能模块

| 测试类型 | 正常路径 | 错误路径 | 目标 |
|---------|---------|---------|------|
| 单元测试 | >90% | >80% | 必须 |
| 集成测试 | >80% | >70% | 必须 |
| E2E 测试 | >75% | >50% | 必须 |

### 安全模块

| 测试类型 | 正常路径 | 错误路径 | 安全审计 | 目标 |
|---------|---------|---------|---------|------|
| 单元测试 | 100% | 100% | 100% | 必须 |
| 集成测试 | 100% | 100% | 100% | 必须 |
| E2E 测试 | >90% | >80% | >90% | 必须 |

## 编写测试的最佳实践

### 1. 测试命名

使用描述性的名称:

```rust
// ✅ 好
#[test]
fn test_verify_password_with_correct_password_succeeds() { }

#[test]
fn test_verify_password_with_wrong_password_fails() { }

// ❌ 不好
#[test]
fn test1() { }

#[test]
fn test_password() { }
```

### 2. AAA 模式

Arrange (准备) → Act (执行) → Assert (断言)

```rust
#[test]
fn test_create_session() {
    // Arrange
    let mut auth = AuthManager::new();
    auth.setup_master_password("password").unwrap();
    
    // Act
    let session = auth.create_session("user".to_string()).unwrap();
    
    // Assert
    assert_eq!(session.user_id, "user");
    assert!(!session.is_expired());
}
```

### 3. 测试隔离

每个测试应该独立:

```rust
// ✅ 好 - 每个测试创建自己的状态
#[test]
fn test_feature_a() {
    let state = setup();
    // 测试 A
}

#[test]
fn test_feature_b() {
    let state = setup();
    // 测试 B
}

// ❌ 不好 - 测试共享状态
static mut SHARED_STATE: Option<State> = None;

#[test]
fn test_feature_a() {
    unsafe { SHARED_STATE = Some(setup()); }
    // 测试 A
}
```

### 4. 测试错误路径

不仅测试成功场景,也要测试失败场景:

```rust
#[test]
fn test_verify_password_success() {
    let mut auth = AuthManager::new();
    auth.setup_master_password("password").unwrap();
    assert!(auth.verify_password("password").is_ok());
}

#[test]
fn test_verify_password_failure() {
    let mut auth = AuthManager::new();
    auth.setup_master_password("password").unwrap();
    assert!(auth.verify_password("wrong").is_err());
}

#[test]
fn test_verify_password_not_set() {
    let auth = AuthManager::new();
    assert!(auth.verify_password("any").is_err());
}
```

### 5. 使用测试辅助函数

```rust
// tests/support/generators.rs
pub fn generate_test_user() -> User {
    User {
        id: "test-user".to_string(),
        name: "Test User".to_string(),
    }
}

// tests/auth_tests.rs
use crate::support::generators::generate_test_user;

#[test]
fn test_user_creation() {
    let user = generate_test_user();
    assert_eq!(user.id, "test-user");
}
```

## 测试工具

### Rust 测试工具

- `cargo test` - 测试运行器
- `proptest` - 属性测试
- `tokio::test` - 异步测试
- `wiremock` - HTTP mock
- `tempfile` - 临时文件
- `serial_test` - 串行测试

### 前端测试工具

- `vitest` - 测试运行器
- `@testing-library/react` - React 测试
- `@testing-library/user-event` - 用户交互
- `cypress` - E2E 测试

## 调试测试

### Rust 测试调试

```bash
# 显示输出
cargo test -- --nocapture

# 运行单个测试
cargo test test_name -- --nocapture

# 显示详细信息
cargo test -- --nocapture --test-threads=1
```

### 前端测试调试

```bash
# 使用 UI 模式
npm run test:ui

# 运行单个测试
npm run test -- -t "test name"

# 使用 VS Code 调试器
# 在测试文件中设置断点,按 F5
```

## 持续集成

### GitHub Actions

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      # Rust 测试
      - name: Run Rust tests
        run: cargo test
      
      # 前端测试
      - name: Run frontend tests
        run: |
          cd src-ui
          npm install
          npm run test
```

## 常见问题

### Q: 测试运行很慢怎么办?

A: 使用并行测试:

```bash
# Rust - 默认并行
cargo test

# 前端 - 使用 --run 避免 watch 模式
npm run test -- --run
```

### Q: 如何跳过某些测试?

```rust
#[test]
#[ignore]
fn expensive_test() {
    // 这个测试会被跳过
}

// 运行被忽略的测试
// cargo test -- --ignored
```

### Q: 如何测试异步代码?

```rust
#[tokio::test]
async fn test_async_function() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

### Q: 如何 mock 外部依赖?

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_api_call() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("GET"))
        .and(path("/api/data"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;
    
    // 测试逻辑
}
```

## 相关文档

- [测试文件组织](../../tests/README.md) - 测试文件结构说明
- [AGENTS.md](../../AGENTS.md) - 测试覆盖率要求
- [测试覆盖率报告](../../TEST_COVERAGE_REPORT.md) - 当前覆盖率

---

**记住**: 好的测试是代码质量的保证,投入时间编写测试是值得的!

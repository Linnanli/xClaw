---
inclusion: manual
---

# TDD Practitioner Skill

## 功能描述
实施测试驱动开发（TDD）方法，针对 Rust 语言和安全关键系统。遵循「红-绿-重构」循环，确保代码质量和测试覆盖率。

## TDD 核心工作流

### 🟥 Red：先写测试
- 定义接口和期望行为
- 编写失败测试（编译失败或断言失败）
- 关注边界条件和错误场景

### 🟩 Green：最小实现
- 实现最简单代码使测试通过
- 避免过度设计，保持代码简洁

### 🔄 Refactor：重构优化
- 改进代码结构而不改变行为
- 消除重复代码，提升可读性和性能

## Rust TDD 最佳实践

### 测试组织

```rust
#[cfg(test)]
mod tests {
    use super::*;

    mod happy_path {
        #[test]
        fn test_normal_operation() { /* 正常流程 */ }
    }

    mod edge_cases {
        #[test]
        fn test_empty_input() { /* 边界条件 */ }
    }

    mod error_conditions {
        #[test]
        fn test_invalid_input() { /* 错误场景 */ }
    }
}
```

### AAA 模式

```rust
#[test]
fn test_user_authentication() {
    // Arrange
    let authenticator = Authenticator::new();

    // Act
    let result = authenticator.authenticate("user", "password");

    // Assert
    assert!(result.is_ok());
}
```

### 属性测试

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_encryption_roundtrip(input in ".*") {
        let encrypted = encrypt(&input);
        let decrypted = decrypt(&encrypted).unwrap();
        prop_assert_eq!(input, decrypted);
    }
}
```

## 安全关键系统的测试策略

```rust
// 验证私钥不会被意外记录到日志
#[test]
fn test_private_key_never_logged() {
    let key = generate_private_key();
    let logs = capture_logs(|| { process_sensitive_data(&key); });
    assert!(!logs.contains(&key.to_string()));
}

// 错误密钥应解密失败
#[test]
fn test_decryption_fails_with_invalid_key() {
    let valid_key = generate_key();
    let invalid_key = generate_key();
    let encrypted = encrypt_with_key("data", &valid_key);
    let result = decrypt_with_key(&encrypted, &invalid_key);
    assert!(result.is_err());
}
```

## 检查清单

- [ ] 是否为每个需求编写了测试用例？
- [ ] 是否覆盖了正常流程和异常流程？
- [ ] 是否测试了边界条件和极端情况？
- [ ] 测试是否独立且可重复运行？
- [ ] 测试名称是否清晰表达了测试意图？

## 何时调用

- 开始新功能开发时（先写测试）
- 修复 bug 时（先写重现测试）
- 重构代码时（确保测试覆盖）
- 实现安全关键功能时

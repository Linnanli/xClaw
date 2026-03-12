---
name: "tdd-practitioner"
description: "Implements Test-Driven Development methodology with Rust-specific patterns. Invoke when practicing TDD, writing tests first, or ensuring test coverage for security-critical code."
---

# TDD Practitioner Skill

## 功能描述
此技能专门实施测试驱动开发（TDD）方法，特别针对 Rust 语言和安全关键系统。遵循「红-绿-重构」循环，确保代码质量和测试覆盖率。

## 辅助工具 (Tooling Support)

为了更高效地实施 TDD，本项目提供了以下自动化脚本：

1.  **[scaffold-tdd-module.sh](file:///c:/codes/ironclaw/scripts/scaffold-tdd-module.sh)**
    - **作用**：一键生成符合 TDD 规范的 Rust 模块。
    - **用法**：`./scripts/scaffold-tdd-module.sh <module_path.rs>`
    - **包含**：错误处理结构、`tracing` 日志、基础单元测试及 `proptest` 属性测试模板。

2.  **测试运行器推荐**
    - 使用 `cargo nextest run` 替代标准的 `cargo test` 以获得更快的并行测试体验。

## TDD 核心工作流

### 1. 🟥 Red: 先写测试
- 定义接口和期望行为
- 编写失败测试（编译失败或断言失败）
- 关注边界条件和错误场景

### 2. 🟩 Green: 最小实现
- 实现最简单代码使测试通过
- 避免过度设计
- 保持代码简洁

### 3. 🔄 Refactor: 重构优化
- 改进代码结构而不改变行为
- 消除重复代码
- 提升可读性和性能

## Rust TDD 最佳实践

### 测试组织
```rust
// 1. 模块化测试组织
#[cfg(test)]
mod tests {
    use super::*;
    
    // 2. 测试分类
    mod happy_path {
        use super::*;
        
        #[test]
        fn test_normal_operation() {
            // 正常流程测试
        }
    }
    
    mod edge_cases {
        use super::*;
        
        #[test]
        fn test_empty_input() {
            // 边界条件测试
        }
    }
    
    mod error_conditions {
        use super::*;
        
        #[test]
        fn test_invalid_input() {
            // 错误场景测试
        }
    }
}
```

### 测试模式

#### AAA 模式 (Arrange-Act-Assert)
```rust
#[test]
fn test_user_authentication() {
    // Arrange: 准备测试数据
    let authenticator = Authenticator::new();
    let username = "test_user";
    let password = "secure_password";
    
    // Act: 执行被测操作
    let result = authenticator.authenticate(username, password);
    
    // Assert: 验证结果
    assert!(result.is_ok());
    assert_eq!(result.unwrap().username, username);
}
```

#### 属性测试 (Property Testing)
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_encryption_roundtrip(input in ".*") {
        // 对任意输入，加密后再解密应该得到原输入
        let encrypted = encrypt(&input);
        let decrypted = decrypt(&encrypted).unwrap();
        prop_assert_eq!(input, decrypted);
    }
}
```

## 安全关键系统的测试策略

### 1. 安全属性测试
```rust
#[test]
fn test_private_key_never_logged() {
    let key = generate_private_key();
    
    // 验证私钥不会被意外记录到日志
    let logs = capture_logs(|| {
        process_sensitive_data(&key);
    });
    
    assert!(!logs.contains(&key.to_string()));
}
```

### 2. 错误处理测试
```rust
#[test]
fn test_decryption_fails_with_invalid_key() {
    let valid_key = generate_key();
    let invalid_key = generate_key(); // 不同的密钥
    
    let data = "sensitive data";
    let encrypted = encrypt_with_key(data, &valid_key);
    
    // 使用错误密钥应该解密失败
    let result = decrypt_with_key(&encrypted, &invalid_key);
    
    assert!(result.is_err());
    assert_matches!(result.unwrap_err(), CryptoError::DecryptionFailed);
}
```

### 3. 边界条件测试
```rust
#[test]
fn test_encryption_empty_data() {
    // 空数据加密应该正常工作或明确拒绝
    let result = encrypt("");
    
    // 根据业务需求选择适当的断言
    // 选项1: 允许空数据加密
    assert!(result.is_ok());
    
    // 选项2: 明确拒绝空数据
    // assert!(result.is_err());
}
```

## TDD 检查清单

- [ ] 是否为每个需求编写了测试用例？
- [ ] 是否覆盖了正常流程和异常流程？
- [ ] 是否测试了边界条件和极端情况？
- [ ] 测试是否独立且可重复运行？
- [ ] 测试名称是否清晰表达了测试意图？
- [ ] 是否避免了测试中的逻辑重复？
- [ ] 测试执行时间是否合理？

## 何时调用此技能

- 开始新功能开发时（先写测试）
- 修复 bug 时（先写重现测试）
- 重构代码时（确保测试覆盖）
- 实现安全关键功能时（需要高测试覆盖率）
- 学习或实践 TDD 方法论时

## 与 Engineer Mindset 技能的协作

此技能与 `engineer-mindset-coding` 技能配合使用：

1. **`engineer-mindset-coding`**：定义代码结构和质量标准
2. **`tdd-practitioner`**：通过测试驱动实现这些标准
3. **循环迭代**：TDD 确保实现符合工程师思维的要求
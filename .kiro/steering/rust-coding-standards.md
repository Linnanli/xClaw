---
inclusion: manual
---

# Rust 编码标准（IronClaw 项目）

手动引用此文件获取详细的 Rust 编码规范和工具用法。  
核心规则已在 `AGENTS.md` 中定义，本文件是补充的代码示例和工具说明。

---

## 质量门禁脚本

提交代码前运行完整质量检查：

```bash
sh scripts/code-quality-gate.sh
```

检查项：
1. `cargo fmt --check` — 格式化
2. `cargo clippy -- -D warnings` — 静态分析（零警告）
3. `cargo nextest run` 或 `cargo test` — 全量测试
4. 安全检查（`scripts/pre-commit-safety.sh`）— UTF-8 安全、秘密泄露、.unwrap() 检测
5. 架构边界检查（`scripts/check-boundaries.sh`）— 模块隔离、crate 独立性验证
6. 可选：`cargo audit` 依赖 CVE 扫描

覆盖率报告：
```bash
./scripts/coverage.sh              # 全量覆盖率
./scripts/coverage.sh safety       # 按模块过滤
COV_FORMAT=lcov ./scripts/coverage.sh  # 输出 lcov 格式
```

## TDD 模块脚手架

快速生成符合 TDD 规范的 Rust 模块：

```bash
./scripts/scaffold-tdd-module.sh <module_path.rs>
```

生成内容包含：错误处理结构（thiserror）、tracing 日志、基础单元测试及 proptest 属性测试模板。

---

## 错误处理规范

```rust
use thiserror::Error;
use tracing::{error, info, instrument};

// 库代码：用 thiserror 定义明确的错误枚举
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    #[error("Internal system error")]
    System(#[from] std::io::Error),
}

// 应用层：用 anyhow 传播错误，保留上下文
use anyhow::Context;
fn load_config() -> anyhow::Result<Config> {
    let content = std::fs::read_to_string("config.toml")
        .context("读取配置文件失败")?;
    toml::from_str(&content).context("解析配置文件失败")
}
```

**禁止**：生产代码中使用 `.unwrap()` 或 `.expect()`，必须处理 `Result`/`Option`。

---

## Tauri Command 封装模式

```rust
#[tauri::command]
pub async fn cmd_sign_data(request: SignRequest) -> Result<String, String> {
    sign_data_sm2(&request.data, &request.key_id)
        .map_err(|e| {
            error!("Signing failed: {:?}", e);
            e.to_string() // 返回给前端的错误信息不应包含敏感数据
        })
}
```

---

## 测试代码模式

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

### 安全审计测试

```rust
#[test]
fn test_private_key_never_logged() {
    let key = generate_private_key();
    let logs = capture_logs(|| { process_sensitive_data(&key); });
    assert!(!logs.contains(&key.to_string()));
}
```

---

## 检查清单

- [ ] 输入是否验证？是否避免了 `unsafe`？敏感数据是否脱敏？
- [ ] 是否使用了 `Result`？是否避免了 `unwrap()`？
- [ ] 关键路径是否有 `tracing` 日志？
- [ ] 是否可以编写单元测试？依赖是否可 Mock（Trait 隔离）？
- [ ] 公共接口是否有 `///` 文档注释？
- [ ] `sh scripts/code-quality-gate.sh` 是否通过？

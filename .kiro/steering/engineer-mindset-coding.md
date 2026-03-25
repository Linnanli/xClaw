---
inclusion: manual
---

# Engineer Mindset Coding Skill (IronClaw Edition)

## 功能描述
确保生成的代码具备高级工程师思维，针对 IronClaw 项目的安全和架构需求。涵盖：零信任安全原则、Rust/Tauri 最佳实践、错误处理、可维护性及生产环境就绪性。

## 核心原则

### 1. 安全优先
- **零信任输入**：对所有外部输入（UI、API、文件）进行严格验证和清洗
- **加密敏捷性**：使用标准库或经过审计的加密库（如国密 SM2/SM3/SM4），严禁硬编码密钥
- **内存安全**：利用 Rust 所有权机制，避免 `unsafe` 代码块（除非有详尽的审计注释）
- **最小权限**：组件间通信仅暴露必要接口

### 2. 鲁棒的错误处理
- **类型化错误**：库代码使用 `thiserror` 定义明确的错误枚举
- **应用层错误**：应用层使用 `anyhow` 处理错误传播，保留上下文（`.context()`）
- **禁止 Panic**：生产代码中严禁使用 `.unwrap()` 或 `.expect()`，必须处理 `Result`/`Option`

### 3. IronClaw 架构规范
- **模块隔离**：严格遵守模块边界（AdminBackend 与 ClientTerminal 分离）
- **Tauri 最佳实践**：前端（UI）与后端（Rust）通过 Tauri Command 通信，逻辑保持在 Rust 侧
- **异步编程**：正确使用 `tokio` 运行时，避免在异步上下文中阻塞线程

### 4. 可观测性与可测试性
- **结构化日志**：使用 `tracing` crate 进行结构化日志记录，便于审计
- **单元测试**：核心逻辑必须包含单元测试
- **依赖注入**：使用 Trait 隔离外部依赖（数据库、网络），便于 Mock 测试

## 代码示例

```rust
use thiserror::Error;
use tracing::{error, info, instrument};

#[derive(Debug, Error)]
pub enum SecureOpError {
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    #[error("Internal system error")]
    System(#[from] std::io::Error),
}

#[instrument(skip(data), fields(data_len = data.len()))]
pub fn sign_data_sm2(data: &str, key_id: &str) -> Result<String, SecureOpError> {
    if data.is_empty() {
        return Err(SecureOpError::Validation("Data cannot be empty".to_string()));
    }
    info!("Starting SM2 signature process");
    // 调用真实国密库...
    Ok(format!("sm2_sig_{}", key_id))
}

#[tauri::command]
pub async fn cmd_sign_data(request: SignRequest) -> Result<String, String> {
    sign_data_sm2(&request.data, &request.key_id)
        .map_err(|e| { error!("Signing failed: {:?}", e); e.to_string() })
}
```

## 检查清单

- [ ] 输入是否验证？是否避免了 `unsafe`？敏感数据是否脱敏？
- [ ] 是否使用了 `Result`？是否避免了 `unwrap()`？错误信息是否清晰？
- [ ] 关键路径是否有 `tracing` 日志？是否包含审计所需的上下文？
- [ ] 是否可以编写单元测试？依赖是否可 Mock？
- [ ] 代码位置是否符合模块边界规范？
- [ ] 公共接口是否有 `///` 文档注释？

## 何时调用

- 实现 Security Kernel（SM2、DLP、WASM）组件时
- 编写 Tauri Command 接口时
- 设计 Admin Backend 的审计与权限逻辑时
- 处理敏感数据（加密、脱敏）时

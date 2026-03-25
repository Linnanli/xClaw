---
inclusion: manual
---

# Code Quality Gate Skill (IronClaw Edition)

## 功能描述
通过自动化的质量门禁脚本，确保代码符合 IronClaw 项目的高标准质量要求。涵盖标准 Rust 工具链检查，以及项目特有的安全和架构约束。

## 门禁检查项

### 1. 基础规范
- **格式化**：`cargo fmt --check`，强制统一代码风格
- **静态分析**：`cargo clippy -- -D warnings`，捕捉潜在 Bug 和不良实践
- **自动化测试**：运行所有单元测试和集成测试

### 2. 安全门禁
- 检查 UTF-8 安全、秘密泄露等（基于 `scripts/pre-commit-safety.sh`）
- 可选：`cargo audit` 扫描依赖库中的已知 CVE

### 3. 架构门禁
- 模块边界检查（基于 `scripts/check-boundaries.sh`）
- 禁止在领域层直接使用数据库驱动等跨层依赖

## 执行

```bash
# 运行完整质量门禁
sh scripts/code-quality-gate.sh
```

## 质量原则

- **零警告原则**：Clippy 和编译器警告必须全部修复
- **测试全覆盖**：核心逻辑必须通过所有现有测试
- **架构一致性**：严禁违反模块隔离原则
- **安全不可妥协**：硬编码秘密等安全风险会导致门禁失败

## 何时调用

- 提交 PR 前
- 功能开发完成后
- CI/CD 调试时（本地模拟流水线）

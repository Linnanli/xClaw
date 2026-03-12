---
name: "code-quality-gate"
description: "Executes a comprehensive code quality gate check, including formatting, clippy, tests, security, and architecture boundaries. Invoke when preparing a PR, finishing a feature, or ensuring codebase health."
---

# Code Quality Gate Skill (IronClaw Edition)

## 功能描述
此技能通过自动化的质量门禁脚本，确保代码符合 **IronClaw** 项目的高标准质量要求。它不仅涵盖标准的 Rust 工具链检查，还集成了项目特有的安全和架构约束。

## 辅助工具 (Tooling Support)

本项目提供了统一的入口脚本来执行所有质量检查：

1.  **[code-quality-gate.sh](file:///c:/codes/ironclaw/scripts/code-quality-gate.sh)**
    - **作用**：一键运行格式化检查、Clippy、测试、安全扫描和架构边界检查。
    - **用法**：`bash scripts/code-quality-gate.sh`
    - **目的**：作为代码合并前的最后一道防线，确保不引入回归或违规。

## 门禁检查项 (Gate Checks)

### 1. 基础规范 (Base Standards)
- **格式化 (Cargo Fmt)**：强制执行统一的代码风格。
- **静态分析 (Clippy)**：捕捉潜在 Bug、不良实践和性能问题（强制开启 `-D warnings`）。
- **自动化测试 (Nextest/Test)**：运行所有单元测试和集成测试。

### 2. 安全门禁 (Security Gate)
- **自定义安全检查**：基于 [pre-commit-safety.sh](file:///c:/codes/ironclaw/scripts/pre-commit-safety.sh)，检查 UTF-8 安全、秘密泄露等。
- **依赖审计 (Cargo Audit)**：可选检查，扫描依赖库中的已知 CVE。

### 3. 架构门禁 (Architecture Gate)
- **边界检查**：基于 [check-boundaries.sh](file:///c:/codes/ironclaw/scripts/check-boundaries.sh)，确保模块间严格隔离（例如：禁止在领域层直接使用数据库驱动）。

## 使用场景

### 何时调用此技能：
- **Solo/自主开发**：在没有 Peer Review 的情况下，作为自动化的「虚拟审查者」确保代码质量。
- **提交 PR 前**：确保你的更改不会破坏现有的质量标准。
- **功能开发完成后**：验证新特性的完整性和规范性。
- **CI/CD 调试**：在本地模拟流水线检查，快速定位构建失败原因。

## 质量原则

- **零警告原则**：Clippy 和编译器警告必须全部修复。
- **测试全覆盖**：核心逻辑必须通过所有现有测试。
- **架构一致性**：严禁违反模块隔离原则。
- **安全不可妥协**：任何潜在的安全风险（如硬编码秘密）都会导致门禁失败。

## 执行建议

建议在本地开发时，至少在提交代码前运行一次完整门禁：

```bash
# 运行完整质量门禁
sh scripts/code-quality-gate.sh
```

如果门禁失败，请根据脚本输出的 `FAILURE` 部分逐一修复，直到看到 `CONGRATULATIONS! The code quality gate has passed.`。

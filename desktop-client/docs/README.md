# Desktop Client 文档索引

> 最后更新: 2025-01-XX

本目录包含 Desktop Client 项目的所有文档,按功能和主题组织。

## 📚 文档结构

```
docs/
├── README.md (本文件 - 文档索引)
├── architecture/
│   ├── overview.md (架构总览)
│   ├── evolution.md (架构演进历史)
│   └── tauri-ipc.md (Tauri IPC 设计)
├── features/
│   ├── dlp/
│   │   ├── overview.md (DLP 功能总览)
│   │   ├── implementation.md (实现细节)
│   │   ├── testing.md (测试策略)
│   │   └── lessons-learned.md (经验教训)
│   ├── chat.md (聊天功能)
│   └── auth.md (认证功能)
├── guides/
│   ├── quick-start.md (快速开始)
│   ├── development.md (开发指南)
│   ├── testing.md (测试指南)
│   └── troubleshooting.md (故障排查)
└── migration/
    ├── tauri-ipc.md (Tauri IPC 迁移)
    ├── path-aliases.md (路径别名迁移)
    └── ui-migration.md (UI 迁移)
```

## 🚀 快速导航

### 新手入门

- [快速开始](guides/quick-start.md) - 5分钟快速上手
- [开发指南](guides/development.md) - 开发环境配置和工作流程
- [故障排查](guides/troubleshooting.md) - 常见问题解决方案

### 架构设计

- [架构总览](architecture/overview.md) - 系统架构和设计原则
- [架构演进](architecture/evolution.md) - 架构变更历史和决策
- [Tauri IPC 设计](architecture/tauri-ipc.md) - Tauri IPC 通信机制

### 功能文档

- [DLP 功能](features/dlp/overview.md) - 数据防泄漏功能
- [聊天功能](features/chat.md) - AI 聊天功能
- [认证功能](features/auth.md) - 用户认证和会话管理

### 测试文档

- [测试指南](guides/testing.md) - 测试策略和最佳实践
- [测试文件组织](../tests/README.md) - 测试文件结构说明
- [测试覆盖率报告](../TEST_COVERAGE_REPORT.md) - 当前测试覆盖率

### 迁移指南

- [Tauri IPC 迁移](migration/tauri-ipc.md) - 从 HTTP/SSE 迁移到 Tauri IPC
- [路径别名迁移](migration/path-aliases.md) - TypeScript 路径别名配置
- [UI 迁移](migration/ui-migration.md) - UI 组件迁移

## 📖 文档分类

### 按主题分类

| 主题 | 文档 |
|------|------|
| 架构 | [总览](architecture/overview.md), [演进](architecture/evolution.md), [Tauri IPC](architecture/tauri-ipc.md) |
| DLP | [总览](features/dlp/overview.md), [实现](features/dlp/implementation.md), [测试](features/dlp/testing.md), [经验](features/dlp/lessons-learned.md) |
| 聊天 | [聊天功能](features/chat.md) |
| 认证 | [认证功能](features/auth.md) |
| 测试 | [测试指南](guides/testing.md), [DLP 测试](features/dlp/testing.md) |
| 迁移 | [Tauri IPC](migration/tauri-ipc.md), [路径别名](migration/path-aliases.md), [UI](migration/ui-migration.md) |

### 按读者分类

| 读者 | 推荐文档 |
|------|---------|
| 新手开发者 | [快速开始](guides/quick-start.md), [开发指南](guides/development.md) |
| 架构师 | [架构总览](architecture/overview.md), [架构演进](architecture/evolution.md) |
| 测试工程师 | [测试指南](guides/testing.md), [DLP 测试](features/dlp/testing.md) |
| 运维人员 | [故障排查](guides/troubleshooting.md), [快速开始](guides/quick-start.md) |

## 🔄 文档维护

### 文档更新原则

1. **及时更新**: 代码变更时同步更新文档
2. **保持简洁**: 避免冗余和过时信息
3. **结构清晰**: 使用标题、列表、表格等组织内容
4. **示例丰富**: 提供代码示例和截图
5. **链接有效**: 定期检查内部和外部链接

### 文档审查清单

- [ ] 标题清晰,层次分明
- [ ] 内容准确,无过时信息
- [ ] 代码示例可运行
- [ ] 链接有效
- [ ] 格式统一

### 贡献指南

如需添加或修改文档:

1. 确定文档类型和位置
2. 遵循现有文档格式
3. 更新本索引文件
4. 提交 PR 并说明变更原因

## 📝 变更日志

### 2025-01-XX

- 创建文档索引
- 整合 44 个分散的 MD 文件
- 建立结构化文档目录
- 删除过时和重复文档

## 🔗 相关资源

- [主项目文档](../../README.md)
- [AGENTS.md](../../AGENTS.md) - 开发规则和测试要求
- [Tauri 官方文档](https://tauri.app/v1/guides/)
- [Rust 官方文档](https://doc.rust-lang.org/)

---

**注意**: 本文档会随着项目发展持续更新。如发现问题或有改进建议,请提交 Issue 或 PR。

# 文档索引

## 📚 快速导航

### 🚀 新手入门

| 文档 | 用途 | 优先级 |
|------|------|--------|
| [README.md](README.md) | 项目介绍和概述 | ⭐⭐⭐ |
| [QUICK_START.md](QUICK_START.md) | 快速启动指南 | ⭐⭐⭐ |
| [STARTUP_QUICK_REFERENCE.md](STARTUP_QUICK_REFERENCE.md) | 启动快速参考 | ⭐⭐⭐ |
| [SERIAL_STARTUP_MODE.md](SERIAL_STARTUP_MODE.md) | 串行启动模式说明 | ⭐⭐ |

### 🐳 Docker 问题排查

| 文档 | 用途 | 何时使用 |
|------|------|----------|
| [FINAL_FIX_SUMMARY.md](FINAL_FIX_SUMMARY.md) | Docker 问题最终修复总结 | 遇到 Docker 问题时首先阅读 ⭐⭐⭐ |
| [DOCKER_DAEMON_CRASH_ANALYSIS.md](DOCKER_DAEMON_CRASH_ANALYSIS.md) | Docker daemon 崩溃分析 | 了解问题根源和技术细节 ⭐⭐ |
| [DOCKER_CONNECTION_FIX.md](DOCKER_CONNECTION_FIX.md) | Docker 连接问题修复 | Docker 连接失败时参考 ⭐⭐ |

### 🖥️ Desktop Client 问题

| 文档 | 用途 | 何时使用 |
|------|------|----------|
| [TAURI_WINDOW_NOT_OPENING.md](TAURI_WINDOW_NOT_OPENING.md) | Tauri 窗口未打开问题 | Tauri 窗口没有自动打开时 ⭐⭐⭐ |
| [RUST_COMPILATION_EXPLAINED.md](RUST_COMPILATION_EXPLAINED.md) | Rust 编译机制说明 | 了解编译过程和缓存机制 ⭐⭐ |
| [CARGO_LOCK_EXPLANATION.md](CARGO_LOCK_EXPLANATION.md) | Cargo 文件锁说明 | 了解 Cargo 编译锁机制 ⭐⭐ |

### 🖥️ Desktop Client

| 文档 | 用途 |
|------|------|
| [DESKTOP_CLIENT_QUICK_START.md](DESKTOP_CLIENT_QUICK_START.md) | Desktop Client 快速启动 |
| [desktop-client/ARCHITECTURE_EVOLUTION.md](desktop-client/ARCHITECTURE_EVOLUTION.md) | 架构演进说明 |
| [desktop-client/TAURI_IPC_QUICK_REFERENCE.md](desktop-client/TAURI_IPC_QUICK_REFERENCE.md) | Tauri IPC 快速参考 |

### 🔧 Admin Backend

| 文档 | 用途 |
|------|------|
| [admin-backend/QUICK_FIX.md](admin-backend/QUICK_FIX.md) | 快速修复指南 |
| [admin-backend/TROUBLESHOOTING.md](admin-backend/TROUBLESHOOTING.md) | 完整故障排查 |
| [admin-backend/scripts/README.md](admin-backend/scripts/README.md) | 脚本使用说明 |
| [admin-backend/scripts/ARCHITECTURE.md](admin-backend/scripts/ARCHITECTURE.md) | 脚本架构说明 |

### 🛠️ 工具脚本

| 脚本 | 用途 |
|------|------|
| [scripts/start-all.sh](scripts/start-all.sh) | 启动所有服务 |
| [scripts/check-docker-stability.sh](scripts/check-docker-stability.sh) | 检查 Docker 稳定性 |
| [scripts/fix-docker-connection.sh](scripts/fix-docker-connection.sh) | 自动诊断和修复 Docker |
| [admin-backend/scripts/start-db.sh](admin-backend/scripts/start-db.sh) | 启动数据库 |
| [admin-backend/scripts/start-admin.sh](admin-backend/scripts/start-admin.sh) | 启动 Admin Backend |

### 🤖 开发规则

| 文档 | 用途 |
|------|------|
| [AGENTS.md](AGENTS.md) | AI Agent 开发规则和最佳实践 |

### 📋 功能文档

| 文档 | 用途 |
|------|------|
| [docs/DLP_IMPLEMENTATION_ROADMAP.md](docs/DLP_IMPLEMENTATION_ROADMAP.md) | DLP 实现路线图 |
| [docs/DLP_MULTI_DIMENSIONAL_RULES_DESIGN.md](docs/DLP_MULTI_DIMENSIONAL_RULES_DESIGN.md) | DLP 多维规则设计 |
| [docs/DLP_RULE_MANAGEMENT_COMPLETE.md](docs/DLP_RULE_MANAGEMENT_COMPLETE.md) | DLP 规则管理完整说明 |

## 🔍 按场景查找

### 场景 1: 首次启动项目

1. 阅读 [README.md](README.md) 了解项目
2. 按照 [QUICK_START.md](QUICK_START.md) 启动
3. 推荐使用串行模式查看编译进度：`./scripts/start-all.sh --serial`
4. 如遇问题，查看 [STARTUP_QUICK_REFERENCE.md](STARTUP_QUICK_REFERENCE.md)
5. 了解编译机制，查看 [RUST_COMPILATION_EXPLAINED.md](RUST_COMPILATION_EXPLAINED.md)

### 场景 2: Docker 连接失败

1. 阅读 [FINAL_FIX_SUMMARY.md](FINAL_FIX_SUMMARY.md) 了解问题和解决方案
2. 运行 `./scripts/check-docker-stability.sh` 检查稳定性
3. 如需详细修复步骤，查看 [DOCKER_CONNECTION_FIX.md](DOCKER_CONNECTION_FIX.md)

### 场景 3: Docker daemon 崩溃

1. 阅读 [DOCKER_DAEMON_CRASH_ANALYSIS.md](DOCKER_DAEMON_CRASH_ANALYSIS.md) 了解根本原因
2. 按照 [FINAL_FIX_SUMMARY.md](FINAL_FIX_SUMMARY.md) 中的修复步骤操作
3. 运行 `./scripts/fix-docker-connection.sh` 自动诊断

### 场景 4: Admin Backend 问题

1. 查看 [admin-backend/QUICK_FIX.md](admin-backend/QUICK_FIX.md) 快速修复
2. 如问题未解决，查看 [admin-backend/TROUBLESHOOTING.md](admin-backend/TROUBLESHOOTING.md)
3. 了解脚本使用，查看 [admin-backend/scripts/README.md](admin-backend/scripts/README.md)

### 场景 5: Desktop Client 开发

1. 阅读 [DESKTOP_CLIENT_QUICK_START.md](DESKTOP_CLIENT_QUICK_START.md)
2. 了解架构，查看 [desktop-client/ARCHITECTURE_EVOLUTION.md](desktop-client/ARCHITECTURE_EVOLUTION.md)
3. Tauri IPC 参考 [desktop-client/TAURI_IPC_QUICK_REFERENCE.md](desktop-client/TAURI_IPC_QUICK_REFERENCE.md)

## 📝 文档维护

### 核心文档（长期维护）

- README.md
- QUICK_START.md
- AGENTS.md
- STARTUP_QUICK_REFERENCE.md

### 问题修复文档（问题解决后可归档）

- FINAL_FIX_SUMMARY.md
- DOCKER_DAEMON_CRASH_ANALYSIS.md
- DOCKER_CONNECTION_FIX.md

### 已删除的过时文档

- ~~DOCKER_ISSUE_SUMMARY.md~~ - 与 FINAL_FIX_SUMMARY 重复
- ~~DOCKER_STABILITY_ISSUE.md~~ - 与 DOCKER_CONNECTION_FIX 重复
- ~~IMMEDIATE_FIX.md~~ - 已包含在 FINAL_FIX_SUMMARY
- ~~POSTGRES_CHECK_ONLY_UPDATE.md~~ - 已包含在 FINAL_FIX_SUMMARY
- ~~START_ALL_UPDATE.md~~ - 过时的更新说明
- ~~SCRIPT_ARCHITECTURE_UPDATE.md~~ - 过时的架构更新
- ~~SCRIPT_REFACTORING_SUMMARY.md~~ - 过时的重构总结
- ~~DOCKER_MANUAL_START_UPDATE.md~~ - 过时的手动启动说明

## 🎯 推荐阅读顺序

### 新用户

1. README.md - 了解项目
2. QUICK_START.md - 快速启动
3. STARTUP_QUICK_REFERENCE.md - 启动参考

### 遇到 Docker 问题

1. FINAL_FIX_SUMMARY.md - 了解问题和解决方案
2. 运行诊断脚本
3. DOCKER_CONNECTION_FIX.md - 详细修复步骤

### 开发者

1. AGENTS.md - 开发规则
2. 相关模块的架构文档
3. 功能实现文档

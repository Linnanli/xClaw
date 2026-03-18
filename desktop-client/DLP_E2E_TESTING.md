# DLP E2E 测试指南

## 概述

本文档说明如何运行 DLP（数据泄露防护）功能的端到端测试。

## 测试覆盖范围

### 功能测试
- ✅ 身份证号检测和脱敏
- ✅ 手机号检测和脱敏
- ✅ API 密钥检测和阻止 
- ✅ 多种敏感信息混合处理
- ✅ 普通消息正常通过

### 配置管理测试
- ✅ 启用/禁用 DLP 功能
- ✅ 查看 DLP 统计信息
- ✅ 配置更新生效

### 性能测试
- ✅ DLP 扫描延迟 < 100ms
- ✅ 大文本处理能力

### 错误处理测试
- ✅ DLP 服务不可用时的降级处理
- ✅ 边界情况处理（空消息、特殊字符等）

### 审计日志测试
- ✅ 敏感信息检测事件记录
- ✅ 审计日志查询

## 快速开始

### 方式一：使用自动化脚本（推荐）

```bash
# 运行完整的 E2E 测试（自动启动后端和前端）
./desktop-client/scripts/run-dlp-e2e-tests.sh
```

### 方式二：手动运行

#### 第一步：启动后端服务

```bash
# 在项目根目录
cargo run -- run --cli-only --no-onboard
```

#### 第二步：启动前端开发服务器

```bash
# 在另一个终端
cd desktop-client/src-ui
npm run dev
```

#### 第三步：运行 Cypress 测试

```bash
# 在 desktop-client/src-ui 目录
npx cypress run --spec "cypress/e2e/dlp_integration.cy.js"

# 或者使用交互式模式
npx cypress open
```

## 测试文件结构

```
desktop-client/
├── src/
│   ├── dlp/
│   │   ├── integration.rs          # DLP 集成服务
│   │   ├── detector.rs             # 检测器
│   │   ├── sanitizer.rs            # 脱敏器
│   │   └── patterns.rs             # 模式定义
│   ├── commands.rs                 # Tauri 命令（包含 DLP 命令）
│   └── main.rs                     # 应用入口
├── src-ui/
│   ├── src/app/hooks/
│   │   ├── useDlpScan.ts          # DLP 扫描 Hook
│   │   └── useAiChat.ts           # AI 聊天 Hook（集成 DLP）
│   └── cypress/
│       └── e2e/
│           └── dlp_integration.cy.js  # E2E 测试
└── scripts/
    └── run-dlp-e2e-tests.sh       # 自动化测试脚本
```

## 测试场景详解

### 1. 身份证号脱敏测试

**输入**: `我的身份证号是 110101199003071234`
**预期**: 显示为 `我的身份证号是 110************234`
**验证**: 
- 消息被脱敏
- 显示 DLP 警告
- 记录审计日志

### 2. API 密钥阻止测试

**输入**: `阿里云密钥：LTAI4G8aB9cD2eFgH3iJ`
**预期**: 消息被阻止，显示错误提示
**验证**:
- 消息未被添加到聊天记录
- 显示错误消息
- 记录审计日志

### 3. 混合敏感信息测试

**输入**: `我的身份证是 110101199003071234，手机号是 13800138000`
**预期**: 两种敏感信息都被脱敏
**验证**:
- 身份证号脱敏为 `110************234`
- 手机号脱敏为 `138*****000`

## 调试技巧

### 查看 Cypress 测试日志

```bash
# 运行测试并保存日志
npx cypress run --spec "cypress/e2e/dlp_integration.cy.js" > test.log 2>&1
```

### 查看后端日志

```bash
# 启动后端时启用详细日志
RUST_LOG=debug cargo run -- run --cli-only --no-onboard
```

### 使用 Cypress 交互式模式

```bash
# 打开 Cypress 测试界面
npx cypress open

# 选择 E2E Testing
# 选择浏览器（Chrome/Firefox/Edge）
# 选择 dlp_integration.cy.js 测试文件
```

## 常见问题

### Q: 测试失败，提示 "Cannot find element"

**A**: 确保前端应用已完全加载。可以增加 `cy.wait()` 的等待时间。

### Q: 后端连接失败

**A**: 检查后端是否正常启动：
```bash
curl http://localhost:3000/health
```

### Q: DLP 功能未生效

**A**: 检查 DLP 配置是否启用：
```bash
# 在 Rust 代码中
let config = dlp.get_config().await;
assert!(config.enabled);
```

## 性能基准

- **单次扫描延迟**: < 10ms
- **消息发送总延迟**: < 100ms（包含 DLP 扫描）
- **并发扫描能力**: 100 QPS
- **大文本处理**: 10KB 文本 < 50ms

## 下一步

- [ ] 添加更多敏感信息类型的测试（邮箱、地址等）
- [ ] 添加自定义模式配置的 E2E 测试
- [ ] 添加策略同步的 E2E 测试
- [ ] 添加跨浏览器兼容性测试
- [ ] 添加性能压力测试

## 参考文档

- `AGENTS.md` - 测试覆盖率规则和浏览器 E2E 测试规则
- `desktop-client/src/dlp/README.md` - DLP 模块详细文档
- `SSE_INTEGRATION_ISSUE_ANALYSIS.md` - SSE 集成经验教训

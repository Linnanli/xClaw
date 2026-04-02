# 管理后台前端 E2E 测试文档

## 概述

本文档描述管理后台前端的端到端（E2E）测试策略和实现。我们使用 Cypress 进行 E2E 测试，并使用真实的后端 API 而不是 mock 数据。

---

## 测试框架

**Cypress 10+**
- 现代化的 E2E 测试框架
- 实时重新加载
- 时间旅行调试
- 自动等待
- 网络流量控制

---

## 测试覆盖率类型

我们实现了 10 种测试覆盖率类型，确保全面的测试覆盖：

### 1. 单元测试（Unit Tests）
- 表单验证逻辑
- 输入字段验证
- 必填项检查
- 长度限制验证

### 2. 集成测试（Integration Tests）
- 前后端登录流程
- API 请求和响应
- 令牌管理
- 会话持久化

### 3. 失败路径测试（Failure Path Tests）
- 错误凭据处理
- 网络错误处理
- 服务器错误处理
- 请求超时处理

### 4. 安全测试（Security Tests）
- 密码隐藏
- 令牌安全存储
- 敏感信息不泄露
- URL 安全

### 5. 可靠性测试（Reliability Tests）
- 防止重复提交
- 失败后重试
- 并发请求处理
- 会话恢复

### 6. 需求级测试（Requirements Tests）
- 功能规范验证
- 业务需求覆盖
- 用户故事测试
- 验收标准检查

### 7. 用户体验测试（UX Tests）
- 加载状态显示
- 错误提示清晰
- 交互反馈及时
- 表单焦点管理

### 8. 代码覆盖测试（Code Coverage Tests）
- 边界情况
- 空值处理
- 特殊字符处理
- 超长输入处理

### 9. 数据覆盖测试（Data Coverage Tests）
- 各种输入格式
- 大小写处理
- 数字和特殊字符
- Unicode 字符

### 10. 性能测试（Performance Tests）
- 响应时间测试
- 页面加载时间
- API 请求时间
- 渲染性能

---

## 项目结构

```
admin-backend/ui/
├── cypress/
│   ├── e2e/
│   │   └── auth_login.cy.ts        # 登录功能 E2E 测试
│   ├── support/
│   │   ├── commands.ts             # 自定义命令
│   │   └── e2e.ts                  # 全局配置
│   └── fixtures/                   # 测试数据
├── scripts/
│   └── run-e2e-tests.sh            # 测试启动脚本
├── cypress.config.ts               # Cypress 配置
└── package.json                    # 测试脚本
```

---

## 运行测试

### 前提条件

1. **后端服务运行**
   ```bash
   cd admin-backend
   cargo run
   ```

2. **前端开发服务器运行**
   ```bash
   cd admin-backend/ui
   npm run dev
   ```

3. **测试用户存在**
   - 用户名：`admin`
   - 密码：`admin123`

### 运行方式

#### 方式 1：使用测试脚本（推荐）

```bash
cd admin-backend/ui
./scripts/run-e2e-tests.sh
```

**带界面运行**：
```bash
./scripts/run-e2e-tests.sh headed
```

#### 方式 2：直接使用 npm 脚本

**Headless 模式**：
```bash
npm run test:e2e
```

**带界面模式**：
```bash
npm run test:e2e:headed
```

---

## 测试统计

- **总测试数**：50+ 个
- **测试套件**：10 个
- **覆盖率类型**：10 种
- **预计执行时间**：约 5-10 分钟

---

## 参考资源

- `admin-backend/ui/cypress/e2e/auth_login.cy.ts` - 登录测试
- `admin-backend/ui/cypress/support/commands.ts` - 自定义命令
- `admin-backend/ui/scripts/run-e2e-tests.sh` - 测试脚本
- `desktop-client/src-ui/cypress/e2e/dlp_integration.cy.js` - 参考示例

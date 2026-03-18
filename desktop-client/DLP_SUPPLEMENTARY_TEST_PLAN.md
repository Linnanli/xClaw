# DLP 补充测试计划

## 目标

填补现有测试的盲区，特别是：
1. 失败路径测试
2. 降级逻辑测试
3. 契约测试
4. 真实环境测试
5. 安全审计测试

## 测试类型 1：失败路径测试

### 测试文件：`desktop-client/src-ui/cypress/e2e/dlp_failure_paths.cy.js`

```javascript
describe('DLP Failure Path Tests', () => {
  beforeEach(() => {
    cy.visit('http://localhost:5173');
  });

  it('should block message when DLP scan fails', () => {
    // 模拟 DLP 扫描失败
    cy.window().then((win) => {
      const originalInvoke = (win as any).__TAURI__.core.invoke;
      (win as any).__TAURI__.core.invoke = async (cmd: string, args: any) => {
        if (cmd === 'scan_user_input') {
          throw new Error('DLP service unavailable');
        }
        return originalInvoke(cmd, args);
      };
    });

    cy.get('.message-input').type('测试消息');
    cy.get('.send-button').click();

    // 验证显示错误
    cy.get('.error-message').should('contain', 'DLP 扫描失败');
    
    // 验证消息未发送
    cy.get('.message-item').should('not.exist');
  });

  it('should block message when Tauri is not available', () => {
    // 模拟 Tauri 不可用
    cy.window().then((win) => {
      (win as any).__TAURI__ = undefined;
    });

    cy.get('.message-input').type('测试消息');
    cy.get('.send-button').click();

    // 验证显示错误
    cy.get('.error-message').should('contain', 'DLP 扫描功能未初始化');
  });

  it('should block message when scanUserInput is not a function', () => {
    // 模拟 scanUserInput 不是函数
    cy.window().then((win) => {
      (win as any).__TAURI__.core.invoke = null;
    });

    cy.get('.message-input').type('测试消息');
    cy.get('.send-button').click();

    // 验证显示错误
    cy.get('.error-message').should('exist');
  });
});
```

### 测试文件：`desktop-client/tests/dlp_failure_tests.rs`

```rust
#[cfg(test)]
mod dlp_failure_tests {
    use super::*;

    #[tokio::test]
    async fn test_dlp_scan_with_invalid_input() {
        let dlp = DlpIntegration::with_default_config().await.unwrap();
        
        // 测试空字符串
        let result = dlp.scan_user_input("").await;
        assert!(result.is_ok());
        
        // 测试超长字符串（>1MB）
        let long_string = "a".repeat(2_000_000);
        let result = dlp.scan_user_input(&long_string).await;
        assert!(result.is_ok()); // 应该处理而不是崩溃
    }

    #[tokio::test]
    async fn test_dlp_scan_with_malformed_patterns() {
        // 测试配置了错误的正则表达式
        let mut config = DlpIntegrationConfig::default();
        config.custom_patterns.push(CustomPattern {
            name: "invalid".to_string(),
            pattern: "[[[invalid regex".to_string(), // 无效的正则
            severity: Severity::High,
            action: Action::Redact,
            enabled: true,
        });
        
        let dlp = DlpIntegration::new(config).await;
        
        // 应该返回错误而不是崩溃
        assert!(dlp.is_err());
    }

    #[tokio::test]
    async fn test_dlp_service_unavailable() {
        // 测试 DLP 服务不可用的情况
        // 这需要模拟服务故障
        
        // TODO: 实现服务故障模拟
    }
}
```

## 测试类型 2：安全审计测试

### 测试文件：`desktop-client/src-ui/cypress/e2e/dlp_security_audit.cy.js`

```javascript
describe('DLP Security Audit Tests', () => {
  const sensitiveData = [
    { type: '身份证', value: '330326199408015618', redacted: '330************618' },
    { type: '手机号', value: '13800138000', redacted: '138****8000' },
    { type: 'API密钥', value: 'sk-1234567890abcdef1234567890abcdef', blocked: true },
  ];

  beforeEach(() => {
    cy.visit('http://localhost:5173');
  });

  it('should never send original sensitive data to backend', () => {
    sensitiveData.forEach(({ type, value, redacted, blocked }) => {
      if (blocked) return; // 跳过被阻止的测试

      // 拦截后端请求
      cy.intercept('POST', '**/api/chat/send', (req) => {
        // 验证请求体中没有原始敏感信息
        expect(req.body.content).not.to.contain(value);
        
        // 验证请求体中包含脱敏后的内容
        if (redacted) {
          expect(req.body.content).to.contain(redacted);
        }
        
        req.reply({ success: true });
      }).as('sendMessage');

      cy.get('.message-input').type(`${type}: ${value}`);
      cy.get('.send-button').click();
      cy.wait('@sendMessage');
    });
  });

  it('should never log original sensitive data', () => {
    // 监听控制台日志
    const logs: string[] = [];
    cy.window().then((win) => {
      const originalLog = win.console.log;
      win.console.log = (...args: any[]) => {
        logs.push(args.join(' '));
        originalLog.apply(win.console, args);
      };
    });

    // 发送包含敏感信息的消息
    cy.get('.message-input').type('我的身份证号是 330326199408015618');
    cy.get('.send-button').click();

    // 等待消息发送完成
    cy.wait(1000);

    // 验证日志中没有原始敏感信息
    cy.window().then(() => {
      const hasOriginalData = logs.some(log => log.includes('330326199408015618'));
      
      // 允许在 "Original:" 日志中出现（用于调试）
      // 但不应该在 "Sending message" 或 "Content:" 日志中出现
      const hasSensitiveLog = logs.some(log => 
        log.includes('Content: 我的身份证号是 330326199408015618') ||
        log.includes('Sending: 我的身份证号是 330326199408015618')
      );
      
      expect(hasSensitiveLog).to.be.false;
    });
  });

  it('should sanitize sensitive data in error messages', () => {
    // 模拟 DLP 扫描返回错误，错误信息中包含敏感数据
    cy.intercept('POST', '**/scan_user_input', {
      statusCode: 500,
      body: { 
        error: '扫描失败：无法处理 330326199408015618' // 错误信息中包含敏感数据
      }
    });

    cy.get('.message-input').type('我的身份证号是 330326199408015618');
    cy.get('.send-button').click();

    // 验证错误提示中没有原始敏感信息
    cy.get('.error-message').should('not.contain', '330326199408015618');
  });
});
```

### 测试文件：`desktop-client/tests/dlp_security_audit_tests.rs`

```rust
#[cfg(test)]
mod dlp_security_audit_tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_no_sensitive_data_in_logs() {
        // 设置日志捕获
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        
        // 初始化 DLP
        let dlp = DlpIntegration::with_default_config().await.unwrap();
        
        // 扫描包含敏感信息的内容
        let sensitive_content = "我的身份证号是 330326199408015618";
        let result = dlp.scan_user_input(sensitive_content).await.unwrap();
        
        // 验证结果中没有原始敏感信息
        assert!(!result.sanitized_content.contains("330326199408015618"));
        
        // 验证日志中没有原始敏感信息
        // TODO: 实现日志捕获和验证
    }

    #[tokio::test]
    async fn test_audit_no_sensitive_data_in_errors() {
        let dlp = DlpIntegration::with_default_config().await.unwrap();
        
        // 测试错误场景
        let result = dlp.scan_user_input("").await;
        
        // 如果返回错误，验证错误信息中没有敏感数据
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(!error_msg.contains("330326199408015618"));
        }
    }
}
```

## 测试类型 3：契约测试

### 测试文件：`desktop-client/src-ui/cypress/e2e/dlp_contract.cy.js`

```javascript
describe('DLP Contract Tests', () => {
  beforeEach(() => {
    cy.visit('http://localhost:5173');
  });

  it('should match backend SanitizationResult schema', async () => {
    cy.window().then(async (win) => {
      const { invoke } = (win as any).__TAURI__.core;
      
      const result = await invoke('scan_user_input', { 
        content: '我的身份证号是 330326199408015618' 
      });
      
      // 验证返回值符合契约
      expect(result).to.have.property('had_sensitive_data');
      expect(result).to.have.property('sanitized_content');
      expect(result).to.have.property('was_blocked');
      expect(result).to.have.property('sanitization_stats');
      
      expect(result.sanitization_stats).to.have.property('total_matches');
      expect(result.sanitization_stats).to.have.property('redacted_count');
      expect(result.sanitization_stats).to.have.property('blocked_count');
      expect(result.sanitization_stats).to.have.property('warned_count');
      
      // 验证类型
      expect(result.had_sensitive_data).to.be.a('boolean');
      expect(result.sanitized_content).to.be.a('string');
      expect(result.was_blocked).to.be.a('boolean');
      expect(result.sanitization_stats.total_matches).to.be.a('number');
    });
  });

  it('should handle all error cases consistently', async () => {
    cy.window().then(async (win) => {
      const { invoke } = (win as any).__TAURI__.core;
      
      // 测试各种错误场景
      const errorCases = [
        { content: null, expectedError: 'Invalid input' },
        { content: undefined, expectedError: 'Invalid input' },
        // 更多错误场景...
      ];
      
      for (const { content, expectedError } of errorCases) {
        try {
          await invoke('scan_user_input', { content });
          throw new Error('Should have thrown error');
        } catch (error) {
          expect(error.message).to.include(expectedError);
        }
      }
    });
  });
});
```

## 测试类型 4：真实环境集成测试

### 测试脚本：`desktop-client/tests/real_environment_integration_test.sh`

```bash
#!/bin/bash

# 真实环境集成测试
# 使用真实的后端和前端，测试完整的用户流程

set -e

echo "🌍 真实环境集成测试"
echo "==================="
echo ""

# 1. 启动真实后端
echo "1️⃣ 启动后端服务..."
cargo run -- run --cli-only --no-onboard > backend.log 2>&1 &
BACKEND_PID=$!
echo "   Backend PID: $BACKEND_PID"

# 等待后端启动
sleep 5

# 检查后端是否启动成功
if ! curl -s http://localhost:3000/health > /dev/null; then
    echo "❌ 后端启动失败"
    kill $BACKEND_PID
    exit 1
fi
echo "✅ 后端启动成功"
echo ""

# 2. 启动真实前端
echo "2️⃣ 启动前端服务..."
cd src-ui
npm run dev > ../frontend.log 2>&1 &
FRONTEND_PID=$!
cd ..
echo "   Frontend PID: $FRONTEND_PID"

# 等待前端启动
sleep 5

# 检查前端是否启动成功
if ! curl -s http://localhost:5173 > /dev/null; then
    echo "❌ 前端启动失败"
    kill $BACKEND_PID $FRONTEND_PID
    exit 1
fi
echo "✅ 前端启动成功"
echo ""

# 3. 运行 Playwright 测试（真实浏览器）
echo "3️⃣ 运行 Playwright 测试..."
cd src-ui
npx playwright test tests/dlp_real_environment.spec.ts
TEST_RESULT=$?
cd ..

if [ $TEST_RESULT -ne 0 ]; then
    echo "❌ Playwright 测试失败"
    kill $BACKEND_PID $FRONTEND_PID
    exit 1
fi
echo "✅ Playwright 测试通过"
echo ""

# 4. 验证后端日志
echo "4️⃣ 验证后端日志..."

# 检查是否有 DLP 扫描日志
if ! grep -q "DLP scan completed" backend.log; then
    echo "❌ 后端日志中没有 DLP 扫描记录"
    kill $BACKEND_PID $FRONTEND_PID
    exit 1
fi
echo "✅ 后端日志正常"

# 检查是否有原始敏感信息泄露
if grep -q "330326199408015618" backend.log; then
    echo "❌ 后端日志中发现原始敏感信息"
    kill $BACKEND_PID $FRONTEND_PID
    exit 1
fi
echo "✅ 没有敏感信息泄露"
echo ""

# 5. 清理
echo "5️⃣ 清理..."
kill $BACKEND_PID $FRONTEND_PID
rm -f backend.log frontend.log
echo "✅ 清理完成"
echo ""

echo "═══════════════════════════════════════"
echo "✅ 真实环境集成测试通过！"
echo "═══════════════════════════════════════"
```

### Playwright 测试：`desktop-client/src-ui/tests/dlp_real_environment.spec.ts`

```typescript
import { test, expect } from '@playwright/test';

test.describe('DLP Real Environment Tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:5173');
  });

  test('should sanitize ID card in real environment', async ({ page }) => {
    // 输入包含身份证号的消息
    await page.fill('.message-input', '我的身份证号是 330326199408015618');
    await page.click('.send-button');

    // 等待消息发送
    await page.waitForSelector('.message-item', { timeout: 5000 });

    // 验证消息内容
    const messageText = await page.textContent('.message-item');
    expect(messageText).toContain('330************618');
    expect(messageText).not.toContain('330326199408015618');

    // 验证 Toast 提示
    const toast = await page.textContent('.dlp-warning-toast');
    expect(toast).toContain('已脱敏');
  });

  test('should block API key in real environment', async ({ page }) => {
    // 输入包含 API 密钥的消息
    await page.fill('.message-input', '阿里云密钥：LTAI4G8aB9cD2eFgH3iJ');
    await page.click('.send-button');

    // 等待错误提示
    await page.waitForSelector('.error-message', { timeout: 5000 });

    // 验证错误提示
    const errorText = await page.textContent('.error-message');
    expect(errorText).toContain('消息包含敏感信息已被阻止');

    // 验证消息未发送
    const messages = await page.$$('.message-item');
    expect(messages.length).toBe(0);
  });

  test('should handle DLP scan failure gracefully', async ({ page }) => {
    // 模拟 DLP 服务故障
    await page.evaluate(() => {
      const originalInvoke = (window as any).__TAURI__.core.invoke;
      (window as any).__TAURI__.core.invoke = async (cmd: string, args: any) => {
        if (cmd === 'scan_user_input') {
          throw new Error('DLP service unavailable');
        }
        return originalInvoke(cmd, args);
      };
    });

    // 尝试发送消息
    await page.fill('.message-input', '测试消息');
    await page.click('.send-button');

    // 验证显示错误
    await page.waitForSelector('.error-message', { timeout: 5000 });
    const errorText = await page.textContent('.error-message');
    expect(errorText).toContain('DLP 扫描失败');

    // 验证消息未发送
    const messages = await page.$$('.message-item');
    expect(messages.length).toBe(0);
  });
});
```

## 测试类型 5：端到端用户流程测试

### 测试文件：`desktop-client/src-ui/cypress/e2e/dlp_user_flow.cy.js`

```javascript
describe('DLP End-to-End User Flow', () => {
  beforeEach(() => {
    cy.visit('http://localhost:5173');
  });

  it('should complete full user flow with DLP protection', () => {
    // 1. 用户输入包含敏感信息的消息
    cy.get('.message-input').type('我的身份证号是 330326199408015618，手机号是 13800138000');
    
    // 2. 点击发送
    cy.get('.send-button').click();
    
    // 3. 验证 DLP 警告显示
    cy.get('.dlp-warning-toast').should('be.visible');
    cy.get('.dlp-warning-toast').should('contain', '已脱敏 2 处敏感信息');
    
    // 4. 验证消息显示脱敏后的内容
    cy.get('.message-item').should('contain', '330************618');
    cy.get('.message-item').should('contain', '138****8000');
    cy.get('.message-item').should('not.contain', '330326199408015618');
    cy.get('.message-item').should('not.contain', '13800138000');
    
    // 5. 等待大模型回复
    cy.get('.message-item').eq(1).should('be.visible', { timeout: 10000 });
    
    // 6. 验证大模型回复中没有原始敏感信息
    cy.get('.message-item').eq(1).should('not.contain', '330326199408015618');
    cy.get('.message-item').eq(1).should('not.contain', '13800138000');
    
    // 7. 验证 DLP 状态指示器
    cy.get('.dlp-status-indicator').should('have.class', 'active');
  });

  it('should prevent sending when DLP blocks message', () => {
    // 1. 用户输入被阻止的内容（API密钥）
    cy.get('.message-input').type('阿里云密钥：LTAI4G8aB9cD2eFgH3iJ');
    
    // 2. 点击发送
    cy.get('.send-button').click();
    
    // 3. 验证显示错误提示
    cy.get('.error-message').should('be.visible');
    cy.get('.error-message').should('contain', '消息包含敏感信息已被阻止');
    
    // 4. 验证消息未发送
    cy.get('.message-item').should('not.exist');
    
    // 5. 验证输入框内容未清空（用户可以修改后重试）
    cy.get('.message-input').should('have.value', '阿里云密钥：LTAI4G8aB9cD2eFgH3iJ');
  });
});
```

## 实施优先级

### P0（紧急）- 已完成 ✅

- [x] 移除降级处理逻辑
- [x] 添加函数类型验证
- [x] 创建诊断工具
- [x] 创建验证脚本

### P1（高优先级）- 待实施

- [ ] 添加失败路径测试（`dlp_failure_paths.cy.js`）
- [ ] 添加安全审计测试（`dlp_security_audit.cy.js`）
- [ ] 添加契约测试（`dlp_contract.cy.js`）

### P2（中优先级）- 待实施

- [ ] 添加真实环境集成测试（`real_environment_integration_test.sh`）
- [ ] 添加端到端用户流程测试（`dlp_user_flow.cy.js`）
- [ ] 添加 Playwright 测试（`dlp_real_environment.spec.ts`）

### P3（低优先级）- 待实施

- [ ] 添加性能测试
- [ ] 添加压力测试
- [ ] 添加兼容性测试

## 测试执行计划

### 每次提交前

```bash
# 1. 运行单元测试
cargo test --lib dlp

# 2. 运行验证脚本
./verify-dlp-fix.sh

# 3. 运行 Cypress E2E 测试
cd src-ui
npm run test:e2e
```

### 每次发布前

```bash
# 1. 运行所有测试
cargo test

# 2. 运行真实环境集成测试
./tests/real_environment_integration_test.sh

# 3. 运行 Playwright 测试
cd src-ui
npx playwright test

# 4. 手动测试关键流程
# - 输入身份证号
# - 输入手机号
# - 输入 API 密钥
# - 验证脱敏和阻止功能
```

### 每周定期

```bash
# 1. 运行安全审计测试
npm run test:security-audit

# 2. 检查测试覆盖率
cargo tarpaulin --out Html

# 3. 审查测试质量
# - 是否有新的测试盲区
# - 是否有新的失败场景
# - 是否需要更新测试用例
```

## 总结

### 关键教训

1. **测试覆盖率高 ≠ 测试质量高**
   - 142 个测试，95% 覆盖率
   - 但仍然出现生产问题
   - 原因：测试只覆盖了正常路径

2. **模拟测试无法发现集成问题**
   - E2E 测试使用了模拟的后端
   - 没有测试真实的 Tauri 命令调用
   - 没有测试真实的 DLP 服务

3. **降级逻辑需要安全审计**
   - 降级逻辑可能引入安全漏洞
   - 需要专门的测试覆盖
   - 安全功能不应该有降级逻辑

4. **错误处理路径同样重要**
   - 错误处理可能泄露敏感信息
   - 需要测试所有错误场景
   - 验证错误处理的安全性

5. **契约测试不可或缺**
   - 前后端接口需要契约测试
   - 验证所有可能的返回值
   - 确保错误处理一致性

### 改进方向

1. ✅ 添加失败路径测试
2. ✅ 添加真实环境测试
3. ✅ 添加契约测试
4. ✅ 添加安全审计测试
5. ✅ 移除不安全的降级逻辑

### 核心原则

> **测试的目标不是追求高覆盖率，而是确保系统在所有情况下都是安全的**

- 测试正常路径 + 错误路径
- 测试模拟环境 + 真实环境
- 测试功能逻辑 + 安全审计
- 测试单个组件 + 集成契约
- 测试成功场景 + 失败场景

## 相关文档

- `DLP_TESTING_LESSONS_LEARNED.md` - 经验教训
- `DLP_ISSUE_FIX_GUIDE.md` - 修复指南
- `DLP_USER_TEST_GUIDE.md` - 用户测试指南
- `verify-dlp-fix.sh` - 自动验证脚本

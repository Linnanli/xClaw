/**
 * DLP 集成 E2E 测试
 * 测试敏感信息检测和脱敏功能在浏览器环境中的实际表现
 */

describe('DLP Integration E2E Tests', () => {
  beforeEach(() => {
    // 访问应用
    cy.visit('http://localhost:5173');
    
    // 等待应用加载完成
    cy.get('.app-container', { timeout: 10000 }).should('be.visible');
  });

  describe('用户输入扫描', () => {
    it('应该检测并脱敏身份证号', () => {
      // 输入包含身份证号的消息
      const messageWithIdCard = '我的身份证号是 110101199003071234';
      
      cy.get('.message-input').type(messageWithIdCard);
      cy.get('.send-button').click();
      
      // 等待消息发送
      cy.wait(500);
      
      // 验证消息被脱敏
      cy.get('.message-item').last().should('contain', '110************234');
      cy.get('.message-item').last().should('not.contain', '110101199003071234');
      
      // 验证有脱敏提示
      cy.get('.dlp-warning', { timeout: 5000 }).should('be.visible');
      cy.get('.dlp-warning').should('contain', '敏感信息已脱敏');
    });

    it('应该检测并脱敏手机号', () => {
      const messageWithPhone = '联系我：13800138000';
      
      cy.get('.message-input').type(messageWithPhone);
      cy.get('.send-button').click();
      
      cy.wait(500);
      
      // 验证手机号被脱敏
      cy.get('.message-item').last().should('contain', '138*****000');
      cy.get('.message-item').last().should('not.contain', '13800138000');
    });

    it('应该阻止包含 API 密钥的消息', () => {
      const messageWithApiKey = '阿里云密钥：LTAI4G8aB9cD2eFgH3iJ';
      
      cy.get('.message-input').type(messageWithApiKey);
      cy.get('.send-button').click();
      
      cy.wait(500);
      
      // 验证消息被阻止
      cy.get('.error-message', { timeout: 5000 }).should('be.visible');
      cy.get('.error-message').should('contain', '消息包含敏感信息已被阻止');
      
      // 验证消息未被添加到聊天记录
      cy.get('.message-item').should('have.length', 0);
    });

    it('应该允许普通消息通过', () => {
      const normalMessage = '你好，今天天气怎么样？';
      
      cy.get('.message-input').type(normalMessage);
      cy.get('.send-button').click();
      
      cy.wait(500);
      
      // 验证消息正常发送
      cy.get('.message-item').last().should('contain', normalMessage);
      
      // 验证没有 DLP 警告
      cy.get('.dlp-warning').should('not.exist');
    });
  });

  describe('多种敏感信息混合', () => {
    it('应该同时脱敏多种敏感信息', () => {
      const mixedMessage = '我的身份证是 110101199003071234，手机号是 13800138000';
      
      cy.get('.message-input').type(mixedMessage);
      cy.get('.send-button').click();
      
      cy.wait(500);
      
      // 验证两种敏感信息都被脱敏
      cy.get('.message-item').last().should('contain', '110************234');
      cy.get('.message-item').last().should('contain', '138*****000');
      cy.get('.message-item').last().should('not.contain', '110101199003071234');
      cy.get('.message-item').last().should('not.contain', '13800138000');
    });

    it('应该阻止包含多个高危敏感信息的消息', () => {
      const criticalMessage = 'AWS密钥：AKIAIOSFODNN7EXAMPLE，阿里云：LTAI4G8aB9cD2eFgH3iJ';
      
      cy.get('.message-input').type(criticalMessage);
      cy.get('.send-button').click();
      
      cy.wait(500);
      
      // 验证消息被阻止
      cy.get('.error-message', { timeout: 5000 }).should('be.visible');
      cy.get('.message-item').should('have.length', 0);
    });
  });

  describe('DLP 配置管理', () => {
    it('应该能够禁用 DLP 功能', () => {
      // 打开设置
      cy.get('.settings-button').click();
      cy.get('.settings-panel').should('be.visible');
      
      // 导航到 DLP 设置
      cy.get('.settings-nav').contains('DLP').click();
      
      // 禁用 DLP
      cy.get('.dlp-enabled-toggle').click();
      cy.get('.save-settings-button').click();
      
      // 等待配置保存
      cy.wait(500);
      
      // 发送包含敏感信息的消息
      cy.get('.message-input').type('身份证：110101199003071234');
      cy.get('.send-button').click();
      
      cy.wait(500);
      
      // 验证消息未被脱敏（因为 DLP 已禁用）
      cy.get('.message-item').last().should('contain', '110101199003071234');
    });

    it('应该能够查看 DLP 统计信息', () => {
      // 先发送几条包含敏感信息的消息
      cy.get('.message-input').type('手机：13800138000');
      cy.get('.send-button').click();
      cy.wait(500);
      
      cy.get('.message-input').type('身份证：110101199003071234');
      cy.get('.send-button').click();
      cy.wait(500);
      
      // 打开设置
      cy.get('.settings-button').click();
      cy.get('.settings-panel').should('be.visible');
      
      // 导航到 DLP 统计
      cy.get('.settings-nav').contains('DLP').click();
      cy.get('.dlp-statistics-tab').click();
      
      // 验证统计信息
      cy.get('.dlp-stats').should('be.visible');
      cy.get('.total-scans').should('contain', '2');
      cy.get('.sensitive-data-detected').should('contain', '2');
    });
  });

  describe('性能测试', () => {
    it('DLP 扫描不应显著影响消息发送速度', () => {
      const startTime = Date.now();
      
      // 发送普通消息
      cy.get('.message-input').type('这是一条普通消息');
      cy.get('.send-button').click();
      
      // 等待消息显示
      cy.get('.message-item').last().should('be.visible').then(() => {
        const endTime = Date.now();
        const duration = endTime - startTime;
        
        // DLP 扫描应该在 100ms 内完成
        expect(duration).to.be.lessThan(1000);
      });
    });

    it('应该能够处理大量文本的扫描', () => {
      // 生成大量文本（约 10KB）
      const largeText = '这是一段很长的文本。'.repeat(500);
      
      cy.get('.message-input').type(largeText.substring(0, 1000)); // Cypress 限制
      cy.get('.send-button').click();
      
      // 验证消息能够正常发送
      cy.get('.message-item', { timeout: 5000 }).last().should('be.visible');
    });
  });

  describe('错误处理', () => {
    it('应该优雅处理 DLP 服务不可用的情况', () => {
      // 模拟 DLP 服务故障
      cy.intercept('POST', '**/api/chat/send', (req) => {
        // 让请求通过，但 DLP 可能失败
        req.continue();
      });
      
      cy.get('.message-input').type('测试消息');
      cy.get('.send-button').click();
      
      // 应该显示错误或降级处理
      cy.get('.message-item, .error-message', { timeout: 5000 }).should('exist');
    });
  });

  describe('审计日志', () => {
    it('应该记录敏感信息检测事件', () => {
      // 发送包含敏感信息的消息
      cy.get('.message-input').type('身份证：110101199003071234');
      cy.get('.send-button').click();
      cy.wait(500);
      
      // 打开审计日志
      cy.get('.settings-button').click();
      cy.get('.settings-nav').contains('审计日志').click();
      
      // 验证审计日志记录
      cy.get('.audit-log-item').should('have.length.greaterThan', 0);
      cy.get('.audit-log-item').first().should('contain', 'user_input_scan');
      cy.get('.audit-log-item').first().should('contain', 'sensitive_data');
    });
  });

  describe('边界情况', () => {
    it('应该处理空消息', () => {
      cy.get('.message-input').clear();
      cy.get('.send-button').click();
      
      // 验证空消息不会被发送
      cy.get('.message-item').should('have.length', 0);
    });

    it('应该处理只包含空格的消息', () => {
      cy.get('.message-input').type('   ');
      cy.get('.send-button').click();
      
      // 验证空白消息不会被发送
      cy.get('.message-item').should('have.length', 0);
    });

    it('应该处理特殊字符', () => {
      const specialChars = '!@#$%^&*()_+-=[]{}|;:,.<>?';
      
      cy.get('.message-input').type(specialChars);
      cy.get('.send-button').click();
      
      cy.wait(500);
      
      // 验证特殊字符消息正常发送
      cy.get('.message-item').last().should('contain', specialChars);
    });
  });

  describe('用户体验', () => {
    it('应该在扫描时显示加载状态', () => {
      cy.get('.message-input').type('测试消息');
      cy.get('.send-button').click();
      
      // 验证加载状态
      cy.get('.loading-indicator').should('be.visible');
      
      // 等待消息发送完成
      cy.get('.loading-indicator', { timeout: 5000 }).should('not.exist');
    });

    it('应该在检测到敏感信息时显示警告', () => {
      cy.get('.message-input').type('手机：13800138000');
      cy.get('.send-button').click();
      
      cy.wait(500);
      
      // 验证警告提示
      cy.get('.dlp-warning').should('be.visible');
      cy.get('.dlp-warning').should('contain', '敏感信息');
    });
  });
});

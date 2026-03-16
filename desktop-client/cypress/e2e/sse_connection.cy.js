/// <reference types="cypress" />

/**
 * SSE 连接端到端测试
 * 
 * 这些测试验证 SSE 连接在浏览器中的行为
 */

describe('SSE Connection Tests', () => {
  beforeEach(() => {
    // 访问应用
    cy.visit('http://localhost:5173');
    
    // 等待应用加载
    cy.get('.app-container', { timeout: 10000 }).should('be.visible');
  });

  describe('SSE Connection Status', () => {
    it('should display SSE connection status', () => {
      // 验证 SSE 状态指示器存在
      cy.get('.sse-status-indicator').should('be.visible');
      
      // 验证初始状态
      cy.get('.sse-status-text').should('contain', 'Connecting');
    });

    it('should show connected status after connection', () => {
      // 等待连接建立
      cy.get('.sse-status-text', { timeout: 5000 })
        .should('contain', 'Connected');
      
      // 验证连接指示器颜色
      cy.get('.sse-status-indicator')
        .should('have.class', 'connected');
    });

    it('should display connection error if connection fails', () => {
      // 模拟网络错误
      cy.intercept('GET', '**/api/chat/events', {
        statusCode: 500,
        body: 'Internal Server Error'
      });
      
      // 刷新页面以重新连接
      cy.reload();
      
      // 等待错误状态
      cy.get('.sse-status-text', { timeout: 5000 })
        .should('contain', 'Error');
    });
  });

  describe('SSE Message Reception', () => {
    it('should receive and display SSE messages', () => {
      // 等待连接建立
      cy.get('.sse-status-text').should('contain', 'Connected');
      
      // 等待消息列表
      cy.get('.message-list', { timeout: 5000 })
        .should('be.visible');
      
      // 验证消息项存在
      cy.get('.message-item').should('have.length.greaterThan', 0);
    });

    it('should display message content correctly', () => {
      // 等待消息加载
      cy.get('.message-item', { timeout: 5000 })
        .first()
        .should('be.visible');
      
      // 验证消息内容
      cy.get('.message-item')
        .first()
        .within(() => {
          cy.get('.message-content').should('not.be.empty');
          cy.get('.message-timestamp').should('not.be.empty');
        });
    });

    it('should update message list in real-time', () => {
      // 获取初始消息数
      cy.get('.message-item').then(($messages) => {
        const initialCount = $messages.length;
        
        // 等待新消息
        cy.get('.message-item', { timeout: 10000 })
          .should('have.length.greaterThan', initialCount);
      });
    });

    it('should handle message deduplication', () => {
      // 等待消息加载
      cy.get('.message-item', { timeout: 5000 });
      
      // 获取消息 ID
      cy.get('.message-item')
        .then(($messages) => {
          const ids = Array.from($messages).map(el => el.dataset.messageId);
          
          // 验证没有重复的消息 ID
          const uniqueIds = new Set(ids);
          expect(uniqueIds.size).to.equal(ids.length);
        });
    });
  });

  describe('SSE Reconnection', () => {
    it('should reconnect when connection is lost', () => {
      // 等待初始连接
      cy.get('.sse-status-text').should('contain', 'Connected');
      
      // 模拟网络中断
      cy.intercept('GET', '**/api/chat/events', { forceNetworkError: true });
      
      // 等待重连状态
      cy.get('.sse-status-text', { timeout: 5000 })
        .should('contain', 'Reconnecting');
      
      // 恢复网络
      cy.intercept('GET', '**/api/chat/events', {
        statusCode: 200,
        body: 'data: {"type":"message","content":"test"}\n\n'
      });
      
      // 验证重连成功
      cy.get('.sse-status-text', { timeout: 10000 })
        .should('contain', 'Connected');
    });

    it('should show reconnection attempts', () => {
      // 模拟网络错误
      cy.intercept('GET', '**/api/chat/events', { forceNetworkError: true });
      
      // 刷新页面
      cy.reload();
      
      // 等待重连状态
      cy.get('.sse-status-text', { timeout: 5000 })
        .should('contain', 'Reconnecting');
      
      // 验证重连计数器
      cy.get('.reconnection-attempt')
        .should('be.visible')
        .and('contain', 'Attempt');
    });

    it('should stop reconnecting after max attempts', () => {
      // 模拟持续的网络错误
      cy.intercept('GET', '**/api/chat/events', { forceNetworkError: true });
      
      // 刷新页面
      cy.reload();
      
      // 等待最大重试次数
      cy.get('.sse-status-text', { timeout: 30000 })
        .should('contain', 'Failed');
    });
  });

  describe('SSE Performance', () => {
    it('should handle high message volume', () => {
      // 等待连接建立
      cy.get('.sse-status-text').should('contain', 'Connected');
      
      // 等待大量消息加载
      cy.get('.message-item', { timeout: 15000 })
        .should('have.length.greaterThan', 100);
      
      // 验证性能指标
      cy.get('.performance-metrics')
        .should('be.visible')
        .within(() => {
          cy.get('.messages-received').should('contain', 'Messages');
          cy.get('.events-processed').should('contain', 'Events');
        });
    });

    it('should maintain performance with continuous messages', () => {
      // 等待连接建立
      cy.get('.sse-status-text').should('contain', 'Connected');
      
      // 获取初始消息数
      cy.get('.message-item').then(($messages) => {
        const initialCount = $messages.length;
        
        // 等待更多消息
        cy.wait(5000);
        
        // 验证消息继续接收
        cy.get('.message-item')
          .should('have.length.greaterThan', initialCount);
      });
    });

    it('should not cause memory leaks', () => {
      // 等待连接建立
      cy.get('.sse-status-text').should('contain', 'Connected');
      
      // 获取初始内存使用
      cy.window().then((win) => {
        const initialMemory = win.performance.memory?.usedJSHeapSize || 0;
        
        // 等待消息接收
        cy.wait(10000);
        
        // 获取最终内存使用
        const finalMemory = win.performance.memory?.usedJSHeapSize || 0;
        
        // 验证内存增长在合理范围内
        const memoryGrowth = finalMemory - initialMemory;
        const maxGrowth = 50 * 1024 * 1024; // 50MB
        
        expect(memoryGrowth).to.be.lessThan(maxGrowth);
      });
    });
  });

  describe('SSE Error Handling', () => {
    it('should handle malformed SSE events', () => {
      // 模拟返回格式错误的事件
      cy.intercept('GET', '**/api/chat/events', {
        statusCode: 200,
        body: 'invalid event data\n\n'
      });
      
      // 刷新页面
      cy.reload();
      
      // 验证应用不会崩溃
      cy.get('.app-container').should('be.visible');
    });

    it('should handle server errors gracefully', () => {
      // 模拟服务器错误
      cy.intercept('GET', '**/api/chat/events', {
        statusCode: 503,
        body: 'Service Unavailable'
      });
      
      // 刷新页面
      cy.reload();
      
      // 验证错误提示
      cy.get('.error-message', { timeout: 5000 })
        .should('be.visible')
        .and('contain', 'Connection failed');
    });

    it('should handle timeout errors', () => {
      // 模拟超时
      cy.intercept('GET', '**/api/chat/events', (req) => {
        req.destroy();
      });
      
      // 刷新页面
      cy.reload();
      
      // 等待超时错误
      cy.get('.sse-status-text', { timeout: 10000 })
        .should('contain', 'Timeout');
    });
  });

  describe('SSE Authentication', () => {
    it('should include auth token in SSE request', () => {
      // 拦截 SSE 请求
      cy.intercept('GET', '**/api/chat/events', (req) => {
        // 验证认证令牌
        const token = req.url.includes('token=') || 
                     req.headers.authorization;
        expect(token).to.exist;
        
        req.reply({
          statusCode: 200,
          body: 'data: {"type":"message"}\n\n'
        });
      });
      
      // 刷新页面
      cy.reload();
      
      // 等待连接建立
      cy.get('.sse-status-text', { timeout: 5000 })
        .should('contain', 'Connected');
    });

    it('should handle authentication failure', () => {
      // 模拟认证失败
      cy.intercept('GET', '**/api/chat/events', {
        statusCode: 401,
        body: 'Unauthorized'
      });
      
      // 刷新页面
      cy.reload();
      
      // 验证错误提示
      cy.get('.error-message', { timeout: 5000 })
        .should('be.visible')
        .and('contain', 'Authentication failed');
    });
  });

  describe('SSE UI Integration', () => {
    it('should update UI when SSE connects', () => {
      // 等待连接
      cy.get('.sse-status-text').should('contain', 'Connected');
      
      // 验证 UI 更新
      cy.get('.chat-input').should('not.be.disabled');
      cy.get('.send-button').should('not.be.disabled');
    });

    it('should disable UI when SSE disconnects', () => {
      // 等待连接
      cy.get('.sse-status-text').should('contain', 'Connected');
      
      // 模拟断开连接
      cy.intercept('GET', '**/api/chat/events', { forceNetworkError: true });
      
      // 等待断开
      cy.get('.sse-status-text', { timeout: 5000 })
        .should('contain', 'Reconnecting');
      
      // 验证 UI 禁用
      cy.get('.chat-input').should('be.disabled');
      cy.get('.send-button').should('be.disabled');
    });

    it('should show loading indicator during reconnection', () => {
      // 模拟网络错误
      cy.intercept('GET', '**/api/chat/events', { forceNetworkError: true });
      
      // 刷新页面
      cy.reload();
      
      // 等待重连状态
      cy.get('.sse-status-text', { timeout: 5000 })
        .should('contain', 'Reconnecting');
      
      // 验证加载指示器
      cy.get('.loading-spinner').should('be.visible');
    });
  });
});

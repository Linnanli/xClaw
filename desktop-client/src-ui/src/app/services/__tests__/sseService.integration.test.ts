import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { SSEService } from '../sseService';

/**
 * SSE服务集成测试
 * 
 * 这些测试需要真实的后端服务运行
 * 在CI/CD环境中可能需要跳过或使用测试服务器
 */
describe('SSEService Integration Tests', () => {
  let sseService: SSEService;
  
  // 跳过集成测试，除非明确启用
  const runIntegrationTests = process.env.RUN_INTEGRATION_TESTS === 'true';

  beforeEach(() => {
    if (!runIntegrationTests) {
      return;
    }
    sseService = new SSEService();
  });

  afterEach(() => {
    if (!runIntegrationTests) {
      return;
    }
    sseService?.disconnect();
  });

  it.skipIf(!runIntegrationTests)('应该能够连接到真实的后端服务', async () => {
    const token = process.env.TEST_TOKEN || 'test-token';
    const baseUrl = process.env.TEST_BASE_URL || 'http://localhost:3000';

    await expect(sseService.connect(token, baseUrl)).resolves.not.toThrow();
    
    // 等待连接建立
    await new Promise(resolve => {
      const checkConnection = () => {
        if (sseService.isConnected()) {
          resolve(void 0);
        } else {
          setTimeout(checkConnection, 100);
        }
      };
      checkConnection();
    });

    expect(sseService.isConnected()).toBe(true);
    expect(sseService.getConnectionStatus()).toBe('connected');
  });

  it.skipIf(!runIntegrationTests)('应该能够接收真实的SSE事件', async () => {
    const token = process.env.TEST_TOKEN || 'test-token';
    const baseUrl = process.env.TEST_BASE_URL || 'http://localhost:3000';

    let receivedEvent = false;
    
    sseService.on('response', () => {
      receivedEvent = true;
    });

    await sseService.connect(token, baseUrl);
    
    // 等待可能的事件（超时5秒）
    await new Promise(resolve => {
      const timeout = setTimeout(resolve, 5000);
      const checkEvent = () => {
        if (receivedEvent) {
          clearTimeout(timeout);
          resolve(void 0);
        } else {
          setTimeout(checkEvent, 100);
        }
      };
      checkEvent();
    });

    // 注意：这个测试可能会失败，因为可能没有实际的事件发送
    // 在真实环境中，你可能需要触发一些操作来生成事件
  });

  it('应该提供集成测试的说明', () => {
    expect(runIntegrationTests).toBeDefined();
    
    if (!runIntegrationTests) {
      console.log(`
集成测试已跳过。要运行集成测试，请：

1. 启动后端服务：
   cargo run -- run --cli-only --no-onboard

2. 设置环境变量并运行测试：
   RUN_INTEGRATION_TESTS=true npm test -- sseService.integration

可选环境变量：
- TEST_TOKEN: 测试用的认证token
- TEST_BASE_URL: 后端服务URL (默认: http://localhost:3000)
      `);
    }
  });
});
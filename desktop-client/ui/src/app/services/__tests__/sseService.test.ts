import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { SSEService, SSEEventType, SSEEventData } from '../sseService';

// Mock EventSource
class MockEventSource {
  public onopen: ((event: Event) => void) | null = null;
  public onerror: ((event: Event) => void) | null = null;
  public onmessage: ((event: MessageEvent) => void) | null = null;
  public readyState: number = 0;
  public url: string;
  
  private listeners: Map<string, ((event: MessageEvent) => void)[]> = new Map();

  constructor(url: string) {
    this.url = url;
    this.readyState = 1; // OPEN
  }

  addEventListener(type: string, listener: (event: MessageEvent) => void) {
    if (!this.listeners.has(type)) {
      this.listeners.set(type, []);
    }
    this.listeners.get(type)!.push(listener);
  }

  removeEventListener(type: string, listener: (event: MessageEvent) => void) {
    const listeners = this.listeners.get(type);
    if (listeners) {
      const index = listeners.indexOf(listener);
      if (index > -1) {
        listeners.splice(index, 1);
      }
    }
  }

  close() {
    this.readyState = 2; // CLOSED
  }

  // Test helper to simulate events
  simulateEvent(type: string, data: any) {
    const listeners = this.listeners.get(type);
    if (listeners) {
      const event = new MessageEvent('message', {
        data: JSON.stringify(data)
      });
      listeners.forEach(listener => listener(event));
    }
  }

  simulateOpen() {
    if (this.onopen) {
      this.onopen(new Event('open'));
    }
  }

  simulateError() {
    if (this.onerror) {
      this.onerror(new Event('error'));
    }
  }
}

// Mock global EventSource
global.EventSource = MockEventSource as any;

describe('SSEService', () => {
  let sseService: SSEService;
  let mockEventSource: MockEventSource;

  beforeEach(() => {
    sseService = new SSEService();
    vi.clearAllMocks();
  });

  afterEach(() => {
    sseService.disconnect();
  });

  describe('连接管理', () => {
    it('应该能够建立SSE连接', async () => {
      const token = 'test-token';
      const baseUrl = 'http://localhost:3000';

      await sseService.connect(token, baseUrl);
      
      // 模拟连接成功
      mockEventSource = (sseService as any).eventSource;
      mockEventSource.simulateOpen();

      expect(sseService.isConnected()).toBe(true);
      expect(sseService.getConnectionStatus()).toBe('connected');
    });

    it('应该能够断开SSE连接', async () => {
      const token = 'test-token';
      const baseUrl = 'http://localhost:3000';

      await sseService.connect(token, baseUrl);
      sseService.disconnect();

      expect(sseService.isConnected()).toBe(false);
      expect(sseService.getConnectionStatus()).toBe('disconnected');
    });

    it('应该在连接错误时自动重连', async () => {
      const token = 'test-token';
      const baseUrl = 'http://localhost:3000';
      
      const reconnectSpy = vi.spyOn(sseService as any, 'scheduleReconnect');

      await sseService.connect(token, baseUrl);
      
      // 模拟连接错误
      mockEventSource = (sseService as any).eventSource;
      mockEventSource.simulateError();

      expect(reconnectSpy).toHaveBeenCalled();
      expect(sseService.getConnectionStatus()).toBe('reconnecting');
    });
  });

  describe('事件处理', () => {
    beforeEach(async () => {
      await sseService.connect('test-token', 'http://localhost:3000');
      mockEventSource = (sseService as any).eventSource;
    });

    it('应该能够接收response事件', () => {
      const callback = vi.fn();
      sseService.on('response', callback);

      const testData = {
        thread_id: 'thread-123',
        content: 'Test response',
        message_id: 'msg-456'
      };

      mockEventSource.simulateEvent('response', testData);

      expect(callback).toHaveBeenCalledWith(testData);
    });

    it('应该能够接收thinking事件', () => {
      const callback = vi.fn();
      sseService.on('thinking', callback);

      const testData = {
        thread_id: 'thread-123',
        content: 'AI is thinking...'
      };

      mockEventSource.simulateEvent('thinking', testData);

      expect(callback).toHaveBeenCalledWith(testData);
    });

    it('应该能够接收job_started事件', () => {
      const callback = vi.fn();
      sseService.on('job_started', callback);

      const testData = {
        job_id: 'job-789',
        title: 'Test Job',
        status: 'in_progress'
      };

      mockEventSource.simulateEvent('job_started', testData);

      expect(callback).toHaveBeenCalledWith(testData);
    });

    it('应该能够接收approval_needed事件', () => {
      const callback = vi.fn();
      sseService.on('approval_needed', callback);

      const testData = {
        operation_id: 'op-123',
        operation_type: 'file_write',
        description: 'Write to sensitive file'
      };

      mockEventSource.simulateEvent('approval_needed', testData);

      expect(callback).toHaveBeenCalledWith(testData);
    });

    it('应该能够移除事件监听器', () => {
      const callback = vi.fn();
      sseService.on('response', callback);
      sseService.off('response', callback);

      const testData = { thread_id: 'thread-123', content: 'Test' };
      mockEventSource.simulateEvent('response', testData);

      expect(callback).not.toHaveBeenCalled();
    });
  });

  describe('连接状态管理', () => {
    it('应该正确报告连接状态', () => {
      expect(sseService.getConnectionStatus()).toBe('disconnected');
      expect(sseService.isConnected()).toBe(false);
    });

    it('应该在连接建立时更新状态', async () => {
      await sseService.connect('test-token', 'http://localhost:3000');
      mockEventSource = (sseService as any).eventSource;
      
      mockEventSource.simulateOpen();

      expect(sseService.getConnectionStatus()).toBe('connected');
      expect(sseService.isConnected()).toBe(true);
    });

    it('应该提供连接状态变化的回调', async () => {
      const statusCallback = vi.fn();
      sseService.onStatusChange(statusCallback);

      await sseService.connect('test-token', 'http://localhost:3000');
      mockEventSource = (sseService as any).eventSource;
      
      mockEventSource.simulateOpen();

      expect(statusCallback).toHaveBeenCalledWith('connected');
    });
  });

  describe('错误处理', () => {
    it('应该处理无效的token', async () => {
      const invalidToken = '';
      
      await expect(sseService.connect(invalidToken, 'http://localhost:3000'))
        .rejects.toThrow('Token is required');
    });

    it('应该处理无效的URL', async () => {
      const invalidUrl = '';
      
      await expect(sseService.connect('valid-token', invalidUrl))
        .rejects.toThrow('Base URL is required');
    });

    it('应该在重连失败时停止重连', async () => {
      const maxRetries = 3;
      sseService = new SSEService({ maxRetries });

      await sseService.connect('test-token', 'http://localhost:3000');
      mockEventSource = (sseService as any).eventSource;

      // 模拟多次连接失败
      for (let i = 0; i < maxRetries + 1; i++) {
        mockEventSource.simulateError();
        // 等待重连逻辑执行
        await new Promise(resolve => setTimeout(resolve, 50));
      }

      // 等待足够长的时间让重连逻辑完成
      await new Promise(resolve => setTimeout(resolve, 200));

      expect(sseService.getConnectionStatus()).toBe('failed');
    });
  });

  describe('日志事件', () => {
    beforeEach(async () => {
      await sseService.connect('test-token', 'http://localhost:3000');
      mockEventSource = (sseService as any).eventSource;
    });

    it('应该能够接收日志事件', () => {
      const callback = vi.fn();
      sseService.on('log', callback);

      const testData = {
        level: 'info',
        message: 'Test log message',
        timestamp: new Date().toISOString(),
        module: 'test'
      };

      mockEventSource.simulateEvent('log', testData);

      expect(callback).toHaveBeenCalledWith(testData);
    });
  });
});
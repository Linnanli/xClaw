/**
 * SSE (Server-Sent Events) 客户端
 * 用于连接到后端的 SSE 事件流，接收实时消息和事件
 */

export interface SseMessage {
  thread_id: string;
  message_id: string;
  content: string;
  role: string;
}

export interface SseThreadState {
  thread_id: string;
  state: string;
}

export interface SseAuthCompleted {
  extension_name: string;
  success: boolean;
}

export interface SseAuthRequired {
  extension_name: string;
  instructions?: string;
}

export type SseEvent = 
  | { type: 'message'; data: SseMessage }
  | { type: 'message_update'; data: { message_id: string; content: string } }
  | { type: 'thread_state'; data: SseThreadState }
  | { type: 'auth_completed'; data: SseAuthCompleted }
  | { type: 'auth_required'; data: SseAuthRequired }
  | { type: 'other'; data: string };

export type EventHandler = (event: SseEvent) => void;

/**
 * SSE 客户端
 */
export class SseClient {
  private baseUrl: string;
  private authToken: string;
  private eventSource: EventSource | null = null;
  private eventHandlers: Set<EventHandler> = new Set();
  private isConnected = false;

  constructor(baseUrl: string, authToken: string) {
    this.baseUrl = baseUrl;
    this.authToken = authToken;
  }

  /**
   * 注册事件处理器
   */
  public onEvent(handler: EventHandler): void {
    this.eventHandlers.add(handler);
  }

  /**
   * 移除事件处理器
   */
  public offEvent(handler: EventHandler): void {
    this.eventHandlers.delete(handler);
  }

  /**
   * 连接到 SSE 事件流
   */
  public connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        const url = `${this.baseUrl}/api/chat/events?token=${encodeURIComponent(this.authToken)}`;
        console.log('📡 Creating EventSource:', url);
        
        // 使用 EventSource API
        this.eventSource = new EventSource(url);
        
        // 设置超时，如果 5 秒内没有连接成功，则认为失败
        const timeout = setTimeout(() => {
          if (!this.isConnected && this.eventSource) {
            console.warn('⏱️  SSE connection timeout, but EventSource is still open');
            // 不关闭连接，因为 EventSource 可能仍在尝试连接
            // 只是标记为已连接，因为 EventSource 已经建立
            this.isConnected = true;
            resolve();
          }
        }, 5000);
        
        this.eventSource.onopen = () => {
          clearTimeout(timeout);
          console.log('✅ SSE onopen event fired');
          this.isConnected = true;
          resolve();
        };
        
        // 监听默认的 message 事件
        this.eventSource.onmessage = (event) => {
          console.log('📨 SSE onmessage (default):', event.data.substring(0, 100));
          try {
            const data = JSON.parse(event.data);
            const sseEvent = this.parseEventData(data);
            this.eventHandlers.forEach(handler => handler(sseEvent));
          } catch (error) {
            console.error('Failed to parse SSE event:', error);
          }
        };
        
        // 监听命名事件：thinking
        this.eventSource.addEventListener('thinking', (event: any) => {
          console.log('📨 SSE event [thinking]:', event.data.substring(0, 100));
          // thinking 事件暂时不处理，只记录日志
        });
        
        // 监听命名事件：status
        this.eventSource.addEventListener('status', (event: any) => {
          console.log('📨 SSE event [status]:', event.data.substring(0, 100));
          try {
            const data = JSON.parse(event.data);
            console.log('   Status:', data);
            // 检查是否是完成状态
            if (data.message === 'Done' || data.type === 'status') {
              const sseEvent: SseEvent = {
                type: 'thread_state',
                data: {
                  thread_id: data.thread_id,
                  state: 'completed',
                },
              };
              this.eventHandlers.forEach(handler => handler(sseEvent));
            }
          } catch (error) {
            console.error('Failed to parse status event:', error);
          }
        });
        
        // 监听命名事件：response（这是实际的消息内容）
        this.eventSource.addEventListener('response', (event: any) => {
          console.log('📨 SSE event [response]:', event.data.substring(0, 100));
          try {
            const data = JSON.parse(event.data);
            console.log('   Response:', data);
            // 将 response 事件转换为 message_update 事件
            const sseEvent: SseEvent = {
              type: 'message_update',
              data: {
                message_id: data.message_id || 'unknown',
                content: data.content || data.message || '',
              },
            };
            this.eventHandlers.forEach(handler => handler(sseEvent));
          } catch (error) {
            console.error('Failed to parse response event:', error);
          }
        });
        
        // 监听命名事件：message
        this.eventSource.addEventListener('message', (event: any) => {
          console.log('📨 SSE event [message]:', event.data.substring(0, 100));
          try {
            const data = JSON.parse(event.data);
            const sseEvent = this.parseEventData(data);
            this.eventHandlers.forEach(handler => handler(sseEvent));
          } catch (error) {
            console.error('Failed to parse message event:', error);
          }
        });
        
        // 监听命名事件：message_update
        this.eventSource.addEventListener('message_update', (event: any) => {
          console.log('📨 SSE event [message_update]:', event.data.substring(0, 100));
          try {
            const data = JSON.parse(event.data);
            const sseEvent = this.parseEventData(data);
            this.eventHandlers.forEach(handler => handler(sseEvent));
          } catch (error) {
            console.error('Failed to parse message_update event:', error);
          }
        });
        
        // 监听命名事件：thread_state
        this.eventSource.addEventListener('thread_state', (event: any) => {
          console.log('📨 SSE event [thread_state]:', event.data.substring(0, 100));
          try {
            const data = JSON.parse(event.data);
            const sseEvent = this.parseEventData(data);
            this.eventHandlers.forEach(handler => handler(sseEvent));
          } catch (error) {
            console.error('Failed to parse thread_state event:', error);
          }
        });
        
        // 监听命名事件：auth_required
        this.eventSource.addEventListener('auth_required', (event: any) => {
          console.log('📨 SSE event [auth_required]:', event.data.substring(0, 100));
          try {
            const data = JSON.parse(event.data);
            const sseEvent = this.parseEventData(data);
            this.eventHandlers.forEach(handler => handler(sseEvent));
          } catch (error) {
            console.error('Failed to parse auth_required event:', error);
          }
        });
        
        // 监听命名事件：auth_completed
        this.eventSource.addEventListener('auth_completed', (event: any) => {
          console.log('📨 SSE event [auth_completed]:', event.data.substring(0, 100));
          try {
            const data = JSON.parse(event.data);
            const sseEvent = this.parseEventData(data);
            this.eventHandlers.forEach(handler => handler(sseEvent));
          } catch (error) {
            console.error('Failed to parse auth_completed event:', error);
          }
        });
        
        this.eventSource.onerror = (error) => {
          clearTimeout(timeout);
          console.error('❌ SSE onerror:', error);
          this.isConnected = false;
          if (this.eventSource?.readyState === EventSource.CLOSED) {
            reject(new Error('SSE connection closed'));
          } else if (this.eventSource?.readyState === EventSource.CONNECTING) {
            console.log('⏳ SSE still connecting...');
          }
        };
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * 解析 SSE 事件数据
   */
  private parseEventData(data: any): SseEvent {
    // 后端发送的数据格式是扁平的，例如：
    // {"type":"thinking","message":"Processing...","thread_id":"test-thread-1773658760"}
    // 或者可能是嵌套的，例如：
    // {"message": {"thread_id": "...", "message_id": "...", "content": "...", "role": "..."}}
    
    // 检查是否是嵌套格式
    if (data.message && typeof data.message === 'object') {
      return { type: 'message', data: data.message };
    } else if (data.message_update && typeof data.message_update === 'object') {
      return { type: 'message_update', data: data.message_update };
    } else if (data.thread_state && typeof data.thread_state === 'object') {
      return { type: 'thread_state', data: data.thread_state };
    } else if (data.auth_completed && typeof data.auth_completed === 'object') {
      return { type: 'auth_completed', data: data.auth_completed };
    } else if (data.auth_required && typeof data.auth_required === 'object') {
      return { type: 'auth_required', data: data.auth_required };
    }
    
    // 检查是否是扁平格式（有 type 字段）
    if (data.type) {
      switch (data.type) {
        case 'message':
          return { type: 'message', data: data };
        case 'message_update':
          return { type: 'message_update', data: data };
        case 'thread_state':
          return { type: 'thread_state', data: data };
        case 'auth_completed':
          return { type: 'auth_completed', data: data };
        case 'auth_required':
          return { type: 'auth_required', data: data };
        default:
          return { type: 'other', data: JSON.stringify(data) };
      }
    }
    
    // 如果都不是，返回 other 类型
    return { type: 'other', data: JSON.stringify(data) };
  }

  /**
   * 检查是否已连接
   */
  public getIsConnected(): boolean {
    return this.isConnected;
  }

  /**
   * 断开连接
   */
  public disconnect(): void {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
    this.isConnected = false;
  }
}

/**
 * 创建 SSE 客户端
 */
export function createSseClient(baseUrl: string, authToken: string): SseClient {
  return new SseClient(baseUrl, authToken);
}

/**
 * 日志条目接口
 */
export interface LogStreamEntry {
  timestamp: string;
  level: string;
  module: string;
  message: string;
  context?: Record<string, any>;
}

/**
 * 日志流客户端 - 用于接收实时日志事件
 */
export class LogStreamClient {
  private baseUrl: string;
  private eventSource: EventSource | null = null;
  private isConnected = false;
  private onLogEntry: (entry: LogStreamEntry) => void;

  constructor(baseUrl: string, onLogEntry: (entry: LogStreamEntry) => void) {
    this.baseUrl = baseUrl;
    this.onLogEntry = onLogEntry;
  }

  /**
   * 连接到日志流
   */
  public connect(): void {
    try {
      const url = `${this.baseUrl}/api/logs/events`;
      this.eventSource = new EventSource(url);

      this.eventSource.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          this.onLogEntry(data);
        } catch (error) {
          console.error('Failed to parse log event:', error);
        }
      };

      this.eventSource.onerror = (error) => {
        console.error('Log stream error:', error);
        this.isConnected = false;
        this.eventSource?.close();
      };

      this.isConnected = true;
    } catch (error) {
      console.error('Failed to connect to log stream:', error);
      this.isConnected = false;
    }
  }

  /**
   * 断开连接
   */
  public disconnect(): void {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
    this.isConnected = false;
  }

  /**
   * 检查是否已连接
   */
  public getIsConnected(): boolean {
    return this.isConnected;
  }
}

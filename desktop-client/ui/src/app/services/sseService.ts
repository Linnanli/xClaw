import { tracing } from '../utils/tracing';

// SSE事件类型定义
export type SSEEventType = 
  | 'response'
  | 'thinking' 
  | 'suggestions'
  | 'tool_started'
  | 'tool_completed'
  | 'tool_result'
  | 'stream_chunk'
  | 'status'
  | 'job_started'
  | 'job_updated'
  | 'job_completed'
  | 'job_failed'
  | 'approval_needed'
  | 'auth_required'
  | 'auth_completed'
  | 'extension_status'
  | 'image_generated'
  | 'error'
  | 'log';

// SSE事件数据接口
export interface SSEEventData {
  [key: string]: any;
}

// 连接状态类型
export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'failed';

// SSE服务配置
export interface SSEServiceConfig {
  maxRetries?: number;
  retryDelay?: number;
  maxRetryDelay?: number;
  retryBackoffFactor?: number;
}

// 事件监听器类型
type EventListener<T = SSEEventData> = (data: T) => void;
type StatusChangeListener = (status: ConnectionStatus) => void;

/**
 * SSE服务类 - 管理与后端的实时连接
 * 
 * 功能：
 * - 建立和维护SSE连接
 * - 自动重连机制
 * - 事件分发和管理
 * - 连接状态监控
 */
export class SSEService {
  private eventSource: EventSource | null = null;
  private token: string = '';
  private baseUrl: string = '';
  private status: ConnectionStatus = 'disconnected';
  
  // 事件监听器管理
  private eventListeners: Map<SSEEventType, EventListener[]> = new Map();
  private statusListeners: StatusChangeListener[] = [];
  
  // 重连配置
  private config: Required<SSEServiceConfig>;
  private retryCount: number = 0;
  private retryTimer: NodeJS.Timeout | null = null;
  private currentRetryDelay: number;

  constructor(config: SSEServiceConfig = {}) {
    this.config = {
      maxRetries: config.maxRetries ?? 5,
      retryDelay: config.retryDelay ?? 1000,
      maxRetryDelay: config.maxRetryDelay ?? 30000,
      retryBackoffFactor: config.retryBackoffFactor ?? 2
    };
    this.currentRetryDelay = this.config.retryDelay;

    tracing.info('SSEService initialized', { config: this.config });
  }

  /**
   * 建立SSE连接
   */
  async connect(token: string, baseUrl: string): Promise<void> {
    if (!token) {
      throw new Error('Token is required');
    }
    if (!baseUrl) {
      throw new Error('Base URL is required');
    }

    this.token = token;
    this.baseUrl = baseUrl;

    tracing.info('Connecting to SSE', { baseUrl });
    
    await this.establishConnection();
  }

  /**
   * 断开SSE连接
   */
  disconnect(): void {
    tracing.info('Disconnecting SSE');
    
    this.clearRetryTimer();
    
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
    
    this.updateStatus('disconnected');
    this.retryCount = 0;
    this.currentRetryDelay = this.config.retryDelay;
  }

  /**
   * 检查是否已连接
   */
  isConnected(): boolean {
    return this.status === 'connected';
  }

  /**
   * 获取当前连接状态
   */
  getConnectionStatus(): ConnectionStatus {
    return this.status;
  }

  /**
   * 添加事件监听器
   */
  on<T = SSEEventData>(eventType: SSEEventType, listener: EventListener<T>): void {
    if (!this.eventListeners.has(eventType)) {
      this.eventListeners.set(eventType, []);
    }
    this.eventListeners.get(eventType)!.push(listener as EventListener);
    
    tracing.debug('Event listener added', { eventType });
  }

  /**
   * 移除事件监听器
   */
  off<T = SSEEventData>(eventType: SSEEventType, listener: EventListener<T>): void {
    const listeners = this.eventListeners.get(eventType);
    if (listeners) {
      const index = listeners.indexOf(listener as EventListener);
      if (index > -1) {
        listeners.splice(index, 1);
        tracing.debug('Event listener removed', { eventType });
      }
    }
  }

  /**
   * 添加状态变化监听器
   */
  onStatusChange(listener: StatusChangeListener): void {
    this.statusListeners.push(listener);
  }

  /**
   * 移除状态变化监听器
   */
  offStatusChange(listener: StatusChangeListener): void {
    const index = this.statusListeners.indexOf(listener);
    if (index > -1) {
      this.statusListeners.splice(index, 1);
    }
  }

  /**
   * 建立EventSource连接
   */
  private async establishConnection(): Promise<void> {
    try {
      this.updateStatus('connecting');
      
      // 关闭现有连接
      if (this.eventSource) {
        this.eventSource.close();
      }

      // 构建SSE URL
      const sseUrl = `${this.baseUrl}/api/chat/events?token=${encodeURIComponent(this.token)}`;
      
      tracing.info('Creating EventSource', { url: sseUrl });
      
      this.eventSource = new EventSource(sseUrl);
      
      // 设置连接事件处理器
      this.setupEventHandlers();
      
    } catch (error) {
      tracing.error('Failed to establish SSE connection', { error });
      this.handleConnectionError();
    }
  }

  /**
   * 设置EventSource事件处理器
   */
  private setupEventHandlers(): void {
    if (!this.eventSource) return;

    // 连接打开
    this.eventSource.onopen = () => {
      tracing.info('SSE connection opened');
      this.updateStatus('connected');
      this.retryCount = 0;
      this.currentRetryDelay = this.config.retryDelay;
    };

    // 连接错误
    this.eventSource.onerror = () => {
      tracing.warn('SSE connection error');
      this.handleConnectionError();
    };

    // 设置各种事件监听器
    this.setupEventListeners();
  }

  /**
   * 设置SSE事件监听器
   */
  private setupEventListeners(): void {
    if (!this.eventSource) return;

    const eventTypes: SSEEventType[] = [
      'response', 'thinking', 'suggestions', 'tool_started', 'tool_completed',
      'tool_result', 'stream_chunk', 'status', 'job_started', 'job_updated',
      'job_completed', 'job_failed', 'approval_needed', 'auth_required',
      'auth_completed', 'extension_status', 'image_generated', 'error', 'log'
    ];

    eventTypes.forEach(eventType => {
      this.eventSource!.addEventListener(eventType, (event: MessageEvent) => {
        try {
          const data = JSON.parse(event.data);
          this.emitEvent(eventType, data);
        } catch (error) {
          tracing.error('Failed to parse SSE event data', { 
            eventType, 
            data: event.data, 
            error 
          });
        }
      });
    });
  }

  /**
   * 分发事件给监听器
   */
  private emitEvent(eventType: SSEEventType, data: SSEEventData): void {
    const listeners = this.eventListeners.get(eventType);
    if (listeners) {
      listeners.forEach(listener => {
        try {
          listener(data);
        } catch (error) {
          tracing.error('Event listener error', { eventType, error });
        }
      });
    }

    tracing.debug('SSE event emitted', { eventType, dataKeys: Object.keys(data) });
  }

  /**
   * 处理连接错误
   */
  private handleConnectionError(): void {
    if (this.status === 'disconnected') {
      return; // 已经主动断开，不需要重连
    }

    // 增加重试计数
    this.retryCount++;
    
    if (this.retryCount > this.config.maxRetries) {
      tracing.error('Max retry attempts reached, giving up', { 
        retryCount: this.retryCount,
        maxRetries: this.config.maxRetries 
      });
      this.updateStatus('failed');
      return;
    }

    this.updateStatus('reconnecting');
    this.scheduleReconnect();
  }

  /**
   * 安排重连
   */
  private scheduleReconnect(): void {
    this.clearRetryTimer();
    
    const delay = Math.min(this.currentRetryDelay, this.config.maxRetryDelay);
    
    tracing.info('Scheduling reconnect', { 
      delay, 
      retryCount: this.retryCount,
      maxRetries: this.config.maxRetries 
    });

    this.retryTimer = setTimeout(async () => {
      this.currentRetryDelay *= this.config.retryBackoffFactor;
      await this.establishConnection();
    }, delay);
  }

  /**
   * 清除重连定时器
   */
  private clearRetryTimer(): void {
    if (this.retryTimer) {
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
  }

  /**
   * 更新连接状态
   */
  private updateStatus(newStatus: ConnectionStatus): void {
    if (this.status !== newStatus) {
      const oldStatus = this.status;
      this.status = newStatus;
      
      tracing.info('SSE status changed', { from: oldStatus, to: newStatus });
      
      // 通知状态监听器
      this.statusListeners.forEach(listener => {
        try {
          listener(newStatus);
        } catch (error) {
          tracing.error('Status listener error', { error });
        }
      });
    }
  }
}

// 导出单例实例
export const sseService = new SSEService();
import { useState, useEffect, useCallback } from 'react';
import { sseService, ConnectionStatus, SSEEventType, SSEEventData } from '../services/sseService';
import { tracing } from '../utils/tracing';

/**
 * SSE连接管理Hook
 * 
 * 提供SSE连接的状态管理和事件处理功能
 */
export function useSSEConnection() {
  const [isConnected, setIsConnected] = useState(sseService.isConnected());
  const [status, setStatus] = useState(sseService.getConnectionStatus());
  const [error, setError] = useState<string | null>(null);

  // 监听连接状态变化
  useEffect(() => {
    const handleStatusChange = (newStatus: ConnectionStatus) => {
      setStatus(newStatus);
      setIsConnected(sseService.isConnected());
      
      // 清除错误状态（如果连接成功）
      if (newStatus === 'connected') {
        setError(null);
      }
    };

    sseService.onStatusChange(handleStatusChange);

    return () => {
      sseService.offStatusChange(handleStatusChange);
    };
  }, []);

  /**
   * 建立SSE连接
   */
  const connect = useCallback(async (token: string, baseUrl: string) => {
    try {
      setError(null);
      await sseService.connect(token, baseUrl);
      tracing.info('SSE connection established via hook');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Connection failed';
      setError(errorMessage);
      tracing.error('SSE connection failed via hook', { error: errorMessage });
      throw err;
    }
  }, []);

  /**
   * 断开SSE连接
   */
  const disconnect = useCallback(() => {
    sseService.disconnect();
    setError(null);
    tracing.info('SSE connection disconnected via hook');
  }, []);

  /**
   * 添加事件监听器
   */
  const addEventListener = useCallback(<T = SSEEventData>(
    eventType: SSEEventType, 
    listener: (data: T) => void
  ) => {
    sseService.on(eventType, listener);
  }, []);

  /**
   * 移除事件监听器
   */
  const removeEventListener = useCallback(<T = SSEEventData>(
    eventType: SSEEventType, 
    listener: (data: T) => void
  ) => {
    sseService.off(eventType, listener);
  }, []);

  return {
    // 状态
    isConnected,
    status,
    error,
    
    // 方法
    connect,
    disconnect,
    addEventListener,
    removeEventListener
  };
}
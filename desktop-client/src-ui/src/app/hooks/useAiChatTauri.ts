/**
 * AI Chat Hook - 使用 Tauri IPC 通信
 * 
 * 替代 HTTP API,使用 Tauri 命令和事件系统进行聊天通信
 * 
 * 架构:
 * ```
 * 前端 ──invoke──► send_chat_message (Tauri Command)
 *                      │
 *                      ▼
 *                  API Client
 *                      │
 *                      ▼
 *                  后端 API
 * 
 * 后端 SSE ──► subscribe_chat_events ──emit──► 前端
 * ```
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useDlpScan } from './useDlpScan';

// ============================================================================
// 类型定义
// ============================================================================

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp?: number;
}

interface SendMessageResponse {
  message_id: string;
  success: boolean;
}

/**
 * 聊天事件类型
 * 
 * 与 Rust 端的 ChatEvent 枚举对应
 */
type ChatEvent =
  | {
      type: 'response';
      message_id: string;
      content: string;
      thread_id: string;
    }
  | {
      type: 'thinking';
      message: string;
    }
  | {
      type: 'status';
      message: string;
      level: string;
    }
  | {
      type: 'error';
      message: string;
      code?: string;
    }
  | {
      type: 'connection_status';
      connected: boolean;
      message: string;
    };

interface UseAiChatTauriOptions {
  threadId: string;
  onError?: (error: string) => void;
  onStatusChange?: (status: string) => void;
}

// ============================================================================
// Hook 实现
// ============================================================================

/**
 * AI 聊天 Hook (Tauri IPC 版本)
 * 
 * 提供基于 Tauri IPC 的聊天功能,替代 HTTP API
 * 
 * @param options - 配置选项
 * @returns 聊天状态和操作方法
 * 
 * @example
 * ```typescript
 * const chat = useAiChatTauri({
 *   threadId: 'thread-123',
 *   onError: (error) => console.error(error),
 * });
 * 
 * // 发送消息
 * await chat.sendMessage('Hello, AI!');
 * 
 * // 访问消息列表
 * console.log(chat.messages);
 * ```
 */
export function useAiChatTauri(options: UseAiChatTauriOptions) {
  const { threadId, onError, onStatusChange } = options;

  // ============================================================================
  // 状态管理
  // ============================================================================

  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [thinkingMessage, setThinkingMessage] = useState<string | null>(null);

  // 使用 ref 跟踪消息 ID,避免重复
  const messageIdRef = useRef(1);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  // DLP 扫描
  const { scanUserInput } = useDlpScan();

  // ============================================================================
  // 事件处理
  // ============================================================================

  /**
   * 处理聊天事件
   * 
   * 注意: 使用 useRef 存储回调函数引用，避免频繁重新订阅
   */
  const handleChatEventRef = useRef((event: ChatEvent) => {
    console.log('📨 Received chat event:', event);

    switch (event.type) {
      case 'response':
        // 添加 AI 响应消息
        setMessages((prev) => [
          ...prev,
          {
            id: event.message_id,
            role: 'assistant',
            content: event.content,
            timestamp: Date.now(),
          },
        ]);
        setIsLoading(false);
        setThinkingMessage(null);
        break;

      case 'thinking':
        // 显示思考状态
        setThinkingMessage(event.message);
        setIsLoading(true);
        break;

      case 'status':
        // 更新状态
        console.log(`📊 Status [${event.level}]:`, event.message);
        onStatusChange?.(event.message);
        break;

      case 'error':
        // 显示错误
        const errorMsg = event.code
          ? `${event.message} (${event.code})`
          : event.message;
        setError(errorMsg);
        setIsLoading(false);
        setThinkingMessage(null);
        onError?.(errorMsg);
        break;

      case 'connection_status':
        // 更新连接状态
        setIsConnected(event.connected);
        console.log(
          event.connected ? '✅ Connected to chat events' : '⚠️  Disconnected from chat events'
        );
        break;
    }
  });

  // 更新 ref 以使用最新的回调
  useEffect(() => {
    handleChatEventRef.current = (event: ChatEvent) => {
      console.log('📨 Received chat event:', event);

      switch (event.type) {
        case 'response':
          setMessages((prev) => [
            ...prev,
            {
              id: event.message_id,
              role: 'assistant',
              content: event.content,
              timestamp: Date.now(),
            },
          ]);
          setIsLoading(false);
          setThinkingMessage(null);
          break;

        case 'thinking':
          setThinkingMessage(event.message);
          setIsLoading(true);
          break;

        case 'status':
          console.log(`📊 Status [${event.level}]:`, event.message);
          onStatusChange?.(event.message);
          break;

        case 'error':
          const errorMsg = event.code
            ? `${event.message} (${event.code})`
            : event.message;
          setError(errorMsg);
          setIsLoading(false);
          setThinkingMessage(null);
          onError?.(errorMsg);
          break;

        case 'connection_status':
          setIsConnected(event.connected);
          console.log(
            event.connected ? '✅ Connected to chat events' : '⚠️  Disconnected from chat events'
          );
          break;
      }
    };
  }, [onError, onStatusChange]);

  // ============================================================================
  // 生命周期管理
  // ============================================================================

  /**
   * 订阅聊天事件
   * 
   * 只在组件挂载时订阅一次，避免频繁重新订阅
   * 支持幂等订阅：如果已订阅，后端会直接返回成功
   */
  useEffect(() => {
    let mounted = true;
    let cleanupExecuted = false;

    const setupEventListener = async () => {
      try {
        console.log('🔗 Setting up chat event listener...');

        // 监听聊天事件
        const unlisten = await listen<ChatEvent>('chat-event', (event) => {
          if (mounted) {
            handleChatEventRef.current(event.payload);
          }
        });

        unlistenRef.current = unlisten;

        // 订阅聊天事件（幂等操作，已订阅时会直接返回成功）
        try {
          await invoke('subscribe_chat_events');
          console.log('✅ Chat events subscribed');
        } catch (err) {
          // 如果是"已订阅"错误，忽略它
          const errorMsg = err instanceof Error ? err.message : String(err);
          if (errorMsg.includes('Already subscribed')) {
            console.log('ℹ️  Already subscribed to chat events');
          } else {
            throw err;
          }
        }
      } catch (err) {
        console.error('❌ Failed to setup chat events:', err);
        const errorMsg = err instanceof Error ? err.message : String(err);
        setError(errorMsg);
        onError?.(errorMsg);
      }
    };

    setupEventListener();

    // 清理函数
    return () => {
      if (cleanupExecuted) {
        console.log('⚠️  Cleanup already executed, skipping...');
        return;
      }

      cleanupExecuted = true;
      mounted = false;

      console.log('🧹 Cleaning up chat event listener...');

      // 取消监听
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
        console.log('✅ Event listener removed');
      }

      // 取消订阅 (异步但不等待，避免阻塞清理)
      invoke('unsubscribe_chat_events')
        .then(() => {
          console.log('✅ Chat events unsubscribed');
        })
        .catch((err) => {
          console.error('❌ Failed to unsubscribe:', err);
        });
    };
  }, []); // 空依赖数组，只在挂载/卸载时执行

  // ============================================================================
  // 消息发送
  // ============================================================================

  /**
   * 发送消息
   * 
   * 1. DLP 扫描
   * 2. 添加用户消息到本地状态
   * 3. 调用 Tauri 命令发送消息
   * 4. 等待 SSE 事件接收 AI 响应
   */
  const sendMessage = useCallback(
    async (content: string) => {
      try {
        console.log('📤 Sending message...');
        setIsLoading(true);
        setError(null);
        setThinkingMessage(null);

        // 步骤 1: DLP 扫描
        console.log('   Step 1: DLP scanning...');
        const dlpResult = await scanUserInput(content);

        if (dlpResult.was_blocked) {
          throw new Error(
            dlpResult.block_reason || 'Message blocked by DLP policy'
          );
        }

        // 使用脱敏后的内容
        const sanitizedContent = dlpResult.sanitized_content;
        console.log('   ✅ DLP scan passed');

        // 步骤 2: 添加用户消息到本地状态
        const userMessage: Message = {
          id: `msg-${messageIdRef.current++}`,
          role: 'user',
          content: sanitizedContent,
          timestamp: Date.now(),
        };

        setMessages((prev) => [...prev, userMessage]);
        console.log('   ✅ User message added to local state');

        // 步骤 3: 调用 Tauri 命令发送消息
        console.log('   Step 3: Invoking send_chat_message...');
        const response = await invoke<SendMessageResponse>('send_chat_message', {
          threadId,
          content: sanitizedContent,
        });

        console.log('   ✅ Message sent:', response.message_id);

        // 步骤 4: 等待 SSE 事件接收 AI 响应
        // (响应会通过 handleChatEvent 处理)
        console.log('   Step 4: Waiting for AI response via SSE...');
      } catch (err) {
        console.error('❌ Failed to send message:', err);
        const errorMsg = err instanceof Error ? err.message : String(err);
        setError(errorMsg);
        setIsLoading(false);
        setThinkingMessage(null);
        onError?.(errorMsg);
      }
    },
    [threadId, scanUserInput, onError]
  );

  // ============================================================================
  // 返回值
  // ============================================================================

  // 为了与 useAiChat 兼容，添加 input 和 setInput 状态
  const [input, setInput] = useState('');

  // 兼容 useAiChat 的 append 方法
  const append = useCallback(
    async (message: { role: 'user' | 'assistant'; content: string }) => {
      if (message.role === 'user') {
        await sendMessage(message.content);
      }
    },
    [sendMessage]
  );

  // 兼容 useAiChat 的 handleSubmit 方法
  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (input.trim()) {
        sendMessage(input);
        setInput('');
      }
    },
    [input, sendMessage]
  );

  // 兼容 useAiChat 的 reload 方法
  const reload = useCallback(async () => {
    if (messages.length === 0) return;
    
    const lastUserMessage = [...messages].reverse().find(m => m.role === 'user');
    if (lastUserMessage) {
      setMessages(prev => prev.filter(m => m.id !== lastUserMessage.id));
      await sendMessage(lastUserMessage.content);
    }
  }, [messages, sendMessage]);

  // 兼容 useAiChat 的 stop 方法
  const stop = useCallback(() => {
    setIsLoading(false);
    setThinkingMessage(null);
  }, []);

  return {
    // 状态
    messages,
    isLoading,
    isConnected,
    error,
    thinkingMessage,
    input,

    // 操作
    sendMessage,
    setInput,
    setMessages,

    // 兼容 useAiChat 的方法
    append,
    handleSubmit,
    reload,
    stop,

    // 工具方法
    clearError: () => setError(null),
    clearMessages: () => setMessages([]),
  };
}

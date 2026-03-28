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
import { useDlpScan, type SanitizationResult, type SanitizationStats } from './useDlpScan';
import { tracing } from '@utils/tracing';

// ============================================================================
// 类型定义
// ============================================================================

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp?: number;
  /** DLP 脱敏统计（仅用户消息，且经过脱敏处理时存在） */
  dlpStats?: SanitizationStats;
  /** AI 思考步骤（仅 assistant 消息，嵌入到消息自身，参考 Vercel AI SDK message.parts 模式） */
  thinkingSteps?: ThinkingStep[];
  /** 本轮激活的技能名称（仅 assistant 消息，在 LLM 调用前收到） */
  activeSkills?: string[];
}

/** DLP 警告事件，供 UI 层消费 */
export interface DlpWarningEvent {
  type: 'redacted' | 'blocked';
  stats: SanitizationStats;
  blockReason?: string;
  /** 事件时间戳，用于去重 */
  timestamp: number;
}

interface SendMessageResponse {
  message_id: string;
  success: boolean;
}

/** 单个思考步骤 */
export interface ThinkingStep {
  id: string;
  message: string;
  timestamp: number;
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
    }
  | {
      /** 本轮对话激活的技能列表，在 LLM 调用前发出 */
      type: 'skills_activated';
      skills: string[];
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
  // Tauri 嵌入式架构：引擎在进程内运行，默认视为已连接
  // 只有收到明确的断连事件时才设为 false
  const [isConnected, setIsConnected] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [thinkingMessage, setThinkingMessage] = useState<string | null>(null);
  /** 思考步骤（useState 驱动实时渲染，ref 同步最新值供事件回调读取） */
  const [thinkingSteps, setThinkingSteps] = useState<ThinkingStep[]>([]);
  const thinkingStepsRef = useRef<ThinkingStep[]>([]);
  const [dlpWarning, setDlpWarning] = useState<DlpWarningEvent | null>(null);
  /** 当前轮次激活的技能（在 response 事件到来前暂存，嵌入到 assistant 消息） */
  const activeSkillsRef = useRef<string[]>([]);

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
    tracing.debug('Received chat event', { type: event.type });

    switch (event.type) {
      case 'response':
        // 添加 AI 响应消息，将累积的 thinkingSteps 嵌入消息自身
        setMessages((prev) => [
          ...prev,
          {
            id: event.message_id,
            role: 'assistant',
            content: event.content,
            timestamp: Date.now(),
            thinkingSteps: thinkingStepsRef.current.length > 0
              ? [...thinkingStepsRef.current]
              : undefined,
            activeSkills: activeSkillsRef.current.length > 0
              ? [...activeSkillsRef.current]
              : undefined,
          },
        ]);
        thinkingStepsRef.current = [];
        activeSkillsRef.current = [];
        setThinkingSteps([]);
        setIsLoading(false);
        setThinkingMessage(null);
        break;

      case 'thinking':
        // 显示思考状态并累积步骤
        setThinkingMessage(event.message);
        {
          const newStep = { id: `step-${Date.now()}`, message: event.message, timestamp: Date.now() };
          thinkingStepsRef.current = [...thinkingStepsRef.current, newStep];
          setThinkingSteps((prev) => [...prev, newStep]);
        }
        setIsLoading(true);
        break;

      case 'status':
        // 更新状态
        tracing.debug('Status update', { level: event.level, message: event.message });
        onStatusChange?.(event.message);
        break;

      case 'error':
        // 显示错误
        const errorMsg = event.code
          ? `${event.message} (${event.code})`
          : event.message;
        
        // SSE 连接错误不应该清除 loading 状态（可能是后台重连）
        // 只有非连接错误才清除 loading
        if (!event.code?.startsWith('HTTP_') && !event.code?.startsWith('CONNECTION_')) {
          setError(errorMsg);
          setIsLoading(false);
          setThinkingMessage(null);
          // 错误时也保留 thinkingStepsRef 供用户查看
        } else {
          // 连接错误：更新连接状态
          setIsConnected(false);
          tracing.warn('SSE connection error', { code: event.code, message: event.message });
        }
        onError?.(errorMsg);
        break;

      case 'connection_status':
        // 更新连接状态
        setIsConnected(event.connected);
        tracing.info(
          event.connected ? 'Connected to chat events' : 'Disconnected from chat events'
        );
        break;

      case 'skills_activated':
        // 暂存激活的技能，等 response 事件到来时嵌入消息
        activeSkillsRef.current = event.skills;
        tracing.debug('Skills activated', { skills: event.skills });
        break;
    }
  });

  // 更新 ref 以使用最新的回调
  useEffect(() => {
    handleChatEventRef.current = (event: ChatEvent) => {
      tracing.debug('Received chat event', { type: event.type });

      switch (event.type) {
        case 'response':
          setMessages((prev) => [
            ...prev,
            {
              id: event.message_id,
              role: 'assistant',
              content: event.content,
              timestamp: Date.now(),
              thinkingSteps: thinkingStepsRef.current.length > 0
                ? [...thinkingStepsRef.current]
                : undefined,
              activeSkills: activeSkillsRef.current.length > 0
                ? [...activeSkillsRef.current]
                : undefined,
            },
          ]);
          thinkingStepsRef.current = [];
          activeSkillsRef.current = [];
          setThinkingSteps([]);
          setIsLoading(false);
          setThinkingMessage(null);
          break;

        case 'thinking':
          setThinkingMessage(event.message);
          {
            const newStep = { id: `step-${Date.now()}`, message: event.message, timestamp: Date.now() };
            thinkingStepsRef.current = [...thinkingStepsRef.current, newStep];
            setThinkingSteps((prev) => [...prev, newStep]);
          }
          setIsLoading(true);
          break;

        case 'status':
          tracing.debug('Status update', { level: event.level, message: event.message });
          onStatusChange?.(event.message);
          break;

        case 'error':
          const errorMsg = event.code
            ? `${event.message} (${event.code})`
            : event.message;
          
          if (!event.code?.startsWith('HTTP_') && !event.code?.startsWith('CONNECTION_')) {
            setError(errorMsg);
            setIsLoading(false);
            setThinkingMessage(null);
            // 错误时也保留 thinkingStepsRef 供用户查看
          } else {
            setIsConnected(false);
            tracing.warn('SSE connection error', { code: event.code, message: event.message });
          }
          onError?.(errorMsg);
          break;

        case 'connection_status':
          setIsConnected(event.connected);
          tracing.info(
            event.connected ? 'Connected to chat events' : 'Disconnected from chat events'
          );
          break;

        case 'skills_activated':
          activeSkillsRef.current = event.skills;
          tracing.debug('Skills activated', { skills: event.skills });
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
        tracing.debug('Setting up chat event listener');

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
          tracing.info('Chat events subscribed');
        } catch (err) {
          // 如果是"已订阅"错误，忽略它
          const errorMsg = err instanceof Error ? err.message : String(err);
          if (errorMsg.includes('Already subscribed')) {
            tracing.debug('Already subscribed to chat events');
          } else {
            throw err;
          }
        }
      } catch (err) {
        tracing.error('Failed to setup chat events', { error: err });
        const errorMsg = err instanceof Error ? err.message : String(err);
        setError(errorMsg);
        onError?.(errorMsg);
      }
    };

    setupEventListener();

    // 清理函数
    return () => {
      if (cleanupExecuted) {
        tracing.warn('Cleanup already executed, skipping');
        return;
      }

      cleanupExecuted = true;
      mounted = false;

      tracing.debug('Cleaning up chat event listener');

      // 取消监听
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
        tracing.debug('Event listener removed');
      }

      // 取消订阅 (异步但不等待，避免阻塞清理)
      invoke('unsubscribe_chat_events')
        .then(() => {
          tracing.info('Chat events unsubscribed');
        })
        .catch((err) => {
          tracing.error('Failed to unsubscribe', { error: err });
        });
    };
  }, []); // 空依赖数组，只在挂载/卸载时执行

  // ============================================================================
  // 消息发送
  // ============================================================================

  // 统一的发送失败处理：重置 loading 状态并上报错误
  // sendMessage / sendMessageWithThread / sendMessageAfterOptimistic 共用
  const handleSendError = useCallback(
    (err: unknown) => {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(errorMsg);
      setIsLoading(false);
      setThinkingMessage(null);
      thinkingStepsRef.current = [];
      setThinkingSteps([]);
      onError?.(errorMsg);
    },
    [onError],
  );

  /**
   * 发送消息
   *
   * 1. DLP 扫描（失败时 Fail-Safe 阻止发送）
   * 2. 添加用户消息到本地状态
   * 3. 调用 Tauri 命令发送消息
   * 4. 等待 SSE 事件接收 AI 响应
   */
  const sendMessage = useCallback(
    async (content: string) => {
      const tempMsgId = `msg-${messageIdRef.current++}`;
      setMessages((prev) => [
        ...prev,
        { id: tempMsgId, role: 'user', content, timestamp: Date.now() },
      ]);
      setIsLoading(true);
      setError(null);
      setThinkingMessage(null);
      thinkingStepsRef.current = [];
      setThinkingSteps([]);

      try {
        tracing.debug('Sending message', { threadId, contentLength: content.length });

        let dlpResult;
        try {
          dlpResult = await scanUserInput(content);
        } catch (dlpErr) {
          // DLP 扫描失败：Fail-Safe，回滚乐观更新并阻止发送
          tracing.error('DLP scan failed, blocking message (fail-safe)', { error: dlpErr });
          setMessages((prev) => prev.filter((m) => m.id !== tempMsgId));
          handleSendError(dlpErr);
          return;
        }

        if (dlpResult.was_blocked) {
          setMessages((prev) => prev.filter((m) => m.id !== tempMsgId));
          setDlpWarning({
            type: 'blocked',
            stats: dlpResult.sanitization_stats,
            blockReason: dlpResult.block_reason || undefined,
            timestamp: Date.now(),
          });
          setIsLoading(false);
          return;
        }

        const sanitizedContent = dlpResult.sanitized_content;
        if (dlpResult.had_sensitive_data) {
          setMessages((prev) =>
            prev.map((m) =>
              m.id === tempMsgId
                ? { ...m, content: sanitizedContent, dlpStats: dlpResult.sanitization_stats }
                : m,
            ),
          );
          setDlpWarning({
            type: 'redacted',
            stats: dlpResult.sanitization_stats,
            timestamp: Date.now(),
          });
        }

        const response = await invoke<SendMessageResponse>('send_chat_message', {
          threadId,
          content: sanitizedContent,
        });

        tracing.info('Message sent', { messageId: response.message_id });
      } catch (err) {
        tracing.error('Failed to send message', { error: err });
        handleSendError(err);
      }
    },
    [threadId, scanUserInput, handleSendError],
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

  // 用指定的 threadId 发送消息（解决新建对话时 state 更新时机问题）
  const sendMessageWithThread = useCallback(
    async (overrideThreadId: string, content: string) => {
      const tempMsgId = `msg-${messageIdRef.current++}`;
      setMessages((prev) => [
        ...prev,
        { id: tempMsgId, role: 'user', content, timestamp: Date.now() },
      ]);
      setIsLoading(true);
      setError(null);
      setThinkingMessage(null);
      thinkingStepsRef.current = [];
      setThinkingSteps([]);

      try {
        tracing.debug('Sending message with thread', { threadId: overrideThreadId });

        const dlpResult = await scanUserInput(content);

        if (dlpResult.was_blocked) {
          setMessages((prev) => prev.filter((m) => m.id !== tempMsgId));
          setDlpWarning({
            type: 'blocked',
            stats: dlpResult.sanitization_stats,
            blockReason: dlpResult.block_reason || undefined,
            timestamp: Date.now(),
          });
          setIsLoading(false);
          return;
        }

        const sanitizedContent = dlpResult.sanitized_content;
        if (dlpResult.had_sensitive_data) {
          setMessages((prev) =>
            prev.map((m) =>
              m.id === tempMsgId
                ? { ...m, content: sanitizedContent, dlpStats: dlpResult.sanitization_stats }
                : m,
            ),
          );
          setDlpWarning({
            type: 'redacted',
            stats: dlpResult.sanitization_stats,
            timestamp: Date.now(),
          });
        }

        const response = await invoke<SendMessageResponse>('send_chat_message', {
          threadId: overrideThreadId,
          content: sanitizedContent,
        });

        tracing.info('Message sent', { messageId: response.message_id });
      } catch (err) {
        tracing.error('Failed to send message', { error: err });
        handleSendError(err);
      }
    },
    [scanUserInput, handleSendError],
  );

  /**
   * 乐观更新版发送：消息已由调用方提前加到列表里，
   * 这里只做 DLP 扫描 + 后端发送，不再重复 setMessages。
   */
  const sendMessageAfterOptimistic = useCallback(
    async (overrideThreadId: string, content: string, tempMsgId: string) => {
      setIsLoading(true);
      setError(null);
      setThinkingMessage(null);
      thinkingStepsRef.current = [];
      setThinkingSteps([]);

      try {
        const dlpResult = await scanUserInput(content);

        if (dlpResult.was_blocked) {
          // 被阻止：移除临时消息
          setMessages((prev) => prev.filter((m) => m.id !== tempMsgId));
          setDlpWarning({
            type: 'blocked',
            stats: dlpResult.sanitization_stats,
            blockReason: dlpResult.block_reason || undefined,
            timestamp: Date.now(),
          });
          setIsLoading(false);
          return;
        }

        const sanitizedContent = dlpResult.sanitized_content;
        if (dlpResult.had_sensitive_data) {
          // 脱敏：原地更新消息内容
          setMessages((prev) =>
            prev.map((m) =>
              m.id === tempMsgId
                ? { ...m, content: sanitizedContent, dlpStats: dlpResult.sanitization_stats }
                : m,
            ),
          );
          setDlpWarning({
            type: 'redacted',
            stats: dlpResult.sanitization_stats,
            timestamp: Date.now(),
          });
        }

        await invoke<SendMessageResponse>('send_chat_message', {
          threadId: overrideThreadId,
          content: sanitizedContent,
        });
      } catch (err) {
        tracing.error('Failed to send message', { error: err });
        handleSendError(err);
      }
    },
    [scanUserInput, handleSendError],
  );

  return {
    // 状态
    messages,
    isLoading,
    isConnected,
    error,
    thinkingMessage,
    thinkingSteps,
    input,
    dlpWarning,

    // 操作
    sendMessage,
    sendMessageWithThread,
    sendMessageAfterOptimistic,
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
    clearDlpWarning: () => setDlpWarning(null),
  };
}

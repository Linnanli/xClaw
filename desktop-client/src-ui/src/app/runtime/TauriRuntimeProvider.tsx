/**
 * TauriRuntimeProvider — 将 Tauri IPC 桥接到 assistant-ui 的 ExternalStoreRuntime
 *
 * 架构：
 *   assistant-ui Thread 组件
 *       ↕ ExternalStoreRuntime
 *   TauriRuntimeProvider（本文件）
 *       ↕ Tauri IPC (invoke / listen)
 *   Rust 后端 (send_chat_message / subscribe_chat_events)
 *
 * 职责：
 * - 管理消息状态（messages, isRunning）
 * - 将 assistant-ui 的 onNew 转换为 Tauri IPC 调用
 * - 监听 SSE chat-event 并更新消息列表
 * - DLP 扫描集成（发送前拦截）
 * - 线程管理（创建、切换、历史加载）
 */

import { type ReactNode, useState, useCallback, useEffect, useRef, createContext, useContext } from 'react';
import {
  AssistantRuntimeProvider,
  useExternalStoreRuntime,
  type ThreadMessageLike,
  type AppendMessage,
} from '@assistant-ui/react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { threadApi } from '@utils/tauri';
import { useDlpScan } from '@hooks/useDlpScan';
import { tracing } from '@utils/tracing';

// ============================================================================
// 类型定义
// ============================================================================

interface TauriMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  /** 思考链内容（仅 assistant 消息） */
  reasoning?: string;
}

/** 后端可能返回的所有 role 类型 */
type BackendRole = 'user' | 'assistant' | 'system' | 'tool' | 'tool_calls' | string;

/** 过滤：只保留 assistant-ui 支持的 role（user / assistant） */
function isDisplayableRole(role: BackendRole): role is 'user' | 'assistant' {
  return role === 'user' || role === 'assistant';
}

interface SendMessageResponse {
  message_id: string;
  success: boolean;
}

type ChatEvent =
  | { type: 'response'; message_id: string; content: string; thread_id: string }
  | { type: 'thinking'; message: string }
  | { type: 'status'; message: string; level: string }
  | { type: 'error'; message: string; code?: string }
  | { type: 'connection_status'; connected: boolean; message: string };

interface TauriRuntimeProviderProps {
  children: ReactNode;
  threadId: string | null;
  onThreadCreated?: (threadId: string) => void;
}

// ============================================================================
// DLP Context — 暴露 DLP 状态给外层组件（弹窗等）
// ============================================================================

interface DlpState {
  blocked: boolean;
  blockReason: string | null;
  clearBlock: () => void;
}

const DlpContext = createContext<DlpState>({
  blocked: false,
  blockReason: null,
  clearBlock: () => {},
});

export const useDlpState = () => useContext(DlpContext);

// ============================================================================
// 消息转换：TauriMessage → assistant-ui ThreadMessageLike
// ============================================================================

function convertMessage(msg: TauriMessage): ThreadMessageLike {
  const parts: Array<{ type: 'text'; text: string } | { type: 'reasoning'; text: string }> = [];

  // 思考链放在文本前面（和 Claude/ChatGPT 的展示顺序一致）
  if (msg.reasoning) {
    parts.push({ type: 'reasoning', text: msg.reasoning });
  }

  if (msg.content) {
    parts.push({ type: 'text', text: msg.content });
  }

  // 至少有一个 part
  if (parts.length === 0) {
    parts.push({ type: 'text', text: '' });
  }

  return {
    role: msg.role,
    content: parts,
    id: msg.id,
    createdAt: new Date(msg.timestamp),
  };
}

// ============================================================================
// Provider 组件
// ============================================================================

export function TauriRuntimeProvider({
  children,
  threadId,
  onThreadCreated,
}: TauriRuntimeProviderProps) {
  const [messages, setMessages] = useState<TauriMessage[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const threadIdRef = useRef(threadId);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const msgIdCounter = useRef(1);
  const { scanUserInput } = useDlpScan();

  // DLP 阻止状态
  const [dlpBlocked, setDlpBlocked] = useState(false);
  const [dlpBlockReason, setDlpBlockReason] = useState<string | null>(null);
  const clearDlpBlock = useCallback(() => {
    setDlpBlocked(false);
    setDlpBlockReason(null);
  }, []);

  // 同步 threadId ref
  useEffect(() => {
    threadIdRef.current = threadId;
  }, [threadId]);

  // ── 加载历史消息 ──
  useEffect(() => {
    if (!threadId) {
      setMessages([]);
      return;
    }
    loadHistory(threadId);
  }, [threadId]);

  const loadHistory = async (tid: string) => {
    try {
      const history = await threadApi.getMessages(tid);
      setMessages(
        history
          .filter((m) => isDisplayableRole(m.role))
          .map((m) => ({
            id: m.id,
            role: m.role as 'user' | 'assistant',
            content: m.content,
            timestamp: new Date(m.created_at).getTime(),
          })),
      );
    } catch (err) {
      tracing.error('Failed to load thread history', { error: err });
    }
  };

  // ── SSE 事件监听 ──
  useEffect(() => {
    let mounted = true;

    const setup = async () => {
      try {
        const unlisten = await listen<ChatEvent>('chat-event', (event) => {
          if (!mounted) return;
          handleChatEvent(event.payload);
        });
        unlistenRef.current = unlisten;
        await invoke('subscribe_chat_events').catch(() => {});
      } catch (err) {
        tracing.error('Failed to setup chat events', { error: err });
      }
    };

    setup();

    return () => {
      mounted = false;
      unlistenRef.current?.();
      unlistenRef.current = null;
      invoke('unsubscribe_chat_events').catch(() => {});
    };
  }, []);

  // ── 处理 SSE 事件 ──
  // 临时 assistant 消息 ID（thinking 阶段创建，response 阶段更新）
  const pendingAssistantId = useRef<string | null>(null);
  const thinkingBuffer = useRef<string>('');

  /** 系统状态消息（非 AI 推理），不显示在思考链中 */
  const SYSTEM_STATUS_PATTERNS = [
    'Processing',
    'Calling LLM',
    'Waiting for',
    'Connecting',
    'Retrying',
  ];

  function isSystemStatus(message: string): boolean {
    return SYSTEM_STATUS_PATTERNS.some((p) => message.startsWith(p));
  }

  const handleChatEvent = useCallback((event: ChatEvent) => {
    switch (event.type) {
      case 'thinking': {
        setIsRunning(true);

        // 过滤系统状态消息，只保留 AI 推理内容
        if (isSystemStatus(event.message)) {
          tracing.debug('Filtered system status from thinking', { message: event.message });
          break;
        }

        // 累积真正的思考内容
        thinkingBuffer.current += (thinkingBuffer.current ? '\n' : '') + event.message;

        if (!pendingAssistantId.current) {
          const tempId = `thinking-${Date.now()}`;
          pendingAssistantId.current = tempId;
          setMessages((prev) => [
            ...prev,
            {
              id: tempId,
              role: 'assistant',
              content: '',
              reasoning: thinkingBuffer.current,
              timestamp: Date.now(),
            },
          ]);
        } else {
          const tempId = pendingAssistantId.current;
          setMessages((prev) =>
            prev.map((m) =>
              m.id === tempId ? { ...m, reasoning: thinkingBuffer.current } : m,
            ),
          );
        }
        break;
      }

      case 'response': {
        if (pendingAssistantId.current) {
          // 有思考链：更新临时消息，加上最终回复内容
          const tempId = pendingAssistantId.current;
          setMessages((prev) =>
            prev.map((m) =>
              m.id === tempId
                ? { ...m, id: event.message_id, content: event.content }
                : m,
            ),
          );
        } else {
          // 没有思考链：直接添加 assistant 消息
          setMessages((prev) => [
            ...prev,
            {
              id: event.message_id,
              role: 'assistant',
              content: event.content,
              timestamp: Date.now(),
            },
          ]);
        }

        // 重置状态
        pendingAssistantId.current = null;
        thinkingBuffer.current = '';
        setIsRunning(false);
        break;
      }

      case 'error':
        pendingAssistantId.current = null;
        thinkingBuffer.current = '';
        setIsRunning(false);
        tracing.error('Chat event error', { message: event.message, code: event.code });
        break;
    }
  }, []);

  // ── 发送新消息（assistant-ui onNew 回调）──
  const onNew = useCallback(
    async (appendMessage: AppendMessage) => {
      const textPart = appendMessage.content.find((p) => p.type === 'text');
      if (!textPart || textPart.type !== 'text') return;

      const rawContent = textPart.text;

      // 1. DLP 扫描
      let content = rawContent;
      try {
        const dlpResult = await scanUserInput(rawContent);
        if (dlpResult.was_blocked) {
          tracing.warn('DLP blocked message');
          setDlpBlocked(true);
          setDlpBlockReason(dlpResult.block_reason || '内容包含敏感信息');
          return;
        }
        content = dlpResult.sanitized_content;
      } catch {
        // DLP 扫描失败时 Fail-Safe：阻止发送
        tracing.error('DLP scan failed, blocking message');
        setDlpBlocked(true);
        setDlpBlockReason('安全扫描失败，无法发送消息');
        return;
      }

      // 2. 确保有 thread
      let tid = threadIdRef.current;
      if (!tid) {
        try {
          const newThread = await threadApi.createThread();
          tid = newThread.id;
          threadIdRef.current = tid;
          onThreadCreated?.(tid);
        } catch (err) {
          tracing.error('Failed to create thread', { error: err });
          return;
        }
      }

      // 3. 乐观更新：立即添加用户消息
      const userMsg: TauriMessage = {
        id: `msg-${msgIdCounter.current++}`,
        role: 'user',
        content,
        timestamp: Date.now(),
      };
      setMessages((prev) => [...prev, userMsg]);
      setIsRunning(true);

      // 4. 发送到后端
      try {
        await invoke<SendMessageResponse>('send_chat_message', {
          threadId: tid,
          content,
        });
      } catch (err) {
        tracing.error('Failed to send message', { error: err });
        setIsRunning(false);
      }
    },
    [scanUserInput, onThreadCreated],
  );

  // ── 取消生成 ──
  const onCancel = useCallback(async () => {
    setIsRunning(false);
  }, []);

  // ── 构建 ExternalStoreRuntime ──
  const runtime = useExternalStoreRuntime({
    messages,
    isRunning,
    convertMessage,
    onNew,
    onCancel,
    setMessages: (newMessages) => {
      if (Array.isArray(newMessages)) {
        setMessages(newMessages as TauriMessage[]);
      }
    },
  });

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <DlpContext.Provider value={{ blocked: dlpBlocked, blockReason: dlpBlockReason, clearBlock: clearDlpBlock }}>
        {children}
      </DlpContext.Provider>
    </AssistantRuntimeProvider>
  );
}

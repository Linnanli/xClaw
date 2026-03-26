/**
 * TauriRuntimeProvider — 将 Tauri IPC 桥接到 assistant-ui 的 ExternalStoreRuntime
 *
 * 架构：
 *   assistant-ui Thread 组件
 *       ↕ ExternalStoreRuntime
 *   TauriRuntimeProvider（本文件）
 *       ↕ Tauri IPC (invoke / listen)
 *   Rust 后端 (send_chat_message / subscribe_chat_events)
 *       ↕ 内嵌 IronClaw Agent（skills + 工具 + 多步推理）
 *       ↕ LLM API（直连，无需 Admin Backend 中转）
 *
 * 职责：
 * - 管理消息状态（messages, isRunning）
 * - 将 assistant-ui 的 onNew 转换为 Tauri IPC 调用
 * - 监听 chat-event 并更新消息列表（含流式、思考链、工具状态）
 * - DLP 扫描集成（发送前拦截，Fail-Safe 设计）
 * - 线程管理（创建、切换、历史加载）
 * - 模型动态切换（通过 model_override metadata 传递给 Agent）
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
import type { SanitizationStats } from '@hooks/useDlpScan';
import { tracing } from '@utils/tracing';

// ============================================================================
// 类型定义
// ============================================================================

interface TauriMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
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
  | { type: 'stream_chunk'; content: string }
  | { type: 'tool_started'; name: string }
  | { type: 'tool_completed'; name: string; success: boolean; error?: string }
  | { type: 'approval_needed'; request_id: string; tool_name: string; description: string }
  | { type: 'status'; message: string; level: string }
  | { type: 'error'; message: string; code?: string }
  | { type: 'connection_status'; connected: boolean; message: string };

interface TauriRuntimeProviderProps {
  children: ReactNode;
  threadId: string | null;
  onThreadCreated?: (threadId: string) => void;
  modelId?: string;
}

// ============================================================================
// DLP Context — 完整版，与 ChatRuntimeProvider 接口对齐
// ============================================================================

interface DlpState {
  blocked: boolean;
  blockReason: string | null;
  clearBlock: () => void;
  redactedStats: SanitizationStats | null;
  clearRedacted: () => void;
  onBlocked: (reason: string) => void;
  onRedacted: (stats: SanitizationStats) => void;
}

const DlpContext = createContext<DlpState>({
  blocked: false,
  blockReason: null,
  clearBlock: () => {},
  redactedStats: null,
  clearRedacted: () => {},
  onBlocked: () => {},
  onRedacted: () => {},
});

export const useDlpState = () => useContext(DlpContext);

// ============================================================================
// 消息转换：TauriMessage → assistant-ui ThreadMessageLike
// ============================================================================

function convertMessage(msg: TauriMessage): ThreadMessageLike {
  const parts: Array<{ type: 'text'; text: string } | { type: 'reasoning'; text: string }> = [];

  if (msg.reasoning) {
    parts.push({ type: 'reasoning', text: msg.reasoning });
  }
  if (msg.content) {
    parts.push({ type: 'text', text: msg.content });
  }
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
// 系统状态过滤（非 AI 推理内容，不显示在思考链中）
// ============================================================================

const SYSTEM_STATUS_PREFIXES = [
  'Processing',
  'Calling LLM',
  'Waiting for',
  'Connecting',
  'Retrying',
];

function isSystemStatus(message: string): boolean {
  return SYSTEM_STATUS_PREFIXES.some((p) => message.startsWith(p));
}

// ============================================================================
// Provider 组件
// ============================================================================

export function TauriRuntimeProvider({
  children,
  threadId,
  onThreadCreated,
  modelId,
}: TauriRuntimeProviderProps) {
  const [messages, setMessages] = useState<TauriMessage[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const threadIdRef = useRef(threadId);
  const modelIdRef = useRef(modelId);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const msgIdCounter = useRef(1);
  const { scanUserInput } = useDlpScan();

  // DLP 状态（完整版）
  const [dlpBlocked, setDlpBlocked] = useState(false);
  const [dlpBlockReason, setDlpBlockReason] = useState<string | null>(null);
  const [dlpRedactedStats, setDlpRedactedStats] = useState<SanitizationStats | null>(null);

  const clearDlpBlock = useCallback(() => {
    setDlpBlocked(false);
    setDlpBlockReason(null);
  }, []);
  const clearDlpRedacted = useCallback(() => setDlpRedactedStats(null), []);
  const onBlockedCb = useCallback((reason: string) => {
    setDlpBlocked(true);
    setDlpBlockReason(reason);
  }, []);
  const onRedactedCb = useCallback((stats: SanitizationStats) => {
    setDlpRedactedStats(stats);
  }, []);

  // 同步 refs
  useEffect(() => { threadIdRef.current = threadId; }, [threadId]);
  useEffect(() => { modelIdRef.current = modelId; }, [modelId]);

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

  // ── chat-event 监听 ──
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

  // ── 处理 chat-event（思考链 + 流式 + 工具 + 响应）──
  const pendingAssistantId = useRef<string | null>(null);
  const thinkingBuffer = useRef<string>('');

  const handleChatEvent = useCallback((event: ChatEvent) => {
    switch (event.type) {
      case 'thinking': {
        setIsRunning(true);
        if (isSystemStatus(event.message)) break;

        thinkingBuffer.current += (thinkingBuffer.current ? '\n' : '') + event.message;

        if (!pendingAssistantId.current) {
          const tempId = `thinking-${Date.now()}`;
          pendingAssistantId.current = tempId;
          setMessages((prev) => [
            ...prev,
            { id: tempId, role: 'assistant', content: '', reasoning: thinkingBuffer.current, timestamp: Date.now() },
          ]);
        } else {
          const tempId = pendingAssistantId.current;
          setMessages((prev) =>
            prev.map((m) => (m.id === tempId ? { ...m, reasoning: thinkingBuffer.current } : m)),
          );
        }
        break;
      }

      case 'stream_chunk': {
        setIsRunning(true);
        if (!pendingAssistantId.current) {
          const tempId = `stream-${Date.now()}`;
          pendingAssistantId.current = tempId;
          setMessages((prev) => [
            ...prev,
            { id: tempId, role: 'assistant', content: event.content, timestamp: Date.now() },
          ]);
        } else {
          const tempId = pendingAssistantId.current;
          setMessages((prev) =>
            prev.map((m) => (m.id === tempId ? { ...m, content: m.content + event.content } : m)),
          );
        }
        break;
      }

      case 'tool_started': {
        setIsRunning(true);
        tracing.debug('Tool started', { name: event.name });
        break;
      }

      case 'tool_completed': {
        if (!event.success) {
          tracing.warn('Tool failed', { name: event.name, error: event.error });
        }
        break;
      }

      case 'approval_needed': {
        tracing.info('Tool approval needed', { tool: event.tool_name, requestId: event.request_id });
        // TODO: 弹出审批对话框，调用 ic_approve_tool / ic_deny_tool
        break;
      }

      case 'response': {
        if (pendingAssistantId.current) {
          const tempId = pendingAssistantId.current;
          setMessages((prev) =>
            prev.map((m) => (m.id === tempId ? { ...m, id: event.message_id, content: event.content } : m)),
          );
        } else {
          setMessages((prev) => [
            ...prev,
            { id: event.message_id, role: 'assistant', content: event.content, timestamp: Date.now() },
          ]);
        }
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

      case 'connection_status':
        tracing.info('Connection status', { connected: event.connected, message: event.message });
        break;
    }
  }, []);

  // ── 发送新消息（assistant-ui onNew 回调）──
  const onNew = useCallback(
    async (appendMessage: AppendMessage) => {
      const textPart = appendMessage.content.find((p) => p.type === 'text');
      if (!textPart || textPart.type !== 'text') return;

      const rawContent = textPart.text;

      // 1. DLP 扫描（Fail-Safe：扫描失败时阻止发送）
      let content = rawContent;
      try {
        const dlpResult = await scanUserInput(rawContent);
        if (dlpResult.was_blocked) {
          tracing.warn('DLP blocked message');
          onBlockedCb(dlpResult.block_reason || '内容包含敏感信息');
          return;
        }
        if (dlpResult.had_sensitive_data) {
          onRedactedCb(dlpResult.sanitization_stats);
          content = dlpResult.sanitized_content;
        }
      } catch {
        tracing.error('DLP scan failed, blocking message (fail-safe)');
        onBlockedCb('安全扫描失败，无法发送消息');
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

      // 4. 发送到 Agent（含可选 modelId）
      try {
        await invoke<SendMessageResponse>('send_chat_message', {
          threadId: tid,
          content,
          modelId: modelIdRef.current ?? null,
        });
      } catch (err) {
        tracing.error('Failed to send message', { error: err });
        setIsRunning(false);
      }
    },
    [scanUserInput, onThreadCreated, onBlockedCb, onRedactedCb],
  );

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
      <DlpContext.Provider
        value={{
          blocked: dlpBlocked,
          blockReason: dlpBlockReason,
          clearBlock: clearDlpBlock,
          redactedStats: dlpRedactedStats,
          clearRedacted: clearDlpRedacted,
          onBlocked: onBlockedCb,
          onRedacted: onRedactedCb,
        }}
      >
        {children}
      </DlpContext.Provider>
    </AssistantRuntimeProvider>
  );
}

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
import { threadApi, modelApi, type ModelConfigItem } from '@utils/tauri';
import { useDlpScan } from '@hooks/useDlpScan';
import type { SanitizationStats } from '@hooks/useDlpScan';
import { tracing } from '@utils/tracing';
import { ModelContext } from '@contexts/ModelContext';
import { isErrorResponse, friendlyErrorMessage } from '@utils/friendlyError';

// ============================================================================
// 类型定义
// ============================================================================

interface TauriMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  reasoning?: string;
  /** DLP 脱敏统计，仅用户消息有值 */
  dlpStats?: SanitizationStats;
  /** 错误消息，assistant 消息出错时设置 */
  error?: string;
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
  | { type: 'response'; message_id: string; content: string; thread_id: string; source?: string }
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
  /** 初始模型 ID，后续切换通过 ModelContext 完成 */
  initialModelId?: string;
  /** 模型切换时通知父组件，使选择状态在组件重新挂载后保持 */
  onModelChange?: (modelId: string) => void;
  /** 打开自定义模型弹窗的回调（由 ChatTabTauri 提供） */
  onOpenCustomModelModal?: () => void;
  /** 定时任务手动触发时的提示词，挂载后自动发送 */
  pendingPrompt?: string | null;
  onPendingPromptSent?: () => void;
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
// Approval Context — 即时工具授权
// ============================================================================

export interface PendingApproval {
  request_id: string;
  tool_name: string;
  description: string;
}

interface ApprovalState {
  pendingApprovals: PendingApproval[];
  approve: (requestId: string, threadId: string) => Promise<void>;
  deny: (requestId: string, threadId: string) => Promise<void>;
}

const ApprovalContext = createContext<ApprovalState>({
  pendingApprovals: [],
  approve: async () => {},
  deny: async () => {},
});

export const useApprovalState = () => useContext(ApprovalContext);

// ============================================================================
// 消息转换：TauriMessage → assistant-ui ThreadMessageLike
// ============================================================================

function convertMessage(msg: TauriMessage): ThreadMessageLike {
  const parts: Array<{ type: 'text'; text: string } | { type: 'reasoning'; text: string }> = [];

  // ironclaw agent loop 在 handle_message 失败时发送 `"Error: {chain}"` 格式的普通 response，
  // 检测后转换为 error 状态，由 ErrorPrimitive 渲染友好提示。
  if (msg.role === 'assistant' && isErrorResponse(msg.content)) {
    return {
      role: msg.role,
      content: [{ type: 'text', text: '' }],
      id: msg.id,
      createdAt: new Date(msg.timestamp),
      status: { type: 'incomplete', reason: 'error', error: friendlyErrorMessage(msg.content) },
    };
  }

  if (msg.reasoning) parts.push({ type: 'reasoning', text: msg.reasoning });
  if (msg.content) parts.push({ type: 'text', text: msg.content });
  if (parts.length === 0) parts.push({ type: 'text', text: '' });

  const base = {
    role: msg.role,
    content: parts,
    id: msg.id,
    createdAt: new Date(msg.timestamp),
    ...(msg.dlpStats ? { metadata: { custom: { dlpStats: msg.dlpStats } } } : {}),
  };

  if (msg.error) {
    return { ...base, status: { type: 'incomplete', reason: 'error', error: msg.error } };
  }

  return base;
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
  initialModelId,
  onModelChange,
  onOpenCustomModelModal,
  pendingPrompt,
  onPendingPromptSent,
}: TauriRuntimeProviderProps) {
  // 若有 pendingMessage，表示有任务正在执行，初始 isRunning = true
  const [messages, setMessages] = useState<TauriMessage[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const threadIdRef = useRef(threadId);
  const modelIdRef = useRef(initialModelId);
  const selectedModelRef = useRef<ModelConfigItem | null>(null);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const msgIdCounter = useRef(1);
  const { scanUserInput } = useDlpScan();

  // ── 模型列表状态（由本 Provider 管理，通过 ModelContext 向下传递）──
  const [models, setModels] = useState<ModelConfigItem[]>([]);
  const [selectedModelId, setSelectedModelId] = useState<string>(initialModelId ?? '');
  const [modelsLoading, setModelsLoading] = useState(true);

  // DLP 状态（完整版）
  const [dlpBlocked, setDlpBlocked] = useState(false);
  const [dlpBlockReason, setDlpBlockReason] = useState<string | null>(null);
  const [dlpRedactedStats, setDlpRedactedStats] = useState<SanitizationStats | null>(null);

  // 即时工具授权状态
  const [pendingApprovals, setPendingApprovals] = useState<PendingApproval[]>([]);

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

  // 即时工具授权：approve / deny 通过 IPC 发送消息给 Agent
  const sendApprovalDecision = useCallback(async (command: 'ic_approve_tool' | 'ic_deny_tool', requestId: string, tid: string) => {
    try {
      await invoke(command, { requestId, threadId: tid });
      setPendingApprovals((prev) => prev.filter((a) => a.request_id !== requestId));
    } catch (err) {
      tracing.error(`Failed to ${command}`, { requestId, error: err });
    }
  }, []);

  const approve = useCallback((requestId: string, tid: string) =>
    sendApprovalDecision('ic_approve_tool', requestId, tid), [sendApprovalDecision]);

  const deny = useCallback((requestId: string, tid: string) =>
    sendApprovalDecision('ic_deny_tool', requestId, tid), [sendApprovalDecision]);

  // 同步 threadId ref
  useEffect(() => { threadIdRef.current = threadId; }, [threadId]);

  // ── 加载模型列表（可被引擎就绪事件重新触发）──
  const loadModels = useCallback(async () => {
    try {
      const allModels = await modelApi.getAvailableModels();
      setModels(allModels);
      // 如果初始 modelId 无效或未设置，选默认模型
      const prevId = modelIdRef.current ?? '';
      const resolvedId = (prevId && allModels.some((m) => m.model_id === prevId))
        ? prevId
        : (allModels.find((m) => m.is_default) ?? allModels[0])?.model_id ?? prevId;
      // 同步 ref，确保首次发消息时 apiBaseUrl/apiKey 一致
      modelIdRef.current = resolvedId;
      selectedModelRef.current = allModels.find((m) => m.model_id === resolvedId) ?? null;
      setSelectedModelId(resolvedId);
      if (resolvedId !== prevId) onModelChange?.(resolvedId);
    } catch (err) {
      tracing.error('Failed to load model list', { error: err });
    } finally {
      setModelsLoading(false);
    }
  }, [onModelChange]);

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  const selectModel = useCallback((modelId: string) => {
    setSelectedModelId(modelId);
    modelIdRef.current = modelId;
    const model = models.find((m) => m.model_id === modelId) ?? null;
    selectedModelRef.current = model;
    onModelChange?.(modelId);
    // 立即激活 provider，确保定时任务等后台操作也使用新模型
    modelApi.activateModel({
      model_id: modelId,
      api_base_url: model?.api_base_url,
      api_key: model?.api_key,
    }).catch((err) => {
      console.warn('[selectModel] ic_activate_model failed:', err);
    });
  }, [onModelChange, models]);

  // ── 加载历史消息 ──
  const pendingPromptSent = useRef(false);
  const onPendingPromptSentRef = useRef(onPendingPromptSent);
  onPendingPromptSentRef.current = onPendingPromptSent;

  useEffect(() => {
    if (!threadId) {
      setMessages([]);
      return;
    }

    // pendingPrompt 已发送过，跳过重新加载（避免 clearPendingPrompt 触发的
    // 依赖变化导致 setMessages(loaded) 覆盖掉本地添加的 user 气泡）
    if (pendingPromptSent.current) return;

    let cancelled = false;

    const load = async () => {
      try {
        const history = await threadApi.getMessages(threadId);
        if (cancelled) return;

        const loaded = history
          .filter((m) => isDisplayableRole(m.role))
          .map((m) => ({
            id: m.id,
            role: m.role as 'user' | 'assistant',
            content: m.content,
            timestamp: new Date(m.created_at).getTime(),
          }));

        if (pendingPrompt && !pendingPromptSent.current) {
          pendingPromptSent.current = true;
          const userMsg: TauriMessage = {
            id: `routine-${msgIdCounter.current++}`,
            role: 'user',
            content: pendingPrompt,
            timestamp: Date.now(),
          };
          setMessages([...loaded, userMsg]);
          setIsRunning(true);
          invoke('send_chat_message', {
            threadId,
            content: pendingPrompt,
            modelId: modelIdRef.current ?? null,
            apiBaseUrl: selectedModelRef.current?.api_base_url ?? null,
            apiKey: selectedModelRef.current?.api_key ?? null,
          }).catch((err) => {
            tracing.error('Failed to send routine prompt', { error: err });
            setIsRunning(false);
          });
          onPendingPromptSentRef.current?.();
        } else {
          setMessages(loaded);
        }
      } catch (err) {
        if (!cancelled) tracing.error('Failed to load thread history', { error: err });
      }
    };

    load();
    return () => { cancelled = true; };
    // onPendingPromptSent 通过 ref 引用，不放入依赖数组，
    // 避免 clearPendingPrompt 触发 effect 重跑覆盖 user 消息。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [threadId, pendingPrompt]);

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
        setPendingApprovals((prev) => {
          // 去重：同一 request_id 不重复添加
          if (prev.some((a) => a.request_id === event.request_id)) return prev;
          return [...prev, { request_id: event.request_id, tool_name: event.tool_name, description: event.description }];
        });
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
            { id: event.message_id, role: 'assistant' as const, content: event.content, timestamp: Date.now() },
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
        setMessages((prev) => {
          // 如果已有 pending assistant 消息，更新它为错误状态
          const lastMsg = prev[prev.length - 1];
          if (lastMsg?.role === 'assistant' && !lastMsg.error) {
            return prev.map((m, i) =>
              i === prev.length - 1 ? { ...m, error: event.message } : m,
            );
          }
          // 否则新增一条错误消息
          return [
            ...prev,
            {
              id: `error-${Date.now()}`,
              role: 'assistant',
              content: '',
              timestamp: Date.now(),
              error: event.message,
            },
          ];
        });
        break;

      case 'connection_status':
        tracing.info('Connection status', { connected: event.connected, message: event.message });
        // 引擎就绪时重新加载模型列表，解决启动时序竞态问题：
        // 前端 mount 时引擎可能还未就绪，此时 get_available_models 会失败。
        // 收到 connected=true 说明引擎已初始化完成，此时重载可以拿到真实模型列表。
        if (event.connected) {
          loadModels();
        }
        break;
    }
  }, [loadModels]);

  // ── 发送新消息（assistant-ui onNew 回调）──
  const onNew = useCallback(
    async (appendMessage: AppendMessage) => {
      const textPart = appendMessage.content.find((p) => p.type === 'text');
      if (!textPart || textPart.type !== 'text') return;

      const rawContent = textPart.text;

      // 1. DLP 扫描（Fail-Safe：扫描失败时阻止发送）
      let content = rawContent;
      let dlpResult: Awaited<ReturnType<typeof scanUserInput>> | null = null;
      try {
        dlpResult = await scanUserInput(rawContent);
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

      // 3. 乐观更新：立即添加用户消息（含 DLP 脱敏统计）
      const userMsg: TauriMessage = {
        id: `msg-${msgIdCounter.current++}`,
        role: 'user',
        content,
        timestamp: Date.now(),
        dlpStats: dlpResult?.had_sensitive_data ? dlpResult.sanitization_stats : undefined,
      };
      setMessages((prev) => [...prev, userMsg]);
      setIsRunning(true);

      // 4. 发送到 Agent（含可选 modelId）
      try {
        await invoke<SendMessageResponse>('send_chat_message', {
          threadId: tid,
          content,
          modelId: modelIdRef.current ?? null,
          apiBaseUrl: selectedModelRef.current?.api_base_url ?? null,
          apiKey: selectedModelRef.current?.api_key ?? null,
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
      <ModelContext.Provider
        value={{
          selectedModelId,
          models,
          selectModel,
          openCustomModelModal: onOpenCustomModelModal ?? (() => {}),
          loading: modelsLoading,
        }}
      >
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
          <ApprovalContext.Provider
            value={{
              pendingApprovals,
              approve: (requestId) => approve(requestId, threadIdRef.current ?? ''),
              deny: (requestId) => deny(requestId, threadIdRef.current ?? ''),
            }}
          >
            {children}
          </ApprovalContext.Provider>
        </DlpContext.Provider>
      </ModelContext.Provider>
    </AssistantRuntimeProvider>
  );
}

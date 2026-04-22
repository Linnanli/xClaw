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
 * - 监听 chat-stream 并更新消息列表（含流式、思考链、工具状态）
 * - DLP 扫描集成（发送前拦截，Fail-Safe 设计）
 * - 线程管理（创建、切换、历史加载）
 * - 模型动态切换（通过 model_override metadata 传递给 Agent）
 */

import { type ReactNode, Component, useState, useCallback, useEffect, useRef, createContext, useContext } from 'react';
import {
  AssistantRuntimeProvider,
  type AttachmentAdapter,
  useExternalStoreRuntime,
  type ThreadMessageLike,
  type AppendMessage,
} from '@assistant-ui/react';
import type { CompleteAttachment, PendingAttachment, ThreadUserMessagePart } from '@assistant-ui/core';
import type { ReadonlyJSONObject } from 'assistant-stream/utils';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { threadApi, modelApi, type ModelConfigItem } from '@utils/tauri';
import { useDlpScan } from '@hooks/useDlpScan';
import type { SanitizationStats } from '@hooks/useDlpScan';
import { tracing } from '@utils/tracing';
import { ModelContext } from '@contexts/ModelContext';
import { isErrorResponse, friendlyErrorMessage } from '@utils/friendlyError';
import type { ChatCommand } from '../types/chatCommand';
import { EchoToolUI } from '../components/assistant-ui/tool-renderers/echo-renderer';
import { FileEditToolUI } from '../components/assistant-ui/tool-renderers/file-edit-renderer';
import { GrepResultToolUI } from '../components/assistant-ui/tool-renderers/grep-result-renderer';
import { ShellOutputToolUI } from '../components/assistant-ui/tool-renderers/shell-output-renderer';
import { GlobResultToolUI } from '../components/assistant-ui/tool-renderers/glob-result-renderer';
import { GitDiffToolUI } from '../components/assistant-ui/tool-renderers/git-diff-renderer';
import { LspResultToolUI } from '../components/assistant-ui/tool-renderers/lsp-result-renderer';
import { PlanModeToolUI } from '../components/assistant-ui/tool-renderers/plan-renderer';
import { SessionForkToolUI } from '../components/assistant-ui/tool-renderers/fork-renderer';
import { SubAgentToolUI } from '../components/assistant-ui/tool-renderers/sub-agent-renderer';
import {
  ApprovalContext,
  useApprovalState,
  type PendingApproval,
} from './contexts/ApprovalProvider';

// ============================================================================
// 类型定义
// ============================================================================

interface TauriMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  attachments?: NonNullable<ThreadMessageLike['attachments']>;
  toolCalls?: Array<{
    toolCallId?: string;
    toolName: string;
    args?: ReadonlyJSONObject;
    result?: string;
    isError?: boolean;
  }>;
  reasoning?: string;
  /** 实时工具执行步骤（流式过程中由 tool_started/tool_completed 事件驱动） */
  toolSteps?: ToolStep[];
  /** DLP 脱敏统计，仅用户消息有值 */
  dlpStats?: SanitizationStats;
  /** 当前用户消息命中的事件任务数量（用于 UI 提示） */
  routineTriggerCount?: number;
  /** 错误消息，assistant 消息出错时设置 */
  error?: string;
}

/** 工具执行步骤（流式过程中追踪 tool_started → tool_completed） */
export interface ToolStep {
  toolName: string;
  status: 'running' | 'complete' | 'error';
  error?: string;
  startedAt: number;
  completedAt?: number;
}

// ---------------------------------------------------------------------------
// Vercel AI Protocol stream events (from chat-stream channel)
// ---------------------------------------------------------------------------

/** Rust `VercelUIStream` events emitted on the `chat-stream` Tauri IPC channel. */
type VercelStreamEvent =
  | { type: 'tool-input-start'; toolCallId: string; toolName: string }
  | { type: 'tool-input-available'; toolCallId: string; toolName: string; input: unknown }
  | { type: 'tool-output-available'; toolCallId: string; output: unknown }
  | { type: 'tool-output-error'; toolCallId: string; errorText: string }
  | { type: 'text-delta'; id: string; delta: string }
  | { type: 'reasoning-delta'; id: string; delta: string }
  | { type: 'finish'; id: string }
  | { type: 'error'; errorText: string }
  | { type: 'data-custom'; id?: string; data: Record<string, unknown> };

type ToolCallEntry = NonNullable<TauriMessage['toolCalls']>[number];

/** 后端可能返回的所有 role 类型 */
type BackendRole = 'user' | 'assistant' | 'system' | 'tool' | 'tool_calls' | string;

/** 过滤：只保留 assistant-ui 支持的 role（user / assistant） */
function isDisplayableRole(role: BackendRole): role is 'user' | 'assistant' {
  return role === 'user' || role === 'assistant';
}

// ---------------------------------------------------------------------------
// 持久化事件解析（已抽到 runtime/shared/persistedEvents.ts）
// ---------------------------------------------------------------------------
import {
  parsePersistedToolCalls,
  parsePersistedUiEvent,
} from './shared/persistedEvents';
// 保持旧测试 `import { parsePersistedToolCalls } from '../TauriRuntimeProvider'` 可用
export { parsePersistedToolCalls };

function attachRoutineTriggeredToLastUser(messages: TauriMessage[], fired: number): void {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].role === 'user') {
      messages[i] = {
        ...messages[i],
        routineTriggerCount: (messages[i].routineTriggerCount ?? 0) + fired,
      };
      return;
    }
  }
}

function attachDlpStatsToLastUser(messages: TauriMessage[], stats: SanitizationStats): void {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].role === 'user') {
      messages[i] = { ...messages[i], dlpStats: stats };
      return;
    }
  }
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
  | { type: 'approval_needed'; thread_id: string; request_id: string; tool_name: string; description: string }
  | { type: 'status'; message: string; level: string }
  | { type: 'error'; message: string; code?: string }
  | { type: 'connection_status'; connected: boolean; message: string }
  | { type: 'job_status'; job_id: string; title: string; status: string }
  | { type: 'routine_triggered'; thread_id: string; fired: number };

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
  /** 外部下发的聊天指令（统一走对话发送链路） */
  outboundCommand?: ChatCommand | null;
  /** 指令执行完成回调（用于消费队列） */
  onOutboundCommandHandled?: (commandId: string) => void;
  /** 引擎就绪计数器，变化时重新加载历史消息 */
  engineReadyKey?: number;
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
// ---------------------------------------------------------------------------
// `ApprovalContext` / `PendingApproval` / `useApprovalState` 的定义已迁移到
// `runtime/contexts/ApprovalProvider.tsx`（新 Runtime 树独立挂载使用）；
// 本文件仍保留相同 context 实例引用，旧 Runtime 树通过内部的
// `ApprovalContext.Provider` 包裹子树，两侧 `useApprovalState` 行为一致。
// ============================================================================

export { useApprovalState };
export type { PendingApproval };

// ============================================================================
// ErrorBoundary: 兜底 assistant-ui 内部 fiber 管理错误
// See: https://github.com/assistant-ui/assistant-ui/issues/3123
// ============================================================================

interface RuntimeErrorBoundaryProps { children: ReactNode }
interface RuntimeErrorBoundaryState { hasError: boolean; errorKey: number }

class RuntimeErrorBoundary extends Component<RuntimeErrorBoundaryProps, RuntimeErrorBoundaryState> {
  state: RuntimeErrorBoundaryState = { hasError: false, errorKey: 0 };

  static getDerivedStateFromError(): Partial<RuntimeErrorBoundaryState> {
    return { hasError: true };
  }

  componentDidCatch(error: Error) {
    tracing.warn('AssistantRuntime recovered from internal error', { message: error.message });
  }

  componentDidUpdate(_: RuntimeErrorBoundaryProps, prevState: RuntimeErrorBoundaryState) {
    if (this.state.hasError && !prevState.hasError) {
      // Auto-recover on next tick by re-mounting the runtime subtree
      requestAnimationFrame(() => this.setState((s) => ({ hasError: false, errorKey: s.errorKey + 1 })));
    }
  }

  render() {
    if (this.state.hasError) return null;
    return <>{this.props.children}</>;
  }
}

// ============================================================================
// 消息转换：TauriMessage → assistant-ui ThreadMessageLike
// ============================================================================

function convertMessage(msg: TauriMessage): ThreadMessageLike {
  const parts: Array<
    { type: 'text'; text: string }
    | { type: 'reasoning'; text: string }
    | {
        type: 'tool-call';
        toolCallId?: string;
        toolName: string;
        args?: ReadonlyJSONObject;
        result?: string;
        isError?: boolean;
      }
  > = [];

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
  if (msg.toolCalls?.length) {
    parts.push(
      ...msg.toolCalls.map((toolCall, idx) => ({
        type: 'tool-call' as const,
        toolCallId: toolCall.toolCallId || `${msg.id}_tool_${idx}`,
        toolName: toolCall.toolName,
        args: toolCall.args,
        result: toolCall.result,
        isError: toolCall.isError,
      })),
    );
  }
  // toolSteps 不再映射为 tool-call parts（它们缺少 args/result，
  // ToolFallback 渲染效果差）。改为通过 metadata.custom.toolSteps 传递，
  // 由 AssistantMessage 用轻量 ToolStepIndicator 渲染。
  if (msg.content) parts.push({ type: 'text', text: msg.content });
  if (parts.length === 0) parts.push({ type: 'text', text: '' });

  const customMetadata = {
    ...(msg.dlpStats ? { dlpStats: msg.dlpStats } : {}),
    ...((msg.routineTriggerCount ?? 0) > 0 ? { routineTriggerCount: msg.routineTriggerCount } : {}),
    ...(msg.toolSteps?.length ? { toolSteps: msg.toolSteps } : {}),
  };
  const hasCustomMetadata = Object.keys(customMetadata).length > 0;

  const base = {
    role: msg.role,
    content: parts,
    id: msg.id,
    createdAt: new Date(msg.timestamp),
    ...(msg.attachments ? { attachments: msg.attachments } : {}),
    ...(hasCustomMetadata ? { metadata: { custom: customMetadata } } : {}),
  };

  if (msg.error) {
    return { ...base, status: { type: 'incomplete', reason: 'error', error: msg.error } };
  }

  return base;
}

function normalizeExternalMessages(next: readonly ThreadMessageLike[]): TauriMessage[] {
  return next.map((m, idx) => {
    let content = '';
    let reasoning: string | undefined;

    if (Array.isArray(m.content)) {
      const textParts: string[] = [];
      for (const part of m.content) {
        if (part.type === 'reasoning') {
          reasoning = typeof part.text === 'string' ? part.text : reasoning;
          continue;
        }
        if (part.type === 'text' && typeof part.text === 'string') {
          textParts.push(part.text);
        }
      }
      content = textParts.join('');
    }

    return {
      id: m.id ?? `external-${Date.now()}-${idx}`,
      role: m.role === 'user' ? 'user' : 'assistant',
      content,
      timestamp: m.createdAt instanceof Date ? m.createdAt.getTime() : Date.now(),
      attachments: m.attachments,
      reasoning,
    };
  });
}

type RuntimeAttachment = NonNullable<ThreadMessageLike['attachments']>[number];

interface SerializedAttachment {
  id: string;
  kind: 'audio' | 'image' | 'document';
  mime_type: string;
  filename: string | null;
  size_bytes: number | null;
  extracted_text: string | null;
  data: number[];
  duration_secs: number | null;
}

const COMPOSER_ATTACHMENT_ACCEPT = [
  'image/*',
  'audio/*',
  'application/pdf',
  'text/plain',
  'text/markdown',
  'text/csv',
  'text/html',
  'text/xml',
  'application/json',
  '.txt',
  '.md',
  '.csv',
  '.pdf',
].join(',');

function composerAttachmentType(file: File): RuntimeAttachment['type'] {
  const mimeType = file.type.toLowerCase();
  if (mimeType.startsWith('image/')) {
    return 'image';
  }
  if (mimeType.startsWith('audio/')) {
    return 'audio';
  }
  return 'document';
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = (error) => reject(error);
    reader.readAsDataURL(file);
  });
}

function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === 'string' ? reader.result : '');
    reader.onerror = (error) => reject(error);
    reader.readAsText(file);
  });
}

function fileCanInlineText(file: File): boolean {
  const mimeType = file.type.toLowerCase();
  return mimeType.startsWith('text/') || mimeType === 'application/json';
}

async function buildComposerAttachmentContent(file: File): Promise<ThreadUserMessagePart[]> {
  const type = composerAttachmentType(file);
  if (type === 'image') {
    return [{ type: 'image', image: await readFileAsDataUrl(file) }];
  }

  if (fileCanInlineText(file)) {
    const text = await readFileAsText(file);
    return text.trim() ? [{ type: 'text', text }] : [];
  }

  return [];
}

const composerAttachmentAdapter: AttachmentAdapter = {
  accept: COMPOSER_ATTACHMENT_ACCEPT,
  async add({ file }) {
    return {
      id: `${file.name}-${file.size}-${file.lastModified}`,
      type: composerAttachmentType(file),
      name: file.name,
      contentType: file.type || undefined,
      file,
      status: { type: 'requires-action', reason: 'composer-send' },
    };
  },
  async remove() {
    return;
  },
  async send(attachment: PendingAttachment): Promise<CompleteAttachment> {
    return {
      ...attachment,
      status: { type: 'complete' },
      content: await buildComposerAttachmentContent(attachment.file),
    };
  },
};

function extractTextContent(content: AppendMessage['content']): string {
  if (typeof content === 'string') {
    return content;
  }

  const textParts: string[] = [];
  for (const part of content) {
    if (part.type === 'text' && typeof part.text === 'string') {
      textParts.push(part.text);
    }
  }
  return textParts.join('');
}

function detectAttachmentKind(attachment: RuntimeAttachment): 'audio' | 'image' | 'document' {
  const contentType = attachment.contentType ?? attachment.file?.type ?? '';
  if (attachment.type === 'image' || contentType.startsWith('image/')) {
    return 'image';
  }
  if (contentType.startsWith('audio/')) {
    return 'audio';
  }
  return 'document';
}

function extractAttachmentText(attachment: RuntimeAttachment): string | null {
  if (!attachment.content) {
    return null;
  }

  const text = attachment.content
    .filter((part) => part.type === 'text')
    .map((part) => part.text)
    .join('\n')
    .trim();

  return text || null;
}

async function serializeAttachment(attachment: RuntimeAttachment): Promise<SerializedAttachment> {
  const file = attachment.file;
  if (!file) {
    throw new Error(`附件 ${attachment.name} 缺少本地文件内容，无法发送`);
  }

  const buffer = await file.arrayBuffer();
  return {
    id: attachment.id,
    kind: detectAttachmentKind(attachment),
    mime_type: attachment.contentType ?? file.type ?? 'application/octet-stream',
    filename: attachment.name || file.name || null,
    size_bytes: Number.isFinite(file.size) ? file.size : null,
    extracted_text: extractAttachmentText(attachment),
    data: Array.from(new Uint8Array(buffer)),
    duration_secs: null,
  };
}

async function serializeAttachments(
  attachments: readonly RuntimeAttachment[],
): Promise<SerializedAttachment[]> {
  if (attachments.length === 0) {
    return [];
  }

  return Promise.all(attachments.map((attachment) => serializeAttachment(attachment)));
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

/** 工具执行状态信息（由后端作为 thinking 事件发出，实际是工具进度，不应混入 reasoning） */
const TOOL_PROGRESS_PATTERNS = [
  /^Running /,
  /^Executing /,
  /^Thinking \(step \d+\)/,
  /^Reading /,
  /^Searching /,
  /^Writing /,
  /^Analyzing /,
];

function isSystemStatus(message: string): boolean {
  return SYSTEM_STATUS_PREFIXES.some((p) => message.startsWith(p));
}

/** 识别后端作为 thinking 发出的工具进度消息 */
function isToolProgress(message: string): boolean {
  return TOOL_PROGRESS_PATTERNS.some((p) => p.test(message));
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
  outboundCommand,
  onOutboundCommandHandled,
  engineReadyKey,
}: TauriRuntimeProviderProps) {
  // 若有 pendingMessage，表示有任务正在执行，初始 isRunning = true
  const [messages, setMessages] = useState<TauriMessage[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const threadIdRef = useRef(threadId);
  const modelIdRef = useRef(initialModelId);
  const selectedModelRef = useRef<ModelConfigItem | null>(null);
  const msgIdCounter = useRef(1);
  const bootstrapThreadIdRef = useRef<string | null>(null);
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

  // ── 处理 chat-stream（思考链 + 流式 + 工具 + 响应）──
  // 声明提前，供 history-loading effect 的 thread 切换清理使用
  const pendingAssistantId = useRef<string | null>(null);
  const thinkingBuffer = useRef<string>('');
  const toolStepsBuffer = useRef<ToolStep[]>([]);
  /** Real tool call data from chat-stream Vercel protocol events. */
  const toolCallsBuffer = useRef<ToolCallEntry[]>([]);
  // 用于检测 threadId 实际切换（区别于同一 thread 的历史重载）
  const prevThreadIdRef = useRef<string | null>(threadId);
  const handledCommandIdRef = useRef<string | null>(null);
  const activeTurnRef = useRef(false);

  const clearPendingAssistantState = useCallback((): void => {
    pendingAssistantId.current = null;
    thinkingBuffer.current = '';
    toolStepsBuffer.current = [];
    toolCallsBuffer.current = [];
    setIsRunning(false);
  }, []);

  const clearThreadRuntimeState = useCallback((): void => {
    clearPendingAssistantState();
    setPendingApprovals([]);
  }, [clearPendingAssistantState]);

  const finalizePreviousThread = useCallback(
    async (previousThreadId: string | null, shouldInterrupt: boolean): Promise<void> => {
      if (!previousThreadId) {
        return;
      }

      if (shouldInterrupt) {
        try {
          await threadApi.interruptThread(previousThreadId);
        } catch (error) {
          tracing.warn('Failed to interrupt previous thread before switch', {
            threadId: previousThreadId,
            error,
          });
        }
      }

      try {
        await threadApi.finalizeThread(previousThreadId);
      } catch (error) {
        tracing.warn('Failed to finalize previous thread', {
          threadId: previousThreadId,
          error,
        });
      }
    },
    [],
  );

  useEffect(() => {
    activeTurnRef.current = isRunning || pendingApprovals.length > 0 || pendingAssistantId.current !== null;
  }, [isRunning, pendingApprovals.length]);

  useEffect(() => {
    if (!threadId) {
      const previousThreadId = prevThreadIdRef.current;
      const shouldInterrupt = activeTurnRef.current;
      prevThreadIdRef.current = null;
      bootstrapThreadIdRef.current = null;
      clearThreadRuntimeState();
      setMessages([]);
      if (previousThreadId) {
        void finalizePreviousThread(previousThreadId, shouldInterrupt);
      }
      return;
    }

    // 新线程由当前 Provider 内部 onNew 创建时，先保留本地乐观消息/流式状态，
    // 避免 threadId 切换触发的历史重载把正在渲染的消息树缩短导致越界。
    if (bootstrapThreadIdRef.current === threadId) {
      bootstrapThreadIdRef.current = null;
      prevThreadIdRef.current = threadId;
      return;
    }

    // thread 切换时立即清理上一个 thread 的流式状态，防止残留内容出现在新 thread 中
    if (threadId !== prevThreadIdRef.current) {
      const previousThreadId = prevThreadIdRef.current;
      const shouldInterrupt = activeTurnRef.current;
      prevThreadIdRef.current = threadId;
      clearThreadRuntimeState();
      if (previousThreadId) {
        void finalizePreviousThread(previousThreadId, shouldInterrupt);
      }
    }

    let cancelled = false;

    const load = async () => {
      try {
        const history = await threadApi.getMessages(threadId);
        if (cancelled) return;

        const loaded: TauriMessage[] = [];
        const restoredApprovals = new Map<string, PendingApproval>();
        // Buffer tool_calls for merging into the next assistant message.
        // DB order: user → tool_calls → assistant. We merge tool_calls into
        // the following assistant message so streaming and history produce
        // the same single-message structure.
        let pendingToolCalls: TauriMessage['toolCalls'] | undefined;
        history.forEach((m) => {
          if (isDisplayableRole(m.role)) {
            const msg: TauriMessage = {
              id: m.id,
              role: m.role as 'user' | 'assistant',
              content: m.content,
              timestamp: new Date(m.created_at).getTime(),
              ...(m.attachments ? { attachments: m.attachments } : {}),
            };
            if (msg.role === 'assistant' && pendingToolCalls) {
              msg.toolCalls = pendingToolCalls;
              pendingToolCalls = undefined;
            }
            loaded.push(msg);
            return;
          }

          if (m.role === 'tool_calls') {
            const toolCalls = parsePersistedToolCalls(m.content);
            if (toolCalls.length === 0) {
              return;
            }
            pendingToolCalls = toolCalls;
            return;
          }

          if (m.role === 'system') {
            const event = parsePersistedUiEvent(m.content);
            if (!event) return;
            if (event.event === 'routine_triggered') {
              attachRoutineTriggeredToLastUser(loaded, event.fired);
              return;
            }
            if (event.event === 'dlp_redacted') {
              attachDlpStatsToLastUser(loaded, event.stats);
              return;
            }
            if (event.event === 'approval_needed') {
              restoredApprovals.set(event.request_id, {
                request_id: event.request_id,
                tool_name: event.tool_name,
                description: event.description,
              });
              return;
            }
            if (event.event === 'approval_resolved') {
              restoredApprovals.delete(event.request_id);
            }
          }
        });

        // Flush any trailing tool_calls that had no following assistant message
        if (pendingToolCalls) {
          const lastMsg = loaded[loaded.length - 1];
          if (lastMsg?.role === 'assistant') {
            lastMsg.toolCalls = pendingToolCalls;
          } else {
            loaded.push({
              id: `orphan-tool-calls-${Date.now()}`,
              role: 'assistant',
              content: '',
              timestamp: Date.now(),
              toolCalls: pendingToolCalls,
            });
          }
        }

        setMessages(loaded);
        setPendingApprovals(Array.from(restoredApprovals.values()));
      } catch (err) {
        if (!cancelled) tracing.error('Failed to load thread history', { error: err });
      }
    };

    load();
    return () => { cancelled = true; };
  }, [threadId, engineReadyKey, clearThreadRuntimeState, finalizePreviousThread]);

  // ── chat-stream 统一监听（Vercel AI protocol + DataCustom 混合事件）──
  useEffect(() => {
    let mounted = true;
    let unlisten: UnlistenFn | null = null;

    const setup = async () => {
      try {
        unlisten = await listen<VercelStreamEvent>('chat-stream', (event) => {
          if (!mounted) return;
          const payload = event.payload;

          switch (payload.type) {
            // ── Native Vercel tool events ──
            case 'tool-input-start':
              handleStreamEvent(payload);
              // Also create ToolStep for UI indicator (formerly from ChatEvent tool_started)
              handleChatEvent({ type: 'tool_started', name: payload.toolName });
              break;
            case 'tool-input-available':
            case 'tool-output-available':
              handleStreamEvent(payload);
              break;
            case 'tool-output-error':
              handleStreamEvent(payload);
              // Synthesize tool_completed for ToolStep update (Rust only sends ToolOutputError when tool_call_id exists)
              {
                const entry = toolCallsBuffer.current.find((tc) => tc.toolCallId === payload.toolCallId);
                if (entry?.toolName) {
                  handleChatEvent({ type: 'tool_completed', name: entry.toolName, success: false, error: payload.errorText });
                }
              }
              break;

            // ── Native Vercel text/reasoning ──
            case 'reasoning-delta':
              handleChatEvent({ type: 'thinking', message: payload.delta });
              break;
            case 'text-delta':
              handleChatEvent({ type: 'stream_chunk', content: payload.delta });
              break;

            // ── Vercel error ──
            case 'error':
              handleChatEvent({ type: 'error', message: payload.errorText });
              break;

            // ── DataCustom wrapper (contains ChatEvent-shaped data) ──
            case 'data-custom':
              handleChatEvent(payload.data as ChatEvent);
              break;

            // ── Finish (no-op — response finalization handled by data-custom response) ──
            case 'finish':
              break;
          }
        });
        await invoke('subscribe_chat_events').catch(() => {});
      } catch (err) {
        tracing.error('Failed to setup chat-stream listener', { error: err });
      }
    };

    setup();

    return () => {
      mounted = false;
      unlisten?.();
      invoke('unsubscribe_chat_events').catch(() => {});
    };
  }, []);

  /** Process Vercel AI protocol stream events from the `chat-stream` channel. */
  const handleStreamEvent = useCallback((event: VercelStreamEvent) => {
    switch (event.type) {
      case 'tool-input-start': {
        // Deduplicate: skip if this toolCallId already exists in buffer
        if (toolCallsBuffer.current.some((tc) => tc.toolCallId === event.toolCallId)) break;
        const entry: ToolCallEntry = {
          toolCallId: event.toolCallId,
          toolName: event.toolName,
        };
        toolCallsBuffer.current = [...toolCallsBuffer.current, entry];
        break;
      }
      case 'tool-input-available': {
        const input =
          event.input && typeof event.input === 'object' && !Array.isArray(event.input)
            ? (event.input as ReadonlyJSONObject)
            : undefined;
        toolCallsBuffer.current = toolCallsBuffer.current.map((tc) =>
          tc.toolCallId === event.toolCallId ? { ...tc, args: input } : tc,
        );
        break;
      }
      case 'tool-output-available': {
        const result =
          typeof event.output === 'string' ? event.output : JSON.stringify(event.output);
        toolCallsBuffer.current = toolCallsBuffer.current.map((tc) =>
          tc.toolCallId === event.toolCallId ? { ...tc, result } : tc,
        );
        break;
      }
      case 'tool-output-error': {
        toolCallsBuffer.current = toolCallsBuffer.current.map((tc) =>
          tc.toolCallId === event.toolCallId
            ? { ...tc, result: event.errorText, isError: true }
            : tc,
        );
        break;
      }
      default:
        return;
    }

    // Sync tool calls into the current pending assistant message
    const calls = toolCallsBuffer.current;
    if (pendingAssistantId.current && calls.length > 0) {
      const tempId = pendingAssistantId.current;
      setMessages((prev) =>
        prev.map((m) => (m.id === tempId ? { ...m, toolCalls: [...calls] } : m)),
      );
    }
  }, []);

  const handleChatEvent = useCallback((event: ChatEvent) => {
    tracing.debug('[chat-stream]', { type: event.type, detail: 'type' in event ? (event as Record<string, unknown>).name ?? (event as Record<string, unknown>).message ?? '' : '' });

    switch (event.type) {
      case 'thinking': {
        setIsRunning(true);
        if (isSystemStatus(event.message)) break;

        // 工具进度消息不混入 reasoning 文本（由 toolSteps UI 承载）
        if (isToolProgress(event.message)) {
          tracing.debug('[thinking-filtered] tool progress', { message: event.message });
          break;
        }

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
        tracing.debug('[tool_started]', { name: event.name, pendingId: pendingAssistantId.current, bufferLen: toolStepsBuffer.current.length });

        const newStep: ToolStep = { toolName: event.name, status: 'running', startedAt: Date.now() };
        toolStepsBuffer.current = [...toolStepsBuffer.current, newStep];
        const currentSteps = toolStepsBuffer.current;

        // 确保有 pending assistant 消息来承载工具步骤
        if (!pendingAssistantId.current) {
          const tempId = `tool-${Date.now()}`;
          pendingAssistantId.current = tempId;
          setMessages((prev) => [
            ...prev,
            {
              id: tempId,
              role: 'assistant',
              content: '',
              reasoning: thinkingBuffer.current || undefined,
              toolSteps: currentSteps,
              timestamp: Date.now(),
            },
          ]);
        } else {
          const tempId = pendingAssistantId.current;
          setMessages((prev) =>
            prev.map((m) => (m.id === tempId ? { ...m, toolSteps: currentSteps } : m)),
          );
        }
        break;
      }

      case 'tool_completed': {
        tracing.debug('[tool_completed]', { name: event.name, success: event.success, error: event.error, bufferLen: toolStepsBuffer.current.length });
        if (!event.success) {
          tracing.warn('Tool failed', { name: event.name, error: event.error });
        }

        // 更新 buffer 中对应 toolStep 状态
        toolStepsBuffer.current = toolStepsBuffer.current.map((step) =>
          step.toolName === event.name && step.status === 'running'
            ? {
                ...step,
                status: event.success ? 'complete' : 'error',
                error: event.error,
                completedAt: Date.now(),
              }
            : step,
        );
        const currentSteps = toolStepsBuffer.current;

        if (pendingAssistantId.current) {
          const tempId = pendingAssistantId.current;
          setMessages((prev) =>
            prev.map((m) => (m.id === tempId ? { ...m, toolSteps: currentSteps } : m)),
          );
        }
        break;
      }

      case 'approval_needed': {
        if (event.thread_id && event.thread_id !== threadIdRef.current) {
          console.warn('[approval_needed] DROPPED: thread_id mismatch', { eventThread: event.thread_id, currentThread: threadIdRef.current });
          break;
        }

        setIsRunning(false);
        console.warn('[approval_needed] Processing', { tool: event.tool_name, requestId: event.request_id, pendingAssistantId: pendingAssistantId.current });
        tracing.info('Tool approval needed', { tool: event.tool_name, requestId: event.request_id });
        setPendingApprovals((prev) => {
          // 去重：同一 request_id 不重复添加
          if (prev.some((a) => a.request_id === event.request_id)) return prev;
          return [...prev, { request_id: event.request_id, tool_name: event.tool_name, description: event.description }];
        });
        break;
      }

      case 'response': {
        // 忽略已切换 thread 后到达的旧 thread 响应（thread 切换时流式调用尚未结束）
        if (event.thread_id && event.thread_id !== threadIdRef.current) {
          tracing.debug('Ignoring response for stale thread', { event_thread: event.thread_id, current: threadIdRef.current });
          break;
        }
        if (pendingAssistantId.current) {
          const tempId = pendingAssistantId.current;
          tracing.debug('[response] finalizing pending message', { tempId, messageId: event.message_id, hasToolSteps: toolStepsBuffer.current.length });
          // 保留 toolSteps — 它们是本轮工具执行的可视化历史，不应在 response 时清除。
          // 优先使用 chat-stream 传来的真实工具调用数据（含 args/result/toolCallId），
          // 降级使用 toolSteps 的合成转换（仅有工具名和状态，用于历史消息恢复）。
          const bufferedCalls = toolCallsBuffer.current;
          const steps = toolStepsBuffer.current;
          const toolCalls =
            bufferedCalls.length > 0
              ? [...bufferedCalls]
              : steps.length > 0
                ? steps.map((step, i) => ({
                    toolCallId: `step_${i}`,
                    toolName: step.toolName,
                    isError: step.status === 'error',
                    ...(step.error ? { result: step.error } : {}),
                  }))
                : undefined;
          setMessages((prev) =>
            prev.map((m) => (m.id === tempId
              ? { ...m, id: event.message_id, content: event.content, ...(toolCalls ? { toolCalls } : {}) }
              : m)),
          );
        } else {
          setMessages((prev) => [
            ...prev,
            { id: event.message_id, role: 'assistant' as const, content: event.content, timestamp: Date.now() },
          ]);
        }
        clearPendingAssistantState();
        break;
      }

      case 'error':
        clearPendingAssistantState();
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

      case 'job_status':
        // 由 useRunningJobs 直接监听处理，此处无需额外操作
        break;

      case 'routine_triggered': {
        if (event.thread_id && event.thread_id !== threadIdRef.current) {
          break;
        }
        setMessages((prev) => {
          const fired = Number.isFinite(event.fired) && event.fired > 0 ? event.fired : 1;
          const next = [...prev];
          attachRoutineTriggeredToLastUser(next, fired);
          return next;
        });
        break;
      }
    }
  }, [loadModels, clearPendingAssistantState, sendApprovalDecision]);

  // ── 统一发送文本入口（Composer / 外部指令共用）──
  const sendUserText = useCallback(
    async (rawContent: string, attachments: readonly RuntimeAttachment[] = []) => {
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
          bootstrapThreadIdRef.current = tid;
          threadIdRef.current = tid;
          onThreadCreated?.(tid);
        } catch (err) {
          tracing.error('Failed to create thread', { error: err });
          return;
        }
      }

      let serializedAttachments: SerializedAttachment[] = [];
      try {
        serializedAttachments = await serializeAttachments(attachments);
      } catch (error) {
        tracing.error('Failed to serialize attachments', { error });
        setMessages((prev) => [
          ...prev,
          {
            id: `error-${Date.now()}`,
            role: 'assistant' as const,
            content: '',
            timestamp: Date.now(),
            error: error instanceof Error ? error.message : '附件读取失败，无法发送消息',
          },
        ]);
        return;
      }

      // 3. 乐观更新：立即添加用户消息（含 DLP 脱敏统计）
      const userMsg: TauriMessage = {
        id: `msg-${msgIdCounter.current++}`,
        role: 'user',
        content,
        timestamp: Date.now(),
        ...(attachments.length > 0 ? { attachments } : {}),
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
          dlpStats: dlpResult?.had_sensitive_data ? dlpResult.sanitization_stats : null,
          attachments: serializedAttachments.length > 0 ? serializedAttachments : null,
        });
      } catch (err) {
        tracing.error('Failed to send message', { error: err });
        setIsRunning(false);
        const errMsg = typeof err === 'string' ? err : (err as Error)?.message ?? '发送失败，请检查模型配置';
        setMessages((prev) => [
          ...prev,
          {
            id: `error-${Date.now()}`,
            role: 'assistant' as const,
            content: '',
            timestamp: Date.now(),
            error: errMsg,
          },
        ]);
      }
    },
    [scanUserInput, onThreadCreated, onBlockedCb, onRedactedCb],
  );

  // ── 发送新消息（assistant-ui onNew 回调）──
  const onNew = useCallback(
    async (appendMessage: AppendMessage) => {
      const textContent = extractTextContent(appendMessage.content);
      const attachments = appendMessage.attachments ?? [];
      if (!textContent.trim() && attachments.length === 0) {
        return;
      }
      await sendUserText(textContent, attachments);
    },
    [sendUserText],
  );

  // ── 处理外部聊天指令（统一走 sendUserText）──
  useEffect(() => {
    if (!outboundCommand) return;
    if (handledCommandIdRef.current === outboundCommand.id) return;
    handledCommandIdRef.current = outboundCommand.id;

    if (outboundCommand.kind === 'send_text') {
      void sendUserText(outboundCommand.text)
        .finally(() => {
          onOutboundCommandHandled?.(outboundCommand.id);
        });
    }
  }, [outboundCommand, sendUserText, onOutboundCommandHandled]);

  const onCancel = useCallback(async () => {
    const tid = threadIdRef.current;
    if (!tid) {
      clearPendingAssistantState();
      return;
    }

    try {
      await threadApi.interruptThread(tid);
    } catch (error) {
      tracing.warn('Failed to interrupt active thread', { threadId: tid, error });
    } finally {
      clearPendingAssistantState();
    }
  }, [clearPendingAssistantState]);

  // ── 构建 ExternalStoreRuntime ──
  const runtime = useExternalStoreRuntime({
    messages,
    isRunning,
    convertMessage,
    onNew,
    onCancel,
    adapters: {
      attachments: composerAttachmentAdapter,
    },
    setMessages: (newMessages) => {
      setMessages(normalizeExternalMessages(newMessages));
    },
  });

  return (
    <RuntimeErrorBoundary>
    <AssistantRuntimeProvider runtime={runtime}>
      <EchoToolUI />
      <FileEditToolUI />
      <GrepResultToolUI />
      <ShellOutputToolUI />
      <GlobResultToolUI />
      <GitDiffToolUI />
      <LspResultToolUI />
      <PlanModeToolUI />
      <SessionForkToolUI />
      <SubAgentToolUI />
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
    </RuntimeErrorBoundary>
  );
}

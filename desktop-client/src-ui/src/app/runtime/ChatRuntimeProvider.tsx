/**
 * ChatRuntimeProvider — 新一代 LLM 运行时（Phase 1）
 *
 * 架构：
 *   useChatRuntime（@assistant-ui/react-ai-sdk）
 *     └─ TauriChatTransport → invoke('send_chat_message') + listen('chat-stream')
 *         └─ 后端 `VercelUIStream` 已对齐 AI SDK v5 `UIMessageChunk`
 *   DLP 拦截在 Composer 层（thread.tsx 的 ComposerAction）
 *
 * 相比旧的 HTTP 版 ChatRuntimeProvider（已废弃），本 Provider：
 * - 直接走 Tauri IPC，不再绕 HTTP
 * - 事件流由后端 `tauri_channel.rs` 推送，无需 streamText
 * - 前端状态管理 100% 交由 assistant-ui SDK（branch / 流式 / 工具 UI 自动处理）
 */

import { type ReactNode, useState, useCallback, createContext, useContext, useRef } from 'react';
import { AssistantRuntimeProvider } from '@assistant-ui/react';
import { useChatRuntime } from '@assistant-ui/react-ai-sdk';
import type { SanitizationStats } from '../hooks/useDlpScan';
import { TauriChatTransport } from './TauriChatTransport';

/**
 * 历史初始消息的结构型，与 `ai@5 UIMessage` 的核心字段兼容（id/role/parts）。
 * 由于 `@ai-sdk/react@1.2` 未导出 `UIMessage` 类型，这里用本地类型避免深入 node_modules 嵌套路径。
 * 传入 `useChatRuntime` 时会通过 `unknown` 中转 cast，运行时由 `AISDKMessageConverter` 正常识别。
 */
export type InitialChatMessage = {
  id: string;
  role: 'user' | 'assistant' | 'system';
  parts: ReadonlyArray<Record<string, unknown>>;
  metadata?: unknown;
};
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
  ApprovalToolUI,
  ApprovalThreadIdProvider,
} from '../components/tool-ui/approval-tool-ui';

// ============================================================================
// DLP Context（与旧 Provider 兼容）
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
// Provider
// ============================================================================

interface ChatRuntimeProviderProps {
  children: ReactNode;
  /** 当前会话 id；未选中会话时可为 null，内部会回落到临时 id */
  threadId?: string | null;
  /** 当前选中模型 id；transport 会在每次发送时读最新值 */
  modelId?: string | null;
  /** 自定义 provider 的 base url / api key（自定义模型场景） */
  apiBaseUrl?: string | null;
  apiKey?: string | null;
  /**
   * 初始历史消息（AI SDK v5 UIMessage 结构）。
   * 由外层 `ThreadHistoryLoader` 通过 `threadApi.getMessages` + `mapTauriMessagesToUIMessages` 预取后注入。
   * `useChatRuntime` 只在挂载时读取一次；要切换 thread 必须让本 Provider 随 threadId 重建（外层通过 `key={threadId}` 实现）。
   */
  initialMessages?: InitialChatMessage[];
}

export function ChatRuntimeProvider({
  children,
  threadId,
  modelId,
  apiBaseUrl,
  apiKey,
  initialMessages,
}: ChatRuntimeProviderProps) {
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

  // 用 ref 保持 transport 闭包读最新的 modelId / threadId 等
  const threadIdRef = useRef(threadId ?? null);
  threadIdRef.current = threadId ?? null;
  const modelIdRef = useRef(modelId ?? null);
  modelIdRef.current = modelId ?? null;
  const apiBaseUrlRef = useRef(apiBaseUrl ?? null);
  apiBaseUrlRef.current = apiBaseUrl ?? null;
  const apiKeyRef = useRef(apiKey ?? null);
  apiKeyRef.current = apiKey ?? null;

  // Transport 只创建一次
  const transportRef = useRef<TauriChatTransport | null>(null);
  if (!transportRef.current) {
    transportRef.current = new TauriChatTransport({
      modelId: () => modelIdRef.current,
      apiBaseUrl: () => apiBaseUrlRef.current,
      apiKey: () => apiKeyRef.current,
      currentThreadId: () => threadIdRef.current,
    });
  }

  // `useChatRuntime` 泛型要求 messages 为 `UIMessage<...>[]`，而我们从持久化映射出来的
  // `InitialChatMessage[]` 结构等价但 TS 类型不重合；统一用 `any` 走运行时 duck-typing。
  const runtimeOptions: Record<string, unknown> = { transport: transportRef.current };
  if (threadId) runtimeOptions.id = threadId;
  if (initialMessages && initialMessages.length > 0) runtimeOptions.messages = initialMessages;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const runtime = useChatRuntime(runtimeOptions as any);

  return (
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
      <ApprovalToolUI />
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
        <ApprovalThreadIdProvider threadId={threadId ?? null}>
          {children}
        </ApprovalThreadIdProvider>
      </DlpContext.Provider>
    </AssistantRuntimeProvider>
  );
}

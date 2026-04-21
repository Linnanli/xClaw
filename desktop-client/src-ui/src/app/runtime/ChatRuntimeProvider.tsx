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
}

export function ChatRuntimeProvider({
  children,
  threadId,
  modelId,
  apiBaseUrl,
  apiKey,
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
    });
  }

  const runtime = useChatRuntime({ transport: transportRef.current });

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

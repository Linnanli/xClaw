/**
 * ChatRuntimeProvider — 基于 @assistant-ui/react-ai-sdk 的 LLM 运行时
 *
 * 架构：
 *   useChatRuntime（@assistant-ui/react-ai-sdk）
 *     └─ AssistantChatTransport → Admin Backend /api/chat/completions
 *         └─ 后端返回 Vercel AI SDK Data Stream 格式
 *   DLP 拦截在 Composer 层（thread.tsx 的 ComposerAction）
 */

import { type ReactNode, useState, useCallback, createContext, useContext, useRef } from 'react';
import { AssistantRuntimeProvider } from '@assistant-ui/react';
import { useChatRuntime, AssistantChatTransport } from '@assistant-ui/react-ai-sdk';
import type { SanitizationStats } from '../hooks/useDlpScan';

// ============================================================================
// DLP Context
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
  apiUrl?: string;
  modelId?: string;
}

export function ChatRuntimeProvider({
  children,
  apiUrl = 'http://localhost:3000/api/chat/completions',
  modelId = 'deepseek-chat',
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

  // 用 ref 保持 transport 闭包能读到最新的 modelId
  const modelIdRef = useRef(modelId);
  modelIdRef.current = modelId;

  // transport 只创建一次，通过 ref 动态读取最新 modelId
  const transportRef = useRef<AssistantChatTransport<never> | null>(null);
  if (!transportRef.current) {
    transportRef.current = new AssistantChatTransport({
      api: apiUrl,
      body: () => ({ model: modelIdRef.current }),
      prepareSendMessagesRequest: async (options) => {
        console.log('[ChatTransport] prepareSendMessagesRequest called');
        console.log('[ChatTransport] messages[0]:', JSON.stringify(options.messages?.[0]).substring(0, 200));
        console.log('[ChatTransport] body keys:', Object.keys(options.body ?? {}));
        return {
          body: {
            ...options.body,
            model: modelIdRef.current,
            messages: options.messages,
          },
        };
      },
    });
  }

  const runtime = useChatRuntime({ transport: transportRef.current });

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

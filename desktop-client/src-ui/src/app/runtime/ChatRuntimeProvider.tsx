/**
 * ChatRuntimeProvider — 双通道 LLM 运行时
 *
 * 架构：
 *   在线模式：useLocalRuntime → fetch Admin Backend /api/chat/completions → OpenAI SSE
 *   离线模式：useLocalRuntime → fetch Ollama localhost:11434 → OpenAI SSE
 *
 * 使用 useLocalRuntime + ChatModelAdapter，直接解析 OpenAI 兼容的 SSE 格式。
 * 不依赖 useDataStreamRuntime（需要 assistant-stream 协议）。
 */

import { type ReactNode, useState, useCallback, useEffect, createContext, useContext } from 'react';
import {
  AssistantRuntimeProvider,
  useLocalRuntime,
  type ChatModelAdapter,
  type ChatModelRunOptions,
} from '@assistant-ui/react';
import { ExportedMessageRepository } from '@assistant-ui/core/runtime/utils/message-repository';
import { threadApi } from '../utils/tauri';

// ============================================================================
// DLP Context
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
// OpenAI SSE 解析
// ============================================================================

/** 解析 OpenAI 兼容的 SSE 流，逐步 yield 文本 token */
async function* parseOpenAISse(
  response: Response,
): AsyncGenerator<string> {
  const reader = response.body?.getReader();
  if (!reader) return;

  const decoder = new TextDecoder();
  let buffer = '';

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() ?? '';

      for (const line of lines) {
        if (!line.startsWith('data: ')) continue;
        const data = line.slice(6).trim();
        if (data === '[DONE]') return;

        try {
          const parsed = JSON.parse(data);
          const content = parsed?.choices?.[0]?.delta?.content;
          if (content) yield content;
        } catch {
          // 忽略解析错误
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}

// ============================================================================
// ChatModelAdapter 工厂
// ============================================================================

function createChatModelAdapter(apiUrl: string, modelId: string): ChatModelAdapter {
  return {
    async *run({ messages, abortSignal }: ChatModelRunOptions) {
      // 转换消息格式
      const openaiMessages = messages.map((m) => ({
        role: m.role,
        content: m.content
          .filter((p) => p.type === 'text')
          .map((p) => (p as { type: 'text'; text: string }).text)
          .join('\n'),
      }));

      const response = await fetch(apiUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          model: modelId,
          messages: openaiMessages,
          stream: true,
        }),
        signal: abortSignal,
      });

      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: response.statusText }));
        throw new Error(error?.details || error?.error || `HTTP ${response.status}`);
      }

      let text = '';
      for await (const chunk of parseOpenAISse(response)) {
        text += chunk;
        yield { content: [{ type: 'text' as const, text }] };
      }
    },
  };
}

// ============================================================================
// Provider
// ============================================================================

interface ChatRuntimeProviderProps {
  children: ReactNode;
  apiUrl?: string;
  modelId?: string;
  threadId?: string;
}

export function ChatRuntimeProvider({
  children,
  apiUrl = 'http://localhost:3000/api/chat/completions',
  modelId = 'deepseek-chat',
  threadId,
}: ChatRuntimeProviderProps) {
  const [dlpBlocked, setDlpBlocked] = useState(false);
  const [dlpBlockReason, setDlpBlockReason] = useState<string | null>(null);
  const clearDlpBlock = useCallback(() => {
    setDlpBlocked(false);
    setDlpBlockReason(null);
  }, []);

  const adapter = createChatModelAdapter(apiUrl, modelId);
  const runtime = useLocalRuntime(adapter);

  // 加载历史消息（切换线程时触发）
  useEffect(() => {
    if (!threadId) return;
    let cancelled = false;

    threadApi.getMessages(threadId).then((messages) => {
      if (cancelled || messages.length === 0) return;
      try {
        const messageLikes = messages.map((m) => ({
          role: m.role as 'user' | 'assistant',
          content: [{ type: 'text' as const, text: m.content }],
        }));
        runtime.thread.import(ExportedMessageRepository.fromArray(messageLikes));
      } catch (err) {
        console.warn('Failed to import thread history:', err);
      }
    }).catch((err) => {
      if (!cancelled) console.warn('Failed to load thread messages:', err);
    });

    return () => { cancelled = true; };
  }, [threadId, runtime]);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <DlpContext.Provider
        value={{ blocked: dlpBlocked, blockReason: dlpBlockReason, clearBlock: clearDlpBlock }}
      >
        {children}
      </DlpContext.Provider>
    </AssistantRuntimeProvider>
  );
}

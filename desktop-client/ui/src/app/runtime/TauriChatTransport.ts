/**
 * TauriChatTransport — 把 Tauri IPC (invoke + listen) 包装成 AI SDK `ChatTransport`。
 *
 * 作为 `useChatRuntime({ transport })` 的底层传输，让 assistant-ui 的
 * `useChatRuntime` 能直接对接 Rust 后端的 `chat-stream` 事件流。
 *
 * 协议：后端 `VercelUIStream` 枚举（`desktop-client/src/vercel_ui_protocol.rs`）
 * 的 `type` 字段（kebab-case）与 AI SDK v5 `UIMessageChunk.type` 1:1 对齐。
 *
 * 流程：
 * ```
 * useChatRuntime
 *   └─ sendMessages(options)
 *       ├─ listen('chat-stream') → enqueue UIMessageChunk
 *       ├─ invoke('send_chat_message', { threadId, content, ... })
 *       └─ 返回 ReadableStream<UIMessageChunk>
 *                               │
 *                               └─ 'finish' 事件到达 → close + unlisten
 * ```
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ChatTransport, UIMessage, UIMessageChunk, ChatRequestOptions } from 'ai';

// ---------------------------------------------------------------------------
// 后端事件 → UIMessageChunk 映射
// ---------------------------------------------------------------------------

/**
 * 后端 `VercelUIStream` 反序列化类型。
 *
 * 与 AI SDK 的 `UIMessageChunk` 保持字段一致，只列出我们当前真正发出的变体。
 * 见 `desktop-client/src/vercel_ui_protocol.rs` 的 `VercelUIStream` 枚举。
 */
type BackendStreamEvent =
  | { type: 'text-start'; id: string }
  | { type: 'text-delta'; id: string; delta: string }
  | { type: 'text-end'; id: string }
  | { type: 'reasoning-start'; id: string }
  | { type: 'reasoning-delta'; id: string; delta: string }
  | { type: 'reasoning-end'; id: string }
  | { type: 'tool-input-start'; toolCallId: string; toolName: string; providerExecuted?: boolean }
  | { type: 'tool-input-delta'; toolCallId: string; inputTextDelta: string }
  | {
      type: 'tool-input-available';
      toolCallId: string;
      toolName: string;
      input: unknown;
      providerExecuted?: boolean;
    }
  | {
      type: 'tool-input-error';
      toolCallId: string;
      toolName: string;
      input: unknown;
      errorText: string;
    }
  | { type: 'tool-output-available'; toolCallId: string; output: unknown; providerExecuted?: boolean }
  | { type: 'tool-output-error'; toolCallId: string; errorText: string; providerExecuted?: boolean }
  | { type: 'error'; errorText: string }
  | { type: 'finish'; id?: string }
  | { type: 'data-custom'; id?: string; data: Record<string, unknown> };

/**
 * 把后端事件转成 AI SDK `UIMessageChunk`。
 *
 * 多数事件是字段级 identity；`data-custom` 需要按 data.type 动态映射成
 * `data-${NAME}` 形式（AI SDK v5 约定）。
 */
function toUIMessageChunk(ev: BackendStreamEvent): UIMessageChunk | null {
  switch (ev.type) {
    case 'text-start':
    case 'text-delta':
    case 'text-end':
    case 'reasoning-start':
    case 'reasoning-delta':
    case 'reasoning-end':
    case 'tool-input-start':
    case 'tool-input-delta':
    case 'tool-input-available':
    case 'tool-input-error':
    case 'tool-output-available':
    case 'tool-output-error':
    case 'error':
      // 字段完全对齐，直接透传
      return ev as unknown as UIMessageChunk;

    case 'finish':
      // AI SDK `finish` chunk 不含 id 字段
      return { type: 'finish' } as UIMessageChunk;

    case 'data-custom': {
      // 后端把所有非标准事件塞进 `data-custom`；AI SDK 用 `data-${NAME}` 区分。
      // 我们约定 data.kind 是子类型名（如 'approval_needed'、'response'、'job_status'）。
      const kind =
        typeof ev.data?.kind === 'string'
          ? (ev.data.kind as string)
          : typeof ev.data?.type === 'string'
          ? (ev.data.type as string)
          : 'custom';
      return {
        type: `data-${kind}` as `data-${string}`,
        id: ev.id,
        data: ev.data,
      } as UIMessageChunk;
    }

    default:
      return null;
  }
}

// ---------------------------------------------------------------------------
// Transport 实现
// ---------------------------------------------------------------------------

/** 从 AI SDK `UIMessage[]` 提取最后一条 user 消息的纯文本内容。 */
function extractUserContent(messages: readonly UIMessage[]): string {
  for (let i = messages.length - 1; i >= 0; i--) {
    const m = messages[i];
    if (m.role !== 'user') continue;
    const parts = m.parts ?? [];
    return parts
      .filter((p): p is { type: 'text'; text: string } => p.type === 'text' && typeof (p as { text?: unknown }).text === 'string')
      .map((p) => p.text)
      .join('');
  }
  return '';
}

export interface TauriChatTransportOptions {
  /** 动态获取当前选中的模型 id（每次发送时读最新值）。 */
  modelId?: () => string | null;
  /** 动态获取 provider api_base_url（自定义模型时可能不同）。 */
  apiBaseUrl?: () => string | null;
  /** 动态获取 api_key（自定义模型）。 */
  apiKey?: () => string | null;
  /**
   * 动态获取 transport 当前绑定的 threadId。
   *
   * 若后端 `chat-stream` envelope 顶层包含 `threadId` 字段（线程专属事件），
   * listener 会比对此 getter 返回值，不匹配则丢弃该 chunk，避免 thread 切换后
   * 旧流污染新线程 state。系统级事件（envelope 无 threadId）总是透传。
   *
   * 未提供时退化为全透传（不做过滤）。
   */
  currentThreadId?: () => string | null;
}

/**
 * 从后端 `chat-stream` envelope payload 中读取 `threadId`。
 * 约定：系统级广播不携带此字段；线程专属事件顶层注入该字段。
 *
 * 导出供测试使用。
 */
export function extractEnvelopeThreadId(payload: unknown): string | null {
  if (!payload || typeof payload !== 'object') return null;
  const value = (payload as Record<string, unknown>).threadId;
  return typeof value === 'string' && value.length > 0 ? value : null;
}

/**
 * `ChatTransport` 实现：通过 Tauri `invoke` + `listen` 做双向通信。
 *
 * 单窗口单活跃会话假设：同一时刻只有一个 thread 在流式输出。
 * 多线程并发场景下 `chat-stream` 需带 thread_id 标签，当前暂不处理。
 */
export class TauriChatTransport<UI_MESSAGE extends UIMessage = UIMessage>
  implements ChatTransport<UI_MESSAGE>
{
  private readonly modelIdFn: () => string | null;
  private readonly apiBaseUrlFn: () => string | null;
  private readonly apiKeyFn: () => string | null;
  private readonly currentThreadIdFn: () => string | null;

  constructor(options: TauriChatTransportOptions = {}) {
    this.modelIdFn = options.modelId ?? (() => null);
    this.apiBaseUrlFn = options.apiBaseUrl ?? (() => null);
    this.apiKeyFn = options.apiKey ?? (() => null);
    this.currentThreadIdFn = options.currentThreadId ?? (() => null);
  }

  async sendMessages(
    options: {
      chatId: string;
      messages: UI_MESSAGE[];
      abortSignal: AbortSignal | undefined;
    } & ChatRequestOptions,
  ): Promise<ReadableStream<UIMessageChunk>> {
    const { chatId, messages, abortSignal } = options;
    const content = extractUserContent(messages);

    let unlisten: UnlistenFn | null = null;
    let abortHandler: (() => void) | null = null;

    const stream = new ReadableStream<UIMessageChunk>({
      start: async (controller) => {
        // 1. 先订阅 chat-stream（必须在 invoke 之前，避免竞态丢事件）
        try {
          unlisten = await listen<BackendStreamEvent>('chat-stream', (event) => {
            // Envelope 过滤：如果事件携带 threadId 且与当前 transport 绑定的
            // threadId 不匹配，直接丢弃（避免 thread 切换时旧流污染新线程 state）。
            // 系统级广播（envelope 无 threadId）总是透传。
            const eventThreadId = extractEnvelopeThreadId(event.payload);
            if (eventThreadId !== null) {
              const currentThreadId = this.currentThreadIdFn();
              if (currentThreadId && currentThreadId !== eventThreadId) {
                return;
              }
            }

            const chunk = toUIMessageChunk(event.payload);
            if (!chunk) return;
            try {
              controller.enqueue(chunk);
            } catch {
              // stream 已关闭
              return;
            }
            // finish → 关闭 stream，退订
            if (chunk.type === 'finish') {
              cleanup();
              try {
                controller.close();
              } catch {
                /* already closed */
              }
            }
          });
        } catch (err) {
          controller.enqueue({
            type: 'error',
            errorText: err instanceof Error ? err.message : String(err),
          } as UIMessageChunk);
          controller.close();
          return;
        }

        // 2. 接入 abort
        if (abortSignal) {
          if (abortSignal.aborted) {
            cleanup();
            controller.close();
            return;
          }
          abortHandler = () => {
            cleanup();
            try {
              controller.close();
            } catch {
              /* already closed */
            }
          };
          abortSignal.addEventListener('abort', abortHandler, { once: true });
        }

        // 3. invoke send_chat_message（异步启动 Agent，不等流完结）
        try {
          await invoke('send_chat_message', {
            threadId: chatId,
            content,
            modelId: this.modelIdFn(),
            apiBaseUrl: this.apiBaseUrlFn(),
            apiKey: this.apiKeyFn(),
            dlpStats: null,
            attachments: null,
          });
        } catch (err) {
          controller.enqueue({
            type: 'error',
            errorText: err instanceof Error ? err.message : String(err),
          } as UIMessageChunk);
          cleanup();
          try {
            controller.close();
          } catch {
            /* already closed */
          }
        }
      },
      cancel: () => {
        cleanup();
      },
    });

    function cleanup() {
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
      if (abortSignal && abortHandler) {
        abortSignal.removeEventListener('abort', abortHandler);
        abortHandler = null;
      }
    }

    return stream;
  }

  async reconnectToStream(): Promise<ReadableStream<UIMessageChunk> | null> {
    // 当前后端不支持断线续传；返回 null 让 SDK 走常规重发路径
    return null;
  }
}

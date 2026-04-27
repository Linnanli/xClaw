/**
 * historyLoader — 将后端持久化的 Tauri `Message[]` 映射为 AI SDK v5
 * 形态的 `UIMessage[]`，用于 `useChatRuntime` 的 `ThreadHistoryAdapter`。
 *
 * Phase 1.3.a：纯函数，不依赖 React / assistant-ui / ai 运行时；
 * 字段取自后端 schema + `persistedEvents.ts`，行为对齐 TauriRuntimeProvider
 * 的历史加载逻辑（见 `TauriRuntimeProvider.tsx` 的 `load()` 闭包）。
 *
 * 为避免把 `ai@5` 内部路径作为公共 import 硬编码，这里本地声明一套
 * 结构等价于 AI SDK v5 `UIMessage` 的类型。1.3.b 接入适配器时会在
 * 类型边界上再做一次显式 cast。
 */

import type { ReadonlyJSONObject } from 'assistant-stream/utils';
import type { SanitizationStats } from '@hooks/useDlpScan';
import type { Message as TauriMessage } from '../../utils/tauri';
import {
  parsePersistedToolCalls,
  parsePersistedUiEvent,
  type ParsedToolCall,
} from './persistedEvents';

// ---------------------------------------------------------------------------
// 本地类型（与 ai@5 UIMessage 结构对齐）
// ---------------------------------------------------------------------------

export type LoadedTextPart = {
  type: 'text';
  text: string;
  state?: 'streaming' | 'done';
};

export type LoadedDynamicToolPart = {
  type: 'dynamic-tool';
  toolName: string;
  toolCallId: string;
} & (
  | { state: 'input-available'; input: ReadonlyJSONObject | Record<string, never> }
  | {
      state: 'output-available';
      input: ReadonlyJSONObject | Record<string, never>;
      output: unknown;
    }
  | {
      state: 'output-error';
      input: ReadonlyJSONObject | Record<string, never>;
      errorText: string;
    }
);

export type LoadedUIMessagePart = LoadedTextPart | LoadedDynamicToolPart;

/**
 * 与 ai@5 `UIMessage` 结构等价的本地类型。
 * `metadata` 承载 IronClaw 专属的历史补充信息（attachments / routine / dlp），
 * 由上层组件按需读取。
 */
export interface LoadedUIMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  parts: LoadedUIMessagePart[];
  metadata?: LoadedUIMessageMetadata;
}

export interface LoadedUIMessageMetadata {
  createdAt?: string;
  attachments?: TauriMessage['attachments'];
  routineFired?: number;
  dlpStats?: SanitizationStats;
}

export interface RestoredApproval {
  request_id: string;
  tool_name: string;
  description: string;
}

export interface HistoryLoadResult {
  messages: LoadedUIMessage[];
  restoredApprovals: RestoredApproval[];
}

// ---------------------------------------------------------------------------
// 主函数
// ---------------------------------------------------------------------------

/**
 * 将后端 `threadApi.getMessages()` 的结果转为 `LoadedUIMessage[]`。
 *
 * 合并规则（与 TauriRuntimeProvider 保持一致）：
 * - role=user / assistant → 展示消息（parts 含一个 text part）
 * - role=tool_calls → 解析为 ParsedToolCall[]，缓冲到下一条 assistant 的 parts 前面；
 *   若末尾没有后续 assistant，则合成一条空 assistant 承载 tool parts。
 * - role=system → 解析 ui_event，分别归并到上一条 user 的 metadata
 *   （routine_triggered / dlp_redacted）或 restoredApprovals 队列
 *   （approval_needed / approval_resolved）。
 * - 解析失败时静默跳过该条，不抛错。
 */
export function mapTauriMessagesToUIMessages(
  history: readonly TauriMessage[],
): HistoryLoadResult {
  const messages: LoadedUIMessage[] = [];
  const restoredApprovals = new Map<string, RestoredApproval>();
  let pendingToolCalls: ParsedToolCall[] | undefined;

  for (const raw of history) {
    if (raw.role === 'user' || raw.role === 'assistant') {
      messages.push(buildDisplayMessage(raw, pendingToolCalls));
      pendingToolCalls = undefined;
      continue;
    }

    if (raw.role === 'tool_calls') {
      const calls = parsePersistedToolCalls(raw.content);
      if (calls.length > 0) pendingToolCalls = calls;
      continue;
    }

    if (raw.role === 'system') {
      applySystemEvent(raw.content, messages, restoredApprovals);
    }
    // 其它未知 role 静默忽略。
  }

  if (pendingToolCalls) {
    attachTrailingToolCalls(messages, pendingToolCalls);
  }

  return { messages, restoredApprovals: Array.from(restoredApprovals.values()) };
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

function buildDisplayMessage(
  raw: TauriMessage,
  pendingToolCalls: ParsedToolCall[] | undefined,
): LoadedUIMessage {
  const role = raw.role as 'user' | 'assistant';
  const parts: LoadedUIMessagePart[] = [];

  if (role === 'assistant' && pendingToolCalls && pendingToolCalls.length > 0) {
    pendingToolCalls.forEach((tc, idx) => {
      parts.push(toolCallToPart(tc, raw.id, idx));
    });
  }

  if (raw.content.length > 0) {
    parts.push({ type: 'text', text: raw.content, state: 'done' });
  }

  const metadata: LoadedUIMessageMetadata = { createdAt: raw.created_at };
  if (raw.attachments && raw.attachments.length > 0) {
    metadata.attachments = raw.attachments;
  }

  return { id: raw.id, role, parts, metadata };
}

function toolCallToPart(
  tc: ParsedToolCall,
  messageId: string,
  index: number,
): LoadedDynamicToolPart {
  const toolCallId = tc.toolCallId ?? `history-${messageId}-${index}`;
  const input = (tc.args ?? {}) as ReadonlyJSONObject | Record<string, never>;

  if (tc.isError && typeof tc.result === 'string') {
    return {
      type: 'dynamic-tool',
      toolName: tc.toolName,
      toolCallId,
      state: 'output-error',
      input,
      errorText: tc.result,
    };
  }

  if (typeof tc.result === 'string') {
    return {
      type: 'dynamic-tool',
      toolName: tc.toolName,
      toolCallId,
      state: 'output-available',
      input,
      output: tc.result,
    };
  }

  return {
    type: 'dynamic-tool',
    toolName: tc.toolName,
    toolCallId,
    state: 'input-available',
    input,
  };
}

function applySystemEvent(
  content: string,
  messages: LoadedUIMessage[],
  restoredApprovals: Map<string, RestoredApproval>,
): void {
  const event = parsePersistedUiEvent(content);
  if (!event) return;

  if (event.event === 'routine_triggered') {
    const lastUser = findLastUserMessage(messages);
    if (lastUser) {
      const md = (lastUser.metadata ??= {});
      md.routineFired = (md.routineFired ?? 0) + event.fired;
    }
    return;
  }

  if (event.event === 'dlp_redacted') {
    const lastUser = findLastUserMessage(messages);
    if (lastUser) {
      const md = (lastUser.metadata ??= {});
      md.dlpStats = event.stats;
    }
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

function findLastUserMessage(messages: LoadedUIMessage[]): LoadedUIMessage | undefined {
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === 'user') return messages[i];
  }
  return undefined;
}

function attachTrailingToolCalls(
  messages: LoadedUIMessage[],
  toolCalls: ParsedToolCall[],
): void {
  const last = messages[messages.length - 1];
  if (last?.role === 'assistant') {
    toolCalls.forEach((tc, idx) => {
      last.parts.unshift(toolCallToPart(tc, last.id, idx));
    });
    return;
  }

  const synthId = `orphan-tool-calls-${messages.length}`;
  const parts = toolCalls.map((tc, idx) => toolCallToPart(tc, synthId, idx));
  messages.push({ id: synthId, role: 'assistant', parts });
}

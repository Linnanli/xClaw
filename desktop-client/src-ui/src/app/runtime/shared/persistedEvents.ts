/**
 * persistedEvents — 后端持久化消息（role=tool_calls / role=system ui_event）的纯函数解析器。
 *
 * Phase 1.1 抽自 `TauriRuntimeProvider.tsx` L113–L251，行为完全一致。
 * 这里只保留"字符串 JSON → 结构化对象"的纯函数逻辑，不依赖 React / assistant-ui。
 *
 * 两份 Provider（TauriRuntimeProvider + ChatRuntimeProvider Phase 1.3 历史回放）
 * 都会复用这组解析器。
 */

import type { ReadonlyJSONObject } from 'assistant-stream/utils';
import type { SanitizationStats } from '@hooks/useDlpScan';

// ---------------------------------------------------------------------------
// role=tool_calls 的 content 解析
// ---------------------------------------------------------------------------

type PersistedToolCall = {
  name?: string;
  id?: string;
  call_id?: string;
  tool_call_id?: string;
  args?: unknown;
  arguments?: unknown;
  parameters?: unknown;
  result?: string;
  error?: string;
};

export interface ParsedToolCall {
  toolCallId?: string;
  toolName: string;
  args?: ReadonlyJSONObject;
  result?: string;
  isError?: boolean;
}

function parsePersistedToolArgs(raw: unknown): ReadonlyJSONObject | undefined {
  if (raw && typeof raw === 'object' && !Array.isArray(raw)) {
    return raw as ReadonlyJSONObject;
  }

  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw) as unknown;
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return parsed as ReadonlyJSONObject;
      }
    } catch {
      return undefined;
    }
  }

  return undefined;
}

export function parsePersistedToolCalls(content: string): ParsedToolCall[] {
  try {
    const parsed = JSON.parse(content) as unknown;
    const calls = Array.isArray(parsed)
      ? parsed
      : parsed && typeof parsed === 'object' && 'calls' in parsed
        ? (parsed as { calls?: unknown }).calls
        : parsed && typeof parsed === 'object' && 'tool_calls' in parsed
          ? (parsed as { tool_calls?: unknown }).tool_calls
        : undefined;

    if (!Array.isArray(calls)) {
      return [];
    }

    return calls
      .filter((call): call is PersistedToolCall => !!call && typeof call === 'object')
      .map((call) => ({
        toolCallId:
          typeof call.tool_call_id === 'string'
            ? call.tool_call_id
            : typeof call.call_id === 'string'
              ? call.call_id
              : typeof call.id === 'string'
                ? call.id
                : undefined,
        toolName: typeof call.name === 'string' ? call.name : 'unknown_tool',
        args: parsePersistedToolArgs(call.args ?? call.arguments ?? call.parameters),
        result: typeof call.result === 'string'
          ? call.result
          : typeof call.error === 'string'
            ? call.error
            : undefined,
        isError: typeof call.error === 'string',
      }));
  } catch {
    return [];
  }
}

// ---------------------------------------------------------------------------
// role=system 的 ui_event content 解析
// ---------------------------------------------------------------------------

export type PersistedUiEvent =
  | { kind: 'ui_event'; event: 'routine_triggered'; fired: number }
  | { kind: 'ui_event'; event: 'dlp_redacted'; stats: SanitizationStats }
  | {
      kind: 'ui_event';
      event: 'approval_needed';
      request_id: string;
      tool_name: string;
      description: string;
    }
  | { kind: 'ui_event'; event: 'approval_resolved'; request_id: string; approved: boolean };

function isSanitizationStats(value: unknown): value is SanitizationStats {
  if (!value || typeof value !== 'object') return false;
  const stats = value as Record<string, unknown>;
  return (
    typeof stats.total_matches === 'number' &&
    typeof stats.redacted_count === 'number' &&
    typeof stats.blocked_count === 'number' &&
    typeof stats.warned_count === 'number'
  );
}

export function parsePersistedUiEvent(content: string): PersistedUiEvent | null {
  try {
    const parsed = JSON.parse(content) as Record<string, unknown>;
    if (parsed.kind !== 'ui_event' || typeof parsed.event !== 'string') return null;

    if (parsed.event === 'routine_triggered') {
      const fired = typeof parsed.fired === 'number' && parsed.fired > 0 ? parsed.fired : 1;
      return { kind: 'ui_event', event: 'routine_triggered', fired };
    }
    if (parsed.event === 'dlp_redacted' && isSanitizationStats(parsed.stats)) {
      return { kind: 'ui_event', event: 'dlp_redacted', stats: parsed.stats };
    }
    if (
      parsed.event === 'approval_needed' &&
      typeof parsed.request_id === 'string' &&
      typeof parsed.tool_name === 'string' &&
      typeof parsed.description === 'string'
    ) {
      return {
        kind: 'ui_event',
        event: 'approval_needed',
        request_id: parsed.request_id,
        tool_name: parsed.tool_name,
        description: parsed.description,
      };
    }
    if (
      parsed.event === 'approval_resolved' &&
      typeof parsed.request_id === 'string' &&
      typeof parsed.approved === 'boolean'
    ) {
      return {
        kind: 'ui_event',
        event: 'approval_resolved',
        request_id: parsed.request_id,
        approved: parsed.approved,
      };
    }
    return null;
  } catch {
    return null;
  }
}

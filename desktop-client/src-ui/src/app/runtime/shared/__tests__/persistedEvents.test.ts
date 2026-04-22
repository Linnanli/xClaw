/**
 * persistedEvents 单元测试
 *
 * 覆盖维度：
 * - Happy path：wrapper 格式 / array 格式 / tool_calls key
 * - 失败路径：非法 JSON / 非数组 / 缺字段
 * - UI 事件：routine_triggered / dlp_redacted / approval_needed / approval_resolved
 */

import { describe, it, expect } from 'vitest';
import {
  parsePersistedToolCalls,
  parsePersistedUiEvent,
} from '../persistedEvents';

describe('parsePersistedToolCalls', () => {
  it('解析 { calls: [...] } 包装格式', () => {
    const parsed = parsePersistedToolCalls(
      JSON.stringify({
        calls: [
          {
            name: 'plan_mode',
            call_id: 'turn1_0',
            result: '{"action":"submit"}',
          },
        ],
      }),
    );
    expect(parsed).toHaveLength(1);
    expect(parsed[0].toolName).toBe('plan_mode');
    expect(parsed[0].toolCallId).toBe('turn1_0');
    expect(parsed[0].isError).toBe(false);
    expect(parsed[0].result).toBe('{"action":"submit"}');
  });

  it('解析 { tool_calls: [...] } 包装格式', () => {
    const parsed = parsePersistedToolCalls(
      JSON.stringify({ tool_calls: [{ name: 'echo', tool_call_id: 't1', args: { x: 1 } }] }),
    );
    expect(parsed).toHaveLength(1);
    expect(parsed[0].toolName).toBe('echo');
    expect(parsed[0].toolCallId).toBe('t1');
    expect(parsed[0].args).toEqual({ x: 1 });
  });

  it('解析裸数组格式', () => {
    const parsed = parsePersistedToolCalls(
      JSON.stringify([{ name: 'grep', id: 'g1', args: '{"pattern":"foo"}' }]),
    );
    expect(parsed).toHaveLength(1);
    expect(parsed[0].toolName).toBe('grep');
    expect(parsed[0].toolCallId).toBe('g1');
    // args 是 JSON 字符串时应被解析成对象
    expect(parsed[0].args).toEqual({ pattern: 'foo' });
  });

  it('error 字段会被映射到 result + isError=true', () => {
    const parsed = parsePersistedToolCalls(
      JSON.stringify([{ name: 'sub_agent', call_id: 't2', error: 'depth limit reached' }]),
    );
    expect(parsed).toHaveLength(1);
    expect(parsed[0].isError).toBe(true);
    expect(parsed[0].result).toBe('depth limit reached');
  });

  it('toolCallId 优先级：tool_call_id > call_id > id', () => {
    const parsed = parsePersistedToolCalls(
      JSON.stringify([{ name: 'x', tool_call_id: 'A', call_id: 'B', id: 'C' }]),
    );
    expect(parsed[0].toolCallId).toBe('A');
    const parsed2 = parsePersistedToolCalls(
      JSON.stringify([{ name: 'x', call_id: 'B', id: 'C' }]),
    );
    expect(parsed2[0].toolCallId).toBe('B');
    const parsed3 = parsePersistedToolCalls(JSON.stringify([{ name: 'x', id: 'C' }]));
    expect(parsed3[0].toolCallId).toBe('C');
  });

  it('缺失 name 时 fallback 到 unknown_tool', () => {
    const parsed = parsePersistedToolCalls(JSON.stringify([{ id: 't1' }]));
    expect(parsed).toHaveLength(1);
    expect(parsed[0].toolName).toBe('unknown_tool');
  });

  it('非法 JSON → 返回空数组', () => {
    expect(parsePersistedToolCalls('not json')).toEqual([]);
    expect(parsePersistedToolCalls('')).toEqual([]);
  });

  it('顶层非数组且无 calls/tool_calls key → 空数组', () => {
    expect(parsePersistedToolCalls(JSON.stringify({ foo: 'bar' }))).toEqual([]);
    expect(parsePersistedToolCalls(JSON.stringify(null))).toEqual([]);
  });

  it('非对象元素被过滤掉', () => {
    const parsed = parsePersistedToolCalls(JSON.stringify([null, 'str', 42, { name: 'ok' }]));
    expect(parsed).toHaveLength(1);
    expect(parsed[0].toolName).toBe('ok');
  });

  it('args 为数组/非法字符串 → 返回 undefined', () => {
    const parsed = parsePersistedToolCalls(
      JSON.stringify([{ name: 'x', args: [1, 2, 3] }]),
    );
    expect(parsed[0].args).toBeUndefined();

    const parsed2 = parsePersistedToolCalls(
      JSON.stringify([{ name: 'x', args: 'not-json' }]),
    );
    expect(parsed2[0].args).toBeUndefined();
  });
});

describe('parsePersistedUiEvent', () => {
  it('routine_triggered 正常', () => {
    const ev = parsePersistedUiEvent(
      JSON.stringify({ kind: 'ui_event', event: 'routine_triggered', fired: 3 }),
    );
    expect(ev).toEqual({ kind: 'ui_event', event: 'routine_triggered', fired: 3 });
  });

  it('routine_triggered 缺 fired 或非正数 → fallback 1', () => {
    const ev = parsePersistedUiEvent(
      JSON.stringify({ kind: 'ui_event', event: 'routine_triggered' }),
    );
    expect(ev).toEqual({ kind: 'ui_event', event: 'routine_triggered', fired: 1 });

    const ev2 = parsePersistedUiEvent(
      JSON.stringify({ kind: 'ui_event', event: 'routine_triggered', fired: 0 }),
    );
    expect(ev2?.event === 'routine_triggered' && ev2.fired).toBe(1);
  });

  it('dlp_redacted 正常', () => {
    const stats = { total_matches: 2, redacted_count: 1, blocked_count: 0, warned_count: 1 };
    const ev = parsePersistedUiEvent(
      JSON.stringify({ kind: 'ui_event', event: 'dlp_redacted', stats }),
    );
    expect(ev).toEqual({ kind: 'ui_event', event: 'dlp_redacted', stats });
  });

  it('dlp_redacted stats 字段缺失 → null', () => {
    const ev = parsePersistedUiEvent(
      JSON.stringify({ kind: 'ui_event', event: 'dlp_redacted', stats: { total_matches: 1 } }),
    );
    expect(ev).toBeNull();
  });

  it('approval_needed 正常', () => {
    const ev = parsePersistedUiEvent(
      JSON.stringify({
        kind: 'ui_event',
        event: 'approval_needed',
        request_id: 'r1',
        tool_name: 'rm',
        description: 'delete file',
      }),
    );
    expect(ev).toEqual({
      kind: 'ui_event',
      event: 'approval_needed',
      request_id: 'r1',
      tool_name: 'rm',
      description: 'delete file',
    });
  });

  it('approval_needed 缺字段 → null', () => {
    const ev = parsePersistedUiEvent(
      JSON.stringify({ kind: 'ui_event', event: 'approval_needed', request_id: 'r1' }),
    );
    expect(ev).toBeNull();
  });

  it('approval_resolved 正常', () => {
    const ev = parsePersistedUiEvent(
      JSON.stringify({
        kind: 'ui_event',
        event: 'approval_resolved',
        request_id: 'r1',
        approved: true,
      }),
    );
    expect(ev).toEqual({
      kind: 'ui_event',
      event: 'approval_resolved',
      request_id: 'r1',
      approved: true,
    });
  });

  it('非 ui_event kind → null', () => {
    expect(
      parsePersistedUiEvent(JSON.stringify({ kind: 'other', event: 'routine_triggered' })),
    ).toBeNull();
  });

  it('未知 event 类型 → null', () => {
    expect(
      parsePersistedUiEvent(JSON.stringify({ kind: 'ui_event', event: 'unknown' })),
    ).toBeNull();
  });

  it('非法 JSON → null', () => {
    expect(parsePersistedUiEvent('not json')).toBeNull();
    expect(parsePersistedUiEvent('')).toBeNull();
  });
});

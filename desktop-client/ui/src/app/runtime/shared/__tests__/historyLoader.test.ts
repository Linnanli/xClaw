import { describe, expect, it } from 'vitest';
import type { Message as TauriMessage } from '../../../utils/tauri';
import { mapTauriMessagesToUIMessages } from '../historyLoader';

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const baseTs = '2025-01-01T00:00:00Z';

function msg(partial: Partial<TauriMessage> & { id: string; role: string; content: string }): TauriMessage {
  return {
    thread_id: 't1',
    created_at: baseTs,
    ...partial,
  } as TauriMessage;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('mapTauriMessagesToUIMessages', () => {
  it('映射 user/assistant 为 text parts，保留 createdAt', () => {
    const { messages, restoredApprovals } = mapTauriMessagesToUIMessages([
      msg({ id: 'u1', role: 'user', content: '你好' }),
      msg({ id: 'a1', role: 'assistant', content: '你好，请问有什么可以帮您？' }),
    ]);

    expect(restoredApprovals).toEqual([]);
    expect(messages).toHaveLength(2);
    expect(messages[0]).toMatchObject({
      id: 'u1',
      role: 'user',
      parts: [{ type: 'text', text: '你好', state: 'done' }],
      metadata: { createdAt: baseTs },
    });
    expect(messages[1].parts[0]).toEqual({
      type: 'text',
      text: '你好，请问有什么可以帮您？',
      state: 'done',
    });
  });

  it('合并 tool_calls 到其后的 assistant 消息的 parts 前面', () => {
    const toolContent = JSON.stringify({
      calls: [
        {
          id: 'call-1',
          name: 'web_search',
          args: { query: 'weather' },
          result: '{"ok":true}',
        },
      ],
    });

    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'u1', role: 'user', content: '查天气' }),
      msg({ id: 'tc1', role: 'tool_calls', content: toolContent }),
      msg({ id: 'a1', role: 'assistant', content: '北京今天晴。' }),
    ]);

    expect(messages).toHaveLength(2);
    const assistant = messages[1];
    expect(assistant.parts).toHaveLength(2);
    expect(assistant.parts[0]).toEqual({
      type: 'dynamic-tool',
      toolName: 'web_search',
      toolCallId: 'call-1',
      state: 'output-available',
      input: { query: 'weather' },
      output: '{"ok":true}',
    });
    expect(assistant.parts[1]).toEqual({ type: 'text', text: '北京今天晴。', state: 'done' });
  });

  it('带 error 的 tool_call 映射为 output-error 状态', () => {
    const toolContent = JSON.stringify({
      calls: [{ id: 'c1', name: 'read_file', args: { path: '/x' }, error: 'permission denied' }],
    });
    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'tc', role: 'tool_calls', content: toolContent }),
      msg({ id: 'a', role: 'assistant', content: '失败了' }),
    ]);

    const part = messages[0].parts[0];
    expect(part).toEqual({
      type: 'dynamic-tool',
      toolName: 'read_file',
      toolCallId: 'c1',
      state: 'output-error',
      input: { path: '/x' },
      errorText: 'permission denied',
    });
  });

  it('没有 result 的 tool_call 映射为 input-available', () => {
    const toolContent = JSON.stringify({
      calls: [{ id: 'c2', name: 'pending_tool', args: { a: 1 } }],
    });
    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'tc', role: 'tool_calls', content: toolContent }),
      msg({ id: 'a', role: 'assistant', content: '' }),
    ]);

    expect(messages[0].parts[0]).toEqual({
      type: 'dynamic-tool',
      toolName: 'pending_tool',
      toolCallId: 'c2',
      state: 'input-available',
      input: { a: 1 },
    });
  });

  it('末尾 tool_calls 无后续 assistant 时合成 orphan assistant 消息', () => {
    const toolContent = JSON.stringify({
      calls: [{ id: 'c', name: 't', result: 'ok' }],
    });
    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'u', role: 'user', content: 'hi' }),
      msg({ id: 'tc', role: 'tool_calls', content: toolContent }),
    ]);

    expect(messages).toHaveLength(2);
    expect(messages[1].role).toBe('assistant');
    expect(messages[1].id).toMatch(/^orphan-tool-calls-/);
    expect(messages[1].parts).toHaveLength(1);
    expect(messages[1].parts[0]).toMatchObject({ type: 'dynamic-tool', toolName: 't' });
  });

  it('末尾 tool_calls 若前一条是 assistant 则 unshift 到其 parts 前面', () => {
    const toolContent = JSON.stringify({
      calls: [{ id: 'c', name: 't', result: 'ok' }],
    });
    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'a', role: 'assistant', content: '先有文本' }),
      msg({ id: 'tc', role: 'tool_calls', content: toolContent }),
    ]);

    expect(messages).toHaveLength(1);
    expect(messages[0].parts).toHaveLength(2);
    expect(messages[0].parts[0].type).toBe('dynamic-tool');
    expect(messages[0].parts[1]).toMatchObject({ type: 'text', text: '先有文本' });
  });

  it('routine_triggered ui_event 累加到上一条 user 的 metadata.routineFired', () => {
    const ev1 = JSON.stringify({ kind: 'ui_event', event: 'routine_triggered', fired: 2 });
    const ev2 = JSON.stringify({ kind: 'ui_event', event: 'routine_triggered', fired: 3 });
    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'u', role: 'user', content: 'q' }),
      msg({ id: 's1', role: 'system', content: ev1 }),
      msg({ id: 's2', role: 'system', content: ev2 }),
    ]);

    expect(messages[0].metadata?.routineFired).toBe(5);
  });

  it('dlp_redacted ui_event 写入上一条 user 的 metadata.dlpStats', () => {
    const stats = { total_matches: 3, redacted_count: 2, blocked_count: 0, warned_count: 1 };
    const content = JSON.stringify({ kind: 'ui_event', event: 'dlp_redacted', stats });

    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'u', role: 'user', content: 'secret' }),
      msg({ id: 's', role: 'system', content }),
    ]);

    expect(messages[0].metadata?.dlpStats).toEqual(stats);
  });

  it('approval_needed/approval_resolved 聚合到 restoredApprovals 队列', () => {
    const needed1 = JSON.stringify({
      kind: 'ui_event',
      event: 'approval_needed',
      request_id: 'r1',
      tool_name: 'dangerous',
      description: 'run?',
    });
    const needed2 = JSON.stringify({
      kind: 'ui_event',
      event: 'approval_needed',
      request_id: 'r2',
      tool_name: 'also_dangerous',
      description: 'ok?',
    });
    const resolved = JSON.stringify({
      kind: 'ui_event',
      event: 'approval_resolved',
      request_id: 'r1',
      approved: true,
    });

    const { restoredApprovals } = mapTauriMessagesToUIMessages([
      msg({ id: 's1', role: 'system', content: needed1 }),
      msg({ id: 's2', role: 'system', content: needed2 }),
      msg({ id: 's3', role: 'system', content: resolved }),
    ]);

    expect(restoredApprovals).toEqual([
      { request_id: 'r2', tool_name: 'also_dangerous', description: 'ok?' },
    ]);
  });

  it('保留 user 消息的 attachments 到 metadata', () => {
    const attachments = [
      { id: 'f1', type: 'file', name: 'a.txt', contentType: 'text/plain' },
    ] as unknown as TauriMessage['attachments'];
    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'u', role: 'user', content: '看看这个', attachments }),
    ]);

    expect(messages[0].metadata?.attachments).toEqual(attachments);
  });

  it('tool_calls 的 content 解析失败时静默跳过，不产生 parts', () => {
    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'tc', role: 'tool_calls', content: 'not json' }),
      msg({ id: 'a', role: 'assistant', content: 'hi' }),
    ]);

    expect(messages).toHaveLength(1);
    expect(messages[0].parts).toEqual([{ type: 'text', text: 'hi', state: 'done' }]);
  });

  it('未知 role 或无效 ui_event 被忽略', () => {
    const { messages, restoredApprovals } = mapTauriMessagesToUIMessages([
      msg({ id: 'x', role: 'mystery', content: 'foo' }),
      msg({ id: 's', role: 'system', content: '{"kind":"ui_event","event":"unknown"}' }),
      msg({ id: 'u', role: 'user', content: 'hi' }),
    ]);

    expect(messages).toHaveLength(1);
    expect(messages[0].id).toBe('u');
    expect(restoredApprovals).toEqual([]);
  });

  it('空 content 的 assistant 仅在有 tool_calls 时保留为空 text-free 消息', () => {
    const toolContent = JSON.stringify({
      calls: [{ id: 'c', name: 't', result: 'ok' }],
    });
    const { messages } = mapTauriMessagesToUIMessages([
      msg({ id: 'tc', role: 'tool_calls', content: toolContent }),
      msg({ id: 'a', role: 'assistant', content: '' }),
    ]);

    expect(messages).toHaveLength(1);
    expect(messages[0].parts).toHaveLength(1);
    expect(messages[0].parts[0].type).toBe('dynamic-tool');
  });
});

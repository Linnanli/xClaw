/**
 * TauriChatTransport 单元测试
 *
 * 验证：
 * 1. sendMessages 调用 invoke('send_chat_message', ...) 且参数正确
 * 2. chat-stream 事件被正确转成 UIMessageChunk
 * 3. 'finish' 事件关闭 ReadableStream
 * 4. abortSignal 关闭 stream
 * 5. invoke 失败时发 error chunk
 * 6. data-custom 事件按 data.kind 映射成 `data-${kind}`
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { UIMessage } from 'ai';

const { invokeMock, listenMock, unlistenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn().mockResolvedValue({ message_id: 'msg-1', success: true }),
  listenMock: vi.fn(),
  unlistenMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }));

import { TauriChatTransport } from '../TauriChatTransport';

type EventHandler = (event: { payload: unknown }) => void;

function setupListenCapture(): { getHandler: () => EventHandler | null } {
  let handler: EventHandler | null = null;
  listenMock.mockImplementation(async (_name: string, cb: EventHandler) => {
    handler = cb;
    return unlistenMock;
  });
  return { getHandler: () => handler };
}

function makeUserMessage(text: string): UIMessage {
  return {
    id: 'u1',
    role: 'user',
    parts: [{ type: 'text', text }],
  } as UIMessage;
}

async function drainStream<T>(stream: ReadableStream<T>, max = 100): Promise<T[]> {
  const reader = stream.getReader();
  const out: T[] = [];
  while (out.length < max) {
    const { value, done } = await reader.read();
    if (done) break;
    out.push(value);
  }
  return out;
}

describe('TauriChatTransport', () => {
  beforeEach(() => {
    invokeMock.mockClear();
    listenMock.mockClear();
    unlistenMock.mockClear();
    invokeMock.mockResolvedValue({ message_id: 'msg-1', success: true });
  });

  it('invoke send_chat_message with correct thread / content / options', async () => {
    const { getHandler } = setupListenCapture();
    const transport = new TauriChatTransport({
      modelId: () => 'gpt-4',
      apiBaseUrl: () => 'https://x.test',
      apiKey: () => 'sk-test',
    });

    const stream = await transport.sendMessages({
      chatId: 'thread-42',
      messages: [makeUserMessage('hello world')],
      abortSignal: undefined,
    });

    // 触发 finish 关闭 stream，让 invoke 有机会被 await
    const handler = getHandler();
    expect(handler).not.toBeNull();
    handler!({ payload: { type: 'finish' } });
    await drainStream(stream);

    expect(invokeMock).toHaveBeenCalledWith('send_chat_message', {
      threadId: 'thread-42',
      content: 'hello world',
      modelId: 'gpt-4',
      apiBaseUrl: 'https://x.test',
      apiKey: 'sk-test',
      dlpStats: null,
      attachments: null,
    });
  });

  it('forwards text-delta / tool-* events as UIMessageChunk', async () => {
    const { getHandler } = setupListenCapture();
    const transport = new TauriChatTransport();

    const stream = await transport.sendMessages({
      chatId: 't1',
      messages: [makeUserMessage('hi')],
      abortSignal: undefined,
    });

    const handler = getHandler()!;
    handler({ payload: { type: 'text-start', id: 'm1' } });
    handler({ payload: { type: 'text-delta', id: 'm1', delta: 'Hello' } });
    handler({ payload: { type: 'text-delta', id: 'm1', delta: ' world' } });
    handler({
      payload: {
        type: 'tool-input-available',
        toolCallId: 'c1',
        toolName: 'read_file',
        input: { path: '/a.md' },
      },
    });
    handler({
      payload: { type: 'tool-output-available', toolCallId: 'c1', output: 'content' },
    });
    handler({ payload: { type: 'finish' } });

    const chunks = await drainStream(stream);
    expect(chunks).toEqual([
      { type: 'text-start', id: 'm1' },
      { type: 'text-delta', id: 'm1', delta: 'Hello' },
      { type: 'text-delta', id: 'm1', delta: ' world' },
      {
        type: 'tool-input-available',
        toolCallId: 'c1',
        toolName: 'read_file',
        input: { path: '/a.md' },
      },
      { type: 'tool-output-available', toolCallId: 'c1', output: 'content' },
      { type: 'finish' },
    ]);
  });

  it("closes stream and unlisten when 'finish' arrives", async () => {
    const { getHandler } = setupListenCapture();
    const transport = new TauriChatTransport();

    const stream = await transport.sendMessages({
      chatId: 't1',
      messages: [makeUserMessage('hi')],
      abortSignal: undefined,
    });

    getHandler()!({ payload: { type: 'finish' } });
    const chunks = await drainStream(stream);
    expect(chunks).toContainEqual({ type: 'finish' });
    expect(unlistenMock).toHaveBeenCalled();
  });

  it('closes stream on abortSignal', async () => {
    const { getHandler } = setupListenCapture();
    const transport = new TauriChatTransport();
    const ac = new AbortController();

    const stream = await transport.sendMessages({
      chatId: 't1',
      messages: [makeUserMessage('hi')],
      abortSignal: ac.signal,
    });

    getHandler()!({ payload: { type: 'text-delta', id: 'x', delta: 'partial' } });
    ac.abort();
    const chunks = await drainStream(stream);
    expect(chunks).toEqual([{ type: 'text-delta', id: 'x', delta: 'partial' }]);
    expect(unlistenMock).toHaveBeenCalled();
  });

  it('emits error chunk if invoke rejects', async () => {
    setupListenCapture();
    invokeMock.mockRejectedValueOnce(new Error('backend dead'));
    const transport = new TauriChatTransport();

    const stream = await transport.sendMessages({
      chatId: 't1',
      messages: [makeUserMessage('hi')],
      abortSignal: undefined,
    });

    const chunks = await drainStream(stream);
    expect(chunks).toEqual([{ type: 'error', errorText: 'backend dead' }]);
  });

  it("maps data-custom to `data-${kind}` using data.kind or data.type", async () => {
    const { getHandler } = setupListenCapture();
    const transport = new TauriChatTransport();

    const stream = await transport.sendMessages({
      chatId: 't1',
      messages: [makeUserMessage('hi')],
      abortSignal: undefined,
    });

    const handler = getHandler()!;
    handler({
      payload: {
        type: 'data-custom',
        id: 'a1',
        data: { kind: 'approval_needed', request_id: 'r1', tool_name: 'rm' },
      },
    });
    handler({
      payload: {
        type: 'data-custom',
        data: { type: 'job_status', job_id: 'j1', status: 'running' },
      },
    });
    handler({ payload: { type: 'finish' } });

    const chunks = await drainStream(stream);
    expect(chunks[0]).toEqual({
      type: 'data-approval_needed',
      id: 'a1',
      data: { kind: 'approval_needed', request_id: 'r1', tool_name: 'rm' },
    });
    expect(chunks[1]).toEqual({
      type: 'data-job_status',
      id: undefined,
      data: { type: 'job_status', job_id: 'j1', status: 'running' },
    });
  });

  it('extracts last user message content (skipping assistant messages)', async () => {
    const { getHandler } = setupListenCapture();
    const transport = new TauriChatTransport();

    const messages: UIMessage[] = [
      { id: 'u1', role: 'user', parts: [{ type: 'text', text: 'first' }] } as UIMessage,
      { id: 'a1', role: 'assistant', parts: [{ type: 'text', text: 'reply' }] } as UIMessage,
      { id: 'u2', role: 'user', parts: [{ type: 'text', text: 'second question' }] } as UIMessage,
    ];

    const stream = await transport.sendMessages({
      chatId: 't1',
      messages,
      abortSignal: undefined,
    });

    getHandler()!({ payload: { type: 'finish' } });
    await drainStream(stream);

    expect(invokeMock).toHaveBeenCalledWith(
      'send_chat_message',
      expect.objectContaining({ content: 'second question' }),
    );
  });

  it('reconnectToStream returns null (unsupported)', async () => {
    const transport = new TauriChatTransport();
    await expect(transport.reconnectToStream()).resolves.toBeNull();
  });

  // ── Envelope 过滤：thread-scoped 事件按 threadId 丢弃 ────────────────────

  it('drops events whose envelope threadId mismatches currentThreadId', async () => {
    const { getHandler } = setupListenCapture();
    let currentThread = 'thread-B';
    const transport = new TauriChatTransport({
      currentThreadId: () => currentThread,
    });

    const stream = await transport.sendMessages({
      chatId: 'thread-B',
      messages: [makeUserMessage('hi')],
      abortSignal: undefined,
    });

    const handler = getHandler()!;
    // 旧 thread A 残留事件 —— envelope 有 threadId=thread-A，当前绑定 thread-B，必须被丢弃
    handler({
      payload: { type: 'text-delta', id: 'm1', delta: 'OLD', threadId: 'thread-A' },
    });
    // 当前 thread B 的事件 —— 必须透传
    handler({
      payload: { type: 'text-delta', id: 'm1', delta: 'NEW', threadId: 'thread-B' },
    });
    handler({ payload: { type: 'finish', threadId: 'thread-B' } });

    const chunks = await drainStream(stream);
    const deltas = chunks.filter((c): c is { type: 'text-delta'; delta: string } & UIMessageChunkBase =>
      c.type === 'text-delta',
    );
    expect(deltas.map((c) => c.delta)).toEqual(['NEW']);
    expect(deltas.map((c) => c.delta)).not.toContain('OLD');
  });

  it('always forwards system-level events (envelope without threadId)', async () => {
    const { getHandler } = setupListenCapture();
    const transport = new TauriChatTransport({
      currentThreadId: () => 'thread-B',
    });

    const stream = await transport.sendMessages({
      chatId: 'thread-B',
      messages: [makeUserMessage('hi')],
      abortSignal: undefined,
    });

    const handler = getHandler()!;
    // 系统级广播：无 threadId，必须透传
    handler({
      payload: {
        type: 'data-custom',
        data: { type: 'connection_status', connected: true },
      },
    });
    handler({
      payload: {
        type: 'data-custom',
        data: { type: 'job_status', job_id: 'j-1', status: 'running' },
      },
    });
    handler({ payload: { type: 'finish' } });

    const chunks = await drainStream(stream);
    const kinds = chunks.map((c) => c.type);
    expect(kinds).toContain('data-connection_status');
    expect(kinds).toContain('data-job_status');
  });

  it('falls back to pass-through when currentThreadId returns null', async () => {
    const { getHandler } = setupListenCapture();
    const transport = new TauriChatTransport({
      currentThreadId: () => null,
    });

    const stream = await transport.sendMessages({
      chatId: 'thread-X',
      messages: [makeUserMessage('hi')],
      abortSignal: undefined,
    });

    const handler = getHandler()!;
    handler({
      payload: { type: 'text-delta', id: 'm1', delta: 'pass', threadId: 'thread-Y' },
    });
    handler({ payload: { type: 'finish' } });

    const chunks = await drainStream(stream);
    expect(chunks.some((c) => c.type === 'text-delta')).toBe(true);
  });
});

// 单独测试 extractEnvelopeThreadId 纯函数
describe('extractEnvelopeThreadId', () => {
  it('returns string when payload.threadId is non-empty string', async () => {
    const { extractEnvelopeThreadId } = await import('../TauriChatTransport');
    expect(extractEnvelopeThreadId({ type: 'text-delta', threadId: 'thread-A' })).toBe('thread-A');
  });

  it('returns null when threadId missing', async () => {
    const { extractEnvelopeThreadId } = await import('../TauriChatTransport');
    expect(extractEnvelopeThreadId({ type: 'data-custom', data: {} })).toBeNull();
  });

  it('returns null when threadId is empty string', async () => {
    const { extractEnvelopeThreadId } = await import('../TauriChatTransport');
    expect(extractEnvelopeThreadId({ type: 'text-delta', threadId: '' })).toBeNull();
  });

  it('returns null for non-object payload', async () => {
    const { extractEnvelopeThreadId } = await import('../TauriChatTransport');
    expect(extractEnvelopeThreadId(null)).toBeNull();
    expect(extractEnvelopeThreadId(undefined)).toBeNull();
    expect(extractEnvelopeThreadId('string')).toBeNull();
  });
});

type UIMessageChunkBase = { type: string };

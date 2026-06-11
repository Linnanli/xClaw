import { describe, expect, it, vi } from 'vitest';
import { DasclawAppServerSessionBridge } from '../src/main/dasclaw/app-server-session-bridge';
import type { AppServerRpc, JsonRpcNotification } from '../src/main/dasclaw/app-server-rpc';
import type { DatabaseInstance, MessageRow, SessionRow } from '../src/main/db/database';
import type { ServerEvent } from '../src/renderer/types';

vi.mock('../src/main/config/config-store', () => ({
  configStore: {
    get: (key: string) => {
      if (key === 'memoryEnabled') return true;
      if (key === 'model') return 'dasclaw-test-model';
      return undefined;
    },
  },
}));

class FakeRpc implements AppServerRpc {
  readonly requests: Array<{ method: string; params?: unknown }> = [];
  beforeTurnStartResponse?: () => void;
  beforeTurnInterruptResponse?: () => void;
  failMethods = new Map<string, Error>();
  private readonly threads = new Set<string>();
  private nextThreadNumber = 1;
  private nextTurnNumber = 1;
  private listener: ((notification: JsonRpcNotification) => void) | null = null;
  private transportClosedListener: ((error: Error) => void) | null = null;

  async request<T>(method: string, params?: unknown): Promise<T> {
    this.requests.push({ method, params });
    if (method === 'thread/start') {
      const threadId = `thread_${this.nextThreadNumber++}`;
      this.threads.add(threadId);
      return { threadId } as T;
    }
    if (method === 'turn/start') {
      this.beforeTurnStartResponse?.();
      const failure = this.failMethods.get(method);
      if (failure) throw failure;
      const threadId = stringParam(params, 'threadId');
      if (!threadId || !this.threads.has(threadId)) {
        throw new Error(`unknown thread id: ${threadId ?? '<missing>'}`);
      }
      const prompt = codexTextInputParam(params);
      if (!prompt) {
        throw new Error('turn/start must use Codex UserInput text array params');
      }
      const turnId = `turn_${this.nextTurnNumber++}`;
      return { turnId, status: 'pending' } as T;
    }
    if (method === 'turn/interrupt') {
      this.beforeTurnInterruptResponse?.();
      const failure = this.failMethods.get(method);
      if (failure) throw failure;
      return {} as T;
    }
    const failure = this.failMethods.get(method);
    if (failure) throw failure;
    return {} as T;
  }

  onNotification(listener: (notification: JsonRpcNotification) => void): () => void {
    this.listener = listener;
    return () => {
      this.listener = null;
    };
  }

  onTransportClosed(listener: (error: Error) => void): () => void {
    this.transportClosedListener = listener;
    return () => {
      this.transportClosedListener = null;
    };
  }

  emit(method: string, params: Record<string, unknown>): void {
    this.listener?.({ jsonrpc: '2.0', method, params });
  }

  closeTransport(error: Error): void {
    this.transportClosedListener?.(error);
  }

  dispose(): void {
    this.listener = null;
    this.transportClosedListener = null;
  }
}

function stringParam(params: unknown, key: string): string | undefined {
  if (!params || typeof params !== 'object' || Array.isArray(params)) return undefined;
  const value = (params as Record<string, unknown>)[key];
  return typeof value === 'string' ? value : undefined;
}

function codexTextInputParam(params: unknown): string | undefined {
  if (!params || typeof params !== 'object' || Array.isArray(params)) return undefined;
  const input = (params as Record<string, unknown>).input;
  if (!Array.isArray(input) || input.length !== 1) return undefined;
  const [item] = input;
  if (!item || typeof item !== 'object' || Array.isArray(item)) return undefined;
  const record = item as Record<string, unknown>;
  return record.type === 'text' &&
    typeof record.text === 'string' &&
    Array.isArray(record.text_elements)
    ? record.text
    : undefined;
}

function createDb(): DatabaseInstance {
  const sessions = new Map<string, SessionRow>();
  const messages: MessageRow[] = [];

  return {
    raw: {} as DatabaseInstance['raw'],
    sessions: {
      create: (session) => sessions.set(session.id, session),
      update: (id, updates) => {
        const existing = sessions.get(id);
        if (existing) sessions.set(id, { ...existing, ...updates });
      },
      get: (id) => sessions.get(id),
      getAll: () => Array.from(sessions.values()),
      delete: (id) => {
        sessions.delete(id);
      },
    },
    messages: {
      create: (message) => messages.push(message),
      update: () => {},
      getBySessionId: (sessionId) => messages.filter((message) => message.session_id === sessionId),
      delete: () => {},
      deleteBySessionId: () => {},
    },
    traceSteps: {
      create: () => {},
      update: () => {},
      getBySessionId: () => [],
      deleteBySessionId: () => {},
    },
    scheduledTasks: {
      create: () => {},
      update: () => {},
      get: () => undefined,
      getAll: () => [],
      delete: () => {},
    },
  };
}

describe('DasclawAppServerSessionBridge', () => {
  it('starts a dasclaw thread and maps stream notifications to renderer events', async () => {
    const rpc = new FakeRpc();
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');

    expect(rpc.requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
    ]);
    expect(rpc.requests[2].params).toMatchObject({
      threadId: 'thread_1',
      input: [{ type: 'text', text: 'hello', text_elements: [] }],
    });
    expect(db.sessions.get(session.id)?.claude_session_id).toBe('thread_1');
    expect(db.messages.getBySessionId(session.id)).toHaveLength(1);

    rpc.emit('item/agentMessage/delta', {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'item_1',
      delta: 'hel',
    });
    rpc.emit('item/completed', {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'item_1',
      status: 'completed',
    });
    expect(events.filter((event) => event.type === 'stream.message')).toHaveLength(0);
    rpc.emit('turn/completed', {
      threadId: 'thread_1',
      turnId: 'turn_1',
      status: 'completed',
      output: 'hello from runtime',
    });

    expect(events).toContainEqual({
      type: 'stream.partial',
      payload: { sessionId: session.id, delta: 'hel' },
    });
    expect(events).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'stream.message',
          payload: expect.objectContaining({
            sessionId: session.id,
            message: expect.objectContaining({
              role: 'assistant',
              content: [{ type: 'text', text: 'hello from runtime' }],
            }),
          }),
        }),
        {
          type: 'session.status',
          payload: { sessionId: session.id, status: 'idle' },
        },
      ])
    );
    expect(db.sessions.get(session.id)?.status).toBe('idle');
    expect(db.messages.getBySessionId(session.id)).toHaveLength(2);
  });

  it('does not duplicate partials when v2 item deltas are paired with legacy turn deltas', async () => {
    const rpc = new FakeRpc();
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');

    rpc.emit('turn/delta', {
      threadId: 'thread_1',
      turnId: 'turn_1',
      delta: 'echo: hello',
    });
    rpc.emit('item/agentMessage/delta', {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'item_1',
      delta: 'echo: hello',
    });
    rpc.emit('turn/completed', {
      threadId: 'thread_1',
      turnId: 'turn_1',
      status: 'completed',
      output: 'echo: hello',
    });

    const partials = events.filter((event) => event.type === 'stream.partial');
    expect(partials).toEqual([
      {
        type: 'stream.partial',
        payload: { sessionId: session.id, delta: 'echo: hello' },
      },
    ]);
    expect(
      db.messages.getBySessionId(session.id).filter((message) => message.role === 'assistant')
    ).toHaveLength(1);
  });

  it('interrupts the active dasclaw turn when stopping a running session', async () => {
    const rpc = new FakeRpc();
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      () => {},
      () => rpc
    );

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    await bridge.stopSession(session.id);

    expect(rpc.requests).toContainEqual({
      method: 'turn/interrupt',
      params: {
        threadId: 'thread_1',
        turnId: 'turn_1',
      },
    });
  });

  it('marks the session as error when starting a dasclaw turn fails', async () => {
    const rpc = new FakeRpc();
    rpc.failMethods.set('turn/start', new Error('runtime failed to start'));
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    await expect(bridge.startSession('Draft', 'hello', '/tmp/workspace')).rejects.toThrow(
      'runtime failed to start'
    );

    const [session] = db.sessions.getAll();
    expect(session.status).toBe('error');
    expect(events).toContainEqual({
      type: 'session.status',
      payload: {
        sessionId: session.id,
        status: 'error',
        error: 'runtime failed to start',
      },
    });
  });

  it('marks the active session as error when the app-server transport closes', async () => {
    const rpc = new FakeRpc();
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    rpc.closeTransport(new Error('dasclaw app-server exited with code 1'));

    expect(db.sessions.get(session.id)?.status).toBe('error');
    expect(events).toEqual(
      expect.arrayContaining([
        {
          type: 'session.status',
          payload: {
            sessionId: session.id,
            status: 'error',
            error: 'dasclaw app-server exited with code 1',
          },
        },
        {
          type: 'error',
          payload: { message: 'dasclaw app-server exited with code 1' },
        },
      ])
    );
  });

  it('reinitializes the app-server after an idle transport close', async () => {
    const rpcs = [new FakeRpc(), new FakeRpc()];
    const events: ServerEvent[] = [];
    const db = createDb();
    let rpcIndex = 0;
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpcs[rpcIndex++]
    );

    const firstSession = await bridge.startSession('First', 'hello', '/tmp/workspace');
    rpcs[0].emit('turn/completed', {
      threadId: 'thread_1',
      turnId: 'turn_1',
      status: 'completed',
      output: 'first done',
    });
    rpcs[0].closeTransport(new Error('idle app-server exit'));

    const secondSession = await bridge.startSession('Second', 'again', '/tmp/workspace');

    expect(db.sessions.get(firstSession.id)?.status).toBe('idle');
    expect(rpcIndex).toBe(2);
    expect(rpcs[1].requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
    ]);
    expect(secondSession.id).not.toBe(firstSession.id);
    expect(events).not.toContainEqual({
      type: 'error',
      payload: { message: 'idle app-server exit' },
    });
  });

  it('continues an existing session after the app-server transport is reset', async () => {
    const rpcs = [new FakeRpc(), new FakeRpc()];
    const events: ServerEvent[] = [];
    const db = createDb();
    let rpcIndex = 0;
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpcs[rpcIndex++]
    );

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    rpcs[0].emit('turn/completed', {
      threadId: 'thread_1',
      turnId: 'turn_1',
      status: 'completed',
      output: 'first response',
    });
    rpcs[0].closeTransport(new Error('idle app-server exit'));

    await bridge.continueSession(session.id, 'again');

    expect(rpcIndex).toBe(2);
    expect(rpcs[1].requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
    ]);
    expect(rpcs[1].requests[2].params).toMatchObject({
      threadId: 'thread_1',
      input: [{ type: 'text', text: 'again', text_elements: [] }],
    });
    expect(db.sessions.get(session.id)?.claude_session_id).toBe('thread_1');
    expect(events).toContainEqual({
      type: 'session.update',
      payload: {
        sessionId: session.id,
        updates: { claudeSessionId: 'thread_1' },
      },
    });
    expect(db.messages.getBySessionId(session.id).filter((message) => message.role === 'user'))
      .toHaveLength(2);
    expect(events).toContainEqual({
      type: 'session.status',
      payload: { sessionId: session.id, status: 'running' },
    });
  });

  it('does not overwrite an error session when stopping after transport failure', async () => {
    const rpc = new FakeRpc();
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    rpc.closeTransport(new Error('dasclaw app-server exited with code 1'));
    await bridge.stopSession(session.id);

    expect(db.sessions.get(session.id)?.status).toBe('error');
    expect(events).toContainEqual({
      type: 'session.status',
      payload: {
        sessionId: session.id,
        status: 'error',
        error: 'dasclaw app-server exited with code 1',
      },
    });
    expect(events).not.toContainEqual({
      type: 'session.status',
      payload: { sessionId: session.id, status: 'idle' },
    });
  });

  it('marks the session as error when interrupting an active dasclaw turn fails', async () => {
    const rpc = new FakeRpc();
    rpc.failMethods.set('turn/interrupt', new Error('interrupt failed'));
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    await expect(bridge.stopSession(session.id)).rejects.toThrow('interrupt failed');

    expect(db.sessions.get(session.id)?.status).toBe('error');
    expect(events).toContainEqual({
      type: 'session.status',
      payload: {
        sessionId: session.id,
        status: 'error',
        error: 'interrupt failed',
      },
    });
  });

  it('does not overwrite a terminal turn when the turn start response fails late', async () => {
    const rpc = new FakeRpc();
    rpc.failMethods.set('turn/start', new Error('late response failure'));
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    rpc.beforeTurnStartResponse = () => {
      rpc.emit('item/agentMessage/delta', {
        threadId: 'thread_1',
        turnId: 'turn_1',
        itemId: 'item_1',
        delta: 'completed before response',
      });
      rpc.emit('turn/completed', {
        threadId: 'thread_1',
        turnId: 'turn_1',
        status: 'completed',
        output: '',
      });
    };

    await expect(bridge.startSession('Draft', 'hello', '/tmp/workspace')).rejects.toThrow(
      'late response failure'
    );

    const [session] = db.sessions.getAll();
    expect(session.status).toBe('idle');
    expect(events).toContainEqual({
      type: 'session.status',
      payload: { sessionId: session.id, status: 'idle' },
    });
    expect(events).not.toContainEqual({
      type: 'session.status',
      payload: {
        sessionId: session.id,
        status: 'error',
        error: 'late response failure',
      },
    });
  });

  it('does not overwrite a terminal turn when the interrupt response fails late', async () => {
    const rpc = new FakeRpc();
    rpc.failMethods.set('turn/interrupt', new Error('late interrupt failure'));
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    rpc.beforeTurnInterruptResponse = () => {
      rpc.emit('turn/cancelled', {
        threadId: 'thread_1',
        turnId: 'turn_1',
        status: 'cancelled',
      });
    };

    await expect(bridge.stopSession(session.id)).rejects.toThrow('late interrupt failure');

    expect(db.sessions.get(session.id)?.status).toBe('idle');
    expect(events).toContainEqual({
      type: 'session.status',
      payload: { sessionId: session.id, status: 'idle' },
    });
    expect(events).not.toContainEqual({
      type: 'session.status',
      payload: {
        sessionId: session.id,
        status: 'error',
        error: 'late interrupt failure',
      },
    });
  });

  it('does not reactivate a turn when terminal notifications arrive before the response', async () => {
    const rpc = new FakeRpc();
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = new DasclawAppServerSessionBridge(
      db,
      (event) => events.push(event),
      () => rpc
    );

    rpc.beforeTurnStartResponse = () => {
      rpc.emit('item/agentMessage/delta', {
        threadId: 'thread_1',
        turnId: 'turn_1',
        itemId: 'item_1',
        delta: 'hello before response',
      });
      rpc.emit('turn/completed', {
        threadId: 'thread_1',
        turnId: 'turn_1',
        status: 'completed',
        output: '',
      });
    };

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    await bridge.stopSession(session.id);

    expect(rpc.requests.map((request) => request.method)).not.toContain('turn/interrupt');
    expect(events).toEqual(
      expect.arrayContaining([
        {
          type: 'stream.partial',
          payload: { sessionId: session.id, delta: 'hello before response' },
        },
        expect.objectContaining({
          type: 'stream.message',
          payload: expect.objectContaining({
            sessionId: session.id,
            message: expect.objectContaining({
              role: 'assistant',
              content: [{ type: 'text', text: 'hello before response' }],
            }),
          }),
        }),
        {
          type: 'session.status',
          payload: { sessionId: session.id, status: 'idle' },
        },
      ])
    );
    expect(db.sessions.get(session.id)?.status).toBe('idle');
  });
});

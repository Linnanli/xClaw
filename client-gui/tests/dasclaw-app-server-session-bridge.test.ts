import { describe, expect, it, vi } from 'vitest';
import { DasclawAppServerSessionBridge } from '../src/main/dasclaw/app-server-session-bridge';
import type { AppServerRpc, JsonRpcNotification } from '../src/main/dasclaw/app-server-rpc';
import type {
  ModelProviderService,
  ResolvedModelProviderConfig,
} from '../src/main/dasclaw/model-provider-service';
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
  deferModelSelectionResponses = false;
  readonly modelSelectionsStarted: string[] = [];
  readonly modelSelectionsAcknowledged: string[] = [];
  failMethods = new Map<string, Error>();
  private readonly modelSelectionResolvers: Array<() => void> = [];
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
    if (method === 'modelProvider/selectForNextTurn') {
      const failure = this.failMethods.get(method);
      if (failure) throw failure;
      const modelId = stringParam(params, 'modelId');
      if (!modelId) {
        throw new Error('modelProvider/selectForNextTurn requires modelId');
      }
      this.modelSelectionsStarted.push(modelId);
      if (this.deferModelSelectionResponses) {
        await new Promise<void>((resolve) => this.modelSelectionResolvers.push(resolve));
      }
      this.modelSelectionsAcknowledged.push(modelId);
      return { selectedModelId: modelId } as T;
    }
    const failure = this.failMethods.get(method);
    if (failure) throw failure;
    return {} as T;
  }

  resolveNextModelSelection(): void {
    const resolve = this.modelSelectionResolvers.shift();
    if (!resolve) {
      throw new Error('No pending model selection response');
    }
    resolve();
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

async function waitUntil(assertion: () => void): Promise<void> {
  let lastError: unknown;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try {
      assertion();
      return;
    } catch (error) {
      lastError = error;
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }
  throw lastError;
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

const TEST_MODEL_PROVIDER_CONFIG: ResolvedModelProviderConfig = {
  runtime: {
    models: [
      {
        modelId: 'admin-gpt',
        displayName: 'Admin GPT',
        provider: 'openai',
        apiBaseUrl: 'https://admin.example/v1',
        apiKey: 'admin-secret-key',
        apiFormat: 'openai',
        source: 'admin',
        capabilities: ['chat'],
      },
      {
        modelId: 'admin-next',
        displayName: 'Admin Next',
        provider: 'openai',
        apiBaseUrl: 'https://admin-next.example/v1',
        apiKey: 'admin-next-secret',
        apiFormat: 'openai',
        source: 'admin',
        capabilities: ['chat', 'tools'],
      },
    ],
    selectedModel: {
      modelId: 'admin-gpt',
      displayName: 'Admin GPT',
      provider: 'openai',
      apiBaseUrl: 'https://admin.example/v1',
      apiKey: 'admin-secret-key',
      apiFormat: 'openai',
      source: 'admin',
      capabilities: ['chat'],
    },
  },
  renderer: {
    models: [
      {
        modelId: 'admin-gpt',
        displayName: 'Admin GPT',
        provider: 'openai',
        apiBaseUrl: 'https://admin.example/v1',
        apiFormat: 'openai',
        source: 'admin',
        capabilities: ['chat'],
        isDefault: true,
        apiKeyConfigured: true,
      },
      {
        modelId: 'admin-next',
        displayName: 'Admin Next',
        provider: 'openai',
        apiBaseUrl: 'https://admin-next.example/v1',
        apiFormat: 'openai',
        source: 'admin',
        capabilities: ['chat', 'tools'],
        isDefault: false,
        apiKeyConfigured: true,
      },
    ],
    selectedModelId: 'admin-gpt',
  },
};

function createModelProviderService(
  config: ResolvedModelProviderConfig = TEST_MODEL_PROVIDER_CONFIG
): ModelProviderService {
  return {
    load: vi.fn().mockResolvedValue(config),
  };
}

function createBridge(
  db: DatabaseInstance,
  sendToRenderer: (event: ServerEvent) => void,
  rpc: FakeRpc,
  modelProviderService: ModelProviderService = createModelProviderService()
): DasclawAppServerSessionBridge {
  return new DasclawAppServerSessionBridge(db, sendToRenderer, () => rpc, modelProviderService);
}

describe('DasclawAppServerSessionBridge', () => {
  it('starts a dasclaw thread and maps stream notifications to renderer events', async () => {
    const rpc = new FakeRpc();
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = createBridge(db, (event) => events.push(event), rpc);

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');

    expect(rpc.requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
    ]);
    expect(rpc.requests[0].params).toMatchObject({
      modelProvider: TEST_MODEL_PROVIDER_CONFIG.runtime,
    });
    expect(rpc.requests[2].params).toMatchObject({
      threadId: 'thread_1',
      input: [{ type: 'text', text: 'hello', text_elements: [] }],
    });
    expect(session.model).toBe('admin-gpt');
    expect(JSON.stringify(bridge.getRendererModelProviderConfig())).not.toContain(
      'admin-secret-key'
    );
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

  it('fails safe before app-server initialize when admin model config cannot load', async () => {
    const rpc = new FakeRpc();
    const db = createDb();
    const modelProviderService: ModelProviderService = {
      load: vi.fn().mockRejectedValue(new Error('admin model config unavailable')),
    };
    const bridge = createBridge(db, () => {}, rpc, modelProviderService);

    await expect(bridge.startSession('Draft', 'hello', '/tmp/workspace')).rejects.toThrow(
      'admin model config unavailable'
    );

    expect(rpc.requests).toEqual([]);
    expect(db.sessions.getAll()).toEqual([]);
  });

  it('selects a model locally before initialization and injects it into app-server initialize', async () => {
    const rpc = new FakeRpc();
    const db = createDb();
    const bridge = createBridge(db, () => {}, rpc);

    const selected = await bridge.selectModelForNextTurn('admin-next');
    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');

    expect(selected.selectedModelId).toBe('admin-next');
    expect(JSON.stringify(selected)).not.toContain('admin-next-secret');
    expect(rpc.requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
    ]);
    expect(rpc.requests[0].params).toMatchObject({
      modelProvider: {
        selectedModel: expect.objectContaining({ modelId: 'admin-next' }),
      },
    });
    expect(session.model).toBe('admin-next');
  });

  it('selects the next-turn model through app-server after initialization', async () => {
    const rpc = new FakeRpc();
    const db = createDb();
    const bridge = createBridge(db, () => {}, rpc);

    await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    const selected = await bridge.selectModelForNextTurn('admin-next');

    expect(selected.selectedModelId).toBe('admin-next');
    expect(rpc.requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
      'modelProvider/selectForNextTurn',
    ]);
    expect(rpc.requests[3]).toEqual({
      method: 'modelProvider/selectForNextTurn',
      params: { modelId: 'admin-next' },
    });
    expect(bridge.getRendererModelProviderConfig()?.selectedModelId).toBe('admin-next');
  });

  it('does not commit a model switch if app-server rejects the acknowledgement', async () => {
    const rpc = new FakeRpc();
    rpc.failMethods.set('modelProvider/selectForNextTurn', new Error('select failed'));
    const db = createDb();
    const bridge = createBridge(db, () => {}, rpc);

    await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    await expect(bridge.selectModelForNextTurn('admin-next')).rejects.toThrow('select failed');

    expect(bridge.getRendererModelProviderConfig()?.selectedModelId).toBe('admin-gpt');
    const nextSession = await bridge.startSession('Next Draft', 'again', '/tmp/workspace');
    expect(nextSession.model).toBe('admin-gpt');
  });

  it('serializes rapid model switches so the final acknowledged model wins', async () => {
    const rpc = new FakeRpc();
    rpc.deferModelSelectionResponses = true;
    const db = createDb();
    const bridge = createBridge(db, () => {}, rpc);

    await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    const firstSelection = bridge.selectModelForNextTurn('admin-next');
    const secondSelection = bridge.selectModelForNextTurn('admin-gpt');

    await waitUntil(() => expect(rpc.modelSelectionsStarted).toEqual(['admin-next']));

    rpc.resolveNextModelSelection();
    await waitUntil(() => expect(rpc.modelSelectionsStarted).toEqual(['admin-next', 'admin-gpt']));

    rpc.resolveNextModelSelection();
    const results = await Promise.all([firstSelection, secondSelection]);

    expect(results.map((result) => result.selectedModelId)).toEqual(['admin-next', 'admin-gpt']);
    expect(rpc.modelSelectionsAcknowledged).toEqual(['admin-next', 'admin-gpt']);
    expect(bridge.getRendererModelProviderConfig()?.selectedModelId).toBe('admin-gpt');
  });

  it('waits for pending model selection before starting a new session', async () => {
    const rpc = new FakeRpc();
    const db = createDb();
    const bridge = createBridge(db, () => {}, rpc);

    await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    rpc.deferModelSelectionResponses = true;
    const selection = bridge.selectModelForNextTurn('admin-next');
    await waitUntil(() => expect(rpc.modelSelectionsStarted).toEqual(['admin-next']));

    const secondSession = bridge.startSession('Second Draft', 'again', '/tmp/workspace');
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(rpc.requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
      'modelProvider/selectForNextTurn',
    ]);

    rpc.resolveNextModelSelection();
    const [, session] = await Promise.all([selection, secondSession]);

    expect(session.model).toBe('admin-next');
    expect(rpc.requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
      'modelProvider/selectForNextTurn',
      'thread/start',
      'turn/start',
    ]);
  });

  it('waits for pending model selection before continuing a session', async () => {
    const rpc = new FakeRpc();
    const db = createDb();
    const bridge = createBridge(db, () => {}, rpc);

    const session = await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    rpc.deferModelSelectionResponses = true;
    const selection = bridge.selectModelForNextTurn('admin-next');
    await waitUntil(() => expect(rpc.modelSelectionsStarted).toEqual(['admin-next']));

    const continuation = bridge.continueSession(session.id, 'again');
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(rpc.requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
      'modelProvider/selectForNextTurn',
    ]);

    rpc.resolveNextModelSelection();
    await Promise.all([selection, continuation]);

    expect(rpc.requests.map((request) => request.method)).toEqual([
      'initialize',
      'thread/start',
      'turn/start',
      'modelProvider/selectForNextTurn',
      'turn/start',
    ]);
  });

  it('rejects unknown model selections before contacting app-server', async () => {
    const rpc = new FakeRpc();
    const db = createDb();
    const bridge = createBridge(db, () => {}, rpc);

    await bridge.startSession('Draft', 'hello', '/tmp/workspace');
    await expect(bridge.selectModelForNextTurn('missing-model')).rejects.toThrow(
      'Unknown modelProvider modelId: missing-model'
    );

    expect(rpc.requests.map((request) => request.method)).not.toContain(
      'modelProvider/selectForNextTurn'
    );
    expect(bridge.getRendererModelProviderConfig()?.selectedModelId).toBe('admin-gpt');
  });

  it('does not duplicate partials when v2 item deltas are paired with legacy turn deltas', async () => {
    const rpc = new FakeRpc();
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = createBridge(db, (event) => events.push(event), rpc);

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
    const bridge = createBridge(db, () => {}, rpc);

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
    const bridge = createBridge(db, (event) => events.push(event), rpc);

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
    const bridge = createBridge(db, (event) => events.push(event), rpc);

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
      () => rpcs[rpcIndex++],
      createModelProviderService()
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
      () => rpcs[rpcIndex++],
      createModelProviderService()
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
    expect(
      db.messages.getBySessionId(session.id).filter((message) => message.role === 'user')
    ).toHaveLength(2);
    expect(events).toContainEqual({
      type: 'session.status',
      payload: { sessionId: session.id, status: 'running' },
    });
  });

  it('does not overwrite an error session when stopping after transport failure', async () => {
    const rpc = new FakeRpc();
    const events: ServerEvent[] = [];
    const db = createDb();
    const bridge = createBridge(db, (event) => events.push(event), rpc);

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
    const bridge = createBridge(db, (event) => events.push(event), rpc);

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
    const bridge = createBridge(db, (event) => events.push(event), rpc);

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
    const bridge = createBridge(db, (event) => events.push(event), rpc);

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
    const bridge = createBridge(db, (event) => events.push(event), rpc);

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

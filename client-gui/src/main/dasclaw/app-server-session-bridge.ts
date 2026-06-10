import { v4 as uuidv4 } from 'uuid';
import type { ContentBlock, Message, ServerEvent, Session } from '../../renderer/types';
import type { DatabaseInstance, MessageRow, SessionRow } from '../db/database';
import { configStore } from '../config/config-store';
import { log, logError } from '../utils/logger';
import { StdioAppServerRpc, type AppServerRpc, type JsonRpcNotification } from './app-server-rpc';

type AppServerRpcFactory = () => AppServerRpc;

interface ThreadStartResponse {
  threadId: string;
}

interface TurnStartResponse {
  turnId: string;
  status: 'pending' | 'completed' | 'failed' | 'cancelled';
}

interface SessionBinding {
  sessionId: string;
  threadId: string;
  activeTurnId?: string;
  activeOutput: string;
  terminalTurnIds: Set<string>;
  needsThreadRebind: boolean;
}

const CODEX_V2_PROFILE = 'codex_app_server_v2';
const WORKSPACE_MOUNT_VIRTUAL_PATH = '/mnt/workspace';

export class DasclawAppServerSessionBridge {
  private readonly rpcFactory: AppServerRpcFactory;
  private rpc: AppServerRpc | null = null;
  private initializePromise: Promise<void> | null = null;
  private removeNotificationListener: (() => void) | null = null;
  private removeTransportClosedListener: (() => void) | null = null;
  private readonly bindingsBySessionId = new Map<string, SessionBinding>();
  private readonly sessionIdByThreadId = new Map<string, string>();

  constructor(
    private readonly db: DatabaseInstance,
    private readonly sendToRenderer: (event: ServerEvent) => void,
    rpcFactory: AppServerRpcFactory = () => new StdioAppServerRpc()
  ) {
    this.rpcFactory = rpcFactory;
  }

  async startSession(
    title: string,
    prompt: string,
    cwd?: string,
    allowedTools?: string[],
    content?: ContentBlock[],
    memoryEnabled?: boolean
  ): Promise<Session> {
    const rpc = await this.ensureInitialized(cwd);
    const thread = await rpc.request<ThreadStartResponse>('thread/start', {
      title,
      workspaceRoot: cwd,
    });
    const session = this.createSession(thread.threadId, title, cwd, allowedTools, memoryEnabled);
    this.saveSession(session);
    this.bindSession(session.id, thread.threadId);
    this.saveUserMessage(session.id, prompt, content);
    this.sendStatus(session.id, 'running');
    await this.startTurn(session.id, thread.threadId, prompt);
    return session;
  }

  async continueSession(
    sessionId: string,
    prompt: string,
    content?: ContentBlock[]
  ): Promise<void> {
    const session = this.db.sessions.get(sessionId);
    if (!session) {
      throw new Error(`Unknown session: ${sessionId}`);
    }

    const storedThreadId = session.claude_session_id;
    if (!storedThreadId) {
      throw new Error(`Session ${sessionId} is not bound to a dasclaw thread`);
    }

    const rpc = await this.ensureInitialized(session.cwd ?? undefined);
    const binding = this.bindSession(sessionId, storedThreadId);
    const threadId = await this.ensureThreadForTurn(session, binding, rpc);
    this.saveUserMessage(sessionId, prompt, content);
    this.sendStatus(sessionId, 'running');
    await this.startTurn(sessionId, threadId, prompt, rpc);
  }

  async stopSession(sessionId: string): Promise<void> {
    const binding = this.bindingsBySessionId.get(sessionId);
    if (!binding?.activeTurnId) {
      if (this.db.sessions.get(sessionId)?.status === 'running') {
        this.sendStatus(sessionId, 'idle');
      }
      return;
    }

    const interruptTurnId = binding.activeTurnId;
    const rpc = await this.ensureInitialized();
    try {
      await rpc.request('turn/interrupt', {
        threadId: binding.threadId,
        turnId: interruptTurnId,
      });
    } catch (error) {
      if (this.isTurnStillActive(binding, interruptTurnId)) {
        this.failBinding(binding, toErrorMessage(error, 'Failed to interrupt dasclaw turn'));
      }
      throw error;
    }
  }

  dispose(): void {
    this.removeNotificationListener?.();
    this.removeNotificationListener = null;
    this.removeTransportClosedListener?.();
    this.removeTransportClosedListener = null;
    this.rpc?.dispose();
    this.rpc = null;
    this.initializePromise = null;
  }

  private async ensureInitialized(workspaceRoot?: string): Promise<AppServerRpc> {
    if (!this.rpc) {
      this.rpc = this.rpcFactory();
      this.removeNotificationListener = this.rpc.onNotification((notification) =>
        this.handleNotification(notification)
      );
      this.removeTransportClosedListener = this.rpc.onTransportClosed((error) =>
        this.handleTransportClosed(error)
      );
    }

    if (!this.initializePromise) {
      this.initializePromise = this.rpc
        .request('initialize', {
          client: {
            name: 'open-cowork-dasclaw-gui-poc',
            version: '0.0.0',
            transport: 'stdio',
          },
          protocolVersion: { major: 0, minor: 1, patch: 0 },
          workspace: workspaceRoot ? { root: workspaceRoot, trust: 'unknown' } : undefined,
          requestedCapabilities: [CODEX_V2_PROFILE],
        })
        .then(() => {
          log('[DasclawAppServer] Initialized app-server protocol session');
        })
        .catch((error) => {
          this.initializePromise = null;
          throw error;
        });
    }

    await this.initializePromise;
    return this.rpc;
  }

  private createSession(
    threadId: string,
    title: string,
    cwd?: string,
    allowedTools?: string[],
    memoryEnabled?: boolean
  ): Session {
    const now = Date.now();
    const resolvedMemoryEnabled =
      typeof memoryEnabled === 'boolean'
        ? memoryEnabled
        : configStore.get('memoryEnabled') !== false;

    return {
      id: uuidv4(),
      title,
      claudeSessionId: threadId,
      status: 'running',
      cwd,
      mountedPaths: cwd ? [{ virtual: WORKSPACE_MOUNT_VIRTUAL_PATH, real: cwd }] : [],
      allowedTools: allowedTools ?? [],
      memoryEnabled: resolvedMemoryEnabled,
      model: configStore.get('model') || undefined,
      createdAt: now,
      updatedAt: now,
    };
  }

  private saveSession(session: Session): void {
    const row: SessionRow = {
      id: session.id,
      title: session.title,
      claude_session_id: session.claudeSessionId ?? null,
      openai_thread_id: session.openaiThreadId ?? null,
      status: session.status,
      cwd: session.cwd ?? null,
      mounted_paths: JSON.stringify(session.mountedPaths),
      allowed_tools: JSON.stringify(session.allowedTools),
      memory_enabled: session.memoryEnabled ? 1 : 0,
      model: session.model ?? null,
      created_at: session.createdAt,
      updated_at: session.updatedAt,
    };
    this.db.sessions.create(row);
  }

  private saveUserMessage(sessionId: string, prompt: string, content?: ContentBlock[]): void {
    const message: Message = {
      id: uuidv4(),
      sessionId,
      role: 'user',
      content: content && content.length > 0 ? content : [{ type: 'text', text: prompt }],
      timestamp: Date.now(),
    };
    this.saveMessage(message);
  }

  private saveMessage(message: Message): void {
    const row: MessageRow = {
      id: message.id,
      session_id: message.sessionId,
      role: message.role,
      content: JSON.stringify(message.content),
      timestamp: message.timestamp,
      token_usage: message.tokenUsage ? JSON.stringify(message.tokenUsage) : null,
      execution_time_ms: message.executionTimeMs ?? null,
    };
    this.db.messages.create(row);
  }

  private bindSession(sessionId: string, threadId: string): SessionBinding {
    const existing = this.bindingsBySessionId.get(sessionId);
    if (existing) {
      return existing;
    }

    const binding: SessionBinding = {
      sessionId,
      threadId,
      activeOutput: '',
      terminalTurnIds: new Set(),
      needsThreadRebind: false,
    };
    this.bindingsBySessionId.set(sessionId, binding);
    this.sessionIdByThreadId.set(threadId, sessionId);
    return binding;
  }

  private async startTurn(
    sessionId: string,
    threadId: string,
    input: string,
    rpc = this.rpc
  ): Promise<void> {
    if (!rpc) {
      throw new Error('dasclaw app-server client is not initialized');
    }

    const binding = this.bindSession(sessionId, threadId);
    binding.activeOutput = '';
    const terminalTurnCount = binding.terminalTurnIds.size;

    let response: TurnStartResponse;
    try {
      response = await rpc.request<TurnStartResponse>('turn/start', {
        threadId,
        input,
      });
    } catch (error) {
      if (binding.terminalTurnIds.size === terminalTurnCount && !this.isSessionInError(sessionId)) {
        this.failBinding(binding, toErrorMessage(error, 'Failed to start dasclaw turn'));
      }
      throw error;
    }
    if (!binding.terminalTurnIds.has(response.turnId)) {
      binding.activeTurnId = response.turnId;
    }
  }

  private handleNotification(notification: JsonRpcNotification): void {
    const params = notification.params;
    if (!isRecord(params)) return;

    switch (notification.method) {
      case 'turn/started':
        this.handleTurnStarted(params);
        break;
      case 'item/agentMessage/delta':
        this.handleDelta(params);
        break;
      case 'turn/completed':
        this.handleCompleted(params);
        break;
      case 'turn/failed':
      case 'error':
        this.handleFailed(params);
        break;
      case 'turn/cancelled':
        this.handleCancelled(params);
        break;
    }
  }

  private handleTurnStarted(params: Record<string, unknown>): void {
    const binding = this.lookupBinding(params);
    const turnId = stringField(params, 'turnId');
    if (!binding || !turnId) return;
    if (binding.terminalTurnIds.has(turnId)) return;
    if (binding.activeTurnId !== turnId) {
      binding.activeTurnId = turnId;
      binding.activeOutput = '';
    }
    this.sendStatus(binding.sessionId, 'running');
  }

  private handleDelta(params: Record<string, unknown>): void {
    const binding = this.lookupBinding(params);
    const turnId = stringField(params, 'turnId');
    const delta = stringField(params, 'delta');
    if (!binding || !delta) return;
    if (turnId && binding.terminalTurnIds.has(turnId)) return;
    if (turnId && binding.activeTurnId !== turnId) {
      binding.activeTurnId = turnId;
      binding.activeOutput = '';
    }
    binding.activeOutput += delta;
    this.sendToRenderer({
      type: 'stream.partial',
      payload: { sessionId: binding.sessionId, delta },
    });
  }

  private handleCompleted(params: Record<string, unknown>): void {
    const binding = this.lookupBinding(params);
    if (!binding) return;

    const turnId = stringField(params, 'turnId');
    if (turnId && binding.terminalTurnIds.has(turnId)) return;

    const output = stringField(params, 'output') || binding.activeOutput;
    if (output) {
      const message: Message = {
        id: uuidv4(),
        sessionId: binding.sessionId,
        role: 'assistant',
        content: [{ type: 'text', text: output }],
        timestamp: Date.now(),
      };
      this.saveMessage(message);
      this.sendToRenderer({
        type: 'stream.message',
        payload: { sessionId: binding.sessionId, message },
      });
    }

    if (turnId) binding.terminalTurnIds.add(turnId);
    this.clearActiveTurn(binding);
    this.sendStatus(binding.sessionId, 'idle');
  }

  private handleFailed(params: Record<string, unknown>): void {
    const binding = this.lookupBinding(params);
    const turnId = stringField(params, 'turnId');
    const message = stringField(params, 'error') || stringField(params, 'message') || 'Turn failed';
    if (binding) {
      if (turnId) binding.terminalTurnIds.add(turnId);
      this.failBinding(binding, message);
    }
    this.sendToRenderer({ type: 'error', payload: { message } });
    logError('[DasclawAppServer] Turn failed:', message);
  }

  private handleCancelled(params: Record<string, unknown>): void {
    const binding = this.lookupBinding(params);
    if (!binding) return;
    const turnId = stringField(params, 'turnId');
    if (turnId) binding.terminalTurnIds.add(turnId);
    this.clearActiveTurn(binding);
    this.sendStatus(binding.sessionId, 'idle');
  }

  private lookupBinding(params: Record<string, unknown>): SessionBinding | undefined {
    const threadId = stringField(params, 'threadId');
    if (!threadId) return undefined;
    const sessionId = this.sessionIdByThreadId.get(threadId);
    if (!sessionId) return undefined;
    return this.bindingsBySessionId.get(sessionId);
  }

  private clearActiveTurn(binding: SessionBinding): void {
    binding.activeTurnId = undefined;
    binding.activeOutput = '';
  }

  private failBinding(binding: SessionBinding, message: string): void {
    this.clearActiveTurn(binding);
    this.sendStatus(binding.sessionId, 'error', message);
  }

  private async ensureThreadForTurn(
    session: SessionRow,
    binding: SessionBinding,
    rpc: AppServerRpc
  ): Promise<string> {
    if (!binding.needsThreadRebind) {
      return binding.threadId;
    }

    const thread = await rpc.request<ThreadStartResponse>('thread/start', {
      title: session.title,
      workspaceRoot: session.cwd ?? undefined,
    });
    this.sessionIdByThreadId.delete(binding.threadId);
    binding.threadId = thread.threadId;
    binding.needsThreadRebind = false;
    binding.activeTurnId = undefined;
    binding.activeOutput = '';
    binding.terminalTurnIds.clear();
    this.sessionIdByThreadId.set(thread.threadId, binding.sessionId);
    this.db.sessions.update(binding.sessionId, {
      claude_session_id: thread.threadId,
      updated_at: Date.now(),
    });
    this.sendToRenderer({
      type: 'session.update',
      payload: {
        sessionId: binding.sessionId,
        updates: { claudeSessionId: thread.threadId },
      },
    });
    return thread.threadId;
  }

  private isTurnStillActive(binding: SessionBinding, turnId: string): boolean {
    return binding.activeTurnId === turnId && !binding.terminalTurnIds.has(turnId);
  }

  private isSessionInError(sessionId: string): boolean {
    return this.db.sessions.get(sessionId)?.status === 'error';
  }

  private handleTransportClosed(error: Error): void {
    const message = toErrorMessage(error, 'dasclaw app-server transport closed');
    let failedSessions = 0;
    for (const binding of this.bindingsBySessionId.values()) {
      if (!binding.activeTurnId) continue;
      this.failBinding(binding, message);
      failedSessions += 1;
    }

    if (failedSessions > 0) {
      this.sendToRenderer({ type: 'error', payload: { message } });
      logError('[DasclawAppServer] Transport closed while turns were active:', message);
    }

    this.resetRpc();
  }

  private resetRpc(): void {
    for (const binding of this.bindingsBySessionId.values()) {
      binding.needsThreadRebind = true;
    }
    this.removeNotificationListener?.();
    this.removeNotificationListener = null;
    this.removeTransportClosedListener?.();
    this.removeTransportClosedListener = null;
    this.rpc = null;
    this.initializePromise = null;
  }

  private sendStatus(sessionId: string, status: Session['status'], error?: string): void {
    const updatedAt = Date.now();
    this.db.sessions.update(sessionId, { status, updated_at: updatedAt });
    this.sendToRenderer({
      type: 'session.status',
      payload: { sessionId, status, ...(error ? { error } : {}) },
    });
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function stringField(value: Record<string, unknown>, key: string): string | undefined {
  const field = value[key];
  return typeof field === 'string' ? field : undefined;
}

function toErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (typeof error === 'string' && error.trim()) {
    return error;
  }
  return fallback;
}

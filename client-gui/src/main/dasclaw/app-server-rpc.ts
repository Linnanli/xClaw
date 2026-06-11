import { spawn, type ChildProcessWithoutNullStreams } from 'child_process';
import { EventEmitter } from 'events';
import { existsSync } from 'fs';
import { join, resolve } from 'path';
import { app } from 'electron';
import { log, logError, logWarn } from '../utils/logger';

export interface JsonRpcNotification {
  jsonrpc: '2.0';
  method: string;
  params?: unknown;
}

interface JsonRpcSuccess<T> {
  jsonrpc: '2.0';
  id: string | number;
  result: T;
}

interface JsonRpcFailure {
  jsonrpc: '2.0';
  id: string | number | null;
  error: {
    code: number;
    message: string;
    data?: unknown;
  };
}

type JsonRpcLine<T = unknown> = JsonRpcSuccess<T> | JsonRpcFailure | JsonRpcNotification;

export interface AppServerRpc {
  request<T>(method: string, params?: unknown): Promise<T>;
  onNotification(listener: (notification: JsonRpcNotification) => void): () => void;
  onTransportClosed(listener: (error: Error) => void): () => void;
  dispose(): void;
}

type PendingRequest = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: NodeJS.Timeout;
};

export type AppServerProcessFactory = () => ChildProcessWithoutNullStreams;

const DEFAULT_REQUEST_TIMEOUT_MS = 30_000;
const BUNDLED_APP_SERVER_DIR = 'dasclaw-app-server';

export class StdioAppServerRpc implements AppServerRpc {
  private readonly child: ChildProcessWithoutNullStreams;
  private readonly pending = new Map<string | number, PendingRequest>();
  private readonly notifications = new EventEmitter();
  private readonly transportEvents = new EventEmitter();
  private nextId = 1;
  private stdoutBuffer = '';
  private disposed = false;

  constructor(
    processFactory: AppServerProcessFactory = spawnDefaultAppServer,
    private readonly requestTimeoutMs = DEFAULT_REQUEST_TIMEOUT_MS
  ) {
    this.child = processFactory();
    this.child.stdout.setEncoding('utf8');
    this.child.stderr.setEncoding('utf8');
    this.child.stdout.on('data', (chunk) => this.handleStdout(String(chunk)));
    this.child.stderr.on('data', (chunk) => {
      const text = String(chunk).trim();
      if (text) logWarn('[DasclawAppServer] stderr:', text);
    });
    this.child.on('exit', (code, signal) => {
      const reason = signal ? `signal ${signal}` : `code ${code ?? 'unknown'}`;
      this.handleTransportClosed(new Error(`dasclaw app-server exited with ${reason}`));
    });
    this.child.on('error', (error) => {
      this.handleTransportClosed(error);
    });
  }

  request<T>(method: string, params?: unknown): Promise<T> {
    if (this.disposed) {
      return Promise.reject(new Error('dasclaw app-server client is disposed'));
    }

    const id = `gui-${this.nextId++}`;
    const request = {
      jsonrpc: '2.0',
      id,
      method,
      ...(params === undefined ? {} : { params }),
    };

    return new Promise<T>((resolvePromise, rejectPromise) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        rejectPromise(new Error(`dasclaw app-server request timed out: ${method}`));
      }, this.requestTimeoutMs);
      this.pending.set(id, {
        resolve: (value) => resolvePromise(value as T),
        reject: rejectPromise,
        timer,
      });

      this.child.stdin.write(`${JSON.stringify(request)}\n`, (error) => {
        if (!error) return;
        const pending = this.pending.get(id);
        if (!pending) return;
        clearTimeout(pending.timer);
        this.pending.delete(id);
        pending.reject(error);
      });
    });
  }

  onNotification(listener: (notification: JsonRpcNotification) => void): () => void {
    this.notifications.on('notification', listener);
    return () => this.notifications.off('notification', listener);
  }

  onTransportClosed(listener: (error: Error) => void): () => void {
    this.transportEvents.on('closed', listener);
    return () => this.transportEvents.off('closed', listener);
  }

  dispose(): void {
    this.disposed = true;
    this.rejectAll(new Error('dasclaw app-server client disposed'));
    this.child.stdin.end();
    if (!this.child.killed) {
      this.child.kill();
    }
  }

  private handleStdout(chunk: string): void {
    this.stdoutBuffer += chunk;
    let newline = this.stdoutBuffer.indexOf('\n');
    while (newline !== -1) {
      const line = this.stdoutBuffer.slice(0, newline).trim();
      this.stdoutBuffer = this.stdoutBuffer.slice(newline + 1);
      if (line) this.handleLine(line);
      newline = this.stdoutBuffer.indexOf('\n');
    }
  }

  private handleLine(line: string): void {
    let message: JsonRpcLine;
    try {
      message = JSON.parse(line) as JsonRpcLine;
    } catch (error) {
      logError('[DasclawAppServer] Failed to parse JSON-RPC line:', error);
      return;
    }

    if ('method' in message && typeof message.method === 'string' && !('id' in message)) {
      this.notifications.emit('notification', message);
      return;
    }

    if (!('id' in message) || message.id === null) {
      logWarn('[DasclawAppServer] Ignoring JSON-RPC line without request id:', line);
      return;
    }

    const pending = this.pending.get(message.id);
    if (!pending) {
      logWarn('[DasclawAppServer] Ignoring response for unknown request id:', String(message.id));
      return;
    }

    clearTimeout(pending.timer);
    this.pending.delete(message.id);

    if ('error' in message && message.error) {
      pending.reject(new Error(message.error.message));
      return;
    }

    pending.resolve((message as JsonRpcSuccess<unknown>).result);
  }

  private rejectAll(error: Error): void {
    for (const [id, pending] of this.pending.entries()) {
      clearTimeout(pending.timer);
      pending.reject(error);
      this.pending.delete(id);
    }
  }

  private handleTransportClosed(error: Error): void {
    this.rejectAll(error);
    if (!this.disposed) {
      this.transportEvents.emit('closed', error);
    }
  }
}

export function spawnDefaultAppServer(): ChildProcessWithoutNullStreams {
  const explicitBinary = process.env.DASCLAW_APP_SERVER_BIN;
  const env = { ...process.env };
  if (explicitBinary) {
    log('[DasclawAppServer] Spawning explicit binary:', explicitBinary);
    return spawn(explicitBinary, [], { stdio: 'pipe', env });
  }

  if (app.isPackaged) {
    const bundledBinary = resolveBundledAppServerBinary(process.resourcesPath, process.platform);
    if (bundledBinary) {
      log('[DasclawAppServer] Spawning bundled binary:', bundledBinary);
      return spawn(bundledBinary, [], { stdio: 'pipe', env });
    }

    throw new Error(
      `Packaged dasclaw app-server binary was not found under ${join(
        process.resourcesPath,
        BUNDLED_APP_SERVER_DIR
      )}; set DASCLAW_APP_SERVER_BIN to override`
    );
  }

  const repoRoot = resolveDasclawRepoRoot();
  log('[DasclawAppServer] Spawning via cargo from:', repoRoot);
  return spawn(
    'cargo',
    ['run', '--quiet', '-p', 'dasclaw_app_server', '--bin', 'dasclaw-app-server'],
    {
      cwd: repoRoot,
      stdio: 'pipe',
      env,
    }
  );
}

export function resolveBundledAppServerBinary(
  resourcesPath: string,
  platform: NodeJS.Platform = process.platform
): string | null {
  const binaryName = platform === 'win32' ? 'dasclaw-app-server.exe' : 'dasclaw-app-server';
  const candidates = [
    join(resourcesPath, BUNDLED_APP_SERVER_DIR, binaryName),
    join(resourcesPath, BUNDLED_APP_SERVER_DIR, 'bin', binaryName),
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function resolveDasclawRepoRoot(): string {
  if (process.env.DASCLAW_REPO_ROOT) {
    return process.env.DASCLAW_REPO_ROOT;
  }

  if (!app.isPackaged) {
    return resolve(join(__dirname, '..', '..', '..'));
  }

  return resolve(process.resourcesPath);
}

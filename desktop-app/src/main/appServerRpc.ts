import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { createInterface } from 'node:readline'

export type JsonRpcId = number | string

export type JsonRpcResponse = {
  type: 'response'
  id: JsonRpcId
  result?: unknown
  error?: { code?: number; message?: string; data?: unknown }
}

export type JsonRpcNotification = {
  type: 'notification'
  method: string
  params?: unknown
}

export type JsonRpcServerRequest = {
  type: 'server-request'
  id: JsonRpcId
  method: string
  params?: unknown
}

export type JsonRpcMessage = JsonRpcResponse | JsonRpcNotification | JsonRpcServerRequest

export type AppServerRpcClient = {
  request<T>(method: string, params?: unknown): Promise<T>
  onNotification(handler: (notification: JsonRpcNotification) => void): () => void
  onServerRequest(handler: (request: JsonRpcServerRequest) => void): () => void
  respond(id: JsonRpcId, result: unknown): void
  dispose(): void
}

export type AppServerLaunchOptions = {
  command: string
  args: string[]
  cwd?: string
  displayBinary: string
  env?: NodeJS.ProcessEnv
}

type PendingRequest = {
  resolve: (value: unknown) => void
  reject: (error: Error) => void
}

type DefaultAppServerLaunchOptionsInput = {
  env?: NodeJS.ProcessEnv
  isPackaged?: boolean
  mainDir?: string
  platform?: NodeJS.Platform
  resourcesPath?: string
}

const BUNDLED_APP_SERVER_DIR = 'dasclaw-app-server'
const CARGO_APP_SERVER_ARGS = [
  'run',
  '--quiet',
  '-p',
  'dasclaw_app_server',
  '--bin',
  'dasclaw-app-server'
]

export function resolveBundledAppServerBinary(
  resourcesPath: string,
  platform: NodeJS.Platform = process.platform
): string | null {
  const binaryName = platform === 'win32' ? 'dasclaw-app-server.exe' : 'dasclaw-app-server'
  const candidates = [
    join(resourcesPath, BUNDLED_APP_SERVER_DIR, binaryName),
    join(resourcesPath, BUNDLED_APP_SERVER_DIR, 'bin', binaryName)
  ]

  return candidates.find((candidate) => existsSync(candidate)) ?? null
}

export function resolveDefaultAppServerLaunchOptions(
  options: DefaultAppServerLaunchOptionsInput = {}
): AppServerLaunchOptions {
  const env = options.env ?? process.env
  const explicitBinary = env.DASCLAW_APP_SERVER_BIN
  if (explicitBinary) {
    return {
      command: explicitBinary,
      args: [],
      displayBinary: explicitBinary,
      env
    }
  }

  const platform = options.platform ?? process.platform
  if (options.isPackaged) {
    const resourcesPath = options.resourcesPath ?? process.resourcesPath
    const bundledBinary = resolveBundledAppServerBinary(resourcesPath, platform)
    if (!bundledBinary) {
      throw new Error(
        `Packaged dasclaw-app-server binary was not found under ${join(
          resourcesPath,
          BUNDLED_APP_SERVER_DIR
        )}; set DASCLAW_APP_SERVER_BIN to override`
      )
    }

    return {
      command: bundledBinary,
      args: [],
      displayBinary: bundledBinary,
      env
    }
  }

  return {
    command: 'cargo',
    args: [...CARGO_APP_SERVER_ARGS],
    cwd: resolveDasclawRepoRoot(options.mainDir ?? __dirname, env),
    displayBinary: `cargo ${CARGO_APP_SERVER_ARGS.join(' ')}`,
    env
  }
}

function resolveDasclawRepoRoot(mainDir: string, env: NodeJS.ProcessEnv): string {
  return env.DASCLAW_REPO_ROOT ?? resolve(mainDir, '..', '..', '..')
}

export function buildJsonRpcRequestLine(id: JsonRpcId, method: string, params?: unknown): string {
  const request = {
    jsonrpc: '2.0',
    id,
    method,
    ...(params === undefined ? {} : { params })
  }

  return `${JSON.stringify(request)}\n`
}

export function buildJsonRpcResponseLine(id: JsonRpcId, result: unknown): string {
  return `${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`
}

export function classifyJsonRpcMessage(raw: unknown): JsonRpcMessage {
  if (!raw || typeof raw !== 'object') {
    throw new Error('app-server emitted a non-object JSON-RPC message')
  }

  const message = raw as Record<string, unknown>
  if (typeof message.method === 'string') {
    if (typeof message.id === 'string' || typeof message.id === 'number') {
      return {
        type: 'server-request',
        id: message.id,
        method: message.method,
        params: message.params
      }
    }

    return {
      type: 'notification',
      method: message.method,
      params: message.params
    }
  }

  if (typeof message.id === 'string' || typeof message.id === 'number') {
    return {
      type: 'response',
      id: message.id,
      result: message.result,
      error: message.error as JsonRpcResponse['error']
    }
  }

  throw new Error('app-server emitted JSON-RPC without method or id')
}

export class JsonRpcLineParser {
  private buffered = ''

  push(chunk: string): unknown[] {
    this.buffered += chunk

    const lines = this.buffered.split(/\r?\n/)
    this.buffered = lines.pop() ?? ''

    return lines.filter(Boolean).map((line) => JSON.parse(line))
  }
}

export class ChildProcessAppServerRpcClient implements AppServerRpcClient {
  private nextId = 1
  private readonly pending = new Map<JsonRpcId, PendingRequest>()
  private readonly notificationHandlers = new Set<(notification: JsonRpcNotification) => void>()
  private readonly serverRequestHandlers = new Set<(request: JsonRpcServerRequest) => void>()
  private readonly child: ChildProcessWithoutNullStreams
  private stderr = ''

  constructor(launchOptions: string | AppServerLaunchOptions) {
    const launch =
      typeof launchOptions === 'string'
        ? {
            command: launchOptions,
            args: [],
            displayBinary: launchOptions,
            env: process.env
          }
        : launchOptions

    this.child = spawn(launch.command, launch.args, {
      cwd: launch.cwd,
      stdio: ['pipe', 'pipe', 'pipe'],
      env: launch.env ?? process.env
    })

    const stdout = createInterface({ input: this.child.stdout })
    stdout.on('line', (line) => this.handleLine(line))
    this.child.stderr.on('data', (chunk: Buffer) => {
      this.stderr += chunk.toString('utf8')
    })
    this.child.once('error', (error) => this.rejectAll(error))
    this.child.once('exit', (code, signal) => {
      this.rejectAll(
        new Error(
          `dasclaw-app-server exited before responding (code=${code ?? 'none'}, signal=${
            signal ?? 'none'
          })${this.stderr ? `: ${this.stderr.trim()}` : ''}`
        )
      )
    })
  }

  get pid(): number | undefined {
    return this.child.pid
  }

  request<T>(method: string, params?: unknown): Promise<T> {
    const id = this.nextId++
    const line = buildJsonRpcRequestLine(id, method, params)

    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (value: unknown) => void, reject })
      this.child.stdin.write(line, 'utf8', (error) => {
        if (!error) return
        this.pending.delete(id)
        reject(error)
      })
    })
  }

  onNotification(handler: (notification: JsonRpcNotification) => void): () => void {
    this.notificationHandlers.add(handler)
    return () => this.notificationHandlers.delete(handler)
  }

  onServerRequest(handler: (request: JsonRpcServerRequest) => void): () => void {
    this.serverRequestHandlers.add(handler)
    return () => this.serverRequestHandlers.delete(handler)
  }

  respond(id: JsonRpcId, result: unknown): void {
    this.child.stdin.write(buildJsonRpcResponseLine(id, result), 'utf8')
  }

  dispose(): void {
    this.child.stdin.destroy()
    this.child.kill()
  }

  private handleLine(line: string): void {
    const message = classifyJsonRpcMessage(JSON.parse(line))
    if (message.type === 'notification') {
      for (const handler of this.notificationHandlers) handler(message)
      return
    }

    if (message.type === 'server-request') {
      for (const handler of this.serverRequestHandlers) handler(message)
      return
    }

    const pending = this.pending.get(message.id)
    if (!pending) return

    this.pending.delete(message.id)
    if (message.error) {
      pending.reject(new Error(message.error.message ?? 'app-server returned an error'))
    } else {
      pending.resolve(message.result)
    }
  }

  private rejectAll(error: Error): void {
    for (const pending of this.pending.values()) pending.reject(error)
    this.pending.clear()
  }
}

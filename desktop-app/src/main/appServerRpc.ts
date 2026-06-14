import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
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

export type JsonRpcMessage = JsonRpcResponse | JsonRpcNotification

export type AppServerRpcClient = {
  request<T>(method: string, params?: unknown): Promise<T>
  onNotification(handler: (notification: JsonRpcNotification) => void): () => void
  dispose(): void
}

type PendingRequest = {
  resolve: (value: unknown) => void
  reject: (error: Error) => void
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

export function classifyJsonRpcMessage(raw: unknown): JsonRpcMessage {
  if (!raw || typeof raw !== 'object') {
    throw new Error('app-server emitted a non-object JSON-RPC message')
  }

  const message = raw as Record<string, unknown>
  if (typeof message.method === 'string') {
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
  private readonly child: ChildProcessWithoutNullStreams
  private stderr = ''

  constructor(binary: string) {
    this.child = spawn(binary, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: process.env
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

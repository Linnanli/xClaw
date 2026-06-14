import {
  ChildProcessAppServerRpcClient,
  type AppServerRpcClient,
  type JsonRpcNotification
} from './appServerRpc'
import type { AppServerRunState, AppServerStatus, ChatSendResponse } from '../shared/appServerApi'

export type { AppServerRunState, AppServerStatus, ChatSendResponse }

type TurnCompletion = {
  threadId: string
  turnId: string
  output?: string
  error?: string
}

type AppServerManagerOptions = {
  binary?: string
  createClient?: () => AppServerRpcClient
}

const DEFAULT_BINARY = process.env.DASCLAW_APP_SERVER_BIN ?? 'dasclaw-app-server'
const TURN_COMPLETION_TIMEOUT_MS = 120_000

export class AppServerManager {
  private client: AppServerRpcClient | undefined
  private unsubscribeNotifications: (() => void) | undefined
  private status: AppServerStatus
  private readonly createClient: () => AppServerRpcClient
  private readonly statusListeners = new Set<(status: AppServerStatus) => void>()
  private readonly pendingTurns = new Map<
    string,
    { resolve: (completion: TurnCompletion) => void; reject: (error: Error) => void }
  >()
  private readonly completedTurns = new Map<string, TurnCompletion>()

  constructor(options: AppServerManagerOptions = {}) {
    const binary = options.binary ?? DEFAULT_BINARY
    this.createClient =
      options.createClient ??
      (() => {
        const client = new ChildProcessAppServerRpcClient(binary)
        this.status = { ...this.status, pid: client.pid }
        return client
      })
    this.status = {
      state: 'stopped',
      binary,
      notificationCount: 0
    }
  }

  getStatus(): AppServerStatus {
    return { ...this.status }
  }

  onStatusChange(listener: (status: AppServerStatus) => void): () => void {
    this.statusListeners.add(listener)
    listener(this.getStatus())
    return () => this.statusListeners.delete(listener)
  }

  async start(): Promise<AppServerStatus> {
    if (this.client && this.status.state === 'ready') return this.getStatus()

    this.setStatus({ state: 'starting', lastError: undefined })
    this.client = this.createClient()
    this.unsubscribeNotifications = this.client.onNotification((notification) =>
      this.handleNotification(notification)
    )

    try {
      await this.client.request('initialize', {
        client: { name: 'desktop-app', version: '0.1.0', transport: 'stdio' },
        protocolVersion: { major: 0, minor: 1, patch: 0 },
        requestedCapabilities: ['protocol', 'lifecycle', 'health', 'session', 'codex_app_server_v2']
      })
      const health = await this.client.request('health/check', { includeDetails: true })
      this.setStatus({
        state: 'ready',
        startedAt: this.status.startedAt ?? new Date().toISOString(),
        lastHealth: health
      })
    } catch (error) {
      this.unsubscribeNotifications?.()
      this.client.dispose()
      this.client = undefined
      this.unsubscribeNotifications = undefined
      this.fail(error)
    }

    return this.getStatus()
  }

  async stop(): Promise<AppServerStatus> {
    if (!this.client) {
      this.setStatus({ state: 'stopped' })
      return this.getStatus()
    }

    this.setStatus({ state: 'stopping' })
    try {
      await this.client.request('shutdown', { reason: 'client_exit' })
    } catch {
      // Shutdown is best-effort; dispose below still owns process cleanup.
    } finally {
      this.rejectPendingTurns(new Error('dasclaw-app-server 已停止'))
      this.unsubscribeNotifications?.()
      this.client.dispose()
      this.client = undefined
      this.unsubscribeNotifications = undefined
      this.setStatus({ state: 'stopped', pid: undefined })
    }

    return this.getStatus()
  }

  async checkHealth(): Promise<AppServerStatus> {
    await this.ensureReady()
    this.setStatus({ state: 'checking' })
    try {
      const health = await this.requireClient().request('health/check', { includeDetails: true })
      this.setStatus({ state: 'ready', lastHealth: health })
    } catch (error) {
      this.fail(error)
    }
    return this.getStatus()
  }

  async sendMessage(prompt: string): Promise<ChatSendResponse> {
    const trimmedPrompt = prompt.trim()
    if (!trimmedPrompt) throw new Error('请输入消息内容')

    await this.ensureReady()
    const client = this.requireClient()
    const threadId = await this.ensureThread(client, trimmedPrompt)
    const started = await client.request<{ turnId: string }>('turn/start', {
      threadId,
      input: [{ type: 'text', text: trimmedPrompt }]
    })

    const completion = await this.waitForTurnCompletion(started.turnId)
    if (completion.error) throw new Error(completion.error)

    return {
      threadId: completion.threadId,
      turnId: completion.turnId,
      output: completion.output ?? ''
    }
  }

  private async ensureReady(): Promise<void> {
    if (this.status.state === 'ready' && this.client) return
    const status = await this.start()
    if (status.state !== 'ready') {
      throw new Error(status.lastError ?? 'dasclaw-app-server 未就绪')
    }
  }

  private async ensureThread(client: AppServerRpcClient, prompt: string): Promise<string> {
    if (this.status.threadId) return this.status.threadId

    const title = prompt.length > 48 ? `${prompt.slice(0, 45)}...` : prompt
    const response = await client.request<{ threadId: string }>('thread/start', { title })
    this.setStatus({ threadId: response.threadId })
    return response.threadId
  }

  private waitForTurnCompletion(turnId: string): Promise<TurnCompletion> {
    const completed = this.completedTurns.get(turnId)
    if (completed) {
      this.completedTurns.delete(turnId)
      return Promise.resolve(completed)
    }

    return new Promise<TurnCompletion>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingTurns.delete(turnId)
        reject(new Error('等待 dasclaw-app-server 响应超时'))
      }, TURN_COMPLETION_TIMEOUT_MS)

      this.pendingTurns.set(turnId, {
        resolve: (completion) => {
          clearTimeout(timeout)
          resolve(completion)
        },
        reject: (error) => {
          clearTimeout(timeout)
          reject(error)
        }
      })
    })
  }

  private handleNotification(notification: JsonRpcNotification): void {
    this.setStatus({
      notificationCount: this.status.notificationCount + 1,
      lastNotification: notification
    })

    if (notification.method === 'turn/completed' || notification.method === 'turn/failed') {
      const params = notification.params as Partial<TurnCompletion> | undefined
      if (!params?.turnId || !params.threadId) return

      const completion: TurnCompletion = {
        threadId: params.threadId,
        turnId: params.turnId,
        output: params.output,
        error: params.error
      }
      const pending = this.pendingTurns.get(params.turnId)
      if (pending) {
        this.pendingTurns.delete(params.turnId)
        pending.resolve(completion)
      } else {
        this.completedTurns.set(params.turnId, completion)
      }
    }
  }

  private requireClient(): AppServerRpcClient {
    if (!this.client) throw new Error('dasclaw-app-server client is not started')
    return this.client
  }

  private fail(error: unknown): void {
    this.setStatus({
      state: 'failed',
      lastError: error instanceof Error ? error.message : String(error)
    })
  }

  private rejectPendingTurns(error: Error): void {
    for (const pending of this.pendingTurns.values()) pending.reject(error)
    this.pendingTurns.clear()
  }

  private setStatus(patch: Partial<AppServerStatus>): void {
    this.status = { ...this.status, ...patch }
    const status = this.getStatus()
    for (const listener of this.statusListeners) listener(status)
  }
}

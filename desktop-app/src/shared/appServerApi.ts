export type AppServerRunState =
  | 'stopped'
  | 'starting'
  | 'ready'
  | 'checking'
  | 'stopping'
  | 'failed'

export type AppServerStatus = {
  state: AppServerRunState
  binary: string
  pid?: number
  startedAt?: string
  lastError?: string
  notificationCount: number
  lastNotification?: unknown
  lastHealth?: unknown
  threadId?: string
}

export type ChatSendResponse = {
  threadId: string
  turnId: string
  output: string
}

export type DesktopAppServerApi = {
  start(): Promise<AppServerStatus>
  stop(): Promise<AppServerStatus>
  getStatus(): Promise<AppServerStatus>
  checkHealth(): Promise<AppServerStatus>
  sendMessage(prompt: string): Promise<ChatSendResponse>
  onStatusChange(callback: (status: AppServerStatus) => void): () => void
}

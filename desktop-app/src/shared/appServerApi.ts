export type AppServerRunState =
  | 'stopped'
  | 'starting'
  | 'ready'
  | 'checking'
  | 'stopping'
  | 'failed'

export type AppServerStatus = {
  state: AppServerRunState
  hostId: string
  binary: string
  pid?: number
  startedAt?: string
  lastError?: string
  notificationCount: number
  lastNotification?: unknown
  lastHealth?: unknown
  threadId?: string
}

export type AppServerRequestOptions = {
  hostId?: string
}

export type AppServerNotification = {
  hostId: string
  method: string
  params?: unknown
}

export type RendererClientModelConfig = {
  modelId: string
  displayName: string
  description?: string
  provider: string
  apiBaseUrl: string
  apiFormat: string
  modelCallMode: string
  source: string
  capabilities: string[]
  apiKeyConfigured: boolean
}

export type RendererModelProviderConfig = {
  models: RendererClientModelConfig[]
  selectedModelId?: string
  unavailableReason?: string
}

export type ModelProviderSelectForNextTurnResponse = {
  selectedModelId: string
}

export type DesktopAppServerApi = {
  request<T = unknown>(
    method: string,
    params?: unknown,
    options?: AppServerRequestOptions
  ): Promise<T>
  stop(): Promise<AppServerStatus>
  getStatus(): Promise<AppServerStatus>
  checkHealth(): Promise<AppServerStatus>
  onStatusChange(callback: (status: AppServerStatus) => void): () => void
  onNotification(callback: (notification: AppServerNotification) => void): () => void
}

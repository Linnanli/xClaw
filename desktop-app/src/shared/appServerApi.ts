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

export type AppServerApprovalDecision =
  | { kind: 'approve' }
  | { kind: 'approve_always' }
  | { kind: 'reject'; data?: { reason?: string } }

export type AppServerServerRequestMethod =
  | 'item/commandExecution/requestApproval'
  | 'item/permissions/requestApproval'
  | 'item/fileChange/requestApproval'
  | 'item/tool/requestUserInput'
  | 'item/tool/call'

export type AppServerServerRequest = {
  requestId: string | number
  hostId: string
  method: AppServerServerRequestMethod
  params: Record<string, unknown>
}

export type AppServerApprovalRequest = AppServerServerRequest & {
  method: 'item/commandExecution/requestApproval' | 'item/permissions/requestApproval'
}

export type AppServerApprovalRespondParams = {
  requestId: string | number
  decision: AppServerApprovalDecision
}

export type AppServerServerRequestResponse =
  | { decision: AppServerApprovalDecision }
  | { decision: 'accept' | 'acceptForSession' | 'decline' | 'cancel' }
  | {
      contentItems: Array<
        { type: 'inputText'; text: string } | { type: 'inputImage'; imageUrl: string }
      >
      success: boolean
    }
  | { answers: Record<string, { answers: string[] }> }
  | { permissions: unknown; scope?: 'turn' | 'session'; strictAutoReview?: boolean }

export type AppServerGenericNotification = {
  hostId: string
  method: string
  params?: unknown
}

export type AppServerNotification = AppServerGenericNotification | AppServerServerRequest

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
  respondServerRequest(
    requestId: string | number,
    response: AppServerServerRequestResponse,
    options?: AppServerRequestOptions
  ): Promise<void>
  stop(): Promise<AppServerStatus>
  getStatus(): Promise<AppServerStatus>
  checkHealth(): Promise<AppServerStatus>
  onStatusChange(callback: (status: AppServerStatus) => void): () => void
  onNotification(callback: (notification: AppServerNotification) => void): () => void
}

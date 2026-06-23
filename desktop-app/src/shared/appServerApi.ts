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

export type AppServerCommandApprovalParams = {
  threadId: string
  turnId: string
  itemId: string
  toolCallId: string
  toolName: string
  command?: string
  description: string
  displayParameters: unknown
  allowAlways: boolean
}

export type AppServerPermissionsApprovalParams = {
  threadId: string
  turnId: string
  itemId: string
  cwd: string
  reason?: string
  permissions: unknown
}

export type AppServerFileChangeApprovalParams = {
  threadId: string
  turnId: string
  itemId: string
  reason?: string
  grantRoot?: string
}

export type AppServerToolUserInputQuestionOption = {
  label: string
  description: string
}

export type AppServerToolUserInputQuestion = {
  id: string
  header: string
  question: string
  isOther?: boolean
  isSecret?: boolean
  options?: AppServerToolUserInputQuestionOption[]
}

export type AppServerToolUserInputParams = {
  threadId: string
  turnId: string
  itemId: string
  questions: AppServerToolUserInputQuestion[]
}

export type AppServerDynamicToolCallParams = {
  threadId: string
  turnId: string
  callId: string
  namespace?: string
  tool: string
  arguments: unknown
}

export type AppServerServerRequestParamsByMethod = {
  'item/commandExecution/requestApproval': AppServerCommandApprovalParams
  'item/permissions/requestApproval': AppServerPermissionsApprovalParams
  'item/fileChange/requestApproval': AppServerFileChangeApprovalParams
  'item/tool/requestUserInput': AppServerToolUserInputParams
  'item/tool/call': AppServerDynamicToolCallParams
}

export type AppServerServerRequest<
  Method extends AppServerServerRequestMethod = AppServerServerRequestMethod
> = {
  [M in Method]: {
    requestId: string | number
    hostId: string
    method: M
    params: AppServerServerRequestParamsByMethod[M]
  }
}[Method]

export type AppServerApprovalRequest = AppServerServerRequest<
  'item/commandExecution/requestApproval' | 'item/permissions/requestApproval'
>

export type AppServerToolUserInputResponse = {
  answers: Record<string, { answers: string[] }>
}

export type AppServerDynamicToolCallResponse = {
  contentItems: Array<
    { type: 'inputText'; text: string } | { type: 'inputImage'; imageUrl: string }
  >
  success: boolean
}

export type AppServerCommandApprovalResponse = {
  decision: AppServerApprovalDecision
}

export type AppServerFileChangeApprovalResponse = {
  decision: 'accept' | 'acceptForSession' | 'decline' | 'cancel'
}

export type AppServerPermissionsApprovalDecision = 'approve' | 'reject'

export type AppServerPermissionsApprovalResponse = {
  decision: AppServerPermissionsApprovalDecision
  permissions: unknown
  scope?: 'turn' | 'session'
  strictAutoReview?: boolean
}

export type AppServerApprovalRespondParams = {
  requestId: string | number
  decision: AppServerApprovalDecision
}

export type AppServerServerRequestResponseByMethod = {
  'item/commandExecution/requestApproval': AppServerCommandApprovalResponse
  'item/permissions/requestApproval': AppServerPermissionsApprovalResponse
  'item/fileChange/requestApproval': AppServerFileChangeApprovalResponse
  'item/tool/requestUserInput': AppServerToolUserInputResponse
  'item/tool/call': AppServerDynamicToolCallResponse
}

export type AppServerServerRequestResponse<
  Method extends AppServerServerRequestMethod = AppServerServerRequestMethod
> = AppServerServerRequestResponseByMethod[Method]

export type AppServerGenericNotification = {
  hostId: string
  method: string
  params?: unknown
  requestId?: never
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
  openExternalHttpUrl(url: string): Promise<void>
  onStatusChange(callback: (status: AppServerStatus) => void): () => void
  onNotification(callback: (notification: AppServerNotification) => void): () => void
}

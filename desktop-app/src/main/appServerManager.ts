import {
  ChildProcessAppServerRpcClient,
  type AppServerRpcClient,
  type JsonRpcNotification,
  type JsonRpcServerRequest,
  resolveDefaultAppServerLaunchOptions
} from './appServerRpc'
import { app } from 'electron'
import type {
  AppServerApprovalRespondParams,
  AppServerNotification,
  AppServerRequestOptions,
  AppServerRunState,
  AppServerServerRequest,
  AppServerServerRequestMethod,
  AppServerServerRequestParamsByMethod,
  AppServerServerRequestResponse,
  AppServerStatus,
  ModelProviderSelectForNextTurnResponse,
  RendererModelProviderConfig
} from '../shared/appServerApi'

export type { AppServerNotification, AppServerRequestOptions, AppServerRunState, AppServerStatus }

type AppServerManagerOptions = {
  hostId?: string
  binary?: string
  createClient?: () => AppServerRpcClient
  loadModelProviderConfig?: ModelProviderConfigLoader
}

const LOCAL_HOST_ID = 'local'
const DEFAULT_ADMIN_BACKEND_URL = 'http://localhost:3000'
const DEFAULT_MODEL_PROVIDER_FETCH_TIMEOUT_MS = 5000
const DEFAULT_MODEL_CALL_MODE = 'stream'

export type AdminClientModelConfig = {
  model_id: string
  display_name: string
  provider: string
  is_default: boolean
  description?: string | null
  capabilities?: unknown
  api_base_url?: string | null
  api_key?: string | null
  api_format?: string | null
  model_call_mode?: string | null
  source?: string | null
}

export type AppServerClientModelConfig = {
  modelId: string
  displayName?: string
  description?: string
  provider: string
  apiBaseUrl: string
  apiKey: string
  apiFormat: string
  modelCallMode: string
  source?: string
  capabilities: string[]
}

export type AppServerModelProviderConfig = {
  models: AppServerClientModelConfig[]
  selectedModel: AppServerClientModelConfig
}

type CodexModelListResponse = {
  data: CodexModel[]
  nextCursor: string | null
}

type CodexModel = {
  id: string
  model: string
  displayName: string
  description: string
  hidden: boolean
  inputModalities: string[]
  isDefault: boolean
}

export type ModelProviderConfigLoader = () => Promise<AppServerModelProviderConfig>

export class ModelProviderConfigError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ModelProviderConfigError'
  }
}

export class AppServerManager {
  private client: AppServerRpcClient | undefined
  private startPromise: Promise<AppServerStatus> | undefined
  private unsubscribeNotifications: (() => void) | undefined
  private unsubscribeServerRequests: (() => void) | undefined
  private status: AppServerStatus
  private readonly createClient: () => AppServerRpcClient
  private readonly loadModelProviderConfig: ModelProviderConfigLoader
  private readonly hostId: string
  private readonly statusListeners = new Set<(status: AppServerStatus) => void>()
  private readonly notificationListeners = new Set<(notification: AppServerNotification) => void>()
  private modelProviderConfig: AppServerModelProviderConfig | undefined

  constructor(options: AppServerManagerOptions = {}) {
    this.hostId = options.hostId ?? LOCAL_HOST_ID
    this.createClient =
      options.createClient ??
      (() => {
        const launchOptions = options.binary
          ? { command: options.binary, args: [], displayBinary: options.binary, env: process.env }
          : resolveDefaultAppServerLaunchOptions({
              env: process.env,
              isPackaged: app.isPackaged,
              mainDir: __dirname,
              platform: process.platform,
              resourcesPath: process.resourcesPath
            })
        const client = new ChildProcessAppServerRpcClient(launchOptions)
        this.status = { ...this.status, binary: launchOptions.displayBinary, pid: client.pid }
        return client
      })
    this.loadModelProviderConfig =
      options.loadModelProviderConfig ?? createModelProviderConfigLoader()
    this.status = {
      state: 'stopped',
      hostId: this.hostId,
      binary: options.binary ?? process.env.DASCLAW_APP_SERVER_BIN ?? 'dasclaw-app-server',
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

  onNotification(listener: (notification: AppServerNotification) => void): () => void {
    this.notificationListeners.add(listener)
    return () => this.notificationListeners.delete(listener)
  }

  async start(): Promise<AppServerStatus> {
    if (this.client && this.status.state === 'ready') return this.getStatus()
    if (this.startPromise) return this.startPromise

    this.startPromise = this.startConnection().finally(() => {
      this.startPromise = undefined
    })
    return this.startPromise
  }

  preconnect(): Promise<AppServerStatus> {
    return this.start()
  }

  private async startConnection(): Promise<AppServerStatus> {
    this.setStatus({ state: 'starting', lastError: undefined })
    try {
      this.client = this.createClient()
      this.unsubscribeNotifications = this.client.onNotification((notification) =>
        this.handleNotification(notification)
      )
      this.unsubscribeServerRequests = this.client.onServerRequest((request) =>
        this.handleServerRequest(request)
      )
      const modelProvider = await this.getModelProviderConfig()
      await this.client.request('initialize', {
        client: { name: 'desktop-app', version: '0.1.0', transport: 'stdio' },
        protocolVersion: { major: 0, minor: 1, patch: 0 },
        requestedCapabilities: ['protocol', 'lifecycle', 'health', 'session'],
        modelProvider
      })
      const health = await this.client.request('health/check', { includeDetails: true })
      this.setStatus({
        state: 'ready',
        startedAt: this.status.startedAt ?? new Date().toISOString(),
        lastHealth: health
      })
    } catch (error) {
      this.unsubscribeNotifications?.()
      this.unsubscribeServerRequests?.()
      this.client?.dispose()
      this.client = undefined
      this.unsubscribeNotifications = undefined
      this.unsubscribeServerRequests = undefined
      this.fail(error)
    }

    return this.getStatus()
  }

  async request<T>(
    method: string,
    params?: unknown,
    options: AppServerRequestOptions = {}
  ): Promise<T> {
    this.assertHostRegistered(options.hostId ?? this.hostId)
    if (!method.trim()) throw new Error('app-server request method is required')

    if (method === 'modelProvider/list') {
      try {
        await this.ensureReady()
        const response = await this.requireClient().request<CodexModelListResponse>(
          'model/list',
          {}
        )
        return toRendererModelProviderConfigFromCodex(response) as T
      } catch (error) {
        return {
          models: [],
          unavailableReason: errorMessage(error)
        } as T
      }
    }

    if (method === 'approval/respond') {
      await this.ensureReady()
      const payload = params as AppServerApprovalRespondParams
      this.requireClient().respond(payload.requestId, { decision: payload.decision })
      return { accepted: true } as T
    }

    await this.ensureReady()
    if (method === 'modelProvider/selectForNextTurn') {
      return this.selectModelForNextTurn(params) as Promise<T>
    }

    return this.requireClient().request<T>(method, params)
  }

  async respondServerRequest(
    requestId: string | number,
    response: AppServerServerRequestResponse,
    options: AppServerRequestOptions = {}
  ): Promise<void> {
    this.assertHostRegistered(options.hostId ?? this.hostId)
    await this.ensureReady()
    this.requireClient().respond(requestId, response)
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
      this.unsubscribeNotifications?.()
      this.unsubscribeServerRequests?.()
      this.client.dispose()
      this.client = undefined
      this.unsubscribeNotifications = undefined
      this.unsubscribeServerRequests = undefined
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

  private async ensureReady(): Promise<void> {
    if (this.status.state === 'ready' && this.client) return
    const status = await this.start()
    if (status.state !== 'ready') {
      throw new Error(status.lastError ?? 'dasclaw-app-server 未就绪')
    }
  }

  private handleNotification(notification: JsonRpcNotification): void {
    this.emitNotification({
      hostId: this.hostId,
      method: notification.method,
      params: notification.params
    })
  }

  private emitNotification(envelope: AppServerNotification): void {
    this.setStatus({
      notificationCount: this.status.notificationCount + 1,
      lastNotification: envelope
    })
    for (const listener of this.notificationListeners) listener(envelope)
  }

  private handleServerRequest(request: JsonRpcServerRequest): void {
    if (isForwardedServerRequestMethod(request.method)) {
      this.emitNotification(
        forwardedServerRequest(this.hostId, { ...request, method: request.method })
      )
      return
    }

    this.requireClient().respond(request.id, failClosedServerRequestResponse(request.method))
  }

  private requireClient(): AppServerRpcClient {
    if (!this.client) throw new Error('dasclaw-app-server client is not started')
    return this.client
  }

  private assertHostRegistered(hostId: string): void {
    if (hostId !== this.hostId) {
      throw new Error(`No app-server connection registered for host ${hostId}`)
    }
  }

  private fail(error: unknown): void {
    this.setStatus({
      state: 'failed',
      lastError: error instanceof Error ? error.message : String(error)
    })
  }

  private setStatus(patch: Partial<AppServerStatus>): void {
    this.status = { ...this.status, ...patch }
    const status = this.getStatus()
    for (const listener of this.statusListeners) listener(status)
  }

  private async getModelProviderConfig(): Promise<AppServerModelProviderConfig> {
    if (!this.modelProviderConfig) {
      this.modelProviderConfig = await this.loadModelProviderConfig()
    }
    return this.modelProviderConfig
  }

  private async selectModelForNextTurn(
    params: unknown
  ): Promise<ModelProviderSelectForNextTurnResponse> {
    const modelId = readRequestedModelId(params)
    const config = await this.getModelProviderConfig()
    const selectedModel = config.models.find((model) => model.modelId === modelId)
    if (!selectedModel) {
      throw new Error(`Unknown modelProvider modelId: ${modelId}`)
    }

    const response = await this.requireClient().request<ModelProviderSelectForNextTurnResponse>(
      'modelProvider/selectForNextTurn',
      { modelId }
    )
    if (response.selectedModelId !== selectedModel.modelId) {
      throw new Error(
        `app-server acknowledged unexpected selected modelId: ${response.selectedModelId}`
      )
    }

    this.modelProviderConfig = {
      ...config,
      selectedModel
    }
    return response
  }
}

export function createModelProviderConfigLoader(
  options: {
    adminBackendUrl?: string
    fetchImpl?: typeof fetch
    timeoutMs?: number
    userId?: string
  } = {}
): ModelProviderConfigLoader {
  const adminBackendUrl =
    options.adminBackendUrl?.trim() ||
    process.env.ADMIN_BACKEND_URL?.trim() ||
    DEFAULT_ADMIN_BACKEND_URL
  const fetchImpl = options.fetchImpl ?? fetch
  const timeoutMs = options.timeoutMs ?? DEFAULT_MODEL_PROVIDER_FETCH_TIMEOUT_MS
  const userId = options.userId?.trim() || undefined

  return async () => {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), timeoutMs)

    try {
      const response = await fetchImpl(clientModelsUrl(adminBackendUrl, userId), {
        method: 'GET',
        signal: controller.signal
      })
      if (!response.ok) {
        throw new ModelProviderConfigError(
          `admin backend /api/client-models returned HTTP ${response.status}`
        )
      }

      return normalizeAdminClientModels(await response.json())
    } catch (error) {
      if (error instanceof ModelProviderConfigError) throw error
      throw new ModelProviderConfigError(
        `failed to fetch admin backend /api/client-models: ${errorMessage(error)}`
      )
    } finally {
      clearTimeout(timeout)
    }
  }
}

export function clientModelsUrl(adminBackendUrl: string, userId?: string): string {
  const base = adminBackendUrl.trim().replace(/\/+$/, '') || DEFAULT_ADMIN_BACKEND_URL
  const url = new URL(`${base}/api/client-models`)
  if (userId?.trim()) url.searchParams.set('user_id', userId.trim())
  return url.toString()
}

export function normalizeAdminClientModels(body: unknown): AppServerModelProviderConfig {
  if (!Array.isArray(body) || body.length === 0) {
    throw new ModelProviderConfigError('admin backend /api/client-models returned no models')
  }

  const seenModelIds = new Set<string>()
  const models = body.map((value, index) => normalizeAdminClientModel(value, index))
  for (const model of models) {
    if (seenModelIds.has(model.runtime.modelId)) {
      throw new ModelProviderConfigError(
        `admin backend /api/client-models returned duplicate model_id: ${model.runtime.modelId}`
      )
    }
    seenModelIds.add(model.runtime.modelId)
  }

  const selectedModel = models.find((model) => model.isDefault)?.runtime ?? models[0].runtime
  return {
    models: models.map((model) => model.runtime),
    selectedModel
  }
}

type NormalizedAdminClientModel = {
  runtime: AppServerClientModelConfig
  isDefault: boolean
}

function normalizeAdminClientModel(value: unknown, index: number): NormalizedAdminClientModel {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new ModelProviderConfigError(`model[${index}] must be an object`)
  }

  const raw = value as Partial<AdminClientModelConfig>
  const modelId = requiredString(raw.model_id, `model[${index}].model_id`)
  const displayName = requiredString(raw.display_name, `model[${index}].display_name`)
  const provider = requiredString(raw.provider, `model[${index}].provider`)
  const apiBaseUrl = requiredString(raw.api_base_url, `model[${index}].api_base_url`)
  const apiKey = requiredString(raw.api_key, `model[${index}].api_key`)
  const apiFormat = optionalString(raw.api_format)?.trim() || 'openai'
  const modelCallMode = optionalString(raw.model_call_mode)?.trim() || DEFAULT_MODEL_CALL_MODE
  const source = optionalString(raw.source)?.trim() || 'admin'
  const description = optionalString(raw.description)?.trim()

  return {
    runtime: {
      modelId,
      displayName,
      ...(description ? { description } : {}),
      provider,
      apiBaseUrl,
      apiKey,
      apiFormat,
      modelCallMode,
      source,
      capabilities: normalizeCapabilities(raw.capabilities)
    },
    isDefault: raw.is_default === true
  }
}

function requiredString(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new ModelProviderConfigError(`${field} is required`)
  }
  return value.trim()
}

function optionalString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined
}

function normalizeCapabilities(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value.filter((item): item is string => typeof item === 'string')
}

function isForwardedServerRequestMethod(method: string): method is AppServerServerRequestMethod {
  return (
    method === 'item/commandExecution/requestApproval' ||
    method === 'item/permissions/requestApproval' ||
    method === 'item/fileChange/requestApproval' ||
    method === 'item/tool/requestUserInput' ||
    method === 'item/tool/call'
  )
}

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}
}

function forwardedServerRequest<Method extends AppServerServerRequestMethod>(
  hostId: string,
  request: JsonRpcServerRequest & { method: Method }
): AppServerServerRequest<Method> {
  return {
    hostId,
    requestId: request.id,
    method: request.method,
    params: serverRequestParams(request.method, request.params)
  }
}

function serverRequestParams<Method extends AppServerServerRequestMethod>(
  method: Method,
  params: unknown
): AppServerServerRequestParamsByMethod[Method] {
  const safeParams = { ...asRecord(params) }
  if (
    method === 'item/commandExecution/requestApproval' ||
    method === 'item/permissions/requestApproval'
  ) {
    delete safeParams.rawArguments
    delete safeParams.requestId
  }
  return safeParams as AppServerServerRequestParamsByMethod[Method]
}

function failClosedServerRequestResponse(method: string): AppServerServerRequestResponse {
  if (method === 'item/tool/call') {
    return {
      contentItems: [
        {
          type: 'inputText',
          text: `unsupported app-server request method: ${method}`
        }
      ],
      success: false
    }
  }
  if (method === 'item/tool/requestUserInput') {
    return { answers: {} }
  }
  if (method === 'item/fileChange/requestApproval') {
    return { decision: 'decline' }
  }
  if (method === 'item/permissions/requestApproval') {
    return { decision: 'reject', permissions: {}, scope: 'turn', strictAutoReview: true }
  }
  return {
    decision: {
      kind: 'reject',
      data: { reason: `unsupported app-server request method: ${method}` }
    }
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function readRequestedModelId(params: unknown): string {
  if (!params || typeof params !== 'object' || Array.isArray(params)) {
    throw new Error('modelProvider/selectForNextTurn requires params')
  }

  const modelId = (params as { modelId?: unknown }).modelId
  if (typeof modelId !== 'string' || modelId.trim() === '') {
    throw new Error('modelProvider/selectForNextTurn requires a non-empty modelId')
  }
  return modelId.trim()
}

function toRendererModelProviderConfigFromCodex(
  response: CodexModelListResponse
): RendererModelProviderConfig {
  const visibleModels = response.data.filter((model) => !model.hidden)
  return {
    models: visibleModels.map((model) => ({
      modelId: model.id,
      displayName: model.displayName || model.model || model.id,
      ...(model.description ? { description: model.description } : {}),
      provider: 'app-server',
      apiBaseUrl: '',
      apiFormat: 'app-server',
      modelCallMode: 'stream',
      source: 'app-server',
      capabilities: model.inputModalities,
      apiKeyConfigured: true
    })),
    selectedModelId: visibleModels.find((model) => model.isDefault)?.id ?? visibleModels[0]?.id
  }
}

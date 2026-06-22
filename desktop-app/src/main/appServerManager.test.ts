import { describe, expect, it, vi } from 'vitest'

import {
  AppServerManager,
  ModelProviderConfigError,
  clientModelsUrl,
  createModelProviderConfigLoader,
  normalizeAdminClientModels
} from './appServerManager'
import type {
  AppServerRpcClient,
  JsonRpcId,
  JsonRpcNotification,
  JsonRpcServerRequest
} from './appServerRpc'
import type { AppServerModelProviderConfig } from './appServerManager'
import type {
  AppServerApprovalRequest,
  AppServerNotification,
  AppServerServerRequestMethod
} from '../shared/appServerApi'

const TEST_MODEL_PROVIDER_CONFIG: AppServerModelProviderConfig = {
  models: [
    {
      modelId: 'gpt-test',
      displayName: 'GPT Test',
      provider: 'openai',
      apiBaseUrl: 'https://api.test/v1',
      apiKey: 'test-api-key',
      apiFormat: 'openai',
      modelCallMode: 'stream',
      source: 'test',
      capabilities: ['chat']
    }
  ],
  selectedModel: {
    modelId: 'gpt-test',
    displayName: 'GPT Test',
    provider: 'openai',
    apiBaseUrl: 'https://api.test/v1',
    apiKey: 'test-api-key',
    apiFormat: 'openai',
    modelCallMode: 'stream',
    source: 'test',
    capabilities: ['chat']
  }
}

function createManager(fake: AppServerRpcClient): AppServerManager {
  return new AppServerManager({
    createClient: () => fake,
    binary: '/tmp/dasclaw-app-server',
    loadModelProviderConfig: async () => TEST_MODEL_PROVIDER_CONFIG
  })
}

function inProgressTurn(id: string): Record<string, unknown> {
  return {
    id,
    items: [],
    status: 'inProgress',
    error: null,
    startedAt: null,
    completedAt: null,
    durationMs: null
  }
}

function completedTurn(id: string, text = ''): Record<string, unknown> {
  return {
    id,
    items: text ? [{ type: 'agentMessage', id, text }] : [],
    status: 'completed',
    error: null,
    startedAt: null,
    completedAt: null,
    durationMs: null
  }
}

class FakeRpcClient implements AppServerRpcClient {
  readonly requests: Array<{ method: string; params?: unknown }> = []
  readonly responses: Array<{ id: JsonRpcId; result: unknown }> = []
  disposed = false
  private notificationHandler: ((notification: JsonRpcNotification) => void) | undefined
  private serverRequestHandler: ((request: JsonRpcServerRequest) => void) | undefined

  async request<T>(method: string, params?: unknown): Promise<T> {
    this.requests.push({ method, params })

    if (method === 'initialize') {
      return { ok: true } as T
    }
    if (method === 'health/check') {
      return { ok: true, lifecycle: { state: 'ready' }, services: [] } as T
    }
    if (method === 'model/list') {
      return {
        data: [
          {
            id: 'gpt-test',
            model: 'gpt-test',
            displayName: 'GPT Test',
            description: '',
            hidden: false,
            supportedReasoningEfforts: [
              { reasoningEffort: 'none', description: 'No reasoning effort override' }
            ],
            defaultReasoningEffort: 'none',
            inputModalities: ['text'],
            supportsPersonality: false,
            additionalSpeedTiers: [],
            isDefault: true,
            upgrade: null,
            upgradeInfo: null,
            availabilityNux: null
          }
        ],
        nextCursor: null
      } as T
    }
    if (method === 'thread/start') {
      return { thread: { id: 'thread-1' } } as T
    }
    if (method === 'turn/start') {
      queueMicrotask(() => {
        this.notificationHandler?.({
          type: 'notification',
          method: 'item/reasoning/summaryTextDelta',
          params: {
            threadId: 'thread-1',
            turnId: 'turn-1',
            itemId: 'turn-1:reasoning',
            summaryIndex: 0,
            delta: 'thinking'
          }
        })
        this.notificationHandler?.({
          type: 'notification',
          method: 'item/agentMessage/delta',
          params: {
            threadId: 'thread-1',
            turnId: 'turn-1',
            itemId: 'turn-1',
            delta: 'pong'
          }
        })
        this.notificationHandler?.({
          type: 'notification',
          method: 'turn/completed',
          params: {
            threadId: 'thread-1',
            turn: completedTurn('turn-1', 'pong')
          }
        })
      })
      return { turn: inProgressTurn('turn-1') } as T
    }
    if (method === 'modelProvider/selectForNextTurn') {
      return { selectedModelId: (params as { modelId: string }).modelId } as T
    }
    if (method === 'shutdown') {
      return { accepted: true } as T
    }

    throw new Error(`unexpected method ${method}`)
  }

  onNotification(handler: (notification: JsonRpcNotification) => void): () => void {
    this.notificationHandler = handler
    return () => {
      this.notificationHandler = undefined
    }
  }

  onServerRequest(handler: (request: JsonRpcServerRequest) => void): () => void {
    this.serverRequestHandler = handler
    return () => {
      this.serverRequestHandler = undefined
    }
  }

  emitServerRequest(request: JsonRpcServerRequest): void {
    this.serverRequestHandler?.(request)
  }

  respond(id: JsonRpcId, result: unknown): void {
    this.responses.push({ id, result })
  }

  dispose(): void {
    this.disposed = true
  }
}

class FailingInitializeRpcClient extends FakeRpcClient {
  async request<T>(method: string, params?: unknown): Promise<T> {
    this.requests.push({ method, params })
    if (method === 'initialize') throw new Error('init failed')
    return super.request(method, params)
  }
}

describe('AppServerManager', () => {
  it('initializes the local connection and forwards app-server requests by envelope', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)

    const response = await manager.request('turn/start', {
      threadId: 'thread-1',
      input: [{ type: 'text', text: 'ping', textElements: [] }],
      reasoningSummary: 'concise'
    })

    expect(manager.getStatus().state).toBe('ready')
    expect(response).toEqual({ turn: inProgressTurn('turn-1') })
    expect(fake.requests.map((request) => request.method)).toEqual([
      'initialize',
      'health/check',
      'turn/start'
    ])
    expect(fake.requests[0].params).toEqual({
      client: { name: 'desktop-app', version: '0.1.0', transport: 'stdio' },
      protocolVersion: { major: 0, minor: 1, patch: 0 },
      requestedCapabilities: ['protocol', 'lifecycle', 'health', 'session'],
      modelProvider: TEST_MODEL_PROVIDER_CONFIG
    })
    expect(fake.requests.at(-1)?.params).toEqual({
      threadId: 'thread-1',
      input: [{ type: 'text', text: 'ping', textElements: [] }],
      reasoningSummary: 'concise'
    })
  })

  it('returns a renderer-safe model provider list without exposing API keys', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)

    const response = await manager.request('modelProvider/list')

    expect(response).toEqual({
      models: [
        {
          modelId: 'gpt-test',
          displayName: 'GPT Test',
          provider: 'app-server',
          apiBaseUrl: '',
          apiFormat: 'app-server',
          modelCallMode: 'stream',
          source: 'app-server',
          capabilities: ['text'],
          apiKeyConfigured: true
        }
      ],
      selectedModelId: 'gpt-test'
    })
    expect(JSON.stringify(response)).not.toContain('test-api-key')
    expect(fake.requests.map((request) => request.method)).toEqual([
      'initialize',
      'health/check',
      'model/list'
    ])
  })

  it('returns a renderer-safe unavailable state when the model config source is down', async () => {
    const fake = new FakeRpcClient()
    const manager = new AppServerManager({
      createClient: () => fake,
      binary: '/tmp/dasclaw-app-server',
      loadModelProviderConfig: async () => {
        throw new ModelProviderConfigError(
          'failed to fetch admin backend /api/client-models: fetch failed'
        )
      }
    })

    await expect(manager.request('modelProvider/list')).resolves.toEqual({
      models: [],
      unavailableReason: 'failed to fetch admin backend /api/client-models: fetch failed'
    })
    expect(fake.requests).toEqual([])
    await expect(
      manager.request('turn/start', { threadId: 'thread-1', input: [] })
    ).rejects.toThrow('failed to fetch admin backend /api/client-models: fetch failed')
  })

  it('forwards model selection to app-server and updates the renderer selection after ack', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)

    await expect(
      manager.request('modelProvider/selectForNextTurn', { modelId: 'gpt-test' })
    ).resolves.toEqual({ selectedModelId: 'gpt-test' })
    await expect(manager.request('modelProvider/list')).resolves.toMatchObject({
      selectedModelId: 'gpt-test'
    })
    expect(fake.requests.map((request) => request.method)).toContain(
      'modelProvider/selectForNextTurn'
    )
  })

  it('rejects requests for unregistered hosts instead of falling back to local', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)

    await expect(
      manager.request('model/list', undefined, { hostId: 'remote-dev' })
    ).rejects.toThrow('No app-server connection registered for host remote-dev')
    expect(fake.requests).toEqual([])
  })

  it('coalesces concurrent requests while the local connection is starting', async () => {
    let resolveInitialize: (() => void) | undefined
    const fake = new (class extends FakeRpcClient {
      async request<T>(method: string, params?: unknown): Promise<T> {
        if (method === 'initialize') {
          await new Promise<void>((resolve) => {
            resolveInitialize = resolve
          })
        }
        if (method === 'model/list') {
          this.requests.push({ method, params })
          return { models: [] } as T
        }
        return super.request(method, params)
      }
    })()
    const createClient = vi.fn(() => fake)
    const manager = new AppServerManager({
      createClient,
      binary: '/tmp/dasclaw-app-server',
      loadModelProviderConfig: async () => TEST_MODEL_PROVIDER_CONFIG
    })

    const firstRequest = manager.request('model/list')
    const secondRequest = manager.request('model/list')
    await new Promise<void>((resolve) => queueMicrotask(() => resolve()))

    expect(createClient).toHaveBeenCalledTimes(1)
    resolveInitialize?.()

    await expect(Promise.all([firstRequest, secondRequest])).resolves.toEqual([
      { models: [] },
      { models: [] }
    ])
  })

  it('preconnects the local app-server without waiting for a renderer request', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)

    const preconnect = manager.preconnect()

    expect(manager.getStatus().state).toBe('starting')
    await expect(preconnect).resolves.toMatchObject({ state: 'ready' })
    expect(fake.requests.map((request) => request.method)).toEqual(['initialize', 'health/check'])
  })

  it('broadcasts app-server notifications with host context', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)
    const notifications: unknown[] = []
    manager.onNotification((notification) => notifications.push(notification))

    await manager.request('turn/start', {
      threadId: 'thread-1',
      input: [{ type: 'text', text: 'ping', textElements: [] }]
    })
    await new Promise<void>((resolve) => queueMicrotask(() => resolve()))

    expect(notifications).toEqual([
      {
        hostId: 'local',
        method: 'item/reasoning/summaryTextDelta',
        params: {
          threadId: 'thread-1',
          turnId: 'turn-1',
          itemId: 'turn-1:reasoning',
          summaryIndex: 0,
          delta: 'thinking'
        }
      },
      {
        hostId: 'local',
        method: 'item/agentMessage/delta',
        params: {
          threadId: 'thread-1',
          turnId: 'turn-1',
          itemId: 'turn-1',
          delta: 'pong'
        }
      },
      {
        hostId: 'local',
        method: 'turn/completed',
        params: {
          threadId: 'thread-1',
          turn: completedTurn('turn-1', 'pong')
        }
      }
    ])
  })

  it('forwards app-server approval requests and writes renderer decisions back', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)
    const approvals: AppServerApprovalRequest[] = []
    manager.onNotification((notification) => {
      if (isApprovalNotification(notification)) {
        approvals.push(notification)
      }
    })

    await manager.start()
    fake.emitServerRequest({
      type: 'server-request',
      id: 'approval_1',
      method: 'item/commandExecution/requestApproval',
      params: {
        threadId: 'thread_1',
        turnId: 'turn_1',
        itemId: 'turn_1:tool:bash',
        toolCallId: 'call_1',
        toolName: 'bash',
        description: 'approve call to bash',
        displayParameters: { cmd: 'echo hello' },
        rawArguments: { apiKey: 'secret-token' },
        allowAlways: true
      }
    })

    expect(approvals).toEqual([
      {
        hostId: 'local',
        requestId: 'approval_1',
        method: 'item/commandExecution/requestApproval',
        params: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          itemId: 'turn_1:tool:bash',
          toolCallId: 'call_1',
          toolName: 'bash',
          description: 'approve call to bash',
          displayParameters: { cmd: 'echo hello' },
          allowAlways: true
        }
      }
    ])

    await manager.request('approval/respond', {
      requestId: 'approval_1',
      decision: { kind: 'approve' }
    })

    expect(fake.responses).toEqual([
      {
        id: 'approval_1',
        result: { decision: { kind: 'approve' } }
      }
    ])
  })

  it('starts the app-server before writing generic server request responses', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)

    await manager.respondServerRequest('approval_1', {
      decision: { kind: 'approve' }
    })

    expect(fake.requests.map((request) => request.method)).toEqual(['initialize', 'health/check'])
    expect(fake.responses).toEqual([
      {
        id: 'approval_1',
        result: { decision: { kind: 'approve' } }
      }
    ])
  })

  it('forwards all supported r1 server request methods', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)
    const notifications: AppServerNotification[] = []
    manager.onNotification((notification) => notifications.push(notification))

    await manager.start()

    const cases: Array<{
      id: string
      method: AppServerServerRequestMethod
      params: Record<string, unknown>
      expectedParams: Record<string, unknown>
    }> = [
      {
        id: 'approval_1',
        method: 'item/commandExecution/requestApproval',
        params: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          itemId: 'turn_1:tool:bash',
          toolCallId: 'call_1',
          toolName: 'bash',
          description: 'approve call to bash',
          displayParameters: { cmd: 'echo hello' },
          rawArguments: { apiKey: 'secret-token' },
          allowAlways: true
        },
        expectedParams: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          itemId: 'turn_1:tool:bash',
          toolCallId: 'call_1',
          toolName: 'bash',
          description: 'approve call to bash',
          displayParameters: { cmd: 'echo hello' },
          allowAlways: true
        }
      },
      {
        id: 'permissions_1',
        method: 'item/permissions/requestApproval',
        params: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          permissions: ['net:fetch'],
          rawArguments: { apiKey: 'secret-token' },
          requestId: 'nested_request'
        },
        expectedParams: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          permissions: ['net:fetch']
        }
      },
      {
        id: 'file_change_1',
        method: 'item/fileChange/requestApproval',
        params: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          path: 'src/app.ts',
          action: 'modify'
        },
        expectedParams: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          path: 'src/app.ts',
          action: 'modify'
        }
      },
      {
        id: 'user_input_1',
        method: 'item/tool/requestUserInput',
        params: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          questions: [{ id: 'name', prompt: 'Name?' }]
        },
        expectedParams: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          questions: [{ id: 'name', prompt: 'Name?' }]
        }
      },
      {
        id: 'tool_1',
        method: 'item/tool/call',
        params: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          callId: 'call_1',
          namespace: 'client',
          tool: 'open_url',
          arguments: { url: 'https://example.test' }
        },
        expectedParams: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          callId: 'call_1',
          namespace: 'client',
          tool: 'open_url',
          arguments: { url: 'https://example.test' }
        }
      }
    ]

    for (const item of cases) {
      fake.emitServerRequest({
        type: 'server-request',
        id: item.id,
        method: item.method,
        params: item.params
      })
    }

    expect(notifications).toEqual(
      cases.map((item) => ({
        hostId: 'local',
        requestId: item.id,
        method: item.method,
        params: item.expectedParams
      }))
    )

    await manager.respondServerRequest('tool_1', {
      contentItems: [{ type: 'inputText', text: 'opened' }],
      success: true
    })

    expect(fake.responses).toContainEqual({
      id: 'tool_1',
      result: {
        contentItems: [{ type: 'inputText', text: 'opened' }],
        success: true
      }
    })
  })

  it('fail-closes unsupported server request methods without notifying the renderer', async () => {
    const fake = new FakeRpcClient()
    const manager = createManager(fake)
    const notifications: AppServerNotification[] = []
    manager.onNotification((notification) => notifications.push(notification))

    await manager.start()
    fake.emitServerRequest({
      type: 'server-request',
      id: 'unsupported_1',
      method: 'item/tool/unsupported',
      params: { threadId: 'thread_1' }
    })

    expect(notifications).toEqual([])
    expect(fake.responses).toEqual([
      {
        id: 'unsupported_1',
        result: {
          decision: {
            kind: 'reject',
            data: { reason: 'unsupported app-server request method: item/tool/unsupported' }
          }
        }
      }
    ])
  })

  it('disposes the client when initialize fails', async () => {
    const fake = new FailingInitializeRpcClient()
    const manager = createManager(fake)

    const status = await manager.start()

    expect(status.state).toBe('failed')
    expect(status.lastError).toBe('init failed')
    expect(fake.disposed).toBe(true)
  })

  it('does not initialize app-server when model provider config cannot be loaded', async () => {
    const fake = new FakeRpcClient()
    const manager = new AppServerManager({
      createClient: () => fake,
      binary: '/tmp/dasclaw-app-server',
      loadModelProviderConfig: async () => {
        throw new Error('model config unavailable')
      }
    })

    const status = await manager.start()

    expect(status.state).toBe('failed')
    expect(status.lastError).toBe('model config unavailable')
    expect(fake.requests).toEqual([])
    expect(fake.disposed).toBe(true)
  })

  it('reports a failed status when the app-server process cannot be created', async () => {
    const manager = new AppServerManager({
      createClient: () => {
        throw new Error('missing app-server binary')
      },
      binary: '/tmp/dasclaw-app-server',
      loadModelProviderConfig: async () => TEST_MODEL_PROVIDER_CONFIG
    })

    const status = await manager.start()

    expect(status.state).toBe('failed')
    expect(status.lastError).toBe('missing app-server binary')
  })
})

describe('model provider config loading', () => {
  it('builds the admin backend client-models URL with an optional user id', () => {
    expect(clientModelsUrl('http://localhost:3000')).toBe('http://localhost:3000/api/client-models')
    expect(clientModelsUrl('http://localhost:3000/', 'user-1')).toBe(
      'http://localhost:3000/api/client-models?user_id=user-1'
    )
  })

  it('normalizes admin backend models into app-server initialize config', () => {
    expect(
      normalizeAdminClientModels([
        adminModel({
          model_id: 'gpt-a',
          is_default: false,
          api_key: 'secret-a'
        }),
        adminModel({ model_id: 'gpt-b', is_default: true, api_key: 'secret-b' })
      ])
    ).toEqual({
      models: [
        {
          modelId: 'gpt-a',
          displayName: 'GPT A',
          provider: 'openai',
          apiBaseUrl: 'https://api.test/v1',
          apiKey: 'secret-a',
          apiFormat: 'openai',
          modelCallMode: 'stream',
          source: 'admin',
          capabilities: ['chat']
        },
        {
          modelId: 'gpt-b',
          displayName: 'GPT A',
          provider: 'openai',
          apiBaseUrl: 'https://api.test/v1',
          apiKey: 'secret-b',
          apiFormat: 'openai',
          modelCallMode: 'stream',
          source: 'admin',
          capabilities: ['chat']
        }
      ],
      selectedModel: {
        modelId: 'gpt-b',
        displayName: 'GPT A',
        provider: 'openai',
        apiBaseUrl: 'https://api.test/v1',
        apiKey: 'secret-b',
        apiFormat: 'openai',
        modelCallMode: 'stream',
        source: 'admin',
        capabilities: ['chat']
      }
    })
  })

  it('fetches /api/client-models and surfaces HTTP errors as config failures', async () => {
    const fetchImpl = vi.fn(async () => new Response('unavailable', { status: 503 }))
    const loadModelProviderConfig = createModelProviderConfigLoader({
      adminBackendUrl: 'http://admin.test',
      fetchImpl
    })

    await expect(loadModelProviderConfig()).rejects.toThrow(
      'admin backend /api/client-models returned HTTP 503'
    )
    expect(fetchImpl).toHaveBeenCalledWith('http://admin.test/api/client-models', {
      method: 'GET',
      signal: expect.any(AbortSignal)
    })
  })
})

function adminModel(overrides: Partial<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    model_id: 'gpt-a',
    display_name: 'GPT A',
    provider: 'openai',
    is_default: true,
    capabilities: ['chat'],
    api_base_url: 'https://api.test/v1',
    api_key: 'secret',
    api_format: 'openai',
    source: 'admin',
    ...overrides
  }
}

function isApprovalNotification(
  notification: AppServerNotification
): notification is AppServerApprovalRequest {
  return (
    'requestId' in notification &&
    (notification.method === 'item/commandExecution/requestApproval' ||
      notification.method === 'item/permissions/requestApproval')
  )
}

import { describe, expect, it, vi } from 'vitest'

import {
  AppServerManager,
  ModelProviderConfigError,
  clientModelsUrl,
  createModelProviderConfigLoader,
  normalizeAdminClientModels
} from './appServerManager'
import type { AppServerRpcClient, JsonRpcNotification } from './appServerRpc'
import type { AppServerModelProviderConfig } from './appServerManager'

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

class FakeRpcClient implements AppServerRpcClient {
  readonly requests: Array<{ method: string; params?: unknown }> = []
  disposed = false
  private notificationHandler: ((notification: JsonRpcNotification) => void) | undefined

  async request<T>(method: string, params?: unknown): Promise<T> {
    this.requests.push({ method, params })

    if (method === 'initialize') {
      return { ok: true } as T
    }
    if (method === 'health/check') {
      return { ok: true, lifecycle: { state: 'ready' }, services: [] } as T
    }
    if (method === 'thread/start') {
      return { threadId: 'thread-1' } as T
    }
    if (method === 'turn/start') {
      queueMicrotask(() => {
        this.notificationHandler?.({
          type: 'notification',
          method: 'turn/delta',
          params: { threadId: 'thread-1', turnId: 'turn-1', delta: 'pong' }
        })
        this.notificationHandler?.({
          type: 'notification',
          method: 'turn/completed',
          params: {
            threadId: 'thread-1',
            turnId: 'turn-1',
            status: 'completed',
            output: 'pong'
          }
        })
      })
      return { turnId: 'turn-1', status: 'pending' } as T
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
      input: [{ type: 'text', text: 'ping' }]
    })

    expect(manager.getStatus().state).toBe('ready')
    expect(response).toEqual({ turnId: 'turn-1', status: 'pending' })
    expect(fake.requests.map((request) => request.method)).toEqual([
      'initialize',
      'health/check',
      'turn/start'
    ])
    expect(fake.requests[0].params).toMatchObject({
      modelProvider: TEST_MODEL_PROVIDER_CONFIG
    })
    expect(fake.requests.at(-1)?.params).toEqual({
      threadId: 'thread-1',
      input: [{ type: 'text', text: 'ping' }]
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
          provider: 'openai',
          apiBaseUrl: 'https://api.test/v1',
          apiFormat: 'openai',
          source: 'test',
          capabilities: ['chat'],
          apiKeyConfigured: true
        }
      ],
      selectedModelId: 'gpt-test'
    })
    expect(JSON.stringify(response)).not.toContain('test-api-key')
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
      input: [{ type: 'text', text: 'ping' }]
    })
    await new Promise<void>((resolve) => queueMicrotask(() => resolve()))

    expect(notifications).toEqual([
      {
        hostId: 'local',
        method: 'turn/delta',
        params: { threadId: 'thread-1', turnId: 'turn-1', delta: 'pong' }
      },
      {
        hostId: 'local',
        method: 'turn/completed',
        params: {
          threadId: 'thread-1',
          turnId: 'turn-1',
          status: 'completed',
          output: 'pong'
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
        adminModel({ model_id: 'gpt-a', is_default: false, api_key: 'secret-a' }),
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

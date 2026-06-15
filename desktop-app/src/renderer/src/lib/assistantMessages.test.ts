// @vitest-environment jsdom

import { act, createElement, useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import type { AppendMessage, ModelContext, ThreadMessage } from '@assistant-ui/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { AppServerNotification } from '../../../shared/appServerApi'
import type { AppServerModelSelectorState } from '../hooks/useDasclawAssistantRuntime'

type ModelContextRegistration = {
  getModelContext: () => ModelContext
}

type ExternalStoreAdapterCapture = {
  messages?: readonly ThreadMessage[]
  isRunning?: boolean
  onNew?: (message: AppendMessage) => Promise<void>
  onEdit?: unknown
}

const runtimeAdapterCapture = vi.hoisted(() => ({
  latest: undefined as ExternalStoreAdapterCapture | undefined
}))

const modelContextRegister = vi.fn<(registration: ModelContextRegistration) => () => void>(() =>
  vi.fn()
)

vi.mock('@assistant-ui/react', async () => {
  const actual = await vi.importActual<typeof import('@assistant-ui/react')>('@assistant-ui/react')
  return {
    ...actual,
    useExternalStoreRuntime: (adapter: ExternalStoreAdapterCapture) => {
      runtimeAdapterCapture.latest = adapter
      return {}
    },
    useAui: () => ({
      modelContext: () => ({
        register: modelContextRegister
      })
    })
  }
})

import { ModelSelector } from '../components/assistant-ui'
import {
  useAppServerModelSelectorState,
  useDasclawAssistantRuntime
} from '../hooks/useDasclawAssistantRuntime'

import {
  assistantModelOptions,
  assistantMessage,
  defaultAssistantModelId,
  extractTextFromAppendMessage,
  initialAssistantMessages,
  modelOptionsFromProviderConfig,
  userMessage
} from './assistantMessages'

describe('assistant-ui message helpers', () => {
  it('extracts only text parts from an assistant-ui append message', () => {
    expect(
      extractTextFromAppendMessage({
        role: 'user',
        content: [
          { type: 'text', text: 'hello' },
          { type: 'image', image: 'data:image/png;base64,a' }
        ],
        attachments: [],
        createdAt: new Date('2026-01-01T00:00:00Z'),
        parentId: null,
        sourceId: null,
        runConfig: undefined,
        metadata: { custom: {} }
      })
    ).toBe('hello')
  })

  it('creates assistant-ui compatible user and assistant messages', () => {
    expect(userMessage('u1', 'hi').role).toBe('user')
    expect(assistantMessage('a1', 'there').status).toEqual({
      type: 'complete',
      reason: 'stop'
    })
  })

  it('starts with an empty thread so the UI can render the welcome composer', () => {
    expect(initialAssistantMessages()).toEqual([])
  })

  it('defines a selectable default assistant model', () => {
    expect(assistantModelOptions.some((model) => model.id === defaultAssistantModelId)).toBe(true)
  })

  it('marks models without configured API keys as disabled options', () => {
    expect(
      modelOptionsFromProviderConfig({
        models: [
          {
            modelId: 'missing-key',
            displayName: 'Missing Key',
            provider: 'openai',
            apiBaseUrl: 'https://api.test/v1',
            apiFormat: 'openai',
            source: 'admin',
            capabilities: ['chat'],
            apiKeyConfigured: false
          }
        ],
        selectedModelId: 'missing-key'
      })
    ).toEqual([
      {
        id: 'missing-key',
        name: 'Missing Key',
        description: 'API Key 未配置',
        disabled: true,
        keywords: ['openai', 'admin']
      }
    ])
  })
})

describe('ModelSelector', () => {
  let container: HTMLDivElement
  let root: ReturnType<typeof createRoot>

  beforeEach(() => {
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
    modelContextRegister.mockClear()
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
  })

  it('registers the selected model in assistant-ui model context', () => {
    act(() => {
      root.render(createElement(ModelSelector, { models: assistantModelOptions }))
    })

    const registration = modelContextRegister.mock.calls.at(-1)?.[0]
    expect(registration?.getModelContext()).toEqual({
      config: {
        modelName: defaultAssistantModelId
      }
    })
  })

  it('registers a controlled model value in assistant-ui model context', () => {
    act(() => {
      root.render(
        createElement(ModelSelector, {
          models: assistantModelOptions,
          value: assistantModelOptions[1].id
        })
      )
    })

    const registration = modelContextRegister.mock.calls.at(-1)?.[0]
    expect(registration?.getModelContext()).toEqual({
      config: {
        modelName: assistantModelOptions[1].id
      }
    })
  })
})

describe('useAppServerModelSelectorState', () => {
  let container: HTMLDivElement
  let root: ReturnType<typeof createRoot>
  let latestState: AppServerModelSelectorState | undefined
  let requestMock: ReturnType<typeof vi.fn>

  function ModelSelectorStateProbe(): null {
    const state = useAppServerModelSelectorState()
    useEffect(() => {
      latestState = state
    }, [state])
    return null
  }

  beforeEach(() => {
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
    latestState = undefined
    requestMock = vi.fn(async (method: string, params?: unknown) => {
      if (method === 'modelProvider/list') {
        return {
          models: [
            {
              modelId: 'server-gpt',
              displayName: 'Server GPT',
              description: 'From admin backend',
              provider: 'openai',
              apiBaseUrl: 'https://api.test/v1',
              apiFormat: 'openai',
              source: 'admin',
              capabilities: ['chat'],
              apiKeyConfigured: true
            },
            {
              modelId: 'server-next',
              displayName: 'Server Next',
              provider: 'openai',
              apiBaseUrl: 'https://api.test/v1',
              apiFormat: 'openai',
              source: 'admin',
              capabilities: ['chat'],
              apiKeyConfigured: true
            }
          ],
          selectedModelId: 'server-gpt'
        }
      }
      if (method === 'modelProvider/selectForNextTurn') {
        expect(params).toEqual({ modelId: 'server-next' })
        return { selectedModelId: 'server-next' }
      }
      throw new Error(`unexpected method ${method}`)
    })
    window.desktopAppServer = {
      request: requestMock as Window['desktopAppServer']['request'],
      stop: vi.fn().mockResolvedValue(undefined),
      getStatus: vi.fn().mockResolvedValue(undefined),
      checkHealth: vi.fn().mockResolvedValue(undefined),
      onStatusChange: vi.fn(() => vi.fn()),
      onNotification: vi.fn(() => vi.fn())
    }
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
  })

  it('loads app-server models for ModelSelector and sends selection changes back', async () => {
    await act(async () => {
      root.render(createElement(ModelSelectorStateProbe))
    })
    await act(async () => {
      await Promise.resolve()
    })

    expect(latestState?.models).toEqual([
      {
        id: 'server-gpt',
        name: 'Server GPT',
        description: 'From admin backend',
        keywords: ['openai', 'admin']
      },
      {
        id: 'server-next',
        name: 'Server Next',
        description: undefined,
        keywords: ['openai', 'admin']
      }
    ])
    expect(latestState?.value).toBe('server-gpt')

    await act(async () => {
      await latestState?.onValueChange('server-next')
    })

    expect(requestMock.mock.calls.map(([method]) => method)).toEqual([
      'modelProvider/list',
      'modelProvider/selectForNextTurn'
    ])
    expect(latestState?.value).toBe('server-next')
  })

  it('shows an unavailable model option when app-server reports model config load failure', async () => {
    requestMock.mockResolvedValueOnce({
      models: [],
      unavailableReason: 'failed to fetch admin backend /api/client-models: fetch failed'
    })

    await act(async () => {
      root.render(createElement(ModelSelectorStateProbe))
    })
    await act(async () => {
      await Promise.resolve()
    })

    expect(latestState?.models).toEqual([
      {
        id: 'model-provider-unavailable',
        name: '模型配置不可用',
        description: 'failed to fetch admin backend /api/client-models: fetch failed',
        disabled: true
      }
    ])
    expect(latestState?.value).toBeUndefined()
  })
})

describe('useDasclawAssistantRuntime', () => {
  let container: HTMLDivElement
  let root: ReturnType<typeof createRoot>
  let removeStatusListener: () => void
  let removeNotificationListener: () => void
  let notificationListener: ((notification: AppServerNotification) => void) | undefined
  let requestMock: ReturnType<typeof vi.fn>

  function RuntimeProbe(): null {
    useDasclawAssistantRuntime()
    return null
  }

  beforeEach(() => {
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
    runtimeAdapterCapture.latest = undefined
    removeStatusListener = vi.fn()
    removeNotificationListener = vi.fn()
    notificationListener = undefined
    requestMock = vi.fn(async (method: string) => {
      if (method === 'thread/start') return { threadId: 'thread-1' }
      if (method === 'turn/start') {
        queueMicrotask(() => {
          notificationListener?.({
            hostId: 'local',
            method: 'turn/completed',
            params: {
              threadId: 'thread-1',
              turnId: 'turn-1',
              output: 'ok'
            }
          })
        })
        return { turnId: 'turn-1' }
      }
      throw new Error(`unexpected method ${method}`)
    })
    window.desktopAppServer = {
      request: requestMock as Window['desktopAppServer']['request'],
      stop: vi.fn().mockResolvedValue(undefined),
      getStatus: vi.fn().mockResolvedValue(undefined),
      checkHealth: vi.fn().mockResolvedValue(undefined),
      onStatusChange: vi.fn(() => removeStatusListener),
      onNotification: vi.fn((callback) => {
        notificationListener = callback
        return removeNotificationListener
      })
    }
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
  })

  it('enables assistant-ui message editing on the external store runtime', () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    expect(runtimeAdapterCapture.latest?.onEdit).toEqual(expect.any(Function))
  })

  it('sends renderer app-server requests through the request envelope', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    await act(async () => {
      await runtimeAdapterCapture.latest?.onNew?.({
        role: 'user',
        content: [{ type: 'text', text: 'ping' }],
        attachments: [],
        createdAt: new Date('2026-01-01T00:00:00Z'),
        parentId: null,
        sourceId: null,
        runConfig: undefined,
        metadata: { custom: {} }
      })
    })

    expect(requestMock.mock.calls.map(([method]) => method)).toEqual(['thread/start', 'turn/start'])
    expect(requestMock.mock.calls[1][1]).toEqual({
      threadId: 'thread-1',
      input: [{ type: 'text', text: 'ping' }]
    })
  })

  it('maps app-server reasoning and agent text notifications to assistant-ui parts', async () => {
    requestMock.mockImplementation(async (method: string) => {
      if (method === 'thread/start') return { threadId: 'thread-1' }
      if (method === 'turn/start') {
        queueMicrotask(() => {
          notificationListener?.({
            hostId: 'local',
            method: 'item/reasoning/summaryTextDelta',
            params: {
              threadId: 'thread-1',
              turnId: 'turn-1',
              itemId: 'turn-1:reasoning',
              summaryIndex: 0,
              delta: 'private scratch'
            }
          })
          notificationListener?.({
            hostId: 'local',
            method: 'item/agentMessage/delta',
            params: {
              threadId: 'thread-1',
              turnId: 'turn-1',
              itemId: 'turn-1',
              delta: 'final answer'
            }
          })
          notificationListener?.({
            hostId: 'local',
            method: 'turn/completed',
            params: {
              threadId: 'thread-1',
              turnId: 'turn-1',
              output: 'final answer'
            }
          })
        })
        return { turnId: 'turn-1' }
      }
      throw new Error(`unexpected method ${method}`)
    })
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    await act(async () => {
      await runtimeAdapterCapture.latest?.onNew?.({
        role: 'user',
        content: [{ type: 'text', text: 'ping' }],
        attachments: [],
        createdAt: new Date('2026-01-01T00:00:00Z'),
        parentId: null,
        sourceId: null,
        runConfig: undefined,
        metadata: { custom: {} }
      })
    })

    const assistant = runtimeAdapterCapture.latest?.messages?.at(-1)
    expect(assistant?.role).toBe('assistant')
    expect(assistant?.content).toEqual([
      { type: 'reasoning', text: 'private scratch' },
      { type: 'text', text: 'final answer' }
    ])
  })
})

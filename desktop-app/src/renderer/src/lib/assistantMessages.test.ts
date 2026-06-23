// @vitest-environment jsdom

import { act, createElement, useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import type { AppendMessage, ModelContext, ThreadMessage } from '@assistant-ui/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type {
  AppServerNotification,
  AppServerServerRequest,
  AppServerServerRequestResponse
} from '../../../shared/appServerApi'
import type {
  AppServerModelSelectorState,
  DasclawAssistantRuntime
} from '../hooks/useDasclawAssistantRuntime'

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
            modelCallMode: 'stream',
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

  it('keeps app-server owned models selectable when availability is server-owned', () => {
    expect(
      modelOptionsFromProviderConfig({
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
    ).toEqual([
      {
        id: 'gpt-test',
        name: 'GPT Test',
        description: undefined,
        keywords: ['app-server', 'app-server']
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
      respondServerRequest: vi.fn().mockResolvedValue(undefined),
      stop: vi.fn().mockResolvedValue(undefined),
      getStatus: vi.fn().mockResolvedValue(undefined),
      checkHealth: vi.fn().mockResolvedValue(undefined),
      openExternalHttpUrl: vi.fn().mockResolvedValue(undefined),
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
  let respondServerRequestMock: ReturnType<typeof vi.fn>
  let openExternalHttpUrlMock: ReturnType<typeof vi.fn>
  let latestRuntime: DasclawAssistantRuntime | undefined

  function RuntimeProbe(): null {
    latestRuntime = useDasclawAssistantRuntime()
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
    respondServerRequestMock = vi.fn().mockResolvedValue(undefined)
    openExternalHttpUrlMock = vi.fn().mockResolvedValue(undefined)
    latestRuntime = undefined
    requestMock = vi.fn(async (method: string) => {
      if (method === 'thread/start') return { thread: { id: 'thread-1' } }
      if (method === 'turn/start') {
        queueMicrotask(() => {
          notificationListener?.({
            hostId: 'local',
            method: 'turn/completed',
            params: {
              threadId: 'thread-1',
              turn: {
                id: 'turn-1',
                status: 'completed',
                items: [],
                error: null,
                startedAt: null,
                completedAt: null,
                durationMs: null
              }
            }
          })
        })
        return { turn: inProgressTurn('turn-1') }
      }
      throw new Error(`unexpected method ${method}`)
    })
    window.desktopAppServer = {
      request: requestMock as Window['desktopAppServer']['request'],
      respondServerRequest:
        respondServerRequestMock as Window['desktopAppServer']['respondServerRequest'],
      stop: vi.fn().mockResolvedValue(undefined),
      getStatus: vi.fn().mockResolvedValue(undefined),
      checkHealth: vi.fn().mockResolvedValue(undefined),
      openExternalHttpUrl:
        openExternalHttpUrlMock as Window['desktopAppServer']['openExternalHttpUrl'],
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
      input: [{ type: 'text', text: 'ping', textElements: [] }]
    })
  })

  it('queues known R1 server requests without immediately responding', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = fileChangeApprovalRequest('file_change_1')
    await act(async () => {
      notificationListener?.(request)
    })

    expect(latestRuntime?.serverRequests).toEqual([request])
    expect(respondServerRequestMock).not.toHaveBeenCalled()
  })

  it('responds to queued server requests and removes them from the queue', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = fileChangeApprovalRequest('file_change_1')
    const response = {
      decision: 'accept'
    } satisfies AppServerServerRequestResponse<'item/fileChange/requestApproval'>
    await act(async () => {
      notificationListener?.(request)
    })

    await act(async () => {
      await latestRuntime?.respondToServerRequest(request, response)
    })

    expect(respondServerRequestMock).toHaveBeenCalledWith('file_change_1', response)
    expect(latestRuntime?.serverRequests).toEqual([])
  })

  it('accepts cancel responses for file change server requests', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = fileChangeApprovalRequest('file_change_1')
    const response = {
      decision: 'cancel'
    } satisfies AppServerServerRequestResponse<'item/fileChange/requestApproval'>
    await act(async () => {
      notificationListener?.(request)
    })

    await act(async () => {
      await latestRuntime?.respondToServerRequest(request, response)
    })

    expect(respondServerRequestMock).toHaveBeenCalledWith('file_change_1', response)
    expect(latestRuntime?.serverRequests).toEqual([])
  })

  it('rejects permissions responses without explicit decision', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = permissionsApprovalRequest('permissions_1')
    const response = {
      permissions: {}
    } as AppServerServerRequestResponse
    await act(async () => {
      notificationListener?.(request)
    })

    await act(async () => {
      await expect(
        latestRuntime?.respondToServerRequest(request as AppServerServerRequest, response)
      ).rejects.toThrow('server request response does not match request method')
    })

    expect(respondServerRequestMock).not.toHaveBeenCalled()
    expect(latestRuntime?.serverRequests).toEqual([request])
  })

  it('accepts permissions responses with array permissions and review scope', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = permissionsApprovalRequest('permissions_1')
    const response = {
      decision: 'approve',
      permissions: ['net:fetch'],
      scope: 'turn',
      strictAutoReview: true
    } satisfies AppServerServerRequestResponse<'item/permissions/requestApproval'>
    await act(async () => {
      notificationListener?.(request)
    })

    await act(async () => {
      await latestRuntime?.respondToServerRequest(request, response)
    })

    expect(respondServerRequestMock).toHaveBeenCalledWith('permissions_1', response)
    expect(latestRuntime?.serverRequests).toEqual([])
  })

  it('rejects mismatched wide server request responses without removing the queued request', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = fileChangeApprovalRequest('file_change_1')
    await act(async () => {
      notificationListener?.(request)
    })

    await expect(
      latestRuntime?.respondToServerRequest(
        request as AppServerServerRequest,
        {
          contentItems: [],
          success: true
        } as AppServerServerRequestResponse
      )
    ).rejects.toThrow('server request response does not match request method')

    expect(respondServerRequestMock).not.toHaveBeenCalled()
    expect(latestRuntime?.serverRequests).toEqual([request])
  })

  it('keeps queued server requests when respondServerRequest rejects', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = fileChangeApprovalRequest('file_change_1')
    const response = {
      decision: 'accept'
    } satisfies AppServerServerRequestResponse<'item/fileChange/requestApproval'>
    respondServerRequestMock.mockRejectedValueOnce(new Error('bridge failed'))
    await act(async () => {
      notificationListener?.(request)
    })

    await expect(latestRuntime?.respondToServerRequest(request, response)).rejects.toThrow(
      'bridge failed'
    )

    expect(respondServerRequestMock).toHaveBeenCalledWith('file_change_1', response)
    expect(latestRuntime?.serverRequests).toEqual([request])
  })

  it('rejects queued server requests with a fail-closed response and removes them from the queue', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = fileChangeApprovalRequest('file_change_1')
    await act(async () => {
      notificationListener?.(request)
    })

    await act(async () => {
      await latestRuntime?.rejectServerRequest(request)
    })

    expect(respondServerRequestMock).toHaveBeenCalledWith('file_change_1', {
      decision: 'decline'
    })
    expect(latestRuntime?.serverRequests).toEqual([])
  })

  it('queues dynamic tool calls without opening external URLs or responding in the background', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    const request = toolCallRequest('dynamic_tool_1', {
      url: 'https://example.test'
    })
    await act(async () => {
      notificationListener?.(request)
    })

    expect(latestRuntime?.serverRequests).toEqual([request])
    expect(openExternalHttpUrlMock).not.toHaveBeenCalled()
    expect(respondServerRequestMock).not.toHaveBeenCalled()
  })

  it('ignores unknown future request methods without queueing or responding', async () => {
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    await act(async () => {
      notificationListener?.({
        hostId: 'local',
        requestId: 'future_1',
        method: 'item/future/requestApproval',
        params: {}
      } as unknown as AppServerNotification)
    })

    expect(latestRuntime?.serverRequests).toEqual([])
    expect(respondServerRequestMock).not.toHaveBeenCalled()
  })

  it('maps app-server reasoning and agent text notifications to assistant-ui parts', async () => {
    requestMock.mockImplementation(async (method: string) => {
      if (method === 'thread/start') return { thread: { id: 'thread-1' } }
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
              turn: completedTurn('turn-1', 'final answer')
            }
          })
        })
        return { turn: inProgressTurn('turn-1') }
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

  it('updates the pending assistant message before turn completion arrives', async () => {
    requestMock.mockImplementation(async (method: string) => {
      if (method === 'thread/start') return { thread: { id: 'thread-1' } }
      if (method === 'turn/start') {
        queueMicrotask(() => {
          notificationListener?.({
            hostId: 'local',
            method: 'item/agentMessage/delta',
            params: {
              threadId: 'thread-1',
              turnId: 'turn-1',
              itemId: 'turn-1',
              delta: 'streamed'
            }
          })
        })
        return { turn: inProgressTurn('turn-1') }
      }
      throw new Error(`unexpected method ${method}`)
    })
    act(() => {
      root.render(createElement(RuntimeProbe))
    })

    let run: Promise<void> | undefined
    await act(async () => {
      run = runtimeAdapterCapture.latest?.onNew?.({
        role: 'user',
        content: [{ type: 'text', text: 'ping' }],
        attachments: [],
        createdAt: new Date('2026-01-01T00:00:00Z'),
        parentId: null,
        sourceId: null,
        runConfig: undefined,
        metadata: { custom: {} }
      })
      await Promise.resolve()
      await Promise.resolve()
    })

    const pendingAssistant = runtimeAdapterCapture.latest?.messages?.at(-1)
    expect(pendingAssistant?.role).toBe('assistant')
    expect(pendingAssistant?.content).toEqual([{ type: 'text', text: 'streamed' }])

    await act(async () => {
      notificationListener?.({
        hostId: 'local',
        method: 'turn/completed',
        params: {
          threadId: 'thread-1',
          turn: completedTurn('turn-1', 'streamed')
        }
      })
      await run
    })
  })
})

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

function fileChangeApprovalRequest(
  requestId: string
): AppServerServerRequest<'item/fileChange/requestApproval'> {
  return {
    hostId: 'local',
    requestId,
    method: 'item/fileChange/requestApproval',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'file_1',
      reason: 'modify src/app.ts'
    }
  }
}

function permissionsApprovalRequest(
  requestId: string
): AppServerServerRequest<'item/permissions/requestApproval'> {
  return {
    hostId: 'local',
    requestId,
    method: 'item/permissions/requestApproval',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'permission_1',
      cwd: '/workspace',
      permissions: ['net:fetch']
    }
  }
}

function toolCallRequest(
  requestId: string,
  args: unknown
): AppServerServerRequest<'item/tool/call'> {
  return {
    hostId: 'local',
    requestId,
    method: 'item/tool/call',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      callId: 'call_1',
      namespace: 'client',
      tool: 'open_url',
      arguments: args
    }
  }
}

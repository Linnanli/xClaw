import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction
} from 'react'
import {
  useExternalStoreRuntime,
  type AppendMessage,
  type AssistantRuntime,
  type ThreadMessage
} from '@assistant-ui/react'

import type {
  AppServerNotification,
  AppServerServerRequest,
  AppServerServerRequestMethod,
  AppServerServerRequestResponse,
  AppServerStatus,
  ModelProviderSelectForNextTurnResponse,
  RendererModelProviderConfig
} from '../../../shared/appServerApi'
import {
  type AssistantModelOption,
  assistantMessage,
  assistantMessageWithContent,
  extractTextFromAppendMessage,
  initialAssistantMessages,
  isPendingAssistantMessageContent,
  modelOptionsFromProviderConfig,
  pendingAssistantMessageText,
  userMessage
} from '../lib/assistantMessages'
import { createAppServerTurnTracker } from '../lib/appServerTurnTracker'
import type {
  AppServerAssistantContentPart,
  AppServerTurnTracker
} from '../lib/appServerTurnTracker'
import {
  failClosedServerRequestResponse,
  queueServerRequest,
  removeServerRequest
} from '../lib/serverRequests'

export type DasclawAssistantRuntime = {
  runtime: AssistantRuntime
  status: AppServerStatus | undefined
  serverRequests: readonly AppServerServerRequest[]
  respondToServerRequest: <Method extends AppServerServerRequestMethod>(
    request: AppServerServerRequest<Method>,
    response: AppServerServerRequestResponse<Method>
  ) => Promise<void>
  rejectServerRequest: (request: AppServerServerRequest) => Promise<void>
}

export type AppServerModelSelectorState = {
  models: readonly AssistantModelOption[]
  value: string | undefined
  onValueChange: (modelId: string) => Promise<void>
}

export function useAppServerModelSelectorState(): AppServerModelSelectorState {
  const [models, setModels] = useState<AssistantModelOption[]>(() =>
    window.desktopAppServer
      ? []
      : [modelProviderUnavailableOption('app-server bridge is unavailable')]
  )
  const [value, setValue] = useState<string | undefined>(undefined)

  useEffect(() => {
    let cancelled = false
    const appServer = window.desktopAppServer
    if (!appServer) {
      return () => {
        cancelled = true
      }
    }

    void appServer
      .request<RendererModelProviderConfig>('modelProvider/list')
      .then((config) => {
        if (cancelled) return
        if (config.unavailableReason) {
          setModels([modelProviderUnavailableOption(config.unavailableReason)])
          setValue(undefined)
          return
        }
        setModels(modelOptionsFromProviderConfig(config))
        setValue(config.selectedModelId)
      })
      .catch((error) => {
        if (cancelled) return
        setModels([modelProviderUnavailableOption(errorMessage(error))])
        setValue(undefined)
      })

    return () => {
      cancelled = true
    }
  }, [])

  const onValueChange = useCallback(async (modelId: string): Promise<void> => {
    if (!window.desktopAppServer) {
      throw new Error('app-server bridge is unavailable')
    }
    const response = await window.desktopAppServer.request<ModelProviderSelectForNextTurnResponse>(
      'modelProvider/selectForNextTurn',
      { modelId }
    )
    setValue(response.selectedModelId)
  }, [])

  return { models, value, onValueChange }
}

export function useDasclawAssistantRuntime(): DasclawAssistantRuntime {
  const [messages, setMessages] = useState<ThreadMessage[]>(initialAssistantMessages)
  const [isRunning, setIsRunning] = useState(false)
  const [status, setStatus] = useState<AppServerStatus>()
  const [serverRequests, setServerRequests] = useState<AppServerServerRequest[]>([])
  const threadIdRef = useRef<string | undefined>(undefined)
  const pendingTurnMessageIdsRef = useRef(new Map<string, string>())
  const turnTrackerRef = useRef<AppServerTurnTracker | undefined>(undefined)

  useEffect(() => {
    const turnTracker = createAppServerTurnTracker({
      onContentDelta: (turnId, part) => {
        const pendingId = pendingTurnMessageIdsRef.current.get(turnId)
        if (!pendingId) return
        appendPendingAssistantMessageContentDelta(setMessages, pendingId, part)
      }
    })
    turnTrackerRef.current = turnTracker
    void window.desktopAppServer.getStatus().then(setStatus)
    const removeStatusListener = window.desktopAppServer.onStatusChange(setStatus)
    const removeNotificationListener = window.desktopAppServer.onNotification((notification) => {
      if (isServerRequest(notification)) {
        setServerRequests((current) => queueServerRequest(current, notification))
        return
      }
      turnTracker.handleNotification(notification)
    })

    return () => {
      removeStatusListener()
      removeNotificationListener()
      turnTracker.clear()
      turnTrackerRef.current = undefined
    }
  }, [])

  const respondToServerRequest = useCallback(
    async <Method extends AppServerServerRequestMethod>(
      request: AppServerServerRequest<Method>,
      response: AppServerServerRequestResponse<Method>
    ) => {
      assertServerRequestResponseMatchesMethod(request.method, response)
      await window.desktopAppServer.respondServerRequest(request.requestId, response)
      setServerRequests((current) => removeServerRequest(current, request))
    },
    []
  )

  const rejectServerRequest = useCallback(async (request: AppServerServerRequest) => {
    await window.desktopAppServer.respondServerRequest(
      request.requestId,
      failClosedServerRequestResponse(request.method)
    )
    setServerRequests((current) => removeServerRequest(current, request))
  }, [])

  const runPromptTurn = useCallback(async (pendingId: string, prompt: string) => {
    const turnTracker = requireTurnTracker(turnTrackerRef)
    const threadId = await ensureThread(threadIdRef)
    const started = await window.desktopAppServer.request<unknown>('turn/start', {
      threadId,
      input: [{ type: 'text', text: prompt, textElements: [] }]
    })
    const turnId = readTurnId(started)
    pendingTurnMessageIdsRef.current.set(turnId, pendingId)
    for (const part of turnTracker.getTurnContent(turnId)) {
      appendPendingAssistantMessageContentDelta(setMessages, pendingId, part)
    }
    try {
      return await turnTracker.waitForTurnCompletion(turnId)
    } finally {
      pendingTurnMessageIdsRef.current.delete(turnId)
    }
  }, [])

  const resolvePendingPrompt = useCallback(
    async (pendingId: string, prompt: string) => {
      setIsRunning(true)

      try {
        const response = await runPromptTurn(pendingId, prompt)
        if (response.content && response.content.length > 0) {
          replacePendingAssistantMessageContent(setMessages, pendingId, response.content)
        } else {
          replacePendingAssistantMessage(
            setMessages,
            pendingId,
            response.output || 'dasclaw-app-server 已完成本轮，但没有返回文本输出。'
          )
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error)
        replacePendingAssistantMessage(setMessages, pendingId, `请求失败：${message}`, {
          type: 'incomplete',
          reason: 'error',
          error: message
        })
      } finally {
        setIsRunning(false)
      }
    },
    [runPromptTurn]
  )

  const onNew = useCallback(
    async (message: AppendMessage) => {
      const prompt = extractTextFromAppendMessage(message)
      if (!prompt) return

      const userId = `user-${crypto.randomUUID()}`
      const pendingId = `assistant-${crypto.randomUUID()}`
      setMessages((current) => [
        ...current,
        userMessage(userId, prompt),
        assistantMessage(pendingId, pendingAssistantMessageText, { type: 'running' })
      ])
      await resolvePendingPrompt(pendingId, prompt)
    },
    [resolvePendingPrompt]
  )

  const onEdit = useCallback(
    async (message: AppendMessage) => {
      const prompt = extractTextFromAppendMessage(message)
      const editedMessageId = message.parentId ?? message.sourceId
      if (!prompt || !editedMessageId) return

      const pendingId = `assistant-${crypto.randomUUID()}`
      setMessages((current) => {
        const editedIndex = current.findIndex((item) => item.id === editedMessageId)
        if (editedIndex === -1) return current

        return [
          ...current.slice(0, editedIndex),
          userMessage(editedMessageId, prompt),
          assistantMessage(pendingId, pendingAssistantMessageText, { type: 'running' })
        ]
      })
      await resolvePendingPrompt(pendingId, prompt)
    },
    [resolvePendingPrompt]
  )

  const runtime = useExternalStoreRuntime<ThreadMessage>(
    useMemo(
      () => ({
        messages,
        isRunning,
        onNew,
        onEdit
      }),
      [messages, isRunning, onNew, onEdit]
    )
  )

  return { runtime, status, serverRequests, respondToServerRequest, rejectServerRequest }
}

function modelProviderUnavailableOption(message: string): AssistantModelOption {
  return {
    id: 'model-provider-unavailable',
    name: '模型配置不可用',
    description: message,
    disabled: true
  }
}

function isServerRequest(
  notification: AppServerNotification
): notification is AppServerServerRequest {
  return 'requestId' in notification && isServerRequestMethod(notification.method)
}

const serverRequestMethods = {
  'item/commandExecution/requestApproval': true,
  'item/permissions/requestApproval': true,
  'item/fileChange/requestApproval': true,
  'item/tool/requestUserInput': true,
  'item/tool/call': true
} satisfies Record<AppServerServerRequestMethod, true>

function isServerRequestMethod(method: string): method is AppServerServerRequestMethod {
  return method in serverRequestMethods
}

function assertServerRequestResponseMatchesMethod(
  method: AppServerServerRequestMethod,
  response: AppServerServerRequestResponse
): void {
  if (isServerRequestResponseForMethod(method, response)) return
  throw new Error('server request response does not match request method')
}

function isServerRequestResponseForMethod(
  method: AppServerServerRequestMethod,
  response: AppServerServerRequestResponse
): boolean {
  if (!isRecord(response)) return false
  const record = response as Record<string, unknown>

  switch (method) {
    case 'item/fileChange/requestApproval':
      return isFileChangeApprovalResponse(record)
    case 'item/tool/call':
      return typeof record.success === 'boolean' && Array.isArray(record.contentItems)
    case 'item/tool/requestUserInput':
      return isRecord(record.answers)
    case 'item/permissions/requestApproval':
      return isPermissionsApprovalResponse(record)
    case 'item/commandExecution/requestApproval':
      return isRecord(record.decision) && typeof record.decision.kind === 'string'
  }
}

function isFileChangeApprovalResponse(response: Record<string, unknown>): boolean {
  return (
    response.decision === 'accept' ||
    response.decision === 'acceptForSession' ||
    response.decision === 'decline' ||
    response.decision === 'cancel'
  )
}

function isPermissionsApprovalResponse(response: Record<string, unknown>): boolean {
  return (
    (response.decision === 'approve' || response.decision === 'reject') &&
    isJsonContainer(response.permissions) &&
    isOptionalPermissionScope(response.scope) &&
    isOptionalBoolean(response.strictAutoReview)
  )
}

function isOptionalPermissionScope(value: unknown): boolean {
  return value === undefined || value === 'turn' || value === 'session'
}

function isOptionalBoolean(value: unknown): boolean {
  return value === undefined || typeof value === 'boolean'
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isJsonContainer(value: unknown): value is Record<string, unknown> | unknown[] {
  return typeof value === 'object' && value !== null
}

function requireTurnTracker(
  turnTrackerRef: MutableRefObject<AppServerTurnTracker | undefined>
): AppServerTurnTracker {
  if (!turnTrackerRef.current) throw new Error('app-server turn tracker is not initialized')
  return turnTrackerRef.current
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function replacePendingAssistantMessage(
  setMessages: Dispatch<SetStateAction<ThreadMessage[]>>,
  pendingId: string,
  text: string,
  status?: Parameters<typeof assistantMessage>[2]
): void {
  setMessages((current) =>
    current.map((item) =>
      item.id === pendingId ? assistantMessage(pendingId, text, status) : item
    )
  )
}

function replacePendingAssistantMessageContent(
  setMessages: Dispatch<SetStateAction<ThreadMessage[]>>,
  pendingId: string,
  content: Parameters<typeof assistantMessageWithContent>[1],
  status?: Parameters<typeof assistantMessageWithContent>[2]
): void {
  setMessages((current) =>
    current.map((item) =>
      item.id === pendingId ? assistantMessageWithContent(pendingId, content, status) : item
    )
  )
}

function appendPendingAssistantMessageContentDelta(
  setMessages: Dispatch<SetStateAction<ThreadMessage[]>>,
  pendingId: string,
  part: AppServerAssistantContentPart
): void {
  if (!part.text) return
  setMessages((current) =>
    current.map((item) => {
      if (item.id !== pendingId || item.role !== 'assistant') return item
      const content = item.content.slice()
      if (isPendingAssistantMessageContent(content)) content.length = 0
      const previous = content.at(-1)
      if (previous?.type === part.type && 'text' in previous) {
        content[content.length - 1] = {
          ...previous,
          text: `${previous.text}${part.text}`
        }
      } else {
        content.push({ type: part.type, text: part.text } as (typeof content)[number])
      }
      return assistantMessageWithContent(pendingId, content, { type: 'running' })
    })
  )
}

async function ensureThread(threadIdRef: MutableRefObject<string | undefined>): Promise<string> {
  if (threadIdRef.current) return threadIdRef.current

  const response = await window.desktopAppServer.request<unknown>('thread/start', {})
  const threadId = readThreadId(response)
  threadIdRef.current = threadId
  return threadId
}

function readThreadId(response: unknown): string {
  if (!response || typeof response !== 'object') {
    throw new Error('thread/start returned an invalid response')
  }
  const record = response as Record<string, unknown>
  const thread = record.thread
  if (thread && typeof thread === 'object') {
    const threadId = (thread as { id?: unknown }).id
    if (typeof threadId === 'string' && threadId.trim()) return threadId
  }
  throw new Error('thread/start response did not include thread.id')
}

function readTurnId(response: unknown): string {
  if (!response || typeof response !== 'object') {
    throw new Error('turn/start returned an invalid response')
  }
  const record = response as Record<string, unknown>
  const turn = record.turn
  if (turn && typeof turn === 'object') {
    const turnId = (turn as { id?: unknown }).id
    if (typeof turnId === 'string' && turnId.trim()) return turnId
  }
  throw new Error('turn/start response did not include turn.id')
}

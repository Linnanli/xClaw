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
  modelOptionsFromProviderConfig,
  userMessage
} from '../lib/assistantMessages'
import { createAppServerTurnTracker } from '../lib/appServerTurnTracker'
import type { AppServerAssistantContentPart } from '../lib/appServerTurnTracker'

type DasclawAssistantRuntime = {
  runtime: AssistantRuntime
  status: AppServerStatus | undefined
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
  const threadIdRef = useRef<string | undefined>(undefined)
  const pendingTurnMessageIdsRef = useRef(new Map<string, string>())
  const turnTrackerRef = useRef(
    createAppServerTurnTracker({
      onContentDelta: (turnId, part) => {
        const pendingId = pendingTurnMessageIdsRef.current.get(turnId)
        if (!pendingId) return
        appendPendingAssistantMessageContentDelta(setMessages, pendingId, part)
      }
    })
  )

  useEffect(() => {
    const turnTracker = turnTrackerRef.current
    void window.desktopAppServer.getStatus().then(setStatus)
    const removeStatusListener = window.desktopAppServer.onStatusChange(setStatus)
    const removeNotificationListener = window.desktopAppServer.onNotification((notification) => {
      turnTracker.handleNotification(notification)
    })

    return () => {
      removeStatusListener()
      removeNotificationListener()
      turnTracker.clear()
    }
  }, [])

  const runPromptTurn = useCallback(async (pendingId: string, prompt: string) => {
    const threadId = await ensureThread(threadIdRef, prompt)
    const started = await window.desktopAppServer.request<{ turnId: string }>('turn/start', {
      threadId,
      input: [{ type: 'text', text: prompt }]
    })
    pendingTurnMessageIdsRef.current.set(started.turnId, pendingId)
    for (const part of turnTrackerRef.current.getTurnContent(started.turnId)) {
      appendPendingAssistantMessageContentDelta(setMessages, pendingId, part)
    }
    try {
      return await turnTrackerRef.current.waitForTurnCompletion(started.turnId)
    } finally {
      pendingTurnMessageIdsRef.current.delete(started.turnId)
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
        assistantMessage(pendingId, 'Dasclaw 正在处理...', { type: 'running' })
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
          assistantMessage(pendingId, 'Dasclaw 正在处理...', { type: 'running' })
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

  return { runtime, status }
}

function modelProviderUnavailableOption(message: string): AssistantModelOption {
  return {
    id: 'model-provider-unavailable',
    name: '模型配置不可用',
    description: message,
    disabled: true
  }
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
      const placeholder =
        content.length === 1 &&
        content[0]?.type === 'text' &&
        content[0].text === 'Dasclaw 正在处理...'
      if (placeholder) content.length = 0
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

async function ensureThread(
  threadIdRef: MutableRefObject<string | undefined>,
  prompt: string
): Promise<string> {
  if (threadIdRef.current) return threadIdRef.current

  const title = prompt.length > 48 ? `${prompt.slice(0, 45)}...` : prompt
  const response = await window.desktopAppServer.request<{ threadId: string }>('thread/start', {
    title
  })
  threadIdRef.current = response.threadId
  return response.threadId
}

import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  useExternalStoreRuntime,
  type AppendMessage,
  type AssistantRuntime,
  type ThreadMessage
} from '@assistant-ui/react'

import type { AppServerStatus } from '../../../shared/appServerApi'
import {
  assistantMessage,
  extractTextFromAppendMessage,
  initialAssistantMessages,
  userMessage
} from '../lib/assistantMessages'

type DasclawAssistantRuntime = {
  runtime: AssistantRuntime
  status: AppServerStatus | undefined
}

export function useDasclawAssistantRuntime(): DasclawAssistantRuntime {
  const [messages, setMessages] = useState<ThreadMessage[]>(initialAssistantMessages)
  const [isRunning, setIsRunning] = useState(false)
  const [status, setStatus] = useState<AppServerStatus>()

  useEffect(() => {
    void window.desktopAppServer.getStatus().then(setStatus)
    return window.desktopAppServer.onStatusChange(setStatus)
  }, [])

  const onNew = useCallback(async (message: AppendMessage) => {
    const prompt = extractTextFromAppendMessage(message)
    if (!prompt) return

    const userId = `user-${crypto.randomUUID()}`
    const pendingId = `assistant-${crypto.randomUUID()}`
    setMessages((current) => [
      ...current,
      userMessage(userId, prompt),
      assistantMessage(pendingId, 'Dasclaw 正在处理...', { type: 'running' })
    ])
    setIsRunning(true)

    try {
      const response = await window.desktopAppServer.sendMessage(prompt)
      setMessages((current) =>
        current.map((item) =>
          item.id === pendingId
            ? assistantMessage(
                pendingId,
                response.output || 'dasclaw-app-server 已完成本轮，但没有返回文本输出。'
              )
            : item
        )
      )
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      setMessages((current) =>
        current.map((item) =>
          item.id === pendingId
            ? assistantMessage(pendingId, `请求失败：${message}`, {
                type: 'incomplete',
                reason: 'error',
                error: message
              })
            : item
        )
      )
    } finally {
      setIsRunning(false)
    }
  }, [])

  const runtime = useExternalStoreRuntime<ThreadMessage>(
    useMemo(
      () => ({
        messages,
        isRunning,
        onNew
      }),
      [messages, isRunning, onNew]
    )
  )

  return { runtime, status }
}

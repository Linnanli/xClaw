import type { AppServerNotification } from '../../../shared/appServerApi'

export type AppServerAssistantContentPart =
  | { type: 'text'; text: string }
  | { type: 'reasoning'; text: string }

export type TurnCompletion = {
  threadId: string
  turnId: string
  output?: string
  error?: string
  content?: AppServerAssistantContentPart[]
}

type TurnTrackerOptions = {
  timeoutMs?: number
  onContentDelta?: (turnId: string, part: AppServerAssistantContentPart) => void
}

type PendingTurn = {
  timeout: ReturnType<typeof setTimeout>
  resolve: (completion: TurnCompletion) => void
  reject: (error: Error) => void
}

export type AppServerTurnTracker = {
  waitForTurnCompletion(turnId: string): Promise<TurnCompletion>
  getTurnContent(turnId: string): readonly AppServerAssistantContentPart[]
  handleNotification(notification: AppServerNotification): void
  clear(error?: Error): void
}

const DEFAULT_TURN_COMPLETION_TIMEOUT_MS = 120_000

export function createAppServerTurnTracker(options: TurnTrackerOptions = {}): AppServerTurnTracker {
  const timeoutMs = options.timeoutMs ?? DEFAULT_TURN_COMPLETION_TIMEOUT_MS
  const onContentDelta = options.onContentDelta
  const pendingTurns = new Map<string, PendingTurn>()
  const completedTurns = new Map<string, TurnCompletion>()
  const turnContent = new Map<string, AppServerAssistantContentPart[]>()

  function waitForTurnCompletion(turnId: string): Promise<TurnCompletion> {
    const completed = completedTurns.get(turnId)
    if (completed) {
      completedTurns.delete(turnId)
      return resolveCompletion(completed)
    }

    return new Promise<TurnCompletion>((resolve, reject) => {
      const timeout = setTimeout(() => {
        pendingTurns.delete(turnId)
        reject(new Error('等待 dasclaw-app-server 响应超时'))
      }, timeoutMs)

      pendingTurns.set(turnId, { timeout, resolve, reject })
    })
  }

  function handleNotification(notification: AppServerNotification): void {
    const contentDelta = parseTurnContentDelta(notification)
    if (contentDelta) {
      appendTurnContentDelta(contentDelta.turnId, contentDelta.part)
      onContentDelta?.(contentDelta.turnId, contentDelta.part)
      return
    }

    if (notification.method !== 'turn/completed' && notification.method !== 'turn/failed') return

    const completion = parseTurnCompletion(notification.params)
    if (!completion) return
    const content = turnContent.get(completion.turnId)
    if (content && content.length > 0) {
      completion.content = content.map((part) => ({ ...part }))
      turnContent.delete(completion.turnId)
    }

    const pending = pendingTurns.get(completion.turnId)
    if (!pending) {
      completedTurns.set(completion.turnId, completion)
      return
    }

    pendingTurns.delete(completion.turnId)
    clearTimeout(pending.timeout)
    if (completion.error) {
      pending.reject(new Error(completion.error))
    } else {
      pending.resolve(completion)
    }
  }

  function clear(error = new Error('dasclaw-app-server 已停止')): void {
    for (const pending of pendingTurns.values()) {
      clearTimeout(pending.timeout)
      pending.reject(error)
    }
    pendingTurns.clear()
    completedTurns.clear()
    turnContent.clear()
  }

  function appendTurnContentDelta(turnId: string, part: AppServerAssistantContentPart): void {
    const content = turnContent.get(turnId) ?? []
    appendPart(content, part)
    turnContent.set(turnId, content)
  }

  function getTurnContent(turnId: string): readonly AppServerAssistantContentPart[] {
    return (turnContent.get(turnId) ?? []).map((part) => ({ ...part }))
  }

  return {
    waitForTurnCompletion,
    getTurnContent,
    handleNotification,
    clear
  }
}

function parseTurnCompletion(params: unknown): TurnCompletion | undefined {
  if (!params || typeof params !== 'object') return undefined

  const record = params as Record<string, unknown>
  if (typeof record.threadId !== 'string' || typeof record.turnId !== 'string') return undefined

  return {
    threadId: record.threadId,
    turnId: record.turnId,
    output: typeof record.output === 'string' ? record.output : undefined,
    error: typeof record.error === 'string' ? record.error : undefined
  }
}

function parseTurnContentDelta(
  notification: AppServerNotification
): { turnId: string; part: AppServerAssistantContentPart } | undefined {
  if (
    notification.method !== 'item/agentMessage/delta' &&
    notification.method !== 'item/reasoning/summaryTextDelta'
  ) {
    return undefined
  }
  if (!notification.params || typeof notification.params !== 'object') return undefined

  const record = notification.params as Record<string, unknown>
  if (typeof record.turnId !== 'string' || typeof record.delta !== 'string') return undefined

  return {
    turnId: record.turnId,
    part: {
      type: notification.method === 'item/reasoning/summaryTextDelta' ? 'reasoning' : 'text',
      text: record.delta
    }
  }
}

function appendPart(
  parts: AppServerAssistantContentPart[],
  part: AppServerAssistantContentPart
): void {
  if (!part.text) return
  const previous = parts.at(-1)
  if (previous?.type === part.type) {
    previous.text += part.text
  } else {
    parts.push({ ...part })
  }
}

function resolveCompletion(completion: TurnCompletion): Promise<TurnCompletion> {
  if (completion.error) return Promise.reject(new Error(completion.error))
  return Promise.resolve(completion)
}

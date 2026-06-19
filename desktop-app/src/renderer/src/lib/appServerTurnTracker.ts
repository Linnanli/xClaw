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

type TurnContentDelta = {
  turnId: string
  part: AppServerAssistantContentPart
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
  const pendingReasoningSectionBreaks = new Set<string>()

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

    const reasoningSectionBreak = parseReasoningSectionBreak(notification)
    if (reasoningSectionBreak) {
      pendingReasoningSectionBreaks.add(reasoningSectionBreak.turnId)
      return
    }

    if (notification.method !== 'turn/completed') return

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
    pendingReasoningSectionBreaks.clear()
  }

  function appendTurnContentDelta(turnId: string, part: AppServerAssistantContentPart): void {
    const content = turnContent.get(turnId) ?? []
    const forceNewPart = part.type === 'reasoning' && pendingReasoningSectionBreaks.delete(turnId)
    appendPart(content, part, forceNewPart)
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
  const threadId = typeof record.threadId === 'string' ? record.threadId : undefined
  const turn =
    record.turn && typeof record.turn === 'object'
      ? (record.turn as Record<string, unknown>)
      : undefined
  const turnId = typeof turn?.id === 'string' ? turn.id : undefined
  const errorRecord =
    turn?.error && typeof turn.error === 'object'
      ? (turn.error as { message?: unknown })
      : undefined
  const error = typeof errorRecord?.message === 'string' ? errorRecord.message : undefined
  const output = readAgentMessageOutput(turn?.items)

  if (!threadId || !turnId) return undefined

  return {
    threadId,
    turnId,
    output,
    error
  }
}

function readAgentMessageOutput(items: unknown): string | undefined {
  if (!Array.isArray(items)) return undefined
  const text = items
    .map((item) => {
      if (!item || typeof item !== 'object') return ''
      const record = item as { type?: unknown; text?: unknown }
      return record.type === 'agentMessage' && typeof record.text === 'string' ? record.text : ''
    })
    .join('')
  return text.length > 0 ? text : undefined
}

function parseTurnContentDelta(notification: AppServerNotification): TurnContentDelta | undefined {
  if (
    notification.method !== 'item/agentMessage/delta' &&
    notification.method !== 'item/reasoning/summaryTextDelta' &&
    notification.method !== 'item/reasoning/textDelta'
  ) {
    return undefined
  }
  if (!notification.params || typeof notification.params !== 'object') return undefined

  const record = notification.params as Record<string, unknown>
  if (typeof record.turnId !== 'string' || typeof record.delta !== 'string') return undefined

  return {
    turnId: record.turnId,
    part: {
      type: isReasoningDeltaMethod(notification.method) ? 'reasoning' : 'text',
      text: record.delta
    }
  }
}

function parseReasoningSectionBreak(
  notification: AppServerNotification
): { turnId: string } | undefined {
  if (notification.method !== 'item/reasoning/summaryPartAdded') return undefined
  if (!notification.params || typeof notification.params !== 'object') return undefined

  const record = notification.params as Record<string, unknown>
  if (typeof record.turnId !== 'string') return undefined

  return { turnId: record.turnId }
}

function isReasoningDeltaMethod(method: string): boolean {
  return method === 'item/reasoning/summaryTextDelta' || method === 'item/reasoning/textDelta'
}

function appendPart(
  parts: AppServerAssistantContentPart[],
  part: AppServerAssistantContentPart,
  forceNewPart = false
): void {
  if (!part.text) return
  const previous = parts.at(-1)
  if (!forceNewPart && previous?.type === part.type) {
    previous.text += part.text
  } else {
    parts.push({ ...part })
  }
}

function resolveCompletion(completion: TurnCompletion): Promise<TurnCompletion> {
  if (completion.error) return Promise.reject(new Error(completion.error))
  return Promise.resolve(completion)
}

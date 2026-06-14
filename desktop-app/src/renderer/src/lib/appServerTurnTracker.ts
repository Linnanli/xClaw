import type { AppServerNotification } from '../../../shared/appServerApi'

export type TurnCompletion = {
  threadId: string
  turnId: string
  output?: string
  error?: string
}

type TurnTrackerOptions = {
  timeoutMs?: number
}

type PendingTurn = {
  timeout: ReturnType<typeof setTimeout>
  resolve: (completion: TurnCompletion) => void
  reject: (error: Error) => void
}

export type AppServerTurnTracker = {
  waitForTurnCompletion(turnId: string): Promise<TurnCompletion>
  handleNotification(notification: AppServerNotification): void
  clear(error?: Error): void
}

const DEFAULT_TURN_COMPLETION_TIMEOUT_MS = 120_000

export function createAppServerTurnTracker(options: TurnTrackerOptions = {}): AppServerTurnTracker {
  const timeoutMs = options.timeoutMs ?? DEFAULT_TURN_COMPLETION_TIMEOUT_MS
  const pendingTurns = new Map<string, PendingTurn>()
  const completedTurns = new Map<string, TurnCompletion>()

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
    if (notification.method !== 'turn/completed' && notification.method !== 'turn/failed') return

    const completion = parseTurnCompletion(notification.params)
    if (!completion) return

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
  }

  return {
    waitForTurnCompletion,
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

function resolveCompletion(completion: TurnCompletion): Promise<TurnCompletion> {
  if (completion.error) return Promise.reject(new Error(completion.error))
  return Promise.resolve(completion)
}

import { describe, expect, it, vi } from 'vitest'

import { createAppServerTurnTracker } from './appServerTurnTracker'

describe('createAppServerTurnTracker', () => {
  it('resolves a pending turn from app-server completion notifications', async () => {
    const tracker = createAppServerTurnTracker()
    const completion = tracker.waitForTurnCompletion('turn-1')

    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        output: 'pong'
      }
    })

    await expect(completion).resolves.toEqual({
      threadId: 'thread-1',
      turnId: 'turn-1',
      output: 'pong'
    })
  })

  it('keeps early completion notifications until the renderer waits for that turn', async () => {
    const tracker = createAppServerTurnTracker()

    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        output: 'pong'
      }
    })

    await expect(tracker.waitForTurnCompletion('turn-1')).resolves.toEqual({
      threadId: 'thread-1',
      turnId: 'turn-1',
      output: 'pong'
    })
  })

  it('rejects waiting turns when the app-server reports failure', async () => {
    const tracker = createAppServerTurnTracker()
    const completion = tracker.waitForTurnCompletion('turn-1')

    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/failed',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        error: 'tool failed'
      }
    })

    await expect(completion).rejects.toThrow('tool failed')
  })

  it('times out when no terminal notification arrives', async () => {
    vi.useFakeTimers()
    const tracker = createAppServerTurnTracker({ timeoutMs: 100 })
    const completion = tracker.waitForTurnCompletion('turn-1')
    const rejection = expect(completion).rejects.toThrow('等待 dasclaw-app-server 响应超时')

    await vi.advanceTimersByTimeAsync(100)

    await rejection
    vi.useRealTimers()
  })
})

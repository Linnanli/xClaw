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
        turn: completedTurn('turn-1', 'pong')
      }
    })

    await expect(completion).resolves.toEqual({
      threadId: 'thread-1',
      turnId: 'turn-1',
      output: 'pong'
    })
  })

  it('resolves a pending turn from Codex nested completion notifications', async () => {
    const tracker = createAppServerTurnTracker()
    const completion = tracker.waitForTurnCompletion('turn-1')

    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turn: completedTurn('turn-1')
      }
    })

    await expect(completion).resolves.toEqual({
      threadId: 'thread-1',
      turnId: 'turn-1'
    })
  })

  it('resolves structured reasoning and agent text as separate assistant parts', async () => {
    const tracker = createAppServerTurnTracker()
    const completion = tracker.waitForTurnCompletion('turn-1')

    tracker.handleNotification({
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
    tracker.handleNotification({
      hostId: 'local',
      method: 'item/agentMessage/delta',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        itemId: 'turn-1',
        delta: 'final answer'
      }
    })
    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turn: completedTurn('turn-1', 'final answer')
      }
    })

    await expect(completion).resolves.toEqual({
      threadId: 'thread-1',
      turnId: 'turn-1',
      output: 'final answer',
      content: [
        { type: 'reasoning', text: 'private scratch' },
        { type: 'text', text: 'final answer' }
      ]
    })
  })

  it('keeps backend text deltas literal instead of stripping think tags in the renderer', async () => {
    const tracker = createAppServerTurnTracker()
    const completion = tracker.waitForTurnCompletion('turn-1')

    tracker.handleNotification({
      hostId: 'local',
      method: 'item/agentMessage/delta',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        itemId: 'turn-1',
        delta: '<think>backend bug</think>visible'
      }
    })
    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turn: completedTurn('turn-1', '<think>backend bug</think>visible')
      }
    })

    await expect(completion).resolves.toMatchObject({
      content: [{ type: 'text', text: '<think>backend bug</think>visible' }]
    })
  })

  it('tracks reasoning section boundaries without creating visible text parts', async () => {
    const tracker = createAppServerTurnTracker()
    const completion = tracker.waitForTurnCompletion('turn-1')

    tracker.handleNotification({
      hostId: 'local',
      method: 'item/reasoning/summaryTextDelta',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        itemId: 'turn-1:reasoning',
        summaryIndex: 0,
        delta: 'first section'
      }
    })
    tracker.handleNotification({
      hostId: 'local',
      method: 'item/reasoning/summaryPartAdded',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        itemId: 'turn-1:reasoning',
        summaryIndex: 1
      }
    })
    tracker.handleNotification({
      hostId: 'local',
      method: 'item/reasoning/summaryTextDelta',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        itemId: 'turn-1:reasoning',
        summaryIndex: 1,
        delta: 'second section'
      }
    })
    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turn: completedTurn('turn-1')
      }
    })

    await expect(completion).resolves.toMatchObject({
      content: [
        { type: 'reasoning', text: 'first section' },
        { type: 'reasoning', text: 'second section' }
      ]
    })
  })

  it('maps raw reasoning text deltas to reasoning parts without polluting agent text', async () => {
    const tracker = createAppServerTurnTracker()
    const completion = tracker.waitForTurnCompletion('turn-1')

    tracker.handleNotification({
      hostId: 'local',
      method: 'item/reasoning/textDelta',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        itemId: 'turn-1:reasoning',
        contentIndex: 0,
        delta: 'raw detail'
      }
    })
    tracker.handleNotification({
      hostId: 'local',
      method: 'item/agentMessage/delta',
      params: {
        threadId: 'thread-1',
        turnId: 'turn-1',
        itemId: 'turn-1',
        delta: 'visible'
      }
    })
    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turn: completedTurn('turn-1', 'visible')
      }
    })

    await expect(completion).resolves.toMatchObject({
      content: [
        { type: 'reasoning', text: 'raw detail' },
        { type: 'text', text: 'visible' }
      ]
    })
  })

  it('keeps early completion notifications until the renderer waits for that turn', async () => {
    const tracker = createAppServerTurnTracker()

    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turn: completedTurn('turn-1', 'pong')
      }
    })

    await expect(tracker.waitForTurnCompletion('turn-1')).resolves.toEqual({
      threadId: 'thread-1',
      turnId: 'turn-1',
      output: 'pong'
    })
  })

  it('rejects waiting turns when completed turn notifications include an error message', async () => {
    const tracker = createAppServerTurnTracker()
    const completion = tracker.waitForTurnCompletion('turn-1')

    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turn: failedTurn('turn-1', 'tool failed')
      }
    })

    await expect(completion).rejects.toThrow('tool failed')
  })

  it('keeps early failure notifications until the renderer waits for that turn', async () => {
    const tracker = createAppServerTurnTracker()

    tracker.handleNotification({
      hostId: 'local',
      method: 'turn/completed',
      params: {
        threadId: 'thread-1',
        turn: failedTurn('turn-1', 'tool failed')
      }
    })

    await expect(tracker.waitForTurnCompletion('turn-1')).rejects.toThrow('tool failed')
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

function failedTurn(id: string, message: string): Record<string, unknown> {
  return {
    id,
    items: [],
    status: 'failed',
    error: {
      message,
      codexErrorInfo: null,
      additionalDetails: null
    },
    startedAt: null,
    completedAt: null,
    durationMs: null
  }
}

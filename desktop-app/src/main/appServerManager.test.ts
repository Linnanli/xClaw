import { describe, expect, it } from 'vitest'

import { AppServerManager } from './appServerManager'
import type { AppServerRpcClient, JsonRpcNotification } from './appServerRpc'

class FakeRpcClient implements AppServerRpcClient {
  readonly requests: Array<{ method: string; params?: unknown }> = []
  disposed = false
  private notificationHandler: ((notification: JsonRpcNotification) => void) | undefined

  async request<T>(method: string, params?: unknown): Promise<T> {
    this.requests.push({ method, params })

    if (method === 'initialize') {
      return { ok: true } as T
    }
    if (method === 'health/check') {
      return { ok: true, lifecycle: { state: 'ready' }, services: [] } as T
    }
    if (method === 'thread/start') {
      return { threadId: 'thread-1' } as T
    }
    if (method === 'turn/start') {
      queueMicrotask(() => {
        this.notificationHandler?.({
          type: 'notification',
          method: 'turn/delta',
          params: { threadId: 'thread-1', turnId: 'turn-1', delta: 'pong' }
        })
        this.notificationHandler?.({
          type: 'notification',
          method: 'turn/completed',
          params: {
            threadId: 'thread-1',
            turnId: 'turn-1',
            status: 'completed',
            output: 'pong'
          }
        })
      })
      return { turnId: 'turn-1', status: 'pending' } as T
    }
    if (method === 'shutdown') {
      return { accepted: true } as T
    }

    throw new Error(`unexpected method ${method}`)
  }

  onNotification(handler: (notification: JsonRpcNotification) => void): () => void {
    this.notificationHandler = handler
    return () => {
      this.notificationHandler = undefined
    }
  }

  dispose(): void {
    this.disposed = true
  }
}

class FailingInitializeRpcClient extends FakeRpcClient {
  async request<T>(method: string, params?: unknown): Promise<T> {
    this.requests.push({ method, params })
    if (method === 'initialize') throw new Error('init failed')
    return super.request(method, params)
  }
}

describe('AppServerManager', () => {
  it('initializes, starts a thread, and resolves a chat turn from notifications', async () => {
    const fake = new FakeRpcClient()
    const manager = new AppServerManager({
      createClient: () => fake,
      binary: '/tmp/dasclaw-app-server'
    })

    const status = await manager.start()
    const response = await manager.sendMessage('ping')

    expect(status.state).toBe('ready')
    expect(response).toEqual({
      threadId: 'thread-1',
      turnId: 'turn-1',
      output: 'pong'
    })
    expect(fake.requests.map((request) => request.method)).toEqual([
      'initialize',
      'health/check',
      'thread/start',
      'turn/start'
    ])
    expect(fake.requests.at(-1)?.params).toEqual({
      threadId: 'thread-1',
      input: [{ type: 'text', text: 'ping' }]
    })
  })

  it('disposes the client when initialize fails', async () => {
    const fake = new FailingInitializeRpcClient()
    const manager = new AppServerManager({
      createClient: () => fake,
      binary: '/tmp/dasclaw-app-server'
    })

    const status = await manager.start()

    expect(status.state).toBe('failed')
    expect(status.lastError).toBe('init failed')
    expect(fake.disposed).toBe(true)
  })
})

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { api } from '@/lib/api'

describe('API 客户端', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  it('应配置 15 秒超时', () => {
    expect(api.defaults.timeout).toBe(15000)
  })

  it('应设置 JSON Content-Type', () => {
    expect(api.defaults.headers['Content-Type']).toBe('application/json')
  })

  it('请求拦截器应在有 token 时添加 Authorization 头', async () => {
    localStorage.setItem('auth_token', 'test-bearer-token')

    // 手动执行请求拦截器
    const config = {
      headers: {
        set: vi.fn(),
        get: vi.fn(),
        has: vi.fn(),
        delete: vi.fn(),
        clear: vi.fn(),
        normalize: vi.fn(),
        concat: vi.fn(),
        toJSON: vi.fn(),
        Authorization: undefined as string | undefined,
      },
    }

    // 获取请求拦截器并执行
    const interceptor = api.interceptors.request as unknown as {
      handlers: Array<{ fulfilled: (config: unknown) => unknown }>
    }
    const handler = interceptor.handlers[0]
    if (handler?.fulfilled) {
      const result = handler.fulfilled(config) as typeof config
      expect(result.headers.Authorization).toBe('Bearer test-bearer-token')
    }
  })

  it('请求拦截器在无 token 时不应添加 Authorization 头', () => {
    const config = {
      headers: {
        Authorization: undefined as string | undefined,
      },
    }

    const interceptor = api.interceptors.request as unknown as {
      handlers: Array<{ fulfilled: (config: unknown) => unknown }>
    }
    const handler = interceptor.handlers[0]
    if (handler?.fulfilled) {
      const result = handler.fulfilled(config) as typeof config
      expect(result.headers.Authorization).toBeUndefined()
    }
  })

  it('401 响应应清除 token 并重定向到登录页', async () => {
    localStorage.setItem('auth_token', 'expired-token')

    // Mock window.location
    const originalLocation = window.location
    Object.defineProperty(window, 'location', {
      writable: true,
      value: { ...originalLocation, href: '' },
    })

    const interceptor = api.interceptors.response as unknown as {
      handlers: Array<{ rejected: (error: unknown) => Promise<unknown> }>
    }
    const handler = interceptor.handlers[0]

    if (handler?.rejected) {
      const error = { response: { status: 401 } }
      await expect(handler.rejected(error)).rejects.toEqual(error)
      expect(localStorage.getItem('auth_token')).toBeNull()
      expect(window.location.href).toBe('/login')
    }

    // 恢复
    Object.defineProperty(window, 'location', {
      writable: true,
      value: originalLocation,
    })
  })
})

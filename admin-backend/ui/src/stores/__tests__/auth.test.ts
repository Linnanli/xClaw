import { describe, it, expect, beforeEach, vi } from 'vitest'
import { useAuthStore } from '@/stores/auth'

// Mock axios
vi.mock('@/lib/api', () => ({
  api: {
    post: vi.fn(),
  },
}))

describe('useAuthStore', () => {
  beforeEach(() => {
    // 重置 store 状态
    useAuthStore.setState({
      token: null,
      user: null,
      isAuthenticated: false,
    })
    localStorage.clear()
    vi.clearAllMocks()
  })

  it('初始状态应为未认证', () => {
    const state = useAuthStore.getState()
    expect(state.token).toBeNull()
    expect(state.user).toBeNull()
    expect(state.isAuthenticated).toBe(false)
  })

  it('login 成功后应设置 token 和用户信息', async () => {
    const { api } = await import('@/lib/api')
    const mockUser = {
      id: '1',
      username: 'admin',
      email: 'admin@test.com',
      role: 'admin',
      mfa_enabled: false,
      status: 'active' as const,
      created_at: '2025-01-01',
    }

    vi.mocked(api.post).mockResolvedValueOnce({
      data: { token: 'test-token-123', user: mockUser },
    })

    await useAuthStore.getState().login({
      username: 'admin',
      password: 'password',
    })

    const state = useAuthStore.getState()
    expect(state.token).toBe('test-token-123')
    expect(state.user).toEqual(mockUser)
    expect(state.isAuthenticated).toBe(true)
    expect(localStorage.getItem('auth_token')).toBe('test-token-123')
  })

  it('login 失败时在开发模式下应 fallback 到 mock 登录', async () => {
    const { api } = await import('@/lib/api')
    vi.mocked(api.post).mockRejectedValueOnce(new Error('Invalid credentials'))

    await useAuthStore.getState().login({
      username: 'admin',
      password: 'wrong',
    })

    // 开发模式下 fallback 到 mock，应该成功登录
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(true)
    expect(state.user?.username).toBe('admin')
    expect(state.token).toBeTruthy()
  })

  it('logout 应清除所有认证状态', () => {
    // 先设置已认证状态
    useAuthStore.setState({
      token: 'some-token',
      user: {
        id: '1',
        username: 'admin',
        email: 'admin@test.com',
        role: 'admin',
        mfa_enabled: false,
        status: 'active',
        created_at: '2025-01-01',
      },
      isAuthenticated: true,
    })
    localStorage.setItem('auth_token', 'some-token')

    useAuthStore.getState().logout()

    const state = useAuthStore.getState()
    expect(state.token).toBeNull()
    expect(state.user).toBeNull()
    expect(state.isAuthenticated).toBe(false)
    expect(localStorage.getItem('auth_token')).toBeNull()
  })

  it('setToken 应更新 token 和认证状态', () => {
    useAuthStore.getState().setToken('new-token')

    const state = useAuthStore.getState()
    expect(state.token).toBe('new-token')
    expect(state.isAuthenticated).toBe(true)
    expect(localStorage.getItem('auth_token')).toBe('new-token')
  })
})

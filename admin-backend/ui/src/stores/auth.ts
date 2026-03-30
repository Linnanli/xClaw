import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { api } from '@/lib/api'
import type { AuthState, LoginRequest, LoginResponse } from '@/types'

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      token: null,
      user: null,
      isAuthenticated: false,

      login: async (data: LoginRequest) => {
        // 开发模式：后端未启动时使用 mock 登录
        if (import.meta.env.DEV) {
          try {
            const response = await api.post<LoginResponse>('/auth/login', data)
            const { token, user } = response.data
            localStorage.setItem('auth_token', token)
            set({ token, user, isAuthenticated: true })
          } catch {
            // 后端不可用时 fallback 到 mock
            const mockToken = 'dev-mock-token-' + Date.now()
            const mockUser = {
              id: 'dev-1',
              username: data.username,
              email: `${data.username}@ironclaw.dev`,
              role: '超级管理员',
              mfa_enabled: false,
              status: 'active' as const,
              created_at: new Date().toISOString(),
            }
            localStorage.setItem('auth_token', mockToken)
            set({ token: mockToken, user: mockUser, isAuthenticated: true })
          }
          return
        }

        const response = await api.post<LoginResponse>('/auth/login', data)
        const { token, user } = response.data
        localStorage.setItem('auth_token', token)
        set({ token, user, isAuthenticated: true })
      },

      logout: () => {
        localStorage.removeItem('auth_token')
        set({ token: null, user: null, isAuthenticated: false })
      },

      setToken: (token: string) => {
        localStorage.setItem('auth_token', token)
        set({ token, isAuthenticated: true })
      },
    }),
    {
      name: 'ironclaw-auth',
      partialize: (state) => ({
        token: state.token,
        user: state.user,
        isAuthenticated: state.isAuthenticated,
      }),
    }
  )
)

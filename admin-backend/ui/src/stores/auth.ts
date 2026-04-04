import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { api } from '@/lib/api'
import type { AuthState, LoginRequest, LoginResponse } from '@/types'

function toStoreUser(resp: LoginResponse) {
  return {
    id: resp.user.id,
    username: resp.user.username,
    email: resp.user.email,
    role: resp.user.roles[0] ?? '',
    mfa_enabled: false,
    status: 'active' as const,
    created_at: new Date().toISOString(),
  }
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      token: null,
      user: null,
      isAuthenticated: false,

      login: async (data: LoginRequest) => {
        const response = await api.post<LoginResponse>('/auth/login', data)
        const token = response.data.access_token
        localStorage.setItem('auth_token', token)
        set({ token, user: toStoreUser(response.data), isAuthenticated: true })
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

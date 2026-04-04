import { create } from 'zustand'
import { api } from '@/lib/api'
import { TOKEN_KEY } from '@/lib/api'
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

const initialToken = localStorage.getItem(TOKEN_KEY)

export const useAuthStore = create<AuthState>()((set) => ({
  token: initialToken,
  user: null,
  isAuthenticated: !!initialToken,

  login: async (data: LoginRequest) => {
    const response = await api.post<LoginResponse>('/auth/login', data)
    const token = response.data.access_token
    localStorage.setItem(TOKEN_KEY, token)
    set({ token, user: toStoreUser(response.data), isAuthenticated: true })
  },

  logout: () => {
    localStorage.removeItem(TOKEN_KEY)
    set({ token: null, user: null, isAuthenticated: false })
  },

  setToken: (token: string) => {
    localStorage.setItem(TOKEN_KEY, token)
    set({ token, isAuthenticated: true })
  },
}))

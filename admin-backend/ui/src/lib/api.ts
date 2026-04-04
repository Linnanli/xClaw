import axios, { type AxiosError, type InternalAxiosRequestConfig } from 'axios'
import { navigate } from '@/lib/navigation'

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api'

export const TOKEN_KEY = 'auth_token'

export const api = axios.create({
  baseURL: API_BASE_URL,
  timeout: 15000,
  headers: { 'Content-Type': 'application/json' },
})

api.interceptors.request.use((config: InternalAxiosRequestConfig) => {
  const token = localStorage.getItem(TOKEN_KEY)
  if (token && config.headers) {
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

api.interceptors.response.use(
  (response) => response,
  (error: AxiosError) => {
    const isLoginRequest = error.config?.url?.includes('/auth/login')
    const isAlreadyOnLogin = window.location.pathname === '/login'

    if (error.response?.status === 401 && !isLoginRequest && !isAlreadyOnLogin) {
      localStorage.removeItem(TOKEN_KEY)
      navigate('/login', { replace: true })
    }

    return Promise.reject(error)
  }
)

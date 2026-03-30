// 用户
export interface User {
  id: string
  username: string
  email: string
  role: string
  department_id?: string
  department_name?: string
  mfa_enabled: boolean
  status: 'active' | 'disabled'
  created_at: string
}

// 认证
export interface LoginRequest {
  username: string
  password: string
  mfa_code?: string
  remember_me?: boolean
}

export interface LoginResponse {
  token: string
  user: User
}

export interface AuthState {
  token: string | null
  user: User | null
  isAuthenticated: boolean
  login: (data: LoginRequest) => Promise<void>
  logout: () => void
  setToken: (token: string) => void
}

// 仪表盘
export interface DashboardStats {
  total_users: number
  online_clients: number
  dlp_blocks_today: number
  sensitive_ops_today: number
  ai_conversations_today: number
  token_usage_today: number
  unhandled_alerts: number
}

export interface TrendData {
  label: string
  value: number
}

export interface RecentLog {
  id: string
  time: string
  operator: string
  action: string
  status: 'success' | 'blocked' | 'warning' | 'info'
}

// API 响应
export interface ApiResponse<T> {
  data: T
  message?: string
}

export interface PaginatedResponse<T> {
  data: T[]
  total: number
  page: number
  page_size: number
}

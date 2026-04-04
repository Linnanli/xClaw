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
  access_token: string
  refresh_token: string
  expires_in: number
  user: {
    id: string
    username: string
    email: string
    roles: string[]
  }
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

// DLP 规则
export interface DlpRule {
  id: string
  name: string
  pattern: string
  severity: 'low' | 'medium' | 'high' | 'critical'
  classification_level?: string
  scan_direction: 'input' | 'output' | 'both'
  rule_type: 'regex' | 'keyword' | 'dictionary'
  hit_count: number
  enabled: boolean
  created_at: string
}

// 客户端
export interface Client {
  id: string
  name: string
  username: string
  status: 'online' | 'offline'
  os: string
  ip_address: string
  policy_version: string
  device_fingerprint_verified: boolean
  needs_upgrade: boolean
  last_active_at: string
}

export interface ClientStats {
  total: number
  online: number
  offline: number
  needs_upgrade: number
}

// 告警
export interface AlertRule {
  id: string
  name: string
  description?: string
  event_type: string
  condition: Record<string, unknown>
  severity: 'low' | 'medium' | 'high' | 'critical'
  notify_channels: string[]
  silence_minutes: number
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface AlertEvent {
  id: string
  rule_id?: string
  rule_name: string
  event_type: string
  severity: 'low' | 'medium' | 'high' | 'critical'
  trigger_detail: string
  event_data?: Record<string, unknown>
  status: 'pending' | 'acknowledged' | 'in_progress' | 'closed'
  resolved_note?: string
  resolved_at?: string
  created_at: string
}

export interface AlertStats {
  pending: number
  in_progress: number
  today: number
  closed: number
}

// 对话
export interface Conversation {
  id: string
  username: string
  topic: string
  message_count: number
  total_tokens: number
  model_id?: string
  dlp_flagged: boolean
  created_at: string
}

export interface ConversationDetail extends Conversation {
  dlp_details?: string
  messages: ConversationMessage[]
}

export interface ConversationMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  model_id?: string
  input_tokens: number
  output_tokens: number
  created_at: string
}

export interface ConversationStats {
  today_count: number
  today_tokens: number
  today_dlp_flagged: number
  today_active_users: number
}

// 知识库
export interface KnowledgeBase {
  id: string
  name: string
  description: string | null
  document_count: number
  enabled: boolean
  allowed_departments: string[]
  allowed_roles: string[]
  created_at: string
  updated_at: string
}

export interface KBDocument {
  id: string
  knowledge_base_id: string
  filename: string
  file_type: string
  /** 字节数，展示时用 formatFileSize 转换 */
  file_size: number
  chunk_count: number
  status: 'pending' | 'processing' | 'completed' | 'failed'
  error_message: string | null
  storage_path: string | null
  uploaded_at: string
  processed_at: string | null
}

// 配额
export interface QuotaOverview {
  today_tokens: string
  month_tokens: string
  month_budget: string
  active_models: number
}

// 审批
export interface ApprovalTicket {
  id: string
  applicant: string
  operation_type: string
  operation_name: string
  reason?: string
  status: 'pending' | 'approved' | 'rejected' | 'expired'
  review_comment?: string
  reviewed_at?: string
  expires_at: string
  created_at: string
}

export interface ApprovalStats {
  pending: number
  approved: number
  rejected: number
  expired: number
}

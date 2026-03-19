// User types
export interface User {
  id: string;
  username: string;
  email: string;
  role: string;
  status: 'active' | 'inactive';
  created_at: string;
  updated_at: string;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResponse {
  token: string;
  user: User;
}

// Role types
export interface Role {
  id: string;
  name: string;
  description?: string;
  permission_count: number;
  user_count: number;
  created_at: string;
  updated_at: string;
}

export interface RoleWithPermissions extends Role {
  permissions: Permission[];
}

// Permission types
export interface Permission {
  id: string;
  name: string;
  description: string;
  resource: string;
  action: string;
}

// DLP Rule types
export interface DlpRule {
  id: string;
  name: string;
  pattern: string;
  replacement?: string;
  severity: 'low' | 'medium' | 'high' | 'critical';
  action: 'redact' | 'block' | 'warn';
  description?: string;
  status: 'enabled' | 'disabled';
  created_at: string;
  updated_at: string;
}

export interface DlpTestRequest {
  pattern: string;
  text: string;
}

export interface DlpTestResponse {
  matched: boolean;
  sanitized_text: string;
  matches: string[];
}

// Sensitive Operation types
export interface SensitiveOperation {
  id: string;
  operation_type: 'file_delete' | 'system_command' | 'network_access';
  requires_approval: boolean;
  approver_roles: string[];
  description?: string;
  status: 'enabled' | 'disabled';
  created_at: string;
  updated_at: string;
}

// Audit Log types
export interface AuditLog {
  id: string;
  user_id: string;
  username: string;
  operation_type: string;
  resource: string;
  ip_address: string;
  user_agent?: string;
  status: 'success' | 'failure';
  error_message?: string;
  request_params?: Record<string, any>;
  response_data?: Record<string, any>;
  created_at: string;
}

// Client types
export interface Client {
  id: string;
  user_id: string;
  username: string;
  version: string;
  os: string;
  ip_address: string;
  last_activity: string;
  online: boolean;
  policy_version?: string;
}

// Policy Version types
export interface PolicyVersion {
  id: string;
  version: string;
  changelog: string;
  signed: boolean;
  signature?: string;
  created_at: string;
}

// Statistics types
export interface DashboardStats {
  total_users: number;
  online_clients: number;
  dlp_blocks_today: number;
  sensitive_ops_today: number;
  user_activity_trend: Array<{ date: string; count: number }>;
  dlp_trend: Array<{ date: string; count: number }>;
  recent_logs: AuditLog[];
  system_health: {
    status: 'healthy' | 'warning' | 'error';
    cpu_usage: number;
    memory_usage: number;
    db_connections: number;
  };
}

// Pagination types
export interface PaginationParams {
  page: number;
  page_size: number;
  search?: string;
  sort_by?: string;
  sort_order?: 'asc' | 'desc';
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

// API Response types
export interface ApiError {
  error: string;
  message?: string;
  details?: Record<string, any>;
}

export interface ApiResponse<T> {
  data?: T;
  error?: ApiError;
}

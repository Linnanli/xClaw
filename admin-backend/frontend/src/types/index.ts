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
export type DlpRuleType = 'regex' | 'keyword' | 'dictionary';

export type KeywordMatchMode = 'exact' | 'contains' | 'whole_word';

export interface KeywordRuleConfig {
  keywords: string[];
  match_mode: KeywordMatchMode;
  case_sensitive: boolean;
}

export interface DictionaryRuleConfig {
  dictionary_id: string;
  dictionary_name?: string;
  match_mode: KeywordMatchMode;
  case_sensitive: boolean;
}

// Dictionary types
export interface DlpDictionary {
  id: string;
  name: string;
  description?: string;
  keywords: string[];
  keyword_count: number;
  created_at: string;
  updated_at: string;
}

export interface CreateDictionaryRequest {
  name: string;
  description?: string;
  keywords: string[];
}

export interface UpdateDictionaryRequest {
  name?: string;
  description?: string;
  keywords?: string[];
}

export interface DlpRule {
  id: string;
  name: string;
  pattern: string;
  replacement?: string;
  severity: 'low' | 'medium' | 'high' | 'critical';
  description?: string;
  enabled: boolean;
  category: string;
  rule_type: DlpRuleType;
  rule_config?: KeywordRuleConfig | DictionaryRuleConfig | null;
  created_at: string;
  updated_at: string;
}

export interface CreateDlpRuleRequest {
  name: string;
  pattern: string;
  replacement?: string;
  severity: 'low' | 'medium' | 'high' | 'critical';
  description?: string;
  category: string;
  rule_type?: DlpRuleType;
  rule_config?: KeywordRuleConfig | DictionaryRuleConfig | null;
}

export interface UpdateDlpRuleRequest {
  name?: string;
  pattern?: string;
  replacement?: string;
  severity?: 'low' | 'medium' | 'high' | 'critical';
  description?: string;
  enabled?: boolean;
  category?: string;
  rule_type?: DlpRuleType;
  rule_config?: KeywordRuleConfig | DictionaryRuleConfig | null;
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
export type SensitiveOperationType = 'file_operation' | 'system_command' | 'network_access' | 'data_export' | 'config_change';

export interface SensitiveOperation {
  id: string;
  name: string;
  operation_type: SensitiveOperationType;
  requires_approval: boolean;
  risk_level: 'low' | 'medium' | 'high' | 'critical';
  approver_roles: string[];
  description?: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateSensitiveOperationRequest {
  name: string;
  operation_type: SensitiveOperationType;
  requires_approval?: boolean;
  risk_level?: string;
  description?: string;
  approver_roles?: string[];
}

export interface UpdateSensitiveOperationRequest {
  name?: string;
  operation_type?: SensitiveOperationType;
  requires_approval?: boolean;
  risk_level?: string;
  description?: string;
  enabled?: boolean;
  approver_roles?: string[];
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

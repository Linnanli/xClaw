// Tauri API Integration Layer
// This file provides a typed interface to Tauri backend commands

import { invoke } from '@tauri-apps/api/core';

// Helper function to invoke Tauri commands with error handling
export async function invokeTauri<T = any>(
  command: string,
  args: Record<string, any> = {}
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    console.error(`Tauri command '${command}' failed:`, error);
    throw error;
  }
}

// Authentication APIs
export interface SetupResult {
  success: boolean;
  message?: string;
}

export interface UnlockResult {
  success: boolean;
  session_id?: string;
  message?: string;
}

export interface SetupStatus {
  password_set: boolean;
}

export const authApi = {
  checkSetupStatus: () => invokeTauri<SetupStatus>('check_setup_status'),
  setupMasterPassword: (password: string) => 
    invokeTauri<SetupResult>('setup_master_password', { password }),
  unlockApp: (password: string) => 
    invokeTauri<UnlockResult>('unlock_app', { password }),
};

// Thread/Conversation APIs
export interface Thread {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface ThreadListResponse {
  assistant_thread?: Thread;
  threads: Thread[];
  active_thread?: string;
}

export interface Message {
  id: string;
  thread_id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  created_at: string;
}

export const threadApi = {
  getThreads: () => invokeTauri<ThreadListResponse>('get_threads'),
  createThread: () => invokeTauri<Thread>('create_thread'),
  sendMessage: (threadId: string, content: string) =>
    invokeTauri('send_message', { threadId, content }),
  getMessages: (threadId: string) =>
    invokeTauri<Message[]>('get_messages', { threadId }),
};

// Extension APIs
export interface ExtensionMetadata {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  tools: string[];
}

export interface InstalledExtension {
  metadata: ExtensionMetadata;
  enabled: boolean;
}

export const extensionApi = {
  getInstalledExtensions: () => invokeTauri<InstalledExtension[]>('get_installed_extensions'),
  getAvailableExtensions: () => invokeTauri<ExtensionMetadata[]>('get_available_extensions'),
  installExtension: (metadata: ExtensionMetadata) =>
    invokeTauri('install_extension', { metadata }),
  uninstallExtension: (extensionId: string) =>
    invokeTauri('uninstall_extension', { extensionId }),
  enableExtension: (extensionId: string) =>
    invokeTauri('enable_extension', { extensionId }),
  disableExtension: (extensionId: string) =>
    invokeTauri('disable_extension', { extensionId }),
};

// Routine APIs
export interface Routine {
  id: string;
  name: string;
  description: string;
  trigger: any;
  status: string;
  actions: any[];
}

export const routineApi = {
  getRoutines: () => invokeTauri<Routine[]>('get_routines'),
  createRoutine: (name: string, description: string, trigger: any, actions: any[]) =>
    invokeTauri<Routine>('create_routine', { name, description, trigger, actions }),
  triggerRoutine: (routineId: string) =>
    invokeTauri('trigger_routine', { routineId }),
  deleteRoutine: (routineId: string) =>
    invokeTauri('delete_routine', { routineId }),
};

// Session APIs
export interface SessionInfo {
  session_id: string;
  user_id: string;
  created_at: string;
  last_activity: string;
}

export const sessionApi = {
  getSessionInfo: () => invokeTauri<SessionInfo>('get_session_info'),
  lockApp: () => invokeTauri('lock_app'),
  updateSessionActivity: () => invokeTauri('update_session_activity'),
};

// Config APIs
export const configApi = {
  storeConfig: (key: string, value: string) =>
    invokeTauri('store_config', { key, value }),
  getConfig: (key: string) =>
    invokeTauri<string>('get_config', { key }),
};

// Audit APIs
export interface AuditLog {
  id: string;
  action: string;
  timestamp: string;
  details: any;
}

export const auditApi = {
  logAuditEvent: (action: string, details?: any) =>
    invokeTauri('log_audit_event', { action, details }),
  getAuditLogs: (limit: number = 100) =>
    invokeTauri<AuditLog[]>('get_audit_logs', { limit }),
};

// Plugin APIs
export interface Plugin {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  enabled: boolean;
}

export const pluginApi = {
  getInstalledPlugins: () => invokeTauri<Plugin[]>('get_installed_plugins'),
  getAvailablePlugins: () => invokeTauri<Plugin[]>('get_available_plugins'),
  checkPluginUpdates: () => invokeTauri<Plugin[]>('check_plugin_updates'),
  installPlugin: (pluginId: string) =>
    invokeTauri('install_plugin', { pluginId }),
  uninstallPlugin: (pluginId: string) =>
    invokeTauri('uninstall_plugin', { pluginId }),
  enablePlugin: (pluginId: string) =>
    invokeTauri('enable_plugin', { pluginId }),
  disablePlugin: (pluginId: string) =>
    invokeTauri('disable_plugin', { pluginId }),
  updatePlugin: (pluginId: string) =>
    invokeTauri('update_plugin', { pluginId }),
};

// Offline Mode APIs
export interface OfflineState {
  is_offline: boolean;
  last_sync: string;
}

export interface OfflineCapabilities {
  can_read: boolean;
  can_write: boolean;
  can_sync: boolean;
}

export const offlineApi = {
  getOfflineState: () => invokeTauri<OfflineState>('get_offline_state'),
  enableOfflineMode: () => invokeTauri('enable_offline_mode'),
  disableOfflineMode: () => invokeTauri('disable_offline_mode'),
  getOfflineCapabilities: () => invokeTauri<OfflineCapabilities>('get_offline_capabilities'),
  canPerformOperation: (operation: string) =>
    invokeTauri<boolean>('can_perform_operation', { operation }),
};

// Approval APIs
export const approvalApi = {
  approveOperation: (operation: string) =>
    invokeTauri('approve_operation', { operation }),
  denyOperation: (operation: string) =>
    invokeTauri('deny_operation', { operation }),
};

// Tool APIs
export interface Tool {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
}

export const toolApi = {
  getEnabledTools: () => invokeTauri<Tool[]>('get_enabled_tools'),
};

// Routine Management Extended APIs
export interface RoutineRun {
  id: string;
  routine_id: string;
  started_at: string;
  completed_at?: string;
  status: string;
}

export const routineExtendedApi = {
  enableRoutine: (routineId: string) =>
    invokeTauri('enable_routine', { routineId }),
  disableRoutine: (routineId: string) =>
    invokeTauri('disable_routine', { routineId }),
  pauseRoutine: (routineId: string) =>
    invokeTauri('pause_routine', { routineId }),
  getRoutineRuns: (routineId: string) =>
    invokeTauri<RoutineRun[]>('get_routine_runs', { routineId }),
};

// Extension Search API
export const extensionSearchApi = {
  searchExtensions: (query: string) =>
    invokeTauri<ExtensionMetadata[]>('search_extensions', { query }),
};

// Memory APIs
export interface TreeEntry {
  path: string;
  is_dir: boolean;
}

export interface MemoryTreeResponse {
  entries: TreeEntry[];
}

export interface MemoryContent {
  path: string;
  content: string;
  updated_at?: string;
}

export interface MemoryWriteResponse {
  path: string;
  status: string;
}

export interface SearchHit {
  path: string;
  content: string;
  score: number;
}

export const memoryApi = {
  getMemoryTree: () => invokeTauri<MemoryTreeResponse>('get_memory_tree'),
  readMemory: (path: string) =>
    invokeTauri<MemoryContent>('read_memory', { memoryId: path }),
  writeMemory: (path: string, content: string) =>
    invokeTauri<MemoryWriteResponse>('write_memory', { memoryId: path, content }),
  searchMemory: (query: string) =>
    invokeTauri<SearchHit[]>('search_memory', { query }),
};

// Job APIs
export interface JobInfo {
  id: string;
  status: string;
  created_at: string;
  updated_at: string;
  title?: string;
}

export interface JobDetail {
  id: string;
  status: string;
  created_at: string;
  updated_at: string;
  title?: string;
  description?: string;
  events: any[];
}

export const jobApi = {
  getJobs: () => invokeTauri<JobInfo[]>('get_jobs'),
  getJobDetail: (jobId: string) =>
    invokeTauri<JobDetail>('get_job_detail', { jobId }),
  cancelJob: (jobId: string) =>
    invokeTauri('cancel_job', { jobId }),
  restartJob: (jobId: string) =>
    invokeTauri('restart_job', { jobId }),
};

// Log APIs
export interface LogEntry {
  timestamp: string;
  level: string;
  module: string;
  message: string;
  context?: any;
}

export const logApi = {
  getLogs: (limit: number = 100) =>
    invokeTauri<LogEntry[]>('get_logs', { limit }),
  searchLogs: (query: string, limit: number = 100) =>
    invokeTauri<LogEntry[]>('search_logs', { query, limit }),
  filterLogs: (level: string, module: string, limit: number = 100) =>
    invokeTauri<LogEntry[]>('filter_logs', { level, module, limit }),
  exportLogs: (format: string) =>
    invokeTauri<string>('export_logs', { format }),
};

// Skill APIs
export interface Skill {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  keywords: string[];
  trust_level: string;
  source: string;
}

export interface InstalledSkill {
  metadata: Skill;
  enabled: boolean;
}

export const skillApi = {
  getAvailableSkills: () => invokeTauri<Skill[]>('get_available_skills'),
  getInstalledSkills: () => invokeTauri<InstalledSkill[]>('get_installed_skills'),
  installSkill: (skillId: string) =>
    invokeTauri('install_skill', { skillId }),
  uninstallSkill: (skillId: string) =>
    invokeTauri('uninstall_skill', { skillId }),
  enableSkill: (skillId: string) =>
    invokeTauri('enable_skill', { skillId }),
  disableSkill: (skillId: string) =>
    invokeTauri('disable_skill', { skillId }),
};

// Message editing/deletion APIs
export const messageApi = {
  editMessage: (threadId: string, messageId: string, content: string) =>
    invokeTauri('edit_message', { threadId, messageId, content }),
  deleteMessage: (threadId: string, messageId: string) =>
    invokeTauri('delete_message', { threadId, messageId }),
};

// Log clearing API
export const logClearApi = {
  clearLogs: () => invokeTauri('clear_logs'),
};

// Message search API
export const messageSearchApi = {
  searchMessages: (threadId: string, query: string) =>
    invokeTauri<Message[]>('search_messages', { threadId, query }),
};

// Thread export API
export const threadExportApi = {
  exportThread: (threadId: string, format: string) =>
    invokeTauri<string>('export_thread', { threadId, format }),
};

// File upload API
export const fileApi = {
  uploadFile: (threadId: string, filePath: string) =>
    invokeTauri<string>('upload_file', { threadId, filePath }),
};

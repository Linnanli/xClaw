// Tauri API Integration Layer
// This file provides a typed interface to Tauri backend commands

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (command: string, args?: Record<string, any>) => Promise<any>;
    };
  }
}

// Helper function to invoke Tauri commands with error handling
export async function invokeTauri<T = any>(
  command: string,
  args: Record<string, any> = {}
): Promise<T> {
  // Wait for Tauri internals to be available (with timeout)
  let attempts = 0;
  while (typeof window.__TAURI_INTERNALS__ === 'undefined' && attempts < 50) {
    await new Promise(resolve => setTimeout(resolve, 100));
    attempts++;
  }

  if (typeof window.__TAURI_INTERNALS__ === 'undefined') {
    throw new Error('Tauri API not available. Make sure you are running this in a Tauri application.');
  }

  return await window.__TAURI_INTERNALS__.invoke(command, args);
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

export interface Message {
  id: string;
  thread_id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  created_at: string;
}

export const threadApi = {
  getThreads: () => invokeTauri<Thread[]>('get_threads'),
  createThread: () => invokeTauri<Thread>('create_thread'),
  sendMessage: (threadId: string, content: string) =>
    invokeTauri('send_message', { thread_id: threadId, content }),
  getMessages: (threadId: string) =>
    invokeTauri<Message[]>('get_messages', { thread_id: threadId }),
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
    invokeTauri('trigger_routine', { routine_id: routineId }),
  deleteRoutine: (routineId: string) =>
    invokeTauri('delete_routine', { routine_id: routineId }),
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
    invokeTauri('install_plugin', { plugin_id: pluginId }),
  uninstallPlugin: (pluginId: string) =>
    invokeTauri('uninstall_plugin', { plugin_id: pluginId }),
  enablePlugin: (pluginId: string) =>
    invokeTauri('enable_plugin', { plugin_id: pluginId }),
  disablePlugin: (pluginId: string) =>
    invokeTauri('disable_plugin', { plugin_id: pluginId }),
  updatePlugin: (pluginId: string) =>
    invokeTauri('update_plugin', { plugin_id: pluginId }),
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
    invokeTauri('enable_routine', { routine_id: routineId }),
  disableRoutine: (routineId: string) =>
    invokeTauri('disable_routine', { routine_id: routineId }),
  pauseRoutine: (routineId: string) =>
    invokeTauri('pause_routine', { routine_id: routineId }),
  getRoutineRuns: (routineId: string) =>
    invokeTauri<RoutineRun[]>('get_routine_runs', { routine_id: routineId }),
};

// Extension Search API
export const extensionSearchApi = {
  searchExtensions: (query: string) =>
    invokeTauri<ExtensionMetadata[]>('search_extensions', { query }),
};

// Memory APIs
export interface MemoryNode {
  id: string;
  name: string;
  node_type: string;
  children: MemoryNode[];
  metadata?: any;
}

export interface MemoryTreeResponse {
  root: MemoryNode;
}

export interface MemoryContent {
  id: string;
  name: string;
  content: string;
  updated_at: string;
}

export const memoryApi = {
  getMemoryTree: () => invokeTauri<MemoryTreeResponse>('get_memory_tree'),
  readMemory: (memoryId: string) =>
    invokeTauri<MemoryContent>('read_memory', { memory_id: memoryId }),
  writeMemory: (memoryId: string, content: string) =>
    invokeTauri<MemoryContent>('write_memory', { memory_id: memoryId, content }),
  searchMemory: (query: string) =>
    invokeTauri<MemoryContent[]>('search_memory', { query }),
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
    invokeTauri<JobDetail>('get_job_detail', { job_id: jobId }),
  cancelJob: (jobId: string) =>
    invokeTauri('cancel_job', { job_id: jobId }),
  restartJob: (jobId: string) =>
    invokeTauri('restart_job', { job_id: jobId }),
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

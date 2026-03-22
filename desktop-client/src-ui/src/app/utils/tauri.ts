// Tauri API Integration Layer — 嵌入式 IronClaw 架构
//
// 命令映射：前端 API → 新 IPC 命令（ic_* 前缀）
// 暂无对应新命令的 API 返回 stub 默认值

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

// ============================================================================
// Authentication APIs (stub — 嵌入式模式无需主密码认证)
// ============================================================================

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
  checkSetupStatus: async (): Promise<SetupStatus> => {
    // 嵌入式模式：跳过主密码，直接标记已设置
    return { password_set: true };
  },
  setupMasterPassword: async (_password: string): Promise<SetupResult> => {
    return { success: true, message: 'Embedded mode: no password needed' };
  },
  unlockApp: async (_password: string): Promise<UnlockResult> => {
    return { success: true, session_id: 'embedded-session', message: 'Embedded mode' };
  },
};

// ============================================================================
// Thread/Conversation APIs → ic_list_threads, ic_create_thread, ic_get_thread_history
// ============================================================================

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

// 后端 ic_list_threads 返回的类型
interface ThreadSummaryBackend {
  id: string;
  title: string | null;
  message_count: number;
  started_at: string;
  last_activity: string;
  channel: string;
}

export const threadApi = {
  getThreads: async (): Promise<ThreadListResponse> => {
    const summaries = await invokeTauri<ThreadSummaryBackend[]>('ic_list_threads');
    const threads: Thread[] = summaries.map(s => ({
      id: s.id,
      title: s.title || '新对话',
      created_at: s.started_at,
      updated_at: s.last_activity,
    }));
    return { threads };
  },

  createThread: async (): Promise<Thread> => {
    const id = await invokeTauri<string>('ic_create_thread');
    const now = new Date().toISOString();
    return { id, title: '新对话', created_at: now, updated_at: now };
  },

  sendMessage: async (threadId: string, content: string): Promise<void> => {
    // 聊天消息通过 send_chat_message 发送（useAiChatTauri 已处理）
    await invokeTauri('send_chat_message', { threadId, content });
  },

  getMessages: async (threadId: string): Promise<Message[]> => {
    const messages = await invokeTauri<Array<{
      id: string;
      role: string;
      content: string;
      created_at: string;
    }>>('ic_get_thread_history', { threadId });
    return messages.map(m => ({
      ...m,
      thread_id: threadId,
      role: m.role as 'user' | 'assistant' | 'system',
    }));
  },
};

// ============================================================================
// Extension APIs → ic_list_extensions, ic_install_extension, etc.
// ============================================================================

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
  getInstalledExtensions: () => invokeTauri<InstalledExtension[]>('ic_list_extensions'),
  getAvailableExtensions: () => invokeTauri<ExtensionMetadata[]>('ic_search_extensions', { query: '' }),
  installExtension: (metadata: ExtensionMetadata) =>
    invokeTauri('ic_install_extension', { name: metadata.id }),
  uninstallExtension: (extensionId: string) =>
    invokeTauri('ic_uninstall_extension', { name: extensionId }),
  // enable/disable 暂无对应新命令
  enableExtension: async (_extensionId: string): Promise<void> => {},
  disableExtension: async (_extensionId: string): Promise<void> => {},
};

// Extension Search API
export const extensionSearchApi = {
  searchExtensions: (query: string) =>
    invokeTauri<ExtensionMetadata[]>('ic_search_extensions', { query }),
};

// Extension Setup API
export interface ExtensionSetupField {
  name: string;
  prompt: string;
  optional: boolean;
  provided: boolean;
  auto_generate: boolean;
}

export interface ExtensionSetupResponse {
  name: string;
  kind: string;
  secrets: ExtensionSetupField[];
}

export interface ExtensionSetupSubmitResponse {
  success: boolean;
  message: string;
  activated: boolean;
  auth_url: string | null;
}

export const extensionSetupApi = {
  getSetupSchema: (name: string) =>
    invokeTauri<ExtensionSetupResponse>('ic_extension_setup', { name }),
  submitSetup: (name: string, secrets: Record<string, string>) =>
    invokeTauri<ExtensionSetupSubmitResponse>('ic_extension_setup_submit', { name, secrets }),
};

// ============================================================================
// Session APIs (stub — 嵌入式模式无 session 管理)
// ============================================================================

export interface SessionInfo {
  session_id: string;
  user_id: string;
  created_at: string;
  last_activity: string;
}

export const sessionApi = {
  getSessionInfo: async (): Promise<SessionInfo> => {
    const now = new Date().toISOString();
    return {
      session_id: 'embedded-session',
      user_id: 'default',
      created_at: now,
      last_activity: now,
    };
  },
  lockApp: async (): Promise<void> => {
    console.warn('lockApp is no-op in embedded mode');
  },
  updateSessionActivity: async (): Promise<void> => {
    // no-op
  },
};

// ============================================================================
// Config APIs (stub)
// ============================================================================

export const configApi = {
  storeConfig: async (_key: string, _value: string): Promise<void> => {
    console.warn('storeConfig is no-op in embedded mode');
  },
  getConfig: async (_key: string): Promise<string> => {
    return '';
  },
};

// ============================================================================
// Audit APIs (stub)
// ============================================================================

export interface AuditLog {
  id: string;
  action: string;
  timestamp: string;
  details: any;
}

export const auditApi = {
  logAuditEvent: async (_action: string, _details?: any): Promise<void> => {},
  getAuditLogs: async (_limit: number = 100): Promise<AuditLog[]> => [],
};

// ============================================================================
// Plugin APIs (stub — 嵌入式模式使用 Skills/Extensions 替代)
// ============================================================================

export interface Plugin {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  enabled: boolean;
}

export const pluginApi = {
  getInstalledPlugins: async (): Promise<Plugin[]> => [],
  getAvailablePlugins: async (): Promise<Plugin[]> => [],
  checkPluginUpdates: async (): Promise<Plugin[]> => [],
  installPlugin: async (_pluginId: string): Promise<void> => {},
  uninstallPlugin: async (_pluginId: string): Promise<void> => {},
  enablePlugin: async (_pluginId: string): Promise<void> => {},
  disablePlugin: async (_pluginId: string): Promise<void> => {},
  updatePlugin: async (_pluginId: string): Promise<void> => {},
};

// ============================================================================
// Offline Mode APIs (stub — 嵌入式模式始终离线可用)
// ============================================================================

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
  getOfflineState: async (): Promise<OfflineState> => ({
    is_offline: false,
    last_sync: new Date().toISOString(),
  }),
  enableOfflineMode: async (): Promise<void> => {},
  disableOfflineMode: async (): Promise<void> => {},
  getOfflineCapabilities: async (): Promise<OfflineCapabilities> => ({
    can_read: true,
    can_write: true,
    can_sync: false,
  }),
  canPerformOperation: async (_operation: string): Promise<boolean> => true,
};

// ============================================================================
// Approval APIs → ic_approve_tool, ic_deny_tool
// ============================================================================

export const approvalApi = {
  approveOperation: (requestId: string, threadId: string = 'default') =>
    invokeTauri('ic_approve_tool', { requestId, threadId }),
  denyOperation: (requestId: string, threadId: string = 'default') =>
    invokeTauri('ic_deny_tool', { requestId, threadId }),
};

// ============================================================================
// Tool APIs (stub)
// ============================================================================

export interface Tool {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
}

export const toolApi = {
  getEnabledTools: async (): Promise<Tool[]> => [],
};

// ============================================================================
// Routine APIs (stub — 暂无对应新命令)
// ============================================================================

export interface Routine {
  id: string;
  name: string;
  description: string;
  trigger: any;
  status: string;
  actions: any[];
}

export const routineApi = {
  getRoutines: async (): Promise<Routine[]> => [],
  createRoutine: async (_name: string, _description: string, _trigger: any, _actions: any[]): Promise<Routine> => {
    throw new Error('Routines not yet supported in embedded mode');
  },
  triggerRoutine: async (_routineId: string): Promise<void> => {},
  deleteRoutine: async (_routineId: string): Promise<void> => {},
};

export interface RoutineRun {
  id: string;
  trigger_type: string;
  started_at: string;
  completed_at?: string;
  status: string;
  result_summary?: string;
  tokens_used?: number;
  job_id?: string;
}

export interface RoutineRunsResponse {
  routine_id: string;
  runs: RoutineRun[];
}

export const routineExtendedApi = {
  enableRoutine: async (_routineId: string): Promise<void> => {},
  disableRoutine: async (_routineId: string): Promise<void> => {},
  pauseRoutine: async (_routineId: string): Promise<void> => {},
  getRoutineRuns: (routineId: string) =>
    invokeTauri<RoutineRunsResponse>('ic_routine_runs', { routineId }),
};

// ============================================================================
// Memory APIs → ic_memory_list, ic_memory_read, ic_memory_write, etc.
// ============================================================================

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

export interface DeleteResult {
  success: boolean;
  message: string;
  is_protected: boolean;
}

// 后端 ic_memory_list 返回的类型
interface MemoryEntryBackend {
  name: string;
  path: string;
  is_directory: boolean;
  content_preview: string | null;
}

export const memoryApi = {
  getMemoryTree: async (): Promise<MemoryTreeResponse> => {
    const entries = await invokeTauri<MemoryEntryBackend[]>('ic_memory_list');
    return {
      entries: entries.map(e => ({
        path: e.path,
        is_dir: e.is_directory,
      })),
    };
  },

  readMemory: async (path: string): Promise<MemoryContent> => {
    const doc = await invokeTauri<{ path: string; content: string; updated_at?: string }>('ic_memory_read', { path });
    return { path: doc.path, content: doc.content, updated_at: doc.updated_at };
  },

  writeMemory: async (path: string, content: string): Promise<MemoryWriteResponse> => {
    await invokeTauri('ic_memory_write', { path, content });
    return { path, status: 'success' };
  },

  deleteMemoryLocal: async (path: string, _force: boolean = false): Promise<DeleteResult> => {
    try {
      await invokeTauri('ic_memory_delete', { path });
      return { success: true, message: 'Deleted', is_protected: false };
    } catch (error) {
      return {
        success: false,
        message: error instanceof Error ? error.message : String(error),
        is_protected: false,
      };
    }
  },

  isMemoryFileProtected: async (path: string): Promise<boolean> => {
    // 保护系统文件
    const fileName = path.split('/').pop() || '';
    return ['SOUL.md', 'IDENTITY.md', 'AGENTS.md'].includes(fileName);
  },

  searchMemory: async (query: string): Promise<SearchHit[]> => {
    return invokeTauri<SearchHit[]>('ic_memory_search', { query });
  },
};

// 扩展 MemoryContent 以支持删除状态检查
export const memoryContentUtils = {
  isDeleted: (content: MemoryContent): boolean => {
    return content.content.trim() === '<!-- DELETED -->';
  },
  
  getActualContent: (content: MemoryContent): string => {
    return memoryContentUtils.isDeleted(content) ? '' : content.content;
  },
  
  shouldShowInTree: (content: MemoryContent): boolean => {
    return !memoryContentUtils.isDeleted(content);
  }
};

// ============================================================================
// Skill APIs → ic_list_skills, ic_search_skills, ic_install_skill, etc.
// ============================================================================

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
  getAvailableSkills: () => invokeTauri<Skill[]>('ic_search_skills', { query: '' }),
  getInstalledSkills: () => invokeTauri<InstalledSkill[]>('ic_list_skills'),
  installSkill: (skillId: string) =>
    invokeTauri('ic_install_skill', { name: skillId }),
  uninstallSkill: (skillId: string) =>
    invokeTauri('ic_uninstall_skill', { name: skillId }),
  // enable/disable 暂无对应新命令
  enableSkill: async (_skillId: string): Promise<void> => {},
  disableSkill: async (_skillId: string): Promise<void> => {},
};

// ============================================================================
// Job APIs (stub — 暂无对应新命令)
// ============================================================================

export interface JobInfo {
  id: string;
  status: string;
  created_at: string;
  updated_at?: string;
  title?: string;
}

export interface JobDetail {
  id: string;
  status: string;
  created_at: string;
  updated_at?: string;
  title?: string;
  description?: string;
  events: any[];
}

export const jobApi = {
  getJobs: async (): Promise<JobInfo[]> => [],
  getJobDetail: async (_jobId: string): Promise<JobDetail> => {
    throw new Error('Jobs not yet supported in embedded mode');
  },
  cancelJob: async (_jobId: string): Promise<void> => {},
  restartJob: async (_jobId: string): Promise<void> => {},
  getJobEvents: (jobId: string) =>
    invokeTauri<JobEventsResponse>('ic_job_events', { jobId }),
  sendJobPrompt: (jobId: string, content: string) =>
    invokeTauri<JobPromptResponse>('ic_job_prompt', { jobId, content }),
};

// Job Events types
export interface JobEvent {
  id: number;
  event_type: string;
  data: any;
  created_at: string;
}

export interface JobEventsResponse {
  job_id: string;
  events: JobEvent[];
}

export interface JobPromptResponse {
  status: string;
  job_id: string;
}

// ============================================================================
// Log APIs (stub)
// ============================================================================

export interface LogEntry {
  timestamp: string;
  level: string;
  module: string;
  message: string;
  context?: any;
}

export const logApi = {
  getLogs: async (_limit: number = 100): Promise<LogEntry[]> => [],
  searchLogs: async (_query: string, _limit: number = 100): Promise<LogEntry[]> => [],
  filterLogs: async (_level: string, _module: string, _limit: number = 100): Promise<LogEntry[]> => [],
  exportLogs: async (_format: string): Promise<string> => '',
};

// ============================================================================
// Message editing/deletion APIs (stub)
// ============================================================================

export const messageApi = {
  editMessage: async (_threadId: string, _messageId: string, _content: string): Promise<void> => {
    console.warn('editMessage not yet supported in embedded mode');
  },
  deleteMessage: async (_threadId: string, _messageId: string): Promise<void> => {
    console.warn('deleteMessage not yet supported in embedded mode');
  },
};

// Log clearing API
export const logClearApi = {
  clearLogs: async (): Promise<void> => {},
};

// Message search API
export const messageSearchApi = {
  searchMessages: async (_threadId: string, _query: string): Promise<Message[]> => [],
};

// Thread export API
export const threadExportApi = {
  exportThread: async (_threadId: string, _format: string): Promise<string> => '',
};

// File upload API
export const fileApi = {
  uploadFile: async (_threadId: string, _filePath: string): Promise<string> => '',
};

// ============================================================================
// App initialization APIs (stub)
// ============================================================================

export interface AppInitInfo {
  auth_token: string;
  api_base_url: string;
  database_type: string;
  log_level: string;
}

export const appApi = {
  getAppInitInfo: async (): Promise<AppInitInfo> => ({
    auth_token: 'embedded-token',
    api_base_url: 'embedded://local',
    database_type: 'libsql',
    log_level: 'info',
  }),
  getAuthToken: async (): Promise<string> => 'embedded-token',
  refreshAuthToken: async (): Promise<string> => 'embedded-token',
};

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

/**
 * useChatNavigation — 对话页面导航状态管理
 *
 * 收拢所有"跳转到对话"相关的 state 和操作，避免在 MainApp 里散落多处。
 */

import { useState, useCallback, useEffect } from 'react';

const STORAGE_KEY = 'ironclaw:selectedThreadId';

function readStoredThreadId(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function writeStoredThreadId(id: string | null): void {
  try {
    if (id) {
      localStorage.setItem(STORAGE_KEY, id);
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
  } catch {
    // localStorage 不可用时静默失败
  }
}

export interface ChatNavigationState {
  selectedThreadId: string | null;
  sidebarRefreshKey: number;
  /** 待发送的 routine 提示词，ChatTabTauri 挂载后自动发送并清除 */
  pendingPrompt: string | null;
}

export interface ChatNavigationActions {
  selectThread: (threadId: string) => void;
  clearThread: () => void;
  /** 手动触发定时任务：跳转到 thread 并暂存 prompt，等 ChatTabTauri 挂载后发送 */
  openRoutineThread: (threadId: string, prompt: string) => void;
  completeRoutineThread: (threadId: string) => void;
  clearPendingPrompt: () => void;
  /** 在新对话里发送一条命令（不需要特定 thread） */
  sendCommand: (command: string) => void;
}

export function useChatNavigation(): ChatNavigationState & ChatNavigationActions {
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(readStoredThreadId);
  const [sidebarRefreshKey, setSidebarRefreshKey] = useState(0);
  const [pendingPrompt, setPendingPrompt] = useState<string | null>(null);

  // selectedThreadId 变化时同步到 localStorage
  useEffect(() => {
    writeStoredThreadId(selectedThreadId);
  }, [selectedThreadId]);

  const navigateToThread = useCallback((threadId: string) => {
    setSelectedThreadId(threadId);
    setSidebarRefreshKey((k) => k + 1);
  }, []);

  const selectThread = useCallback((threadId: string) => {
    navigateToThread(threadId);
  }, [navigateToThread]);

  const clearThread = useCallback(() => {
    setSelectedThreadId(null);
    setPendingPrompt(null);
  }, []);

  const openRoutineThread = useCallback((threadId: string, prompt: string) => {
    navigateToThread(threadId);
    setPendingPrompt(prompt);
  }, [navigateToThread]);

  /** 在当前对话（或新对话）里发送一条命令 */
  const sendCommand = useCallback((command: string) => {
    // 保持当前 thread，避免重新挂载 TauriRuntimeProvider 导致模型配置丢失
    setPendingPrompt(command);
  }, []);

  const completeRoutineThread = useCallback((threadId: string) => {
    navigateToThread(threadId);
  }, [navigateToThread]);

  const clearPendingPrompt = useCallback(() => {
    setPendingPrompt(null);
  }, []);

  return {
    selectedThreadId,
    sidebarRefreshKey,
    pendingPrompt,
    selectThread,
    clearThread,
    openRoutineThread,
    completeRoutineThread,
    clearPendingPrompt,
    sendCommand,
  };
}

/**
 * useChatNavigation — 对话页面导航状态管理
 *
 * 收拢所有"跳转到对话"相关的 state 和操作，避免在 MainApp 里散落多处。
 */

import { useState, useCallback } from 'react';

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
}

export function useChatNavigation(): ChatNavigationState & ChatNavigationActions {
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(null);
  const [sidebarRefreshKey, setSidebarRefreshKey] = useState(0);
  const [pendingPrompt, setPendingPrompt] = useState<string | null>(null);

  const selectThread = useCallback((threadId: string) => {
    setSelectedThreadId(threadId);
    setSidebarRefreshKey((k) => k + 1);
  }, []);

  const clearThread = useCallback(() => {
    setSelectedThreadId(null);
    setPendingPrompt(null);
  }, []);

  const openRoutineThread = useCallback((threadId: string, prompt: string) => {
    setSelectedThreadId(threadId);
    setSidebarRefreshKey((k) => k + 1);
    setPendingPrompt(prompt);
  }, []);

  const completeRoutineThread = useCallback((threadId: string) => {
    setSelectedThreadId(threadId);
    setSidebarRefreshKey((k) => k + 1);
  }, []);

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
  };
}

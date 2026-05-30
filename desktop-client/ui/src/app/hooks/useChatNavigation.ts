/**
 * useChatNavigation — 对话页面导航状态管理
 *
 * 收拢所有"跳转到对话"相关的 state 和操作，避免在 MainApp 里散落多处。
 */

import { useState, useCallback, useEffect } from 'react';
import type { ChatCommand } from '../types/chatCommand';

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
  pendingCommand: ChatCommand | null;
}

export interface ChatNavigationActions {
  selectThread: (threadId: string) => void;
  clearThread: () => void;
  queueSendTextCommand: (text: string) => string;
  consumeCommand: (commandId: string) => void;
}

export function useChatNavigation(): ChatNavigationState & ChatNavigationActions {
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(readStoredThreadId);
  const [sidebarRefreshKey, setSidebarRefreshKey] = useState(0);
  const [pendingCommand, setPendingCommand] = useState<ChatCommand | null>(null);

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
  }, []);

  const queueSendTextCommand = useCallback((text: string): string => {
    const commandId = `cmd-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    setPendingCommand({ id: commandId, kind: 'send_text', text });
    return commandId;
  }, []);

  const consumeCommand = useCallback((commandId: string) => {
    setPendingCommand((current) => {
      if (!current || current.id !== commandId) return current;
      return null;
    });
  }, []);

  return {
    selectedThreadId,
    sidebarRefreshKey,
    pendingCommand,
    selectThread,
    clearThread,
    queueSendTextCommand,
    consumeCommand,
  };
}

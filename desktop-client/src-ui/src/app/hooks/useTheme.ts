/**
 * useTheme - 主题切换 Hook
 *
 * 支持三种模式：light（白天）、dark（黑暗）、system（跟随系统）
 * 持久化到 localStorage，并在 <html> 上切换 .dark class。
 */

import { useState, useEffect, useCallback } from 'react';

export type ThemeMode = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'xclaw-theme';

function getSystemDark(): boolean {
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

function applyTheme(mode: ThemeMode): void {
  const isDark = mode === 'dark' || (mode === 'system' && getSystemDark());
  document.documentElement.classList.toggle('dark', isDark);
}

export function useTheme() {
  const [theme, setThemeState] = useState<ThemeMode>(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY) as ThemeMode | null;
      return stored ?? 'system';
    } catch {
      return 'system';
    }
  });

  // 初始化时应用主题
  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  // 监听系统主题变化（仅 system 模式下生效）
  useEffect(() => {
    if (theme !== 'system') return;
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = () => applyTheme('system');
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, [theme]);

  const setTheme = useCallback((mode: ThemeMode) => {
    try {
      localStorage.setItem(STORAGE_KEY, mode);
    } catch {
      // ignore storage errors
    }
    setThemeState(mode);
  }, []);

  return { theme, setTheme };
}
